use serde::de::DeserializeOwned;
use serde::Serialize;
use std::time::Duration;

/// Cliente HTTP para el modo "client" del Tauri.
/// Cada comando de Tauri hace proxying a la API REST del servidor.
#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
    base_url: String,
    jwt_token: Option<String>,
}

#[allow(dead_code)]
impl HttpClient {
    pub fn new(base_url: &str) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .danger_accept_invalid_certs(true) // Para desarrollo LAN con TLS autofirmado
            .build()
            .map_err(|e| format!("Error creando HTTP client: {}", e))?;

        let base_url = base_url.trim_end_matches('/').to_string();

        Ok(HttpClient {
            client,
            base_url,
            jwt_token: None,
        })
    }

    /// Establece el token JWT para requests autenticados
    pub fn set_token(&mut self, token: String) {
        self.jwt_token = Some(token);
    }

    pub fn has_token(&self) -> bool {
        self.jwt_token.is_some()
    }

    // ---- Generic HTTP methods ----

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.get(&url);

        if let Some(token) = &self.jwt_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("HTTP GET error: {}", e))?;

        if resp.status().is_success() {
            resp.json::<T>()
                .await
                .map_err(|e| format!("JSON parse error: {}", e))
        } else {
            let status = resp.status();
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            Err(format!("HTTP {}: {}", status.as_u16(), body))
        }
    }

    pub async fn post<Req: Serialize, Res: DeserializeOwned>(
        &self,
        path: &str,
        body: &Req,
    ) -> Result<Res, String> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.post(&url).json(body);

        if let Some(token) = &self.jwt_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("HTTP POST error: {}", e))?;

        if resp.status().is_success() {
            resp.json::<Res>()
                .await
                .map_err(|e| format!("JSON parse error: {}", e))
        } else {
            let status = resp.status();
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            Err(format!("HTTP {}: {}", status.as_u16(), body))
        }
    }

    pub async fn post_json_value<Res: DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<Res, String> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.post(&url).json(body);

        if let Some(token) = &self.jwt_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("HTTP POST error: {}", e))?;

        if resp.status().is_success() {
            resp.json::<Res>()
                .await
                .map_err(|e| format!("JSON parse error: {}", e))
        } else {
            let status = resp.status();
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            Err(format!("HTTP {}: {}", status.as_u16(), body))
        }
    }

    // ---- Convenience methods for Tauri commands ----

    pub async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<serde_json::Value, String> {
        let body = serde_json::json!({
            "username": username,
            "password": password,
        });
        self.post_json_value("/api/auth/login", &body).await
    }

    pub async fn list_payments(
        &self,
        desde: Option<&str>,
        hasta: Option<&str>,
        monto_exacto: Option<f64>,
        monto_min: Option<f64>,
        monto_max: Option<f64>,
    ) -> Result<serde_json::Value, String> {
        let mut query = String::new();
        if let Some(d) = desde {
            query.push_str(&format!("desde={}", d));
        }
        if let Some(h) = hasta {
            if !query.is_empty() { query.push('&'); }
            query.push_str(&format!("hasta={}", h));
        }
        if let Some(m) = monto_exacto {
            if !query.is_empty() { query.push('&'); }
            query.push_str(&format!("monto_exacto={}", m));
        }
        if let Some(m) = monto_min {
            if !query.is_empty() { query.push('&'); }
            query.push_str(&format!("monto_min={}", m));
        }
        if let Some(m) = monto_max {
            if !query.is_empty() { query.push('&'); }
            query.push_str(&format!("monto_max={}", m));
        }
        let path = if query.is_empty() {
            "/api/payments".to_string()
        } else {
            format!("/api/payments?{}", query)
        };
        self.get(&path).await
    }

    pub async fn verify_payment(
        &self,
        usuario: &str,
        monto: f64,
        fecha: &str,
    ) -> Result<serde_json::Value, String> {
        let body = serde_json::json!({
            "usuario_empresa": usuario,
            "monto_empresa": monto,
            "fecha_empresa": fecha,
        });
        self.post_json_value("/api/payments/verify", &body).await
    }

    pub async fn trigger_sync(
        &self,
        mode: Option<&str>,
        since_date: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let body = serde_json::json!({
            "mode": mode,
            "since_date": since_date,
        });
        self.post_json_value("/api/sync/trigger", &body).await
    }

    pub async fn quick_verify_payment(
        &self,
        id: &str,
    ) -> Result<serde_json::Value, String> {
        let body = serde_json::json!({
            "observaciones": serde_json::Value::Null,
        });
        self.post_json_value(&format!("/api/payments/{}/verify", id), &body).await
    }

    pub async fn list_users(
        &self,
    ) -> Result<serde_json::Value, String> {
        self.get("/api/users").await
    }

    pub async fn create_user(
        &self,
        username: &str,
        password: &str,
        role: &str,
        station_name: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let body = serde_json::json!({
            "username": username,
            "password": password,
            "role": role,
            "station_name": station_name,
        });
        self.post_json_value("/api/users", &body).await
    }

    pub async fn update_user(
        &self,
        id: &str,
        updates: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        // The server expects PUT, but we use post_json_value which does POST.
        // We need a put_json_value helper or use put directly.
        let url = format!("{}/api/users/{}", self.base_url, id);
        let mut req = self.client.put(&url).json(updates);

        if let Some(token) = &self.jwt_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("HTTP PUT error: {}", e))?;

        if resp.status().is_success() {
            resp.json::<serde_json::Value>()
                .await
                .map_err(|e| format!("JSON parse error: {}", e))
        } else {
            let status = resp.status();
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            Err(format!("HTTP {}: {}", status.as_u16(), body))
        }
    }
}
