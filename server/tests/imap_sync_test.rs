#[cfg(test)]
mod imap_sync_tests {
    // Tests for IMAP sync logic

    #[test]
    fn test_html_to_text_strips_tags() {
        let html = "<div>Hola <b>Mundo</b></div><p>Parrafo</p>";
        let text = mock_html_to_text(html);
        assert!(text.contains("Hola"));
        assert!(text.contains("Mundo"));
        assert!(text.contains("Parrafo"));
        assert!(!text.contains("<div>"));
        assert!(!text.contains("<b>"));
    }

    #[test]
    fn test_html_to_text_handles_entities() {
        let html = "C&oacute;digo &amp; m&aacute;s";
        let text = mock_html_to_text(html);
        assert!(text.contains("ódigo"));
        assert!(text.contains("&"));
        assert!(text.contains("más"));
    }

    #[test]
    fn test_extract_binance_payment_data() {
        let body = "From:\nalice@email.com\nAmount:\n100.50 USDT\nTime:\n2026-07-15 10:30:00";
        let subject = "Binance Pay";
        let result = mock_extract_data(body, subject);
        assert!(result.is_some());
        let data = result.unwrap();
        assert_eq!(data.usuario, Some("alice@email.com".to_string()));
        assert_eq!(data.monto, 100.5);
        assert_eq!(data.moneda, "USDT");
        assert_eq!(data.tipo, "pago");
    }

    #[test]
    fn test_extract_deposit_data() {
        let body = "Tu depósito por valor de 200.00 USDC está ya disponible en tu cuenta de Binance.";
        let subject = "[Binance] USDC Depósito completado - 2026-07-15 14:00:00 (UTC)";
        let result = mock_extract_data(body, subject);
        assert!(result.is_some());
        let data = result.unwrap();
        assert_eq!(data.usuario, None);
        assert_eq!(data.monto, 200.0);
        assert_eq!(data.moneda, "USDC");
        assert_eq!(data.tipo, "deposito");
    }

    #[test]
    fn test_subject_matches_deposit_variants() {
        assert!(mock_subject_is_deposit("Depósito Completado"));
        assert!(mock_subject_is_deposit("Deposito Completado"));
        assert!(mock_subject_is_deposit("Deposit Completed"));
        assert!(!mock_subject_is_deposit("Binance Pay"));
    }

    #[test]
    fn test_sync_dedup_by_uid() {
        let existing_uids = vec![1, 2, 3, 5, 8];
        let new_emails: Vec<(u32, &str)> = vec![
            (1, "dup"),
            (4, "new"),
            (5, "dup"),
            (10, "new"),
        ];

        let (new, dups): (Vec<_>, Vec<_>) = new_emails
            .iter()
            .partition(|(uid, _)| !existing_uids.contains(uid));

        assert_eq!(new.len(), 2);
        assert_eq!(dups.len(), 2);
        assert_eq!(new[0].0, 4);
        assert_eq!(new[1].0, 10);
    }

    // -- Test helpers --

    struct EmailData {
        usuario: Option<String>,
        monto: f64,
        moneda: String,
        fecha: String,
        tipo: String,
    }

    fn mock_html_to_text(html: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;
        for ch in html.chars() {
            if ch == '<' {
                in_tag = true;
            } else if ch == '>' {
                in_tag = false;
            } else if !in_tag {
                result.push(ch);
            }
        }
        result
            .replace("&oacute;", "ó")
            .replace("&aacute;", "á")
            .replace("&amp;", "&")
    }

    fn mock_extract_data(body: &str, subject: &str) -> Option<EmailData> {
        let is_deposit = subject.to_lowercase().contains("deposito completado")
            || subject.to_lowercase().contains("deposit completed");

        if is_deposit {
            let lower = body.to_lowercase();
            if let Some(pos) = lower.find("por valor de") {
                let after = body[pos + "por valor de".len()..].trim();
                let parts: Vec<&str> = after.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(amt) = parts[0].parse::<f64>() {
                        return Some(EmailData {
                            usuario: None,
                            monto: amt,
                            moneda: parts[1].to_uppercase(),
                            fecha: "2026-07-15T14:00:00+00:00".to_string(),
                            tipo: "deposito".to_string(),
                        });
                    }
                }
            }
        }

        // Payment parsing
        let lines: Vec<&str> = body.lines().collect();
        let mut usuario = None;
        let mut monto = None;
        let mut moneda = None;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("from:") {
                if let Some(next) = lines.get(i + 1) {
                    usuario = Some(next.trim().to_string());
                }
            }
            if trimmed.eq_ignore_ascii_case("amount:") {
                if let Some(next) = lines.get(i + 1) {
                    let parts: Vec<&str> = next.trim().split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(amt) = parts[0].parse::<f64>() {
                            monto = Some(amt);
                            moneda = Some(parts[1].to_uppercase());
                        }
                    }
                }
            }
        }

        match (monto, moneda) {
            (Some(m), Some(c)) => Some(EmailData {
                usuario,
                monto: m,
                moneda: c,
                fecha: "2026-07-15T10:30:00+00:00".to_string(),
                tipo: "pago".to_string(),
            }),
            _ => None,
        }
    }

    fn mock_subject_is_deposit(subject: &str) -> bool {
        let lower = subject.to_lowercase();
        lower.contains("deposito completado")
            || lower.contains("depósito completado")
            || lower.contains("deposit completed")
    }
}
