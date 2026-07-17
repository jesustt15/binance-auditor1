use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Enum types shared across API and DB
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum PaymentType {
    #[serde(rename = "pago")]
    #[sqlx(rename = "pago")]
    Pago,
    #[serde(rename = "deposito")]
    #[sqlx(rename = "deposito")]
    Deposito,
}

impl std::fmt::Display for PaymentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentType::Pago => write!(f, "pago"),
            PaymentType::Deposito => write!(f, "deposito"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum PaymentStatus {
    #[serde(rename = "disponible")]
    #[sqlx(rename = "disponible")]
    Disponible,
    #[serde(rename = "verificado")]
    #[sqlx(rename = "verificado")]
    Verificado,
    #[serde(rename = "rechazado")]
    #[sqlx(rename = "rechazado")]
    Rechazado,
    #[serde(rename = "por_revisar")]
    #[sqlx(rename = "por_revisar")]
    PorRevisar,
    #[serde(rename = "cuarentena")]
    #[sqlx(rename = "cuarentena")]
    Cuarentena,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum FraudVerdict {
    #[serde(rename = "clean")]
    #[sqlx(rename = "clean")]
    Clean,
    #[serde(rename = "dkim_fail")]
    #[sqlx(rename = "dkim_fail")]
    DkimFail,
    #[serde(rename = "spf_fail")]
    #[sqlx(rename = "spf_fail")]
    SpfFail,
    #[serde(rename = "dmarc_fail")]
    #[sqlx(rename = "dmarc_fail")]
    DmarcFail,
    #[serde(rename = "duplicate")]
    #[sqlx(rename = "duplicate")]
    Duplicate,
    #[serde(rename = "amount_tamper")]
    #[sqlx(rename = "amount_tamper")]
    AmountTamper,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum UserRole {
    #[serde(rename = "admin")]
    #[sqlx(rename = "admin")]
    Admin,
    #[serde(rename = "cashier")]
    #[sqlx(rename = "cashier")]
    Cashier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum ReviewDecision {
    #[serde(rename = "pending")]
    #[sqlx(rename = "pending")]
    Pending,
    #[serde(rename = "accept")]
    #[sqlx(rename = "accept")]
    Accept,
    #[serde(rename = "reject")]
    #[sqlx(rename = "reject")]
    Reject,
}

// ---------------------------------------------------------------------------
// Table row structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: UserRole,
    pub station_name: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Public-safe user view (no password hash)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPublic {
    pub id: Uuid,
    pub username: String,
    pub role: UserRole,
    pub station_name: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        UserPublic {
            id: u.id,
            username: u.username,
            role: u.role,
            station_name: u.station_name,
            is_active: u.is_active,
            created_at: u.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ImapCredentials {
    pub id: Uuid,
    pub email: String,
    pub encrypted_password: String,
    pub imap_host: String,
    pub imap_port: i32,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Public-safe IMAP creds view (no encrypted password)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapCredentialsPublic {
    pub id: Uuid,
    pub email: String,
    pub imap_host: String,
    pub imap_port: i32,
    pub last_sync_at: Option<DateTime<Utc>>,
}

impl From<ImapCredentials> for ImapCredentialsPublic {
    fn from(c: ImapCredentials) -> Self {
        ImapCredentialsPublic {
            id: c.id,
            email: c.email,
            imap_host: c.imap_host,
            imap_port: c.imap_port,
            last_sync_at: c.last_sync_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Payment {
    pub id: Uuid,
    pub tipo: PaymentType,
    pub usuario_remitente: Option<String>,
    pub monto: f64,
    pub moneda: String,
    pub fecha_correo: DateTime<Utc>,
    pub estado: PaymentStatus,
    pub observaciones: Option<String>,
    pub verificado_en: Option<DateTime<Utc>>,
    pub verified_by: Option<Uuid>,
    pub fraud_verdict: Option<String>,
    pub fraud_details: Option<serde_json::Value>,
    pub email_uid: Option<i32>,
    pub raw_headers: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// API response shape (matches existing TipoScript `PagoBinance` layout)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResponse {
    pub id: String, // UUID as string
    pub tipo: String,
    pub usuario_remitente: Option<String>,
    pub monto: f64,
    pub moneda: String,
    pub fecha_correo: String,
    pub estado: String,
    pub observaciones: Option<String>,
    pub verificado_en: Option<String>,
    pub fraud_verdict: Option<String>,
    pub created_at: String,
}

impl From<Payment> for PaymentResponse {
    fn from(p: Payment) -> Self {
        PaymentResponse {
            id: p.id.to_string(),
            tipo: p.tipo.to_string(),
            usuario_remitente: p.usuario_remitente,
            monto: p.monto,
            moneda: p.moneda,
            fecha_correo: p.fecha_correo.to_rfc3339(),
            estado: match p.estado {
                PaymentStatus::Disponible => "disponible".to_string(),
                PaymentStatus::Verificado => "verificado".to_string(),
                PaymentStatus::Rechazado => "rechazado".to_string(),
                PaymentStatus::PorRevisar => "por_revisar".to_string(),
                PaymentStatus::Cuarentena => "cuarentena".to_string(),
            },
            observaciones: p.observaciones,
            verificado_en: p.verificado_en.map(|v| v.to_rfc3339()),
            fraud_verdict: p.fraud_verdict,
            created_at: p.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct QuarantinedEmail {
    pub id: Uuid,
    pub raw_email: String,
    pub subject: Option<String>,
    pub from_address: Option<String>,
    pub failure_reason: String,
    pub dkim_result: Option<String>,
    pub spf_result: Option<String>,
    pub dmarc_result: Option<String>,
    pub parsed_data: Option<serde_json::Value>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub review_decision: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditEntry {
    pub id: i64,
    pub user_id: Option<Uuid>,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub details: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Import {
    pub id: Uuid,
    pub filename: String,
    pub file_type: String,
    pub total_rows: i32,
    pub verified_rows: i32,
    pub failed_rows: i32,
    pub uploaded_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ImportRow {
    pub id: Uuid,
    pub import_id: Uuid,
    pub row_number: i32,
    pub usuario: Option<String>,
    pub monto: Option<f64>,
    pub fecha: Option<String>,
    pub result: String,
    pub error_message: Option<String>,
    pub matched_payment_id: Option<Uuid>,
}

// ---------------------------------------------------------------------------
// Auth DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub refresh_token: String,
    pub user: UserPublic,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub token: String,
}

// ---------------------------------------------------------------------------
// JWT claims
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,      // user UUID
    pub username: String,
    pub role: String,     // "admin" or "cashier"
    pub iat: usize,
    pub exp: usize,
}

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct VerifyPaymentRequest {
    pub usuario_empresa: String,
    pub monto_empresa: f64,
    pub fecha_empresa: String, // YYYY-MM-DD
}

#[derive(Debug, Serialize)]
pub struct VerifyPaymentResponse {
    pub verificado: bool,
    pub mensaje: String,
    pub data: Option<PaymentResponse>,
    pub fraud_verdict: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaymentQuery {
    pub desde: Option<String>,
    pub hasta: Option<String>,
    pub monto_exacto: Option<f64>,
    pub monto_min: Option<f64>,
    pub monto_max: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct BySenderQuery {
    pub desde: Option<String>,
    pub hasta: Option<String>,
    pub remitente: Option<String>,
    pub monto_exacto: Option<f64>,
    pub monto_min: Option<f64>,
    pub monto_max: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct SyncStatusResponse {
    pub last_sync: Option<DateTime<Utc>>,
    pub next_sync: Option<i64>, // seconds until next poll
    pub status: String,         // "idle", "syncing", "error"
}

#[derive(Debug, Deserialize)]
pub struct TriggerSyncRequest {
    pub mode: Option<String>,     // "normal" or "historical"
    pub since_date: Option<String>, // YYYY-MM-DD
}

#[derive(Debug, Serialize)]
pub struct TriggerSyncResponse {
    pub success: bool,
    pub nuevos: i64,
    pub total: usize,
}

#[derive(Debug, Deserialize)]
pub struct ImapConfigUpdate {
    pub email: String,
    pub password: String,
    pub host: Option<String>,
    pub port: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct ImportResultResponse {
    pub total: usize,
    pub verified: usize,
    pub failed: usize,
    pub details: Vec<ImportRowDetail>,
}

#[derive(Debug, Serialize)]
pub struct ImportRowDetail {
    pub row: usize,
    pub usuario: String,
    pub monto: f64,
    pub fecha: String,
    pub result: String,
}

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub action: Option<String>,
    pub user_id: Option<Uuid>,
    pub desde: Option<String>,
    pub hasta: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct QuarantineReview {
    pub decision: String, // "accept" or "reject"
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: String,
    pub station_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub role: Option<String>,
    pub is_active: Option<bool>,
    pub station_name: Option<String>,
}

// ---------------------------------------------------------------------------
// Fraud engine types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FraudResult {
    pub verdict: String,     // "clean", "dkim_fail", etc.
    pub details: serde_json::Value,
    pub quarantined: bool,
    pub reason: Option<String>,
}

/// Raw email data extracted before persistence (matches existing BinanceEmailData)
#[derive(Debug, Clone)]
pub struct RawEmailData {
    pub usuario: Option<String>,
    pub monto: f64,
    pub moneda: String,
    pub fecha: String, // RFC3339
    pub tipo: String,  // "pago" or "deposito"
}

// ---------------------------------------------------------------------------
// Client mode types (shared for server-side reference)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientModeConfig {
    pub mode: String,       // "standalone" or "client"
    pub server_url: Option<String>,
    pub tls_fingerprint: Option<String>,
}
