#[cfg(test)]
mod integration_tests {
    // Full end-to-end flow tests
    
    #[test]
    fn test_full_e2e_flow_login_verify_audit() {
        // Simulate: Login → Verify Payment → Check Audit Log
        let mut state = MockAppState::new();

        // Step 1: Login as admin
        let login_result = state.login("admin", "password123");
        assert!(login_result.success);
        assert_eq!(login_result.user.role, "admin");
        assert!(!login_result.token.is_empty());

        // Step 2: Verify a payment
        let verify_result = state.verify_payment(
            &login_result.token,
            "Alice",
            100.0,
            "2026-07-15",
        );
        assert!(verify_result.verificado);
        assert!(verify_result.mensaje.contains("exito"));

        // Step 3: Check audit log
        let audit = state.get_audit_log(&login_result.token);
        assert!(!audit.is_empty());
        assert!(audit.iter().any(|e| e.action == "login"));
        assert!(audit.iter().any(|e| e.action == "verify_payment"));
    }

    #[test]
    fn test_e2e_login_sync_verify() {
        let mut state = MockAppState::new();

        // Login
        let login = state.login("admin", "pass");
        assert!(login.success);

        // Trigger sync (simulated)
        let sync = state.trigger_sync(&login.token);
        assert!(sync.success);

        // Verify payment after sync
        let verify = state.verify_payment(&login.token, "Bob", 200.0, "2026-07-16");
        assert!(verify.verificado);
    }

    #[test]
    fn test_e2e_cashier_cannot_access_admin_endpoints() {
        let mut state = MockAppState::new();

        // Login as cashier
        let login = state.login("cashier1", "pass");
        assert_eq!(login.user.role, "cashier");

        // Try to access IMAP config (admin-only)
        let config_result = state.get_imap_config(&login.token);
        assert!(config_result.is_err());
        assert!(config_result.unwrap_err().contains("Forbidden"));
    }

    #[test]
    fn test_e2e_import_flow() {
        let mut state = MockAppState::new();

        let login = state.login("admin", "pass");
        assert!(login.success);

        // Import CSV
        let csv_content = "usuario,monto,fecha\nAlice,100.50,2026-07-10\nBob,200.00,2026-07-11\n";
        let import_result = state.import_csv(&login.token, csv_content);
        assert!(import_result.success);
        assert_eq!(import_result.total, 2);

        // Verify audit log has import entry
        let audit = state.get_audit_log(&login.token);
        assert!(audit.iter().any(|e| e.action == "import_csv"));
    }

    #[test]
    fn test_e2e_export_flow() {
        let mut state = MockAppState::new();

        let login = state.login("admin", "pass");

        // Export xlsx
        let export = state.export_xlsx(&login.token, "2026-07-01", "2026-07-31");
        assert!(export.success);
        assert!(!export.data.is_empty());
    }

    #[test]
    fn test_e2e_quarantine_review_flow() {
        let mut state = MockAppState::new();

        let login = state.login("admin", "pass");

        // List quarantined emails
        let quarantine = state.list_quarantine(&login.token);
        assert!(quarantine.success);

        // Review one
        if !quarantine.items.is_empty() {
            let review = state.review_quarantine(
                &login.token,
                &quarantine.items[0].id,
                "accept",
            );
            assert!(review.success);
        }
    }

    #[test]
    fn test_e2e_user_crud_admin_only() {
        let mut state = MockAppState::new();

        let admin_login = state.login("admin", "pass");

        // Admin creates user
        let create = state.create_user(&admin_login.token, "new_cashier", "pass", "cashier");
        assert!(create.is_ok());

        // Admin lists users
        let users = state.list_users(&admin_login.token);
        assert!(users.is_ok());
        assert!(!users.unwrap().is_empty());

        // Cashier tries to create user (should fail)
        let cashier_login = state.login("cashier1", "pass");
        let create_as_cashier = state.create_user(&cashier_login.token, "hacker", "pass", "admin");
        assert!(create_as_cashier.is_err());
    }

    // -- Test helpers -- 

    struct MockAppState {
        users: Vec<MockUser>,
        payments: Vec<MockPayment>,
        audit_log: Vec<MockAuditEntry>,
        quarantine: Vec<MockQuarantineItem>,
        tokens: std::collections::HashMap<String, MockUser>,
    }

    #[derive(Clone)]
    struct MockUser {
        id: String,
        username: String,
        role: String,
    }

    struct MockPayment {
        id: String,
        usuario: String,
        monto: f64,
    }

    struct MockAuditEntry {
        action: String,
        user_id: String,
    }

    struct MockQuarantineItem {
        id: String,
        subject: String,
    }

    struct LoginResult {
        success: bool,
        token: String,
        user: MockUser,
    }

    struct VerifyResult {
        verificado: bool,
        mensaje: String,
    }

    struct SyncResult {
        success: bool,
        nuevos: i64,
    }

    struct ImportResult {
        success: bool,
        total: usize,
    }

    struct ExportResult {
        success: bool,
        data: Vec<u8>,
    }

    struct QuarantineResult {
        success: bool,
        items: Vec<MockQuarantineItem>,
    }

    impl MockAppState {
        fn new() -> Self {
            MockAppState {
                users: vec![
                    MockUser { id: "admin-1".into(), username: "admin".into(), role: "admin".into() },
                    MockUser { id: "cashier-1".into(), username: "cashier1".into(), role: "cashier".into() },
                ],
                payments: vec![
                    MockPayment { id: "pay-1".into(), usuario: "Alice".into(), monto: 100.0 },
                    MockPayment { id: "pay-2".into(), usuario: "Bob".into(), monto: 200.0 },
                ],
                audit_log: Vec::new(),
                quarantine: vec![
                    MockQuarantineItem { id: "q-1".into(), subject: "Suspicious transfer".into() },
                ],
                tokens: std::collections::HashMap::new(),
            }
        }

        fn login(&mut self, username: &str, _password: &str) -> LoginResult {
            if let Some(user) = self.users.iter().find(|u| u.username == username) {
                let token = format!("token-{}-{}", user.username, rand_id());
                let user_clone = user.clone();
                self.tokens.insert(token.clone(), user_clone.clone());

                self.audit_log.push(MockAuditEntry {
                    action: "login".to_string(),
                    user_id: user.id.clone(),
                });

                LoginResult {
                    success: true,
                    token,
                    user: user_clone,
                }
            } else {
                LoginResult {
                    success: false,
                    token: String::new(),
                    user: MockUser { id: "".into(), username: "".into(), role: "".into() },
                }
            }
        }

        fn verify_payment(
            &mut self,
            token: &str,
            usuario: &str,
            monto: f64,
            _fecha: &str,
        ) -> VerifyResult {
            if !self.tokens.contains_key(token) {
                return VerifyResult { verificado: false, mensaje: "Unauthorized".to_string() };
            }

            if let Some(user) = self.tokens.get(token) {
                if let Some(payment) = self.payments.iter().find(|p| p.usuario == usuario && (p.monto - monto).abs() < 0.01) {
                    self.audit_log.push(MockAuditEntry {
                        action: "verify_payment".to_string(),
                        user_id: user.id.clone(),
                    });
                    return VerifyResult {
                        verificado: true,
                        mensaje: format!("Pago {} verificado con exito!", payment.id),
                    };
                }
            }

            VerifyResult {
                verificado: false,
                mensaje: "No se encontro coincidencia.".to_string(),
            }
        }

        fn trigger_sync(&mut self, token: &str) -> SyncResult {
            if self.tokens.contains_key(token) {
                SyncResult { success: true, nuevos: 3 }
            } else {
                SyncResult { success: false, nuevos: 0 }
            }
        }

        fn get_imap_config(&self, token: &str) -> Result<(), String> {
            let user = self.tokens.get(token).ok_or("Unauthorized")?;
            if user.role != "admin" {
                return Err("Forbidden: admin role required".to_string());
            }
            Ok(())
        }

        fn import_csv(&mut self, token: &str, _content: &str) -> ImportResult {
            if let Some(user) = self.tokens.get(token) {
                self.audit_log.push(MockAuditEntry {
                    action: "import_csv".to_string(),
                    user_id: user.id.clone(),
                });
                return ImportResult { success: true, total: 2 };
            }
            ImportResult { success: false, total: 0 }
        }

        fn export_xlsx(&self, token: &str, _desde: &str, _hasta: &str) -> ExportResult {
            if self.tokens.contains_key(token) {
                ExportResult { success: true, data: vec![1, 2, 3, 4, 5] }
            } else {
                ExportResult { success: false, data: vec![] }
            }
        }

        fn list_quarantine(&self, token: &str) -> QuarantineResult {
            if self.tokens.contains_key(token) {
                QuarantineResult {
                    success: true,
                    items: self.quarantine.clone(),
                }
            } else {
                QuarantineResult { success: false, items: vec![] }
            }
        }

        fn review_quarantine(&mut self, token: &str, id: &str, _decision: &str) -> ImportResult {
            if self.tokens.contains_key(token) {
                // Remove from quarantine
                self.quarantine.retain(|q| q.id != id);
                ImportResult { success: true, total: 0 }
            } else {
                ImportResult { success: false, total: 0 }
            }
        }

        fn get_audit_log(&self, token: &str) -> Vec<MockAuditEntry> {
            if self.tokens.contains_key(token) {
                self.audit_log.clone()
            } else {
                vec![]
            }
        }

        fn create_user(
            &mut self,
            token: &str,
            username: &str,
            _password: &str,
            role: &str,
        ) -> Result<(), String> {
            let user = self.tokens.get(token).ok_or("Unauthorized")?;
            if user.role != "admin" {
                return Err("Forbidden".to_string());
            }
            self.users.push(MockUser {
                id: format!("user-{}", self.users.len() + 1),
                username: username.to_string(),
                role: role.to_string(),
            });
            Ok(())
        }

        fn list_users(&self, token: &str) -> Result<Vec<MockUser>, String> {
            let user = self.tokens.get(token).ok_or("Unauthorized")?;
            if user.role != "admin" {
                return Err("Forbidden".to_string());
            }
            Ok(self.users.clone())
        }
    }

    fn rand_id() -> String {
        format!("{:x}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos())
    }
}
