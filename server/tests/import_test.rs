#[cfg(test)]
mod import_tests {
    // Tests for CSV/Excel import logic

    #[test]
    fn test_csv_parsing_valid_file() {
        let csv_content = "usuario,monto,fecha\nAlice,100.50,2026-07-10\nBob,200.00,2026-07-11\n";

        let results = mock_parse_csv(csv_content);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].usuario, "Alice");
        assert_eq!(results[0].monto, 100.50);
        assert_eq!(results[1].usuario, "Bob");
        assert_eq!(results[1].monto, 200.00);
    }

    #[test]
    fn test_csv_handles_malformed_rows() {
        let csv_content = "usuario,monto,fecha\n,,\nAlice,invalid,2026-07-10\n";

        let results = mock_parse_csv(csv_content);
        assert_eq!(results.len(), 2);

        // First row (empty fields) should be an error
        assert!(results[0].resultado.contains("campos vacios"));

        // Second row (invalid monto) should be an error
        assert!(results[1].resultado.contains("monto invalido"));
    }

    #[test]
    fn test_csv_handles_comma_in_monto() {
        let csv_content = "usuario,monto,fecha\nAlice,\"1,000.50\",2026-07-10\n";

        let results = mock_parse_csv(csv_content);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].monto, 1000.50);
    }

    #[test]
    fn test_csv_auto_detects_headers() {
        let csv_content = "user,amount,date\nAlice,100.50,2026-07-10\n";

        let results = mock_parse_csv(csv_content);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].usuario, "Alice");
    }

    #[test]
    fn test_csv_rejects_invalid_date() {
        let csv_content = "usuario,monto,fecha\nAlice,100.50,10/07/2026\n";

        let results = mock_parse_csv(csv_content);
        assert!(results[0].resultado.contains("fecha invalida"));
    }

    #[test]
    fn test_import_audit_logging() {
        // Simulate audit log entry creation on import
        let audit_entries = mock_audit_on_import("test.csv", 5, 3, 2);

        assert_eq!(audit_entries.len(), 1);
        assert!(audit_entries[0].contains("import_csv"));
        assert!(audit_entries[0].contains("test.csv"));
        assert!(audit_entries[0].contains("\"verified\":3"));
    }

    // -- Test helpers --

    struct ImportRowResult {
        usuario: String,
        monto: f64,
        fecha: String,
        resultado: String,
    }

    fn mock_parse_csv(content: &str) -> Vec<ImportRowResult> {
        use std::io::Read;

        let mut reader = csv::ReaderBuilder::new()
            .flexible(true)
            .from_reader(content.as_bytes());

        let headers = match reader.headers() {
            Ok(h) => h.clone(),
            Err(_) => return vec![],
        };

        let find_col = |candidates: &[&str]| -> Option<usize> {
            for i in 0..headers.len() {
                if let Some(h) = headers.get(i) {
                    let lower = h.trim().to_lowercase();
                    for c in candidates {
                        if lower == *c {
                            return Some(i);
                        }
                    }
                }
            }
            None
        };

        let col_u = find_col(&["usuario", "user", "remitente", "nombre"]).unwrap_or(0);
        let col_m = find_col(&["monto", "amount", "importe"]).unwrap_or(1);
        let col_f = find_col(&["fecha", "date"]).unwrap_or(2);

        let mut results = Vec::new();

        for record in reader.records() {
            let record = match record {
                Ok(r) => r,
                Err(e) => {
                    results.push(ImportRowResult {
                        usuario: String::new(),
                        monto: 0.0,
                        fecha: String::new(),
                        resultado: format!("error: {}", e),
                    });
                    continue;
                }
            };

            let usuario = record.get(col_u).unwrap_or("").trim().to_string();
            let monto_str = record.get(col_m).unwrap_or("").trim().replace(",", "");
            let fecha = record.get(col_f).unwrap_or("").trim().to_string();

            if usuario.is_empty() || monto_str.is_empty() || fecha.is_empty() {
                results.push(ImportRowResult {
                    usuario,
                    monto: 0.0,
                    fecha,
                    resultado: "error: campos vacios".to_string(),
                });
                continue;
            }

            let monto: f64 = match monto_str.parse() {
                Ok(m) => m,
                Err(e) => {
                    results.push(ImportRowResult {
                        usuario,
                        monto: 0.0,
                        fecha,
                        resultado: format!("error: monto invalido: {}", e),
                    });
                    continue;
                }
            };

            // Validate date
            let date_ok = chrono::NaiveDate::parse_from_str(&fecha, "%Y-%m-%d").is_ok();
            if !date_ok {
                results.push(ImportRowResult {
                    usuario,
                    monto,
                    fecha,
                    resultado: "error: fecha invalida (use YYYY-MM-DD)".to_string(),
                });
                continue;
            }

            results.push(ImportRowResult {
                usuario,
                monto,
                fecha,
                resultado: "verificado".to_string(),
            });
        }

        results
    }

    fn mock_audit_on_import(
        filename: &str,
        total: usize,
        verified: usize,
        failed: usize,
    ) -> Vec<String> {
        vec![format!(
            "import_csv file={} total={} verified={} failed={}",
            filename, total, verified, failed
        )]
    }
}
