use imap::ClientBuilder;
use mailparse::parse_mail;
use crate::db;
use rusqlite::Connection;
use std::result::Result;

#[derive(Debug)]
pub struct BinanceEmailData {
    pub usuario: String,
    pub monto: f64,
    pub moneda: String,
    pub fecha: String,
}

fn extract_binance_data(text: &str) -> Option<BinanceEmailData> {
    let lines: Vec<&str> = text.lines().collect();

    let mut usuario: Option<String> = None;
    let mut monto: Option<f64> = None;
    let mut moneda: Option<String> = None;
    let mut fecha: Option<String> = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if trimmed.eq_ignore_ascii_case("time:") {
            if let Some(next_line) = lines.get(i + 1) {
                fecha = parse_time_field(next_line.trim());
            }
        } else if trimmed.eq_ignore_ascii_case("from:") {
            if let Some(next_line) = lines.get(i + 1) {
                usuario = Some(next_line.trim().to_string());
            }
        } else if trimmed.eq_ignore_ascii_case("amount:") {
            if let Some(next_line) = lines.get(i + 1) {
                if let Some((amt, cur)) = parse_amount_field(next_line.trim()) {
                    monto = Some(amt);
                    moneda = Some(cur);
                }
            }
        }
    }

    match (usuario, monto, moneda, fecha) {
        (Some(u), Some(m), Some(c), Some(f)) => Some(BinanceEmailData {
            usuario: u,
            monto: m,
            moneda: c,
            fecha: f,
        }),
        _ => None,
    }
}

fn parse_time_field(s: &str) -> Option<String> {
    // Input: "2026-07-10 16:16:07(UTC)"
    let cleaned = s.replace("(UTC)", "").trim().to_string();
    let naive = chrono::NaiveDateTime::parse_from_str(&cleaned, "%Y-%m-%d %H:%M:%S").ok()?;
    // Assume UTC
    let datetime = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(naive, chrono::Utc);
    Some(datetime.to_rfc3339())
}

fn parse_amount_field(s: &str) -> Option<(f64, String)> {
    // Input: "4.8 USDT" or "1,234.56 BTC"
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

pub fn sync_emails(
    imap_user: &str,
    imap_password: &str,
    conn: &Connection,
) -> Result<i64, String> {
    let client = ClientBuilder::new("imap.gmail.com", 993)
        .connect()
        .map_err(|e| format!("IMAP connection error: {}", e))?;

    let mut session = client
        .login(imap_user, imap_password)
        .map_err(|e| format!("IMAP login error: {}", e.0))?;

    session
        .select("INBOX")
        .map_err(|e| format!("IMAP inbox error: {}", e))?;

    // Buscar solo correos NO LEIDOS con "Binance" en el asunto
    // Usamos SUBJECT en vez de FROM porque los correos reenviados cambian el FROM original
    let unseen = session
        .search("UNSEEN SUBJECT \"Binance\"")
        .map_err(|e| format!("IMAP search error: {}", e))?;

    eprintln!("[IMAP] Correos con 'Binance' en el asunto: {}", unseen.len());
    let mut processed = 0i64;

    for msg_id in unseen {
        let messages = session
            .fetch(msg_id.to_string(), "RFC822")
            .map_err(|e| format!("IMAP fetch error: {}", e))?;

        if let Some(msg) = messages.iter().next() {
            if let Some(body) = msg.body() {
                let parsed = parse_mail(body).map_err(|e| format!("Parse error: {}", e))?;

                let text_body = get_text_body(&parsed);

                eprintln!("[IMAP DEBUG] === Email #{} ({} bytes raw, {} chars body) ===",
                    msg_id, body.len(), text_body.len());
                eprintln!("[IMAP DEBUG] --- BODY START ---\n{}\n[IMAP DEBUG] --- BODY END ---",
                    text_body);

                if let Some(data) = extract_binance_data(&text_body) {
                    eprintln!("[IMAP] Pago detectado: usuario='{}' monto={} moneda='{}' fecha='{}'",
                        data.usuario, data.monto, data.moneda, data.fecha);
                    let exists = db::pago_exists(conn, &data.usuario, data.monto, &data.fecha)
                        .map_err(|e| format!("DB check error: {}", e))?;

                    if !exists {
                        db::insert_pago(conn, &data.usuario, data.monto, &data.moneda, &data.fecha)
                            .map_err(|e| format!("DB insert error: {}", e))?;
                        processed += 1;
                    } else {
                        eprintln!("[IMAP] Pago DUPLICADO, no se inserta.");
                    }
                } else {
                    eprintln!("[IMAP] Correo ignorado (no parece de Binance).");
                    eprintln!("[IMAP] Campos detectados: none completos.");
                }
            }
        }

        session
            .store(msg_id.to_string(), "+FLAGS (\\Seen)")
            .map_err(|e| format!("IMAP flags error: {}", e))?;
    }

    session.logout().map_err(|e| format!("IMAP logout error: {}", e))?;

    Ok(processed)
}

fn get_text_body(parsed: &mailparse::ParsedMail) -> String {
    // Recursive: handles multipart/mixed → message/rfc822 → multipart/alternative → text/plain
    // Common in forwarded emails from Gmail
    get_text_body_recursive(parsed, 0)
}

fn get_text_body_recursive(parsed: &mailparse::ParsedMail, depth: usize) -> String {
    if depth > 10 {
        return String::new(); // safety: prevent infinite recursion
    }

    let ctype = parsed.ctype.mimetype.clone();

    // Try root body, but SKIP if it's empty (multipart containers return empty body)
    if let Ok(body) = parsed.get_body() {
        let trimmed = body.trim();
        if !trimmed.is_empty() {
            eprintln!("[IMAP MIME] depth={} type='{}' body_found ({} chars)", depth, ctype, trimmed.len());
            return trimmed.to_string();
        }
    }

    // Recurse into subparts to find first non-empty text body
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
