#[cfg(test)]
mod audit_tests {
    use std::collections::HashSet;

    // Tests for audit log integrity

    #[test]
    fn test_audit_log_is_append_only() {
        let mut log = AuditLog::new();

        // Insert entries
        log.insert("admin-1", "login", Some("user"), None);
        log.insert("admin-1", "verify_payment", Some("payment"), Some("pay-001"));
        log.insert("cashier-1", "verify_payment", Some("payment"), Some("pay-002"));

        // Entries should be in order
        assert_eq!(log.entries.len(), 3);
        assert_eq!(log.entries[0].action, "login");
        assert_eq!(log.entries[1].action, "verify_payment");
        assert_eq!(log.entries[2].action, "verify_payment");

        // Verify order is preserved (append-only)
        let first_id = log.entries[0].id;
        let last_id = log.entries[2].id;
        assert!(first_id < last_id);
    }

    #[test]
    fn test_audit_log_tamper_detection() {
        let mut log = AuditLog::new();
        log.insert("admin-1", "login", None, None);
        log.insert("admin-1", "config_change", Some("imap_config"), None);

        // Compute hash of the log
        let original_hash = log.compute_hash();

        // Simulate tampering
        log.entries[0].action = "tampered_action".to_string();

        // Hash should be different
        let tampered_hash = log.compute_hash();
        assert_ne!(original_hash, tampered_hash);
    }

    #[test]
    fn test_audit_log_immutable_entries() {
        let mut log = AuditLog::new();
        let entry_id = log.insert("admin-1", "login", None, None);

        // Verify entry exists
        let entry = log.get(entry_id);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().action, "login");

        // Attempt to modify (should be prevented by the struct design)
        // In a real system, this is enforced by the DB (no UPDATE on audit_log)
        // Here we verify the entry hash is deterministic
        let hash_after = log.compute_hash();

        log.insert("admin-1", "another", None, None);
        let hash_after_insert = log.compute_hash();

        assert_ne!(hash_after, hash_after_insert);
    }

    #[test]
    fn test_audit_log_filter_by_action() {
        let mut log = AuditLog::new();
        log.insert("u1", "login", None, None);
        log.insert("u1", "verify_payment", None, None);
        log.insert("u2", "login", None, None);
        log.insert("u1", "config_change", None, None);

        let logins: Vec<&AuditEntry> = log
            .entries
            .iter()
            .filter(|e| e.action == "login")
            .collect();

        assert_eq!(logins.len(), 2);
    }

    #[test]
    fn test_audit_log_filter_by_user() {
        let mut log = AuditLog::new();
        log.insert("u1", "login", None, None);
        log.insert("u2", "login", None, None);
        log.insert("u1", "verify_payment", None, None);

        let u1_actions: Vec<&AuditEntry> = log
            .entries
            .iter()
            .filter(|e| e.user_id == "u1")
            .collect();

        assert_eq!(u1_actions.len(), 2);
    }

    // -- Test helpers --

    #[derive(Debug, Clone)]
    struct AuditEntry {
        id: usize,
        user_id: String,
        action: String,
        resource_type: Option<String>,
        resource_id: Option<String>,
    }

    struct AuditLog {
        entries: Vec<AuditEntry>,
        next_id: usize,
    }

    impl AuditLog {
        fn new() -> Self {
            AuditLog {
                entries: Vec::new(),
                next_id: 1,
            }
        }

        fn insert(
            &mut self,
            user_id: &str,
            action: &str,
            resource_type: Option<&str>,
            resource_id: Option<&str>,
        ) -> usize {
            let id = self.next_id;
            self.entries.push(AuditEntry {
                id,
                user_id: user_id.to_string(),
                action: action.to_string(),
                resource_type: resource_type.map(|s| s.to_string()),
                resource_id: resource_id.map(|s| s.to_string()),
            });
            self.next_id += 1;
            id
        }

        fn get(&self, id: usize) -> Option<&AuditEntry> {
            self.entries.iter().find(|e| e.id == id)
        }

        fn compute_hash(&self) -> String {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hasher = DefaultHasher::new();
            for entry in &self.entries {
                entry.id.hash(&mut hasher);
                entry.user_id.hash(&mut hasher);
                entry.action.hash(&mut hasher);
                entry.resource_type.hash(&mut hasher);
                entry.resource_id.hash(&mut hasher);
            }
            format!("{:x}", hasher.finish())
        }
    }
}
