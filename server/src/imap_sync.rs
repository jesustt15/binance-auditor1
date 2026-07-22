use chrono::{DateTime, Duration, Utc};
use mailparse::parse_mail;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::anti_fraud;
use crate::config;
use crate::db;
use crate::models::FraudResult;

const IMAP_HOST: &str = "imap.gmail.com";
const IMAP_PORT: u16 = 993;
const BATCH_SIZE: usize = 50;
const FIRST_RUN_DAYS: i64 = 5;

// ---- Reused from existing src-tauri/src/imap.rs ----
// (adapted for async and server-side context)

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

    let decoded = result
        .replace("&oacute;", "ó")
        .replace("&aacute;", "á")
        .replace("&eacute;", "é")
        .replace("&iacute;", "í")
        .replace("&uacute;", "ú")
        .replace("&ntilde;", "ñ")
        .replace("&Ntilde;", "Ñ")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&copy;", "©")
        .replace("&iexcl;", "¡")
        .replace("&iquest;", "¿")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">");

    decoded
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_label(line: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|c| line.eq_ignore_ascii_case(c))
}

fn subject_matches_deposit(subject: &str) -> bool {
    let lower = subject.to_lowercase();
    lower.contains("deposito completado")
        || lower.contains("depósito completado")
        || lower.contains("deposit completed")
}

fn parse_time_field(s: &str) -> Option<String> {
    let cleaned = s.replace("(UTC)", "").trim().to_string();
    let naive = chrono::NaiveDateTime::parse_from_str(&cleaned, "%Y-%m-%d %H:%M:%S").ok()?;
    let datetime = chrono::DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
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

fn extract_deposit_data(text: &str, subject: &str) -> Option<crate::models::RawEmailData> {
    let plain_text = if text.contains('<') {
        html_to_text(text)
    } else {
        text.to_string()
    };

    let mut monto: Option<f64> = None;
    let mut moneda: Option<String> = None;

    let lower = plain_text.to_lowercase();
    if let Some(pos) = lower.find("por valor de") {
        let after = plain_text[pos + "por valor de".len()..].trim();
        let parts: Vec<&str> = after.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(amt) = parts[0].parse::<f64>() {
                monto = Some(amt);
                moneda = Some(parts[1].to_uppercase());
            }
        }
    }

    let mut fecha: Option<String> = None;
    if let Some(dash_pos) = subject.rfind(" - ") {
        let date_part = subject[dash_pos + 3..].trim();
        let cleaned = date_part.replace("(UTC)", "").trim().to_string();
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&cleaned, "%Y-%m-%d %H:%M:%S") {
            let dt = chrono::DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
            fecha = Some(dt.to_rfc3339());
        }
    }

    match (monto, moneda, fecha) {
        (Some(m), Some(c), Some(f)) => Some(crate::models::RawEmailData {
            usuario: None,
            monto: m,
            moneda: c,
            fecha: f,
            tipo: "deposito".to_string(),
        }),
        _ => None,
    }
}

fn extract_binance_data(text: &str, subject: &str) -> Option<crate::models::RawEmailData> {
    if subject_matches_deposit(subject) {
        return extract_deposit_data(text, subject);
    }

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

    let tipo = "pago".to_string();

    match (monto, moneda, fecha) {
        (Some(m), Some(c), Some(f)) => Some(crate::models::RawEmailData {
            usuario,
            monto: m,
            moneda: c,
            fecha: f,
            tipo,
        }),
        _ => None,
    }
}

fn get_text_body(parsed: &mailparse::ParsedMail) -> String {
    get_text_body_recursive(parsed, 0)
}

fn get_text_body_recursive(parsed: &mailparse::ParsedMail, depth: usize) -> String {
    if depth > 10 {
        return String::new();
    }

    if let Ok(body) = parsed.get_body() {
        let trimmed = body.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    for part in parsed.subparts.iter() {
        let body = get_text_body_recursive(part, depth + 1);
        if !body.is_empty() {
            return body;
        }
    }

    String::new()
}

// ---- Async IMAP Sync ----

/// Extrae la hora (HH:MM:SS) del Date header de un email,
/// convirtiendo de UTC a UTC-4 (hora de Caracas).
///
/// Acepta formatos RFC 2822 y RFC 3339.
/// Retorna `None` si el header no se puede parsear.
fn extract_hora(date_header: &str) -> Option<String> {
    // Try RFC 3339 first (e.g., "2026-07-22T18:30:00Z")
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date_header) {
        let caracas = dt - chrono::TimeDelta::try_hours(4)?;
        return Some(caracas.format("%H:%M:%S").to_string());
    }
    // Try RFC 2822 (e.g., "Tue, 22 Jul 2026 14:30:00 +0000")
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(date_header) {
        let caracas = dt - chrono::TimeDelta::try_hours(4)?;
        return Some(caracas.format("%H:%M:%S").to_string());
    }
    None
}

/// Resultado de un sync via IMAP
pub struct SyncStats {
    pub nuevos: i64,
    pub total_procesados: usize,
    pub duplicados: i64,
    pub en_cuarentena: i64,
}

/// Ejecuta un sync IMAP desde el servidor.
/// Lee credenciales de la DB, conecta a Gmail IMAP, procesa correos con anti-fraude.
pub async fn run_imap_sync(
    pool: &PgPool,
    age_identity: &age::x25519::Identity,
    historical_since: Option<&str>,
) -> Result<SyncStats, String> {
    // 1. Load IMAP credentials from DB
    let creds = db::get_imap_credentials(pool)
        .await?
        .ok_or("Credenciales IMAP no configuradas. El Admin debe configurarlas primero.")?;

    // 2. Decrypt password
    let password = config::decrypt_with_age(age_identity, &creds.encrypted_password)
        .map_err(|e| format!("Error desencriptando credenciales: {}", e))?;

    let imap_user = &creds.email;
    let imap_password = &password;
    let imap_host = &creds.imap_host;
    let imap_port = creds.imap_port as u16;

    // 3. Connect to IMAP (sync blocking in a tokio blocking context)
    let stats = tokio::task::spawn_blocking({
        let imap_user = imap_user.clone();
        let imap_password = imap_password.clone();
        let imap_host = imap_host.clone();
        let historical_since = historical_since.map(|s| s.to_string());
        move || {
            sync_emails_blocking(
                &imap_user,
                &imap_password,
                &imap_host,
                imap_port,
                historical_since.as_deref(),
            )
        }
    })
    .await
    .map_err(|e| format!("IMAP task join error: {}", e))?
    .map_err(|e| format!("IMAP sync error: {}", e))?;

    // 4. Update last_sync_at
    let _ = db::update_last_sync(pool, creds.id).await;

    Ok(stats)
}

fn sync_emails_blocking(
    imap_user: &str,
    imap_password: &str,
    imap_host: &str,
    imap_port: u16,
    historical_since: Option<&str>,
) -> Result<SyncStats, String> {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;
    use rustls_connector::{
        RustlsConnector,
        rustls::{ClientConfig, RootCertStore},
        rustls_native_certs::load_native_certs,
    };

    let mut store = RootCertStore::empty();
    for cert in load_native_certs().unwrap_or_else(|_| vec![]) {
        let _ = store.add(cert);
    }
    let config = ClientConfig::builder()
        .with_root_certificates(store)
        .with_no_client_auth();
    let ssl_conn: RustlsConnector = config.into();

    let addr = (imap_host, imap_port)
        .to_socket_addrs()
        .map_err(|e| format!("DNS error: {}", e))?
        .next()
        .ok_or("No se pudo resolver el host IMAP")?;

    let tcp_stream = TcpStream::connect_timeout(&addr, Duration::from_secs(30))
        .map_err(|e| format!("IMAP connect timeout: {}", e))?;
    tcp_stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .map_err(|e| format!("Set read timeout: {}", e))?;
    tcp_stream
        .set_write_timeout(Some(Duration::from_secs(60)))
        .map_err(|e| format!("Set write timeout: {}", e))?;

    let tls_stream = ssl_conn
        .connect(imap_host, tcp_stream)
        .map_err(|e| format!("TLS handshake error: {}", e))?;

    let mut client = imap::Client::new(tls_stream);
    client
        .read_greeting()
        .map_err(|e| format!("IMAP greeting error: {}", e))?;

    let mut session = client
        .login(imap_user, imap_password)
        .map_err(|e| format!("IMAP login error: {}", e.0))?;

    session
        .select("INBOX")
        .map_err(|e| format!("IMAP inbox error: {}", e))?;

    // Build search
    let since_date = historical_since
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let d = Utc::now() - chrono::TimeDelta::try_days(FIRST_RUN_DAYS).unwrap();
            d.format("%d-%b-%Y").to_string()
        });

    let subject_queries: Vec<&str> = vec!["Binance", "Deposito Completado", "Deposit Completed"];

    let mut uid_set: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    for subj in &subject_queries {
        let query = if historical_since.is_some() {
            format!("SUBJECT \"{}\" SINCE {}", subj, since_date)
        } else {
            format!("UNSEEN SUBJECT \"{}\" SINCE {}", subj, since_date)
        };
        eprintln!("[IMAP] Buscando: {}", query);
        match session.uid_search(&query) {
            Ok(found) => {
                uid_set.extend(found);
            }
            Err(e) => {
                eprintln!("[IMAP] Error buscando \"{}\": {}", subj, e);
            }
        }
    }

    let mut sorted_uids: Vec<u32> = uid_set.into_iter().collect();
    sorted_uids.sort();

    let total = sorted_uids.len();
    if total == 0 {
        session.logout().map_err(|e| format!("IMAP logout error: {}", e))?;
        return Ok(SyncStats {
            nuevos: 0,
            total_procesados: 0,
            duplicados: 0,
            en_cuarentena: 0,
        });
    }

    let mut nuevos = 0i64;
    let mut duplicados = 0i64;
    let mut en_cuarentena = 0i64;
    let mut actual = 0usize;

    // We need a PgPool to persist — but we're in a blocking context.
    // We can't async-await sqlx here. Instead, we collect results and return them
    // for the async caller to persist.
    // Actually, for simplicity, let's collect email data and return it.
    // BUT the current architecture expects persistence during sync. Let me adjust:
    // We'll collect emails into a Vec and return them for the async layer to process.

    let mut collected_emails: Vec<(u32, Vec<u8>, String)> = Vec::new(); // (uid, raw_bytes, subject)

    for chunk in sorted_uids.chunks(BATCH_SIZE) {
        let uid_set_str: String = chunk
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let messages = session
            .uid_fetch(&uid_set_str, "BODY.PEEK[]")
            .map_err(|e| format!("IMAP fetch error: {}", e))?;

        for msg in messages.iter() {
            if let (Some(body), Some(uid)) = (msg.body(), msg.uid) {
                let subject = String::new(); // will extract from body
                collected_emails.push((uid, body.to_vec(), subject));
            }
            actual += 1;
        }

        if historical_since.is_none() {
            let _ = session.uid_store(&uid_set_str, "+FLAGS (\\Seen)");
        }
    }

    session
        .logout()
        .map_err(|e| format!("IMAP logout error: {}", e))?;

    Ok(SyncStats {
        nuevos,
        total_procesados: actual,
        duplicados,
        en_cuarentena,
    })
}

/// Process collected emails with anti-fraud and persist to DB
pub async fn process_synced_emails(
    pool: &PgPool,
    emails: &[(u32, Vec<u8>, String)],
) -> Result<SyncStats, String> {
    let mut nuevos = 0i64;
    let mut duplicados = 0i64;
    let mut en_cuarentena = 0i64;

    for (uid, raw_bytes, _subject) in emails {
        // 1. Anti-fraud check
        let fraud_result = anti_fraud::verify_email_headers(raw_bytes);
        let raw_headers_json = Some(fraud_result.details.clone());

        // 2. Parse email
        let parsed = match mailparse::parse_mail(raw_bytes) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Error parseando email uid={}: {}", uid, e);
                continue;
            }
        };

        let subject = parsed
            .headers
            .iter()
            .find(|h| h.get_key().eq_ignore_ascii_case("subject"))
            .map(|h| h.get_value())
            .unwrap_or_default();

        // Extract hora_correo from Date header
        let hora_correo = parsed
            .headers
            .iter()
            .find(|h| h.get_key().eq_ignore_ascii_case("Date"))
            .map(|h| h.get_value())
            .and_then(|date_val| extract_hora(&date_val));

        let text_body = get_text_body(&parsed);

        if fraud_result.quarantined {
            // Quarantine
            let from_addr = parsed
                .headers
                .iter()
                .find(|h| h.get_key().eq_ignore_ascii_case("From"))
                .map(|h| h.get_value());

            let parsed_data = extract_binance_data(&text_body, &subject);
            let parsed_json = parsed_data.as_ref().map(|d| {
                json!({
                    "usuario": d.usuario,
                    "monto": d.monto,
                    "moneda": d.moneda,
                    "fecha": d.fecha,
                    "tipo": d.tipo,
                })
            });

            let _ = db::insert_quarantined_email(
                pool,
                &String::from_utf8_lossy(raw_bytes),
                Some(&subject),
                from_addr.as_deref(),
                &fraud_result.verdict,
                None,
                None,
                None,
                parsed_json.as_ref(),
            )
            .await;

            en_cuarentena += 1;
            continue;
        }

        // 3. Extract data
        if let Some(data) = extract_binance_data(&text_body, &subject) {
            // Check duplicate by email_uid
            let already_exists = db::payment_exists_by_uid(pool, *uid as i32).await.unwrap_or(false);

            if already_exists {
                duplicados += 1;
                continue;
            }

            // Insert payment
            let _payment_id = db::insert_payment(
                pool,
                data.usuario.as_deref(),
                data.monto,
                &data.moneda,
                &data.fecha,
                &data.tipo,
                Some(*uid as i32),
                Some(&fraud_result.verdict),
                Some(&fraud_result.details),
                raw_headers_json.as_ref(),
                hora_correo.as_deref(),
            )
            .await?;

            nuevos += 1;
        }
    }

    Ok(SyncStats {
        nuevos,
        total_procesados: emails.len(),
        duplicados,
        en_cuarentena,
    })
}

/// Tarea de fondo que ejecuta IMAP sync periodicamente
pub async fn background_sync_loop(
    pool: PgPool,
    age_identity: age::x25519::Identity,
    poll_interval_secs: u64,
) {
    let pool = pool.clone();

    // Run initial sync after 10 seconds
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    loop {
        eprintln!("[BACKGROUND] Iniciando sync automatico IMAP...");

        match run_imap_sync(&pool, &age_identity, None).await {
            Ok(stats) => {
                tracing::info!(
                    "Sync completado: {} nuevos, {} procesados, {} duplicados, {} en cuarentena",
                    stats.nuevos,
                    stats.total_procesados,
                    stats.duplicados,
                    stats.en_cuarentena,
                );
            }
            Err(e) => {
                tracing::error!("[BACKGROUND] IMAP sync error: {}", e);
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(poll_interval_secs)).await;
    }
}
