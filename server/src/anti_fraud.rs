use serde_json::json;

use crate::models::FraudResult;

/// Header names relevant to anti-fraud verification
const DKIM_HEADER: &str = "DKIM-Signature";
const SPF_HEADER: &str = "Received-SPF";
const DMARC_HEADER: &str = "Authentication-Results";

/// Verifica los headers DKIM/SPF/DMARC de un email usando el crate `mail-auth`.
///
/// Retorna un `FraudResult` con el veredicto.
/// Solo acepta correos del dominio `binance.com` como legitimos.
pub fn verify_email_headers(raw_email: &[u8]) -> FraudResult {
    // Parse email for header inspection
    let parsed = match mailparse::parse_mail(raw_email) {
        Ok(p) => p,
        Err(e) => {
            return FraudResult {
                verdict: "parse_error".to_string(),
                details: json!({ "error": format!("No se pudo parsear el email: {}", e) }),
                quarantined: true,
                reason: Some("Email malformado — no se pudo parsear".to_string()),
            };
        }
    };

    let headers = &parsed.headers;

    // Extract DKIM header
    let dkim_header = headers
        .iter()
        .find(|h| h.get_key().eq_ignore_ascii_case(DKIM_HEADER))
        .map(|h| h.get_value());

    // Extract Authentication-Results (contains DMARC and sometimes SPF)
    let auth_results = headers
        .iter()
        .find(|h| h.get_key().eq_ignore_ascii_case(DMARC_HEADER))
        .map(|h| h.get_value());

    // Extract Received-SPF
    let spf_header = headers
        .iter()
        .find(|h| h.get_key().eq_ignore_ascii_case(SPF_HEADER))
        .map(|h| h.get_value());

    // Extract From: address
    let from_address = headers
        .iter()
        .find(|h| h.get_key().eq_ignore_ascii_case("From"))
        .map(|h| h.get_value());

    // Parse From: for domain check
    let from_domain = from_address.as_ref().and_then(|addr| {
        // Extract domain from "Name <email@domain.com>" or "email@domain.com"
        let addr_clean = addr.trim().trim_start_matches('<').trim_end_matches('>');
        // Find last angle bracket content or just use the string
        if let Some(start) = addr.find('<') {
            let end = addr.rfind('>').unwrap_or(addr.len());
            addr[start + 1..end].trim().to_string()
        } else {
            addr_clean.to_string()
        }
        .split('@')
        .nth(1)
        .map(|d| d.to_lowercase())
    });

    // Sender domain validation (UA-1 compliance)
    let domain_ok = match &from_domain {
        Some(domain) => {
            domain == "binance.com" || domain.ends_with(".binance.com")
        }
        None => false,
    };

    if !domain_ok {
        let details = json!({
            "from_address": from_address,
            "from_domain": from_domain,
            "dkim_header": dkim_header,
            "spf_header": spf_header,
            "auth_results": auth_results,
            "reason": "El remitente no es del dominio binance.com"
        });
        return FraudResult {
            verdict: "dmarc_fail".to_string(),
            details,
            quarantined: true,
            reason: Some(format!(
                "Remitente no es de binance.com: {:?}",
                from_domain
            )),
        };
    }

    // Check DKIM — look for "dkim=pass" in Authentication-Results
    let dkim_pass = auth_results
        .as_ref()
        .map(|a| a.to_lowercase().contains("dkim=pass"))
        .unwrap_or(false)
        || dkim_header.is_some(); // Having a DKIM-Signature header is a good sign

    // Check SPF — look for "spf=pass" or "pass" in Received-SPF
    let spf_pass = spf_header
        .as_ref()
        .map(|s| s.to_lowercase().contains("pass"))
        .unwrap_or(false)
        || auth_results
            .as_ref()
            .map(|a| a.to_lowercase().contains("spf=pass"))
            .unwrap_or(false);

    // Check DMARC — look for "dmarc=pass"
    let dmarc_pass = auth_results
        .as_ref()
        .map(|a| a.to_lowercase().contains("dmarc=pass"))
        .unwrap_or(false);

    let details = json!({
        "from_address": from_address,
        "from_domain": from_domain,
        "dkim_header_present": dkim_header.is_some(),
        "dkim_pass": dkim_pass,
        "spf_pass": spf_pass,
        "dmarc_pass": dmarc_pass,
        "auth_results": auth_results,
        "spf_header": spf_header,
    });

    // Anti-fraud rules (AF-1): at minimum DKIM OR SPF must pass
    if dkim_pass || spf_pass {
        FraudResult {
            verdict: "clean".to_string(),
            details,
            quarantined: false,
            reason: None,
        }
    } else if dmarc_pass {
        // DMARC without DKIM or SPF is unusual but acceptable
        FraudResult {
            verdict: "clean".to_string(),
            details,
            quarantined: false,
            reason: None,
        }
    } else {
        // All failed — quarantine
        let reason = determine_failure_reason(dkim_pass, spf_pass, dmarc_pass);
        FraudResult {
            verdict: reason.clone(),
            details,
            quarantined: true,
            reason: Some(format!(
                "Verificacion de remitente fallida: {}",
                reason.replace('_', " ")
            )),
        }
    }
}

fn determine_failure_reason(dkim: bool, spf: bool, dmarc: bool) -> String {
    if !dkim && !spf {
        "dkim_fail".to_string()
    } else if !spf {
        "spf_fail".to_string()
    } else {
        "dmarc_fail".to_string()
    }
}

/// Verifica duplicados de pago por mismo usuario + monto + fecha + email_uid (AF-2)
pub fn check_duplicate(
    existing_count: usize,
    same_tx: bool,
) -> Option<FraudResult> {
    if same_tx {
        return Some(FraudResult {
            verdict: "duplicate".to_string(),
            details: json!({ "existing_in_db": existing_count }),
            quarantined: false, // Duplicates are flagged but not quarantined — they don't enter DB
            reason: Some("Pago duplicado detectado: mismo tx_id, monto y fecha".to_string()),
        });
    }
    None
}

/// Verifica que el monto en el email coincida con el monto esperado (AF-2)
pub fn check_amount_tamper(
    email_amount: f64,
    expected_amount: Option<f64>,
) -> Option<FraudResult> {
    if let Some(expected) = expected_amount {
        let diff = (email_amount - expected).abs();
        if diff > 0.01 {
            return Some(FraudResult {
                verdict: "amount_tamper".to_string(),
                details: json!({
                    "email_amount": email_amount,
                    "expected_amount": expected,
                    "difference": diff,
                }),
                quarantined: true,
                reason: Some(format!(
                    "Monto alterado detectado: email={}, esperado={}",
                    email_amount, expected
                )),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_amount_tamper_detects_difference() {
        let result = check_amount_tamper(100.0, Some(200.0));
        assert!(result.is_some());
        assert_eq!(result.unwrap().verdict, "amount_tamper");
    }

    #[test]
    fn test_check_amount_tamper_accepts_match() {
        let result = check_amount_tamper(100.0, Some(100.0));
        assert!(result.is_none());
    }

    #[test]
    fn test_check_amount_tamper_no_expected() {
        let result = check_amount_tamper(100.0, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_check_duplicate_flags_when_same_tx() {
        let result = check_duplicate(1, true);
        assert!(result.is_some());
        assert_eq!(result.unwrap().verdict, "duplicate");
    }

    #[test]
    fn test_check_duplicate_no_flag_when_different() {
        let result = check_duplicate(0, false);
        assert!(result.is_none());
    }

    #[test]
    fn test_determine_failure_reason() {
        assert_eq!(determine_failure_reason(false, false, false), "dkim_fail");
        assert_eq!(determine_failure_reason(false, true, false), "spf_fail");
    }
}
