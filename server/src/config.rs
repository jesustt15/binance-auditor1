use age::secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuracion del servidor, cargada de variables de entorno
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// PostgreSQL connection string
    pub database_url: String,
    /// JWT signing secret (256-bit random)
    pub jwt_secret: String,
    /// Server listen address
    pub listen_addr: String,
    /// TLS certificate path (PEM)
    pub tls_cert_path: Option<String>,
    /// TLS key path (PEM)
    pub tls_key_path: Option<String>,
    /// age encryption key path for credential storage
    pub age_key_path: String,
    /// IMAP poll interval in seconds (default: 300 = 5 min)
    pub imap_poll_interval_secs: u64,
}

impl ServerConfig {
    /// Carga configuracion desde variables de entorno (env vars)
    pub fn from_env() -> Result<Self, String> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://auditor:auditor@localhost:5432/auditor_db".to_string());

        let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
            // NOTA: en produccion esto DEBE ser un secret fuerte. El default de 32 bytes
            // es solo para desarrollo local.
            "CHANGE_ME_IN_PRODUCTION_32_BYTES_MINIMUM!!".to_string()
        });

        // Warn if using default secret
        if jwt_secret == "CHANGE_ME_IN_PRODUCTION_32_BYTES_MINIMUM!!" {
            eprintln!("[WARN] Usando JWT_SECRET por defecto. Configuralo en produccion!");
        }
        if jwt_secret.len() < 32 {
            return Err(format!(
                "JWT_SECRET debe tener al menos 32 caracteres (tiene {})",
                jwt_secret.len()
            ));
        }

        let listen_addr =
            std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8443".to_string());

        let tls_cert_path = std::env::var("TLS_CERT_PATH").ok();
        let tls_key_path = std::env::var("TLS_KEY_PATH").ok();

        let age_key_path =
            std::env::var("AGE_KEY_PATH").unwrap_or_else(|_| get_default_age_key_path());

        let imap_poll_interval_secs = std::env::var("IMAP_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300); // 5 minutes default

        Ok(ServerConfig {
            database_url,
            jwt_secret,
            listen_addr,
            tls_cert_path,
            tls_key_path,
            age_key_path,
            imap_poll_interval_secs,
        })
    }
}

fn get_default_age_key_path() -> String {
    let app_data = std::env::var("APPDATA")
        .or_else(|_| std::env::var("LOCALAPPDATA"))
        .unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(app_data).join("binance-auditor-server");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("age_key.txt").to_string_lossy().to_string()
}

/// Genera una clave age si no existe, o la carga del disco
pub fn load_or_create_age_key(path: &str) -> Result<age::x25519::Identity, String> {
    let key_path = std::path::Path::new(path);

    if key_path.exists() {
        let key_str = std::fs::read_to_string(key_path)
            .map_err(|e| format!("Error leyendo age key: {}", e))?;
        let trimmed = key_str.trim();
        trimmed
            .parse::<age::x25519::Identity>()
            .map_err(|e| format!("Error parseando age key: {}", e))
    } else {
        let identity = age::x25519::Identity::generate();
        let key_data = identity.to_string();
        std::fs::write(key_path, key_data.expose_secret())
            .map_err(|e| format!("Error guardando age key: {}", e))?;
        Ok(identity)
    }
}

/// Encripta texto con age (usando la clave publica derivada del Identity)
pub fn encrypt_with_age(identity: &age::x25519::Identity, plaintext: &str) -> Result<String, String> {
    let recipient = identity.to_public();
    let encryptor = age::Encryptor::with_recipients([&recipient as &dyn age::Recipient].into_iter())
        .map_err(|e| format!("Error creando encryptor age: {}", e))?;

    let mut encrypted = vec![];
    let mut writer = encryptor
        .wrap_output(&mut encrypted)
        .map_err(|e| format!("Error encriptando con age: {}", e))?;

    std::io::Write::write_all(&mut writer, plaintext.as_bytes())
        .map_err(|e| format!("Error escribiendo datos encriptados: {}", e))?;
    std::io::Write::flush(&mut writer)
        .map_err(|e| format!("Error finalizando encriptacion: {}", e))?;

    Ok(String::from_utf8_lossy(&encrypted).to_string())
}

/// Desencripta texto con age
pub fn decrypt_with_age(identity: &age::x25519::Identity, encrypted: &str) -> Result<String, String> {
    let decryptor = age::Decryptor::new(std::io::Cursor::new(encrypted.as_bytes()))
        .map_err(|e| format!("Error creando decryptor age: {}", e))?;

    let mut decrypted = vec![];
    let mut reader = decryptor
        .decrypt(std::iter::once(identity as &dyn age::Identity))
        .map_err(|e| format!("Error desencriptando con age: {}", e))?;

    std::io::Read::read_to_end(&mut reader, &mut decrypted)
        .map_err(|e| format!("Error leyendo datos desencriptados: {}", e))?;

    String::from_utf8(decrypted).map_err(|e| format!("Datos desencriptados no son UTF-8: {}", e))
}
