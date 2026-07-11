use imap::ClientBuilder;
use mailparse::parse_mail;
use regex::Regex;
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
    let regex_monto = Regex::new(r"(?i)Has recibido ([\d.,]+)\s*(USDT|BTC|ETH)").ok()?;
    let regex_usuario = Regex::new(r"(?i)de parte de ([a-zA-Z0-9_\s\-]+)").ok()?;

    let monto_match = regex_monto.captures(text)?;
    let usuario_match = regex_usuario.captures(text)?;

    let monto_str = monto_match.get(1)?.as_str().replace(",", "");
    let monto: f64 = monto_str.parse().ok()?;
    let moneda = monto_match.get(2)?.as_str().to_uppercase();
    let usuario = usuario_match.get(1)?.as_str().trim().to_string();

    Some(BinanceEmailData {
        usuario,
        monto,
        moneda,
        fecha: chrono::Utc::now().to_rfc3339(),
    })
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

    let unseen = session
        .search("UNSEEN FROM binance.com")
        .map_err(|e| format!("IMAP search error: {}", e))?;

    let mut processed = 0i64;

    for msg_id in unseen {
        let messages = session
            .fetch(msg_id.to_string(), "RFC822")
            .map_err(|e| format!("IMAP fetch error: {}", e))?;

        if let Some(msg) = messages.iter().next() {
            if let Some(body) = msg.body() {
                let parsed = parse_mail(body).map_err(|e| format!("Parse error: {}", e))?;

                let text_body = get_text_body(&parsed);

                if let Some(data) = extract_binance_data(&text_body) {
                    let exists = db::pago_exists(conn, &data.usuario, data.monto, &data.fecha)
                        .map_err(|e| format!("DB check error: {}", e))?;

                    if !exists {
                        db::insert_pago(conn, &data.usuario, data.monto, &data.moneda, &data.fecha)
                            .map_err(|e| format!("DB insert error: {}", e))?;
                        processed += 1;
                    }
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
    if let Ok(body) = parsed.get_body() {
        return body;
    }

    for part in &parsed.subparts {
        if let Ok(body) = part.get_body() {
            if !body.trim().is_empty() {
                return body;
            }
        }
    }

    String::new()
}
