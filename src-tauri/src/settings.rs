use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub imap_user: String,
    pub imap_password: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            imap_user: String::new(),
            imap_password: String::new(),
        }
    }
}

fn get_settings_path() -> PathBuf {
    let app_data = std::env::var("APPDATA")
        .or_else(|_| std::env::var("LOCALAPPDATA"))
        .unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(app_data).join("binance-auditor");
    std::fs::create_dir_all(&dir).ok();
    dir.join("settings.json")
}

pub fn load_settings() -> AppSettings {
    let path = get_settings_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str(&content) {
                return settings;
            }
        }
    }
    AppSettings::default()
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = get_settings_path();
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Serialize error: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("Write error: {}", e))?;
    Ok(())
}
