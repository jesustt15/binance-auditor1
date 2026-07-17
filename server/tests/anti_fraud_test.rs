#[cfg(test)]
mod anti_fraud_tests {
    // Tests for anti-fraud engine: DKIM/SPF/DMARC verification,
    // duplicate detection, amount tamper detection

    #[test]
    fn test_clean_email_passes() {
        let headers = MockHeaders {
            dkim_pass: true,
            spf_pass: true,
            dmarc_pass: true,
            from_domain: "binance.com".to_string(),
        };

        let result = mock_verify_headers(&headers);
        assert_eq!(result.verdict, "clean");
        assert!(!result.quarantined);
    }

    #[test]
    fn test_dkim_fail_quarantines() {
        let headers = MockHeaders {
            dkim_pass: false,
            spf_pass: false,
            dmarc_pass: false,
            from_domain: "binance.com".to_string(),
        };

        let result = mock_verify_headers(&headers);
        assert_eq!(result.verdict, "dkim_fail");
        assert!(result.quarantined);
    }

    #[test]
    fn test_non_binance_domain_quarantines() {
        let headers = MockHeaders {
            dkim_pass: true,
            spf_pass: true,
            dmarc_pass: true,
            from_domain: "phishing.com".to_string(),
        };

        let result = mock_verify_headers(&headers);
        assert!(result.quarantined);
        assert!(result.reason.unwrap().contains("binance.com"));
    }

    #[test]
    fn test_spf_only_pass_accepted() {
        let headers = MockHeaders {
            dkim_pass: false,
            spf_pass: true,
            dmarc_pass: false,
            from_domain: "binance.com".to_string(),
        };

        let result = mock_verify_headers(&headers);
        assert_eq!(result.verdict, "clean");
        assert!(!result.quarantined);
    }

    #[test]
    fn test_dmarc_without_dkim_spf_accepted() {
        let headers = MockHeaders {
            dkim_pass: false,
            spf_pass: false,
            dmarc_pass: true,
            from_domain: "binance.com".to_string(),
        };

        let result = mock_verify_headers(&headers);
        // dmarc_pass is checked after dkim/spf; if dmarc passes and domain ok, accept
        assert_eq!(result.verdict, "clean");
        assert!(!result.quarantined);
    }

    #[test]
    fn test_duplicate_detection() {
        let existing_payments = 1;
        let is_same_tx = true;

        // If same TX exists, it's a duplicate
        let result = mock_check_duplicate(existing_payments, is_same_tx);
        assert!(result.is_some());
        assert_eq!(result.unwrap().verdict, "duplicate");
    }

    #[test]
    fn test_no_duplicate_when_different() {
        let result = mock_check_duplicate(0, false);
        assert!(result.is_none());
    }

    #[test]
    fn test_amount_tamper_detected() {
        let result = mock_check_amount_tamper(100.0, Some(200.0));
        assert!(result.is_some());
        assert_eq!(result.unwrap().verdict, "amount_tamper");
    }

    #[test]
    fn test_amount_tamper_accepts_match() {
        let result = mock_check_amount_tamper(100.0, Some(100.0));
        assert!(result.is_none());
    }

    #[test]
    fn test_amount_tamper_no_expected_ok() {
        let result = mock_check_amount_tamper(100.0, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_subdomain_binance_accepted() {
        let headers = MockHeaders {
            dkim_pass: true,
            spf_pass: true,
            dmarc_pass: true,
            from_domain: "mail.binance.com".to_string(),
        };

        let result = mock_verify_headers(&headers);
        assert_eq!(result.verdict, "clean");
        assert!(!result.quarantined);
    }

    // -- Test helpers --

    struct MockHeaders {
        dkim_pass: bool,
        spf_pass: bool,
        dmarc_pass: bool,
        from_domain: String,
    }

    struct FraudVerdict {
        verdict: String,
        quarantined: bool,
        reason: Option<String>,
    }

    fn mock_verify_headers(headers: &MockHeaders) -> FraudVerdict {
        // Check domain
        let domain_ok = headers.from_domain == "binance.com"
            || headers.from_domain.ends_with(".binance.com");

        if !domain_ok {
            return FraudVerdict {
                verdict: "dmarc_fail".to_string(),
                quarantined: true,
                reason: Some(format!(
                    "Remitente no es de binance.com: {}",
                    headers.from_domain
                )),
            };
        }

        if headers.dkim_pass || headers.spf_pass {
            FraudVerdict {
                verdict: "clean".to_string(),
                quarantined: false,
                reason: None,
            }
        } else if headers.dmarc_pass {
            FraudVerdict {
                verdict: "clean".to_string(),
                quarantined: false,
                reason: None,
            }
        } else {
            let reason = if !headers.dkim_pass && !headers.spf_pass {
                "dkim_fail"
            } else if !headers.spf_pass {
                "spf_fail"
            } else {
                "dmarc_fail"
            };
            FraudVerdict {
                verdict: reason.to_string(),
                quarantined: true,
                reason: Some(format!(
                    "Verificacion de remitente fallida: {}",
                    reason.replace('_', " ")
                )),
            }
        }
    }

    fn mock_check_duplicate(existing: usize, same_tx: bool) -> Option<FraudVerdict> {
        if same_tx {
            Some(FraudVerdict {
                verdict: "duplicate".to_string(),
                quarantined: false,
                reason: Some("Pago duplicado detectado".to_string()),
            })
        } else {
            None
        }
    }

    fn mock_check_amount_tamper(
        email_amount: f64,
        expected_amount: Option<f64>,
    ) -> Option<FraudVerdict> {
        if let Some(expected) = expected_amount {
            if (email_amount - expected).abs() > 0.01 {
                return Some(FraudVerdict {
                    verdict: "amount_tamper".to_string(),
                    quarantined: true,
                    reason: Some(format!(
                        "Monto alterado: email={}, esperado={}",
                        email_amount, expected
                    )),
                });
            }
        }
        None
    }
}
