use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuracion de modo de operacion del cliente Tauri
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientModeConfig {
    pub mode: String,                     // "standalone" | "client"
    pub server_url: Option<String>,       // URL base del servidor, ej: https://192.168.1.100:8443
    pub tls_fingerprint: Option<String>,  // sha256 fingerprint para pinning TLS
}

impl Default for ClientModeConfig {
    fn default() -> Self {
        ClientModeConfig {
            mode: "standalone".to_string(),
            server_url: None,
            tls_fingerprint: None,
        }
    }
}

impl ClientModeConfig {
    /// Determina el modo de operacion desde archivo de config o variable de entorno.
    /// Prioridad:
    /// 1. Archivo auditor-mode.json en %APPDATA%\binance-auditor\
    /// 2. Variable de entorno AUDITOR_MODE
    /// 3. Default: standalone
    pub fn detect() -> Self {
        let config_file = get_config_path();

        if config_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_file) {
                if let Ok(config) = serde_json::from_str::<ClientModeConfig>(&content) {
                    if !config.mode.is_empty() {
                        return config;
                    }
                }
            }
        }

        // Fallback to env var
        if let Ok(mode) = std::env::var("AUDITOR_MODE") {
            return ClientModeConfig {
                mode,
                server_url: std::env::var("AUDITOR_SERVER_URL").ok(),
                tls_fingerprint: std::env::var("AUDITOR_TLS_FINGERPRINT").ok(),
            };
        }

        ClientModeConfig::default()
    }

    pub fn is_client_mode(&self) -> bool {
        self.mode == "client"
    }

    pub fn server_url(&self) -> Option<&str> {
        self.server_url.as_deref()
    }
}

fn get_config_path() -> PathBuf {
    let app_data = std::env::var("APPDATA")
        .or_else(|_| std::env::var("LOCALAPPDATA"))
        .unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(app_data).join("binance-auditor");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("auditor-mode.json")
}
