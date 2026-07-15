use imap::Client;
use mailparse::parse_mail;
use crate::db;
use rusqlite::Connection;
use std::net::{TcpStream, ToSocketAddrs};
use std::result::Result;
use std::time::Duration;
use rustls_connector::{
    RustlsConnector,
    rustls::{ClientConfig, RootCertStore},
    rustls_native_certs::load_native_certs,
};
use tauri::{AppHandle, Emitter};
use serde::Serialize;

const IMAP_HOST: &str = "imap.gmail.com";
const IMAP_PORT: u16 = 993;
const BATCH_SIZE: usize = 50;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const READ_TIMEOUT: Duration = Duration::from_secs(60);
const FIRST_RUN_DAYS: i64 = 5;

#[derive(Debug)]
pub struct BinanceEmailData {
    pub usuario: Option<String>,
    pub monto: f64,
    pub moneda: String,
    pub fecha: String,
    pub tipo: String,
}

#[derive(Serialize, Clone)]
pub struct SyncProgress {
    pub actual: usize,
    pub total: usize,
    pub nuevos: i64,
}

#[derive(Serialize, Clone)]
pub struct SyncStats {
    pub nuevos: i64,
    pub total_procesados: usize,
    pub duplicados: i64,
}

fn html_to_text(html: &str) -> String {
    let s = html
        .replace("</div>", "\n")
        .replace("</p>", "\n")
        .replace("</td>", "\n")
        .replace("</tr>", "\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n");

    let mut result = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }

    result
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_label(line: &str, candidates: &[&str]) -> bool {
    candidates.iter().any(|c| line.eq_ignore_ascii_case(c))
}

fn subject_matches_deposit(subject: &str) -> bool {
    let lower = subject.to_lowercase();
    lower.contains("deposito completado")
        || lower.contains("depósito completado")
        || lower.contains("deposit completed")
}

fn extract_binance_data(text: &str, subject: &str) -> Option<BinanceEmailData> {
    let plain_text = if text.contains('<') {
        html_to_text(text)
    } else {
        text.to_string()
    };

    let lines: Vec<&str> = plain_text.lines().collect();

    let mut usuario: Option<String> = None;
    let mut monto: Option<f64> = None;
    let mut moneda: Option<String> = None;
    let mut fecha: Option<String> = None;

    let time_labels = ["time:", "hora:", "fecha:", "fecha y hora:"];
    let from_labels = ["from:", "de:", "remitente:", "enviado por:"];
    let amount_labels = ["amount:", "monto:", "importe:", "cantidad:"];

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if is_label(trimmed, &time_labels) {
            if let Some(next_line) = lines.get(i + 1) {
                fecha = parse_time_field(next_line.trim());
            }
        } else if is_label(trimmed, &from_labels) {
            if let Some(next_line) = lines.get(i + 1) {
                usuario = Some(next_line.trim().to_string());
            }
        } else if is_label(trimmed, &amount_labels) {
            if let Some(next_line) = lines.get(i + 1) {
                if let Some((amt, cur)) = parse_amount_field(next_line.trim()) {
                    monto = Some(amt);
                    moneda = Some(cur);
                }
            }
        }
    }

    let tipo = if subject_matches_deposit(subject) {
        "deposito".to_string()
    } else {
        "pago".to_string()
    };

    match (monto, moneda, fecha) {
        (Some(m), Some(c), Some(f)) => Some(BinanceEmailData {
            usuario,
            monto: m,
            moneda: c,
            fecha: f,
            tipo,
        }),
        _ => None,
    }
}

fn parse_time_field(s: &str) -> Option<String> {
    let cleaned = s.replace("(UTC)", "").trim().to_string();
    let naive = chrono::NaiveDateTime::parse_from_str(&cleaned, "%Y-%m-%d %H:%M:%S").ok()?;
    let datetime = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(naive, chrono::Utc);
    Some(datetime.to_rfc3339())
}

fn parse_amount_field(s: &str) -> Option<(f64, String)> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() >= 2 {
        let amount_str = parts[0].replace(",", "");
        let amount: f64 = amount_str.parse().ok()?;
        let currency = parts[1].to_uppercase();
        Some((amount, currency))
    } else {
        None
    }
}

fn build_tls_connector() -> Result<RustlsConnector, String> {
    let mut store = RootCertStore::empty();
    for cert in load_native_certs().unwrap_or_else(|_| vec![]) {
        let _ = store.add(cert);
    }
    let config = ClientConfig::builder()
        .with_root_certificates(store)
        .with_no_client_auth();
    Ok(config.into())
}

fn connect_with_timeout(ssl_conn: &RustlsConnector) -> Result<Client<rustls_connector::TlsStream<TcpStream>>, String> {
    let addr = (IMAP_HOST, IMAP_PORT)
        .to_socket_addrs()
        .map_err(|e| format!("DNS error: {}", e))?
        .next()
        .ok_or("No se pudo resolver imap.gmail.com")?;

    let tcp_stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)
        .map_err(|e| format!("IMAP connect timeout ({}s): {}", CONNECT_TIMEOUT.as_secs(), e))?;

    tcp_stream
        .set_read_timeout(Some(READ_TIMEOUT))
        .map_err(|e| format!("Set read timeout: {}", e))?;
    tcp_stream
        .set_write_timeout(Some(READ_TIMEOUT))
        .map_err(|e| format!("Set write timeout: {}", e))?;

    let tls_stream = ssl_conn
        .connect(IMAP_HOST, tcp_stream)
        .map_err(|e| format!("TLS handshake error: {}", e))?;

    let mut client = Client::new(tls_stream);
    client
        .read_greeting()
        .map_err(|e| format!("IMAP greeting error: {}", e))?;

    Ok(client)
}

pub fn sync_emails(
    imap_user: &str,
    imap_password: &str,
    conn: &Connection,
    app: &AppHandle,
) -> Result<SyncStats, String> {
    sync_emails_internal(imap_user, imap_password, conn, app, None)
}

/// Historical sync: searches ALL emails (not just UNSEEN) since a given date.
/// Uses BODY.PEEK so flags are NOT changed — old emails stay unread in Gmail.
/// Dedup is handled by the DB check.
pub fn sync_emails_historical(
    imap_user: &str,
    imap_password: &str,
    conn: &Connection,
    app: &AppHandle,
    since_date: &str, // format: "01-Jan-2026"
) -> Result<SyncStats, String> {
    sync_emails_internal(imap_user, imap_password, conn, app, Some(since_date))
}

fn sync_emails_internal(
    imap_user: &str,
    imap_password: &str,
    conn: &Connection,
    app: &AppHandle,
    historical_since: Option<&str>,
) -> Result<SyncStats, String> {
    let ssl_conn = build_tls_connector()?;
    let client = connect_with_timeout(&ssl_conn)?;

    let mut session = client
        .login(imap_user, imap_password)
        .map_err(|e| format!("IMAP login error: {}", e.0))?;

    session
        .select("INBOX")
        .map_err(|e| format!("IMAP inbox error: {}", e))?;

    // Build search query: UNSEEN for normal sync, no UNSEEN for historical
    let since_date = historical_since.map(|s| s.to_string()).unwrap_or_else(|| {
        let d = chrono::Utc::now() - chrono::Duration::days(FIRST_RUN_DAYS);
        d.format("%d-%b-%Y").to_string()
    });

    let deposit_subjects = r#"OR OR SUBJECT "Deposito Completado" SUBJECT "Depósito Completado" SUBJECT "Deposit Completed""#;
    let search_query = if historical_since.is_some() {
        format!("OR SUBJECT \"Binance\" {} SINCE {}", deposit_subjects, since_date)
    } else {
        format!("UNSEEN OR SUBJECT \"Binance\" {} SINCE {}", deposit_subjects, since_date)
    };

    eprintln!("[IMAP] Buscando: {}", search_query);

    let uids: Vec<u32> = session
        .uid_search(&search_query)
        .map_err(|e| format!("IMAP search error: {}", e))?
        .into_iter()
        .collect();

    eprintln!("[IMAP] Correos encontrados con 'Binance' desde {}: {}", since_date, uids.len());

    let mut sorted_uids = uids;
    sorted_uids.sort();

    let total = sorted_uids.len();

    if total == 0 {
        let _ = app.emit("sync-progress", SyncProgress { actual: 0, total: 0, nuevos: 0 });
        session.logout().map_err(|e| format!("IMAP logout error: {}", e))?;
        return Ok(SyncStats { nuevos: 0, total_procesados: 0, duplicados: 0 });
    }

    let mut nuevos = 0i64;
    let mut duplicados = 0i64;
    let mut actual = 0usize;

    for chunk in sorted_uids.chunks(BATCH_SIZE) {
        let uid_set: String = chunk
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let messages = session
            .uid_fetch(&uid_set, "BODY.PEEK[]")
            .map_err(|e| format!("IMAP fetch error: {}", e))?;

        for msg in messages.iter() {
            if let Some(body) = msg.body() {
                let parsed = parse_mail(body).map_err(|e| format!("Parse error: {}", e))?;
                let text_body = get_text_body(&parsed);

                let subject = parsed.headers.iter()
                    .find(|h| h.get_key().eq_ignore_ascii_case("subject"))
                    .map(|h| h.get_value())
                    .unwrap_or_default();

                eprintln!("[IMAP] --- Body del correo #{} ---", actual + 1);
                eprintln!("{}", text_body);
                eprintln!("[IMAP] Subject: {}", subject);
                eprintln!("[IMAP] --- Fin body ---");

                if let Some(data) = extract_binance_data(&text_body, &subject) {
                    eprintln!(
                        "[IMAP] {} detectado: usuario='{}' monto={} moneda='{}' fecha='{}'",
                        data.tipo,
                        data.usuario.as_deref().unwrap_or("N/A"),
                        data.monto, data.moneda, data.fecha
                    );
                    let exists = db::pago_exists(conn, data.usuario.as_deref(), data.monto, &data.fecha)
                        .map_err(|e| format!("DB check error: {}", e))?;

                    if !exists {
                        db::insert_pago(conn, data.usuario.as_deref(), data.monto, &data.moneda, &data.fecha, &data.tipo)
                            .map_err(|e| format!("DB insert error: {}", e))?;
                        nuevos += 1;
                    } else {
                        duplicados += 1;
                        eprintln!("[IMAP] {} DUPLICADO, no se inserta.", data.tipo);
                    }
                }
            }
            actual += 1;
        }

        // Only mark as SEEN for normal (non-historical) sync
        // Historical sync uses PEEK and leaves flags untouched
        if historical_since.is_none() {
            session
                .uid_store(&uid_set, "+FLAGS (\\Seen)")
                .map_err(|e| format!("IMAP flags error: {}", e))?;
        }

        let _ = app.emit("sync-progress", SyncProgress { actual, total, nuevos });
        eprintln!("[IMAP] Progreso: {}/{} (nuevos: {}, duplicados: {})", actual, total, nuevos, duplicados);
    }

    session
        .logout()
        .map_err(|e| format!("IMAP logout error: {}", e))?;

    Ok(SyncStats { nuevos, total_procesados: actual, duplicados })
}

fn get_text_body(parsed: &mailparse::ParsedMail) -> String {
    get_text_body_recursive(parsed, 0)
}

fn get_text_body_recursive(parsed: &mailparse::ParsedMail, depth: usize) -> String {
    if depth > 10 {
        return String::new();
    }

    let ctype = parsed.ctype.mimetype.clone();

    if let Ok(body) = parsed.get_body() {
        let trimmed = body.trim();
        if !trimmed.is_empty() {
            eprintln!("[IMAP MIME] depth={} type='{}' body_found ({} chars)", depth, ctype, trimmed.len());
            return trimmed.to_string();
        }
    }

    for (i, part) in parsed.subparts.iter().enumerate() {
        let sub_ctype = &part.ctype.mimetype;
        eprintln!("[IMAP MIME] depth={} subpart[{}] type='{}'", depth, i, sub_ctype);
        let body = get_text_body_recursive(part, depth + 1);
        if !body.is_empty() {
            return body;
        }
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Task 4.1: Parser with deposit body (no sender)
    #[test]
    fn extract_deposito_body_no_sender() {
        let body = "Amount:\n100.00 USDT\nTime:\n2026-07-15 10:30:00\n";
        let subject = "Deposito Completado";
        let result = extract_binance_data(body, subject).unwrap();
        assert_eq!(result.tipo, "deposito");
        assert_eq!(result.usuario, None);
        assert_eq!(result.monto, 100.0);
        assert_eq!(result.moneda, "USDT");
        assert!(!result.fecha.is_empty());
    }

    // Task 4.1: Deposit without accent
    #[test]
    fn extract_deposito_sin_acento() {
        let body = "Monto:\n200.00 USDC\nHora:\n2026-07-15 14:00:00\n";
        let subject = "Deposito Completado";
        let result = extract_binance_data(body, subject).unwrap();
        assert_eq!(result.tipo, "deposito");
        assert_eq!(result.usuario, None);
        assert_eq!(result.monto, 200.0);
        assert_eq!(result.moneda, "USDC");
    }

    // Task 4.2: Parser with payment body (has sender)
    #[test]
    fn extract_pago_body_with_sender() {
        let body = "From:\nalice@example.com\nAmount:\n50.00 USDT\nTime:\n2026-07-15 10:30:00\n";
        let subject = "Binance Pay";
        let result = extract_binance_data(body, subject).unwrap();
        assert_eq!(result.tipo, "pago");
        assert_eq!(result.usuario.as_deref(), Some("alice@example.com"));
        assert_eq!(result.monto, 50.0);
        assert_eq!(result.moneda, "USDT");
    }

    // Task 4.2: Parser requires amount, currency, and date
    #[test]
    fn extract_returns_none_when_no_amount() {
        let body = "From:\nalice\nTime:\n2026-07-15 10:30:00\n";
        let subject = "Binance Pay";
        let result = extract_binance_data(body, subject);
        assert!(result.is_none());
    }

    // Task 4.3: Subject matching — 3 deposit variants + binance subject
    #[test]
    fn subject_matches_deposito_completado_sin_acento() {
        assert!(subject_matches_deposit("Deposito Completado"));
    }

    #[test]
    fn subject_matches_deposito_completado_con_acento() {
        assert!(subject_matches_deposit("Depósito Completado"));
    }

    #[test]
    fn subject_matches_deposit_completed() {
        assert!(subject_matches_deposit("Deposit Completed"));
    }

    #[test]
    fn subject_does_not_match_binance_pay() {
        assert!(!subject_matches_deposit("Binance Pay"));
    }

    #[test]
    fn subject_case_insensitive() {
        assert!(subject_matches_deposit("DEPOSITO COMPLETADO"));
        assert!(subject_matches_deposit("deposit completed"));
    }

    // Deposit with Spanish "De:" label (sender present) still gets deposit tipo from subject
    #[test]
    fn extract_deposito_with_sender_label_still_deposito() {
        let body = "De:\nremitente@test.com\nMonto:\n150.00 USDT\nFecha:\n2026-07-15 16:00:00\n";
        let subject = "Depósito Completado";
        let result = extract_binance_data(body, subject).unwrap();
        assert_eq!(result.tipo, "deposito");
        assert_eq!(result.usuario.as_deref(), Some("remitente@test.com"));
    }
}
