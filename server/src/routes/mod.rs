use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{NaiveDate, Utc};
use serde_json::json;
use std::sync::Arc;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use uuid::Uuid;

use crate::auth::{self, AuthUser, require_admin};
use crate::config::{self, ServerConfig};
use crate::db;
use crate::imap_sync;
use crate::models::*;

pub struct AppState {
    pub pool: sqlx::PgPool,
    pub config: ServerConfig,
    pub age_identity: age::x25519::Identity,
}

pub type SharedState = Arc<AppState>;

/// Construye el router de la API REST con rate limiting
pub fn build_router(state: SharedState) -> Router {
    // Rate limiting estricto para auth (anti brute-force): 5 intentos burst, 1/seg
    let auth_gov_conf = GovernorConfigBuilder::default()
        .per_second(1)
        .burst_size(5)
        .finish()
        .expect("Failed to build auth rate limiter config");

    // Rate limiting moderado para todas las demas rutas: 30 req/s con burst 60
    let api_gov_conf = GovernorConfigBuilder::default()
        .per_second(30)
        .burst_size(60)
        .finish()
        .expect("Failed to build API rate limiter config");

    let auth_routes = Router::new()
        .route("/api/auth/login", post(login_handler))
        .route("/api/auth/refresh", post(refresh_handler))
        .layer(GovernorLayer::new(auth_gov_conf))
        .with_state(Arc::clone(&state));

    let api_routes = Router::new()
        // Health check (no auth)
        .route("/health", get(health_handler))
        // Payments
        .route("/api/payments", get(list_payments_handler))
        .route("/api/payments/by-sender", get(list_payments_by_sender_handler))
        .route("/api/payments/verify", post(verify_payment_handler))
        .route("/api/payments/{id}", get(get_payment_handler))
        .route("/api/payments/{id}/verify", post(quick_verify_handler))
        // Sync
        .route("/api/sync/status", get(sync_status_handler))
        .route("/api/sync/trigger", post(trigger_sync_handler))
        // Change password (authenticated)
        .route("/api/auth/change-password", post(change_password_handler))
        // IMAP
        .route("/api/config/imap", get(get_imap_config_handler).put(put_imap_config_handler))
        // Import
        .route("/api/import/csv", post(import_csv_handler))
        .route("/api/import/excel", post(import_excel_handler))
        // Export
        .route("/api/export/xlsx", get(export_xlsx_handler))
        // Audit (admin)
        .route("/api/audit", get(audit_log_handler))
        // Quarantine (admin)
        .route("/api/quarantine", get(list_quarantine_handler))
        .route("/api/quarantine/{id}/review", post(review_quarantine_handler))
        // Users (admin)
        .route("/api/users", get(list_users_handler).post(create_user_handler))
        .route("/api/users/{id}", put(update_user_handler).delete(delete_user_handler))
        .route("/api/users/{id}/reset-password", post(admin_reset_password_handler))
        .layer(GovernorLayer::new(api_gov_conf))
        .with_state(state);

    auth_routes.merge(api_routes)
}

// ===================== HELPERS =====================

/// Loguea el error real y devuelve un 500 genérico al cliente (sin leak de BD)
fn internal_error(context: &str, err: &str) -> axum::response::Response {
    tracing::error!("{}: {}", context, err);
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Error interno del servidor"}))).into_response()
}

/// Valida complejidad de contraseña. Retorna Some(mensaje) si es inválida.
fn validate_password(password: &str) -> Option<&'static str> {
    if password.len() < 8 {
        return Some("La contraseña debe tener al menos 8 caracteres");
    }
    let has_upper = password.chars().any(|c| c.is_uppercase());
    let has_lower = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_alphanumeric());
    if !has_upper || !has_lower || !has_digit || !has_special {
        return Some("La contraseña debe incluir mayúsculas, minúsculas, números y caracteres especiales");
    }
    None
}

// ===================== HEALTH =====================

/// Health check para Docker / load balancers.
/// Verifica conectividad con la base de datos.
async fn health_handler(
    State(state): State<SharedState>,
) -> impl IntoResponse {
    // Verificar que la DB responde
    let db_ok = sqlx::query("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();

    if db_ok {
        (StatusCode::OK, Json(json!({"status": "ok", "db": "connected"})))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"status": "degraded", "db": "disconnected"})),
        )
    }
}

// ===================== AUTH =====================

async fn login_handler(
    State(state): State<SharedState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let user = match db::find_user_by_username(&state.pool, &req.username).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Credenciales invalidas"})),
            )
                .into_response();
        }
        Err(e) => {
            return internal_error("Error buscando usuario en login", &e);
        }
    };

    if !user.is_active {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Usuario desactivado"})),
        )
            .into_response();
    }

    match auth::verify_password(&req.password, &user.password_hash) {
        Ok(true) => {}
        _ => {
            // Log failed attempt
            let _ = db::insert_audit_entry(
                &state.pool,
                Some(user.id),
                "login_failed",
                Some("user"),
                Some(user.id),
                Some(&json!({"username": req.username})),
                None,
                None,
            )
            .await;
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Credenciales invalidas"})),
            )
                .into_response();
        }
    }

    // Generate tokens
    let access_token = auth::generate_access_token(
        &user.id.to_string(),
        &user.username,
        match user.role {
            UserRole::Admin => "admin",
            UserRole::Cashier => "cashier",
        },
        user.company_group.as_deref(),
        &state.config.jwt_secret,
    )
    .unwrap_or_default();

    let refresh_token = auth::generate_refresh_token(&user.id.to_string(), &state.config.jwt_secret)
        .unwrap_or_default();

    // Log successful login
    let _ = db::insert_audit_entry(
        &state.pool,
        Some(user.id),
        "login",
        Some("user"),
        Some(user.id),
        Some(&json!({"username": req.username})),
        None,
        None,
    )
    .await;

    let user_public: UserPublic = user.into();
    (
        StatusCode::OK,
        Json(json!({
            "token": access_token,
            "refresh_token": refresh_token,
            "user": user_public,
        })),
    )
        .into_response()
}

async fn refresh_handler(
    State(state): State<SharedState>,
    Json(req): Json<RefreshRequest>,
) -> impl IntoResponse {
    let claims = match auth::decode_token(&req.refresh_token, &state.config.jwt_secret) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Refresh token invalido o expirado"})),
            )
                .into_response();
        }
    };

    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Token malformado"})),
            )
                .into_response();
        }
    };

    let user = match db::find_user_by_id(&state.pool, user_id).await {
        Ok(Some(u)) => u,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Usuario no encontrado"})),
            )
                .into_response();
        }
    };

    let new_token = auth::generate_access_token(
        &user.id.to_string(),
        &user.username,
        match user.role {
            UserRole::Admin => "admin",
            UserRole::Cashier => "cashier",
        },
        user.company_group.as_deref(),
        &state.config.jwt_secret,
    )
    .unwrap_or_default();

    (StatusCode::OK, Json(json!({"token": new_token}))).into_response()
}

// ===================== PAYMENTS =====================

async fn list_payments_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Query(query): Query<PaymentQuery>,
) -> impl IntoResponse {
    let desde = query.desde.unwrap_or_else(|| {
        (Utc::now() - chrono::Duration::days(30))
            .format("%Y-%m-%dT00:00:00Z")
            .to_string()
    });
    let hasta = query.hasta.unwrap_or_else(|| {
        Utc::now().format("%Y-%m-%dT23:59:59Z").to_string()
    });

    // Resolve cashier filter: cashiers see only their own verified payments + all available ones
    let cashier_id = if auth.role == "cashier" {
        Uuid::parse_str(&auth.user_id).ok()
    } else {
        None
    };

    match db::list_payments(
        &state.pool,
        &desde,
        &hasta,
        query.monto_exacto,
        query.monto_min,
        query.monto_max,
    )
    .await
    {
        Ok(payments) => {
            let filtered: Vec<_> = if let Some(cid) = cashier_id {
                payments
                    .into_iter()
                    .filter(|p| p.verified_by.is_none() || p.verified_by == Some(cid))
                    .collect()
            } else {
                payments
            };
            let response: Vec<PaymentResponse> =
                filtered.into_iter().map(PaymentResponse::from).collect();
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Error interno del servidor"})),
        )
            .into_response(),
    }
}

async fn list_payments_by_sender_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Query(query): Query<BySenderQuery>,
) -> impl IntoResponse {
    let desde = query.desde.unwrap_or_else(|| {
        (Utc::now() - chrono::Duration::days(30))
            .format("%Y-%m-%dT00:00:00Z")
            .to_string()
    });
    let hasta = query.hasta.unwrap_or_else(|| {
        Utc::now().format("%Y-%m-%dT23:59:59Z").to_string()
    });

    let remitente = query.remitente.unwrap_or_default();
    if remitente.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Se requiere el parametro 'remitente'"})),
        )
            .into_response();
    }

    // Resolve cashier filter: cashiers see only their own verified payments + all available ones
    let cashier_id = if auth.role == "cashier" {
        Uuid::parse_str(&auth.user_id).ok()
    } else {
        None
    };

    match db::list_payments_by_sender(
        &state.pool,
        &desde,
        &hasta,
        &remitente,
        query.monto_exacto,
        query.monto_min,
        query.monto_max,
    )
    .await
    {
        Ok(payments) => {
            let filtered: Vec<_> = if let Some(cid) = cashier_id {
                payments
                    .into_iter()
                    .filter(|p| p.verified_by.is_none() || p.verified_by == Some(cid))
                    .collect()
            } else {
                payments
            };
            let response: Vec<PaymentResponse> =
                filtered.into_iter().map(PaymentResponse::from).collect();
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Error interno del servidor"})),
        )
            .into_response(),
    }
}

async fn verify_payment_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(req): Json<VerifyPaymentRequest>,
) -> impl IntoResponse {
    let fecha_base = match NaiveDate::parse_from_str(&req.fecha_empresa, "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Fecha invalida: {}", e)})),
            )
                .into_response();
        }
    };

    let inicio_rango = (fecha_base - chrono::Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let fin_rango = (fecha_base + chrono::Duration::days(1))
        .and_hms_opt(23, 59, 59)
        .unwrap();

    let pago = match db::find_payment_for_verification(
        &state.pool,
        &req.usuario_empresa,
        req.monto_empresa,
        &inicio_rango.to_string(),
        &fin_rango.to_string(),
    )
    .await
    {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Error interno del servidor"})),
            )
                .into_response();
        }
    };

    match pago {
        Some(p) => {
            // Guard: skip deposits
            if matches!(p.tipo, PaymentType::Deposito) {
                return (
                    StatusCode::OK,
                    Json(json!({
                        "verificado": false,
                        "mensaje": "Los depositos no pueden ser verificados automaticamente porque no tienen remitente.",
                        "data": null,
                        "fraud_verdict": null,
                    })),
                )
                    .into_response();
            }

            let user_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
            let obs = "Conciliado automaticamente con reporte de empresa.".to_string();

            let _ = db::verify_payment(&state.pool, p.id, user_id, &obs, auth.company_group.as_deref()).await;

            // Audit log
            let _ = db::insert_audit_entry(
                &state.pool,
                Some(user_id),
                "verify_payment",
                Some("payment"),
                Some(p.id),
                Some(&json!({
                    "usuario_empresa": req.usuario_empresa,
                    "monto_empresa": req.monto_empresa,
                    "fecha_empresa": req.fecha_empresa,
                })),
                None,
                None,
            )
            .await;

            let response = PaymentResponse::from(p.clone());
            (
                StatusCode::OK,
                Json(json!({
                    "verificado": true,
                    "mensaje": format!("Pago verificado y firmado con exito! ID: {}", p.id),
                    "data": response,
                    "fraud_verdict": p.fraud_verdict,
                })),
            )
                .into_response()
        }
        None => (
            StatusCode::OK,
            Json(json!({
                "verificado": false,
                "mensaje": "No se encontro ningun correo coincidente o el pago ya fue reclamado previamente.",
                "data": null,
                "fraud_verdict": null,
            })),
        )
            .into_response(),
    }
}

async fn get_payment_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match db::find_payment_by_id(&state.pool, id).await {
        Ok(Some(p)) => {
            // Cashiers can only see their own verified payments or non-verified ones
            if auth.role == "cashier" {
                let cashier_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
                if p.verified_by.is_some() && p.verified_by != Some(cashier_id) {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(json!({"error": "No tienes permiso para ver este pago"})),
                    )
                        .into_response();
                }
            }
            let response = PaymentResponse::from(p);
            (StatusCode::OK, Json(response)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Pago no encontrado"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Error interno del servidor"})),
        )
            .into_response(),
    }
}

async fn quick_verify_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(payment_id): Path<Uuid>,
    Json(req): Json<QuickVerifyRequest>,
) -> impl IntoResponse {
    let user_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
    let obs = req.observaciones.as_deref().unwrap_or("Verificado manualmente");

    match db::quick_verify_payment(&state.pool, payment_id, user_id, obs, auth.company_group.as_deref()).await {
        Ok(payment) => {
            // Audit log
            let _ = db::insert_audit_entry(
                &state.pool,
                Some(user_id),
                "quick_verify",
                Some("payment"),
                Some(payment.id),
                Some(&json!({
                    "payment_id": payment.id.to_string(),
                    "observaciones": obs,
                })),
                None,
                None,
            )
            .await;

            let response = PaymentResponse::from(payment);
            (
                StatusCode::OK,
                Json(QuickVerifyResponse {
                    verificado: true,
                    mensaje: "Pago verificado exitosamente".to_string(),
                    data: response,
                }),
            )
                .into_response()
        }
        Err(e) => {
            let status = if e.contains("no encontrado") || e.contains("ya verificado") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (status, Json(json!({"error": "Error interno del servidor"}))).into_response()
        }
    }
}

// ===================== SYNC =====================

async fn sync_status_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
) -> impl IntoResponse {
    // Admin only
    if let Err(e) = require_admin(&auth) {
        return e.into_response();
    }

    let creds = db::get_imap_credentials(&state.pool).await.ok().flatten();
    let last_sync = creds.as_ref().and_then(|c| c.last_sync_at);

    (
        StatusCode::OK,
        Json(json!({
            "last_sync": last_sync.map(|t| t.to_rfc3339()),
            "next_sync": state.config.imap_poll_interval_secs,
            "status": "idle",
        })),
    )
        .into_response()
}

async fn trigger_sync_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(req): Json<TriggerSyncRequest>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&auth) {
        return e.into_response();
    }

    let historical_since = if req.mode.as_deref() == Some("historical") {
        req.since_date.as_deref()
    } else {
        None
    };

    match imap_sync::run_imap_sync(&state.pool, &state.age_identity, historical_since).await {
        Ok(stats) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "nuevos": stats.nuevos,
                "total": stats.total_procesados,
                "duplicados": stats.duplicados,
                "en_cuarentena": stats.en_cuarentena,
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Error verificando pago: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "Error interno del servidor"})),
            ).into_response()
        },
    }
}

// ===================== IMAP CONFIG =====================

async fn get_imap_config_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&auth) {
        return e.into_response();
    }

    match db::get_imap_credentials(&state.pool).await {
        Ok(Some(c)) => {
            let public: ImapCredentialsPublic = c.into();
            (StatusCode::OK, Json(public)).into_response()
        }
        Ok(None) => (
            StatusCode::OK,
            Json(json!({"email": "", "imap_host": "imap.gmail.com", "imap_port": 993})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Error interno del servidor"})),
        )
            .into_response(),
    }
}

async fn put_imap_config_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(req): Json<ImapConfigUpdate>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&auth) {
        return e.into_response();
    }

    // Encrypt password with age
    let encrypted = match config::encrypt_with_age(&state.age_identity, &req.password) {
        Ok(e) => e,
        Err(err) => {
            tracing::error!("Error encriptando credenciales IMAP: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Error interno del servidor"})),
            )
                .into_response();
        }
    };

    let host = req.host.unwrap_or_else(|| "imap.gmail.com".to_string());
    let port = req.port.unwrap_or(993);

    match db::upsert_imap_credentials(&state.pool, &req.email, &encrypted, &host, port).await {
        Ok(_) => {
            // Audit log
            let user_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
            let _ = db::insert_audit_entry(
                &state.pool,
                Some(user_id),
                "config_change",
                Some("imap_config"),
                None,
                Some(&json!({"email": req.email, "host": host, "port": port})),
                None,
                None,
            )
            .await;

            (StatusCode::OK, Json(json!({"success": true}))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Error interno del servidor"})),
        )
            .into_response(),
    }
}

// ===================== IMPORT =====================

async fn import_csv_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Token invalido"})),
            )
                .into_response();
        }
    };

    // Extract file from multipart
    let mut contenido = String::new();
    let mut filename = "upload.csv".to_string();
    while let Ok(Some(field)) = multipart.next_field().await {
        if let Some(name) = field.file_name() {
            filename = name.to_string();
        }
        if let Ok(bytes) = field.bytes().await {
            contenido = String::from_utf8_lossy(&bytes).to_string();
        }
    }

    if contenido.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Archivo CSV vacio"})),
        )
            .into_response();
    }

    // Parse and process CSV
    let (total, verified, failed, details) = process_csv(&state, &contenido, user_id, auth.company_group.as_deref()).await;

    // Create import record
    let import_id = db::create_import(&state.pool, &filename, "csv", total as i32, user_id)
        .await
        .unwrap_or_default();

    let _ = db::update_import_counts(&state.pool, import_id, verified as i32, failed as i32).await;

    // Audit log
    let _ = db::insert_audit_entry(
        &state.pool,
        Some(user_id),
        "import_csv",
        Some("import"),
        Some(import_id),
        Some(&json!({"filename": filename, "total": total, "verified": verified, "failed": failed})),
        None,
        None,
    )
    .await;

    (
        StatusCode::OK,
        Json(json!({
            "total": total,
            "verified": verified,
            "failed": failed,
            "details": details,
        })),
    )
        .into_response()
}

async fn process_csv(
    state: &SharedState,
    contenido: &str,
    user_id: Uuid,
    company_group: Option<&str>,
) -> (usize, usize, usize, Vec<ImportRowDetail>) {
    let mut builder = csv::ReaderBuilder::new();
    builder.flexible(true);
    let mut reader = builder.from_reader(contenido.as_bytes());

    let headers = match reader.headers() {
        Ok(h) => h.clone(),
        Err(_) => return (0, 0, 0, vec![]),
    };

    let col_usuario = find_column(&headers, &["usuario", "user", "remitente", "nombre"]);
    let col_monto = find_column(&headers, &["monto", "amount", "importe"]);
    let col_fecha = find_column(&headers, &["fecha", "date"]);

    let (col_u, col_m, col_f) = match (col_usuario, col_monto, col_fecha) {
        (Some(u), Some(m), Some(f)) => (u, m, f),
        _ => (0usize, 1usize, 2usize),
    };

    let mut details: Vec<ImportRowDetail> = Vec::new();
    let mut verified = 0usize;
    let mut failed = 0usize;

    for (i, record) in reader.records().enumerate() {
        let fila_num = i + 2;

        let record = match record {
            Ok(r) => r,
            Err(e) => {
                details.push(ImportRowDetail {
                    row: fila_num,
                    usuario: String::new(),
                    monto: 0.0,
                    fecha: String::new(),
                    result: format!("error: {}", e),
                });
                failed += 1;
                continue;
            }
        };

        let usuario = record.get(col_u).unwrap_or("").trim().to_string();
        let monto_str = record.get(col_m).unwrap_or("").trim().replace(",", "");
        let fecha = record.get(col_f).unwrap_or("").trim().to_string();

        if usuario.is_empty() || monto_str.is_empty() || fecha.is_empty() {
            details.push(ImportRowDetail {
                row: fila_num,
                usuario,
                monto: 0.0,
                fecha,
                result: "error: campos vacios".to_string(),
            });
            failed += 1;
            continue;
        }

        let monto: f64 = match monto_str.parse() {
            Ok(m) => m,
            Err(e) => {
                details.push(ImportRowDetail {
                    row: fila_num,
                    usuario,
                    monto: 0.0,
                    fecha,
                    result: format!("error: monto invalido: {}", e),
                });
                failed += 1;
                continue;
            }
        };

        let fecha_base = match NaiveDate::parse_from_str(&fecha, "%Y-%m-%d") {
            Ok(d) => d,
            Err(e) => {
                details.push(ImportRowDetail {
                    row: fila_num,
                    usuario,
                    monto,
                    fecha,
                    result: format!("error: fecha invalida: {}", e),
                });
                failed += 1;
                continue;
            }
        };

        let inicio_rango = (fecha_base - chrono::Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let fin_rango = (fecha_base + chrono::Duration::days(1))
            .and_hms_opt(23, 59, 59)
            .unwrap();

        match db::find_payment_for_verification(
            &state.pool,
            &usuario,
            monto,
            &inicio_rango.to_string(),
            &fin_rango.to_string(),
        )
        .await
        {
            Ok(Some(p)) => {
                let obs = format!("Conciliado via CSV (fila {}).", fila_num);
                let _ = db::verify_payment(&state.pool, p.id, user_id, &obs, company_group).await;
                details.push(ImportRowDetail {
                    row: fila_num,
                    usuario,
                    monto,
                    fecha,
                    result: "verificado".to_string(),
                });
                verified += 1;
            }
            Ok(None) => {
                details.push(ImportRowDetail {
                    row: fila_num,
                    usuario,
                    monto,
                    fecha,
                    result: "no_encontrado".to_string(),
                });
                failed += 1;
            }
            Err(e) => {
                details.push(ImportRowDetail {
                    row: fila_num,
                    usuario,
                    monto,
                    fecha,
                    result: format!("error: {}", e),
                });
                failed += 1;
            }
        }
    }

    (details.len(), verified, failed, details)
}

fn find_column(headers: &csv::StringRecord, candidates: &[&str]) -> Option<usize> {
    for i in 0..headers.len() {
        if let Some(header) = headers.get(i) {
            let h = header.trim().to_lowercase();
            for &c in candidates {
                if h == c {
                    return Some(i);
                }
            }
        }
    }
    None
}

async fn import_excel_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Token invalido"})),
            )
                .into_response();
        }
    };

    let mut file_bytes = Vec::new();
    let mut filename = "upload.xlsx".to_string();
    while let Ok(Some(field)) = multipart.next_field().await {
        if let Some(name) = field.file_name() {
            filename = name.to_string();
        }
        if let Ok(bytes) = field.bytes().await {
            file_bytes = bytes.to_vec();
        }
    }

    if file_bytes.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Archivo Excel vacio"})),
        )
            .into_response();
    }

    // Process Excel using calamine
    let (total, verified, failed, details) =
        process_excel_async(&state, &file_bytes, user_id, auth.company_group.as_deref()).await;

    let import_id = db::create_import(&state.pool, &filename, "xlsx", total as i32, user_id)
        .await
        .unwrap_or_default();

    let _ = db::update_import_counts(&state.pool, import_id, verified as i32, failed as i32).await;

    let _ = db::insert_audit_entry(
        &state.pool,
        Some(user_id),
        "import_excel",
        Some("import"),
        Some(import_id),
        Some(&json!({"filename": filename, "total": total})),
        None,
        None,
    )
    .await;

    (
        StatusCode::OK,
        Json(json!({
            "total": total,
            "verified": verified,
            "failed": failed,
            "details": details,
        })),
    )
        .into_response()
}

async fn process_excel_async(
    state: &SharedState,
    file_bytes: &[u8],
    user_id: Uuid,
    company_group: Option<&str>,
) -> (usize, usize, usize, Vec<ImportRowDetail>) {
    use calamine::{Reader, DataType, open_workbook_auto};

    // Write bytes to temp file because open_workbook_auto expects AsRef<Path>
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!("import_excel_{}.xlsx", Utc::now().timestamp()));
    if let Err(e) = std::fs::write(&tmp_path, file_bytes) {
        return (
            0,
            0,
            1,
            vec![ImportRowDetail {
                row: 0,
                usuario: String::new(),
                monto: 0.0,
                fecha: String::new(),
                result: format!("error: {}", e),
            }],
        );
    }
    let mut workbook: calamine::Sheets<_> = match open_workbook_auto(&tmp_path) {
        Ok(wb) => wb,
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            return (
                0,
                0,
                1,
                vec![ImportRowDetail {
                    row: 0,
                    usuario: String::new(),
                    monto: 0.0,
                    fecha: String::new(),
                    result: format!("error: {}", e),
                }],
            );
        }
    };

    // File no longer needed after workbook is loaded
    let _ = std::fs::remove_file(&tmp_path);

    let range = match workbook.worksheet_range_at(0) {
        Some(Ok(r)) => r,
        _ => return (0, 0, 0, vec![]),
    };

    let headers: Vec<String> = match range.rows().next() {
        Some(row) => row
            .iter()
            .map(|c| c.as_string().unwrap_or_default().trim().to_lowercase())
            .collect(),
        None => return (0, 0, 0, vec![]),
    };

    let find_col = |candidates: &[&str]| -> Option<usize> {
        headers
            .iter()
            .position(|h| candidates.iter().any(|c| h.contains(c)))
    };

    let col_usuario = find_col(&["usuario", "user", "remitente", "nombre"]);
    let col_monto = find_col(&["monto", "amount", "importe"]);
    let col_fecha = find_col(&["fecha", "date"]);

    let (col_u, col_m, col_f) = match (col_usuario, col_monto, col_fecha) {
        (Some(u), Some(m), Some(f)) => (u, m, f),
        _ => (0usize, 1usize, 2usize),
    };

    let mut details: Vec<ImportRowDetail> = Vec::new();
    let mut verified = 0usize;
    let mut failed = 0usize;

    for (row_idx, row) in range.rows().enumerate().skip(1) {
        let fila_num = row_idx + 2;

        let usuario = row
            .get(col_u)
            .and_then(|c| c.as_string())
            .unwrap_or_default()
            .trim()
            .to_string();
        let monto_str = row
            .get(col_m)
            .and_then(|c| c.as_f64())
            .map(|v| v.to_string())
            .or_else(|| {
                row.get(col_m)
                    .and_then(|c| c.as_string())
                    .map(|s| s.replace(",", "").trim().to_string())
            })
            .unwrap_or_default();
        let fecha = row
            .get(col_f)
            .and_then(|c| c.as_string())
            .unwrap_or_default()
            .trim()
            .to_string();

        if usuario.is_empty() || monto_str.is_empty() || fecha.is_empty() {
            details.push(ImportRowDetail {
                row: fila_num,
                usuario,
                monto: 0.0,
                fecha,
                result: "error: campos vacios".to_string(),
            });
            failed += 1;
            continue;
        }

        let monto: f64 = match monto_str.parse() {
            Ok(m) => m,
            Err(e) => {
                details.push(ImportRowDetail {
                    row: fila_num,
                    usuario,
                    monto: 0.0,
                    fecha,
                    result: format!("error: monto invalido: {}", e),
                });
                failed += 1;
                continue;
            }
        };

        let fecha_base = NaiveDate::parse_from_str(&fecha, "%Y-%m-%d")
            .or_else(|_| NaiveDate::parse_from_str(&fecha, "%d/%m/%Y"))
            .or_else(|_| NaiveDate::parse_from_str(&fecha, "%m/%d/%Y"));

        let fecha_base = match fecha_base {
            Ok(d) => d,
            Err(e) => {
                details.push(ImportRowDetail {
                    row: fila_num,
                    usuario,
                    monto,
                    fecha,
                    result: format!("error: fecha invalida: {}", e),
                });
                failed += 1;
                continue;
            }
        };

        let inicio_rango = (fecha_base - chrono::Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let fin_rango = (fecha_base + chrono::Duration::days(1))
            .and_hms_opt(23, 59, 59)
            .unwrap();

        match db::find_payment_for_verification(
            &state.pool,
            &usuario,
            monto,
            &inicio_rango.to_string(),
            &fin_rango.to_string(),
        )
        .await
        {
            Ok(Some(p)) => {
                let obs = format!("Conciliado via Excel (fila {}).", fila_num);
                let _ = db::verify_payment(&state.pool, p.id, user_id, &obs, company_group).await;
                details.push(ImportRowDetail {
                    row: fila_num,
                    usuario,
                    monto,
                    fecha,
                    result: "verificado".to_string(),
                });
                verified += 1;
            }
            Ok(None) => {
                details.push(ImportRowDetail {
                    row: fila_num,
                    usuario,
                    monto,
                    fecha,
                    result: "no_encontrado".to_string(),
                });
                failed += 1;
            }
            Err(e) => {
                details.push(ImportRowDetail {
                    row: fila_num,
                    usuario,
                    monto,
                    fecha,
                    result: format!("error: {}", e),
                });
                failed += 1;
            }
        }
    }

    (details.len(), verified, failed, details)
}

// ===================== EXPORT =====================

async fn export_xlsx_handler(
    State(state): State<SharedState>,
    _auth: AuthUser,
    Query(query): Query<PaymentQuery>,
) -> impl IntoResponse {
    let desde = query.desde.unwrap_or_else(|| {
        (Utc::now() - chrono::Duration::days(30))
            .format("%Y-%m-%dT00:00:00Z")
            .to_string()
    });
    let hasta = query.hasta.unwrap_or_else(|| {
        Utc::now().format("%Y-%m-%dT23:59:59Z").to_string()
    });

    let payments = match db::list_payments(
        &state.pool,
        &desde,
        &hasta,
        query.monto_exacto,
        query.monto_min,
        query.monto_max,
    )
    .await
    {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Error interno del servidor"})),
            )
                .into_response();
        }
    };

    // Generate xlsx in memory using rust_xlsxwriter
    match generate_xlsx(&payments) {
        Ok(data) => (
            StatusCode::OK,
            [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", &format!("attachment; filename=\"reporte_{}_{}.xlsx\"", desde[..10].to_string(), hasta[..10].to_string())),
            ],
            data,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Error interno del servidor"})),
        )
            .into_response(),
    }
}

fn generate_xlsx(payments: &[Payment]) -> Result<Vec<u8>, String> {
    use rust_xlsxwriter::{Color, Format, Workbook};

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    let headers = ["Tipo", "Usuario Remitente", "Monto", "Moneda", "Fecha Correo", "Estado"];
    let header_format = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x1E293B))
        .set_font_color(Color::RGB(0xFBBF24));

    for (col, header) in headers.iter().enumerate() {
        worksheet
            .write_with_format(0, col as u16, *header, &header_format)
            .map_err(|e| format!("Excel write error: {}", e))?;
    }

    let disponible_fmt = Format::new().set_background_color(Color::RGB(0xFEF3C7));
    let verificado_fmt = Format::new().set_background_color(Color::RGB(0xD1FAE5));
    let por_revisar_fmt = Format::new().set_background_color(Color::RGB(0xFDE68A));

    for (row_idx, pago) in payments.iter().enumerate() {
        let row = (row_idx + 1) as u32;

        worksheet
            .write(row, 0, pago.tipo.to_string())
            .map_err(|e| format!("Excel write error: {}", e))?;
        worksheet
            .write(
                row,
                1,
                pago.usuario_remitente.as_deref().unwrap_or("—"),
            )
            .map_err(|e| format!("Excel write error: {}", e))?;

        let monto_f64: f64 = pago.monto.to_string().parse().unwrap_or(0.0);
        worksheet
            .write(row, 2, monto_f64)
            .map_err(|e| format!("Excel write error: {}", e))?;
        worksheet
            .write(row, 3, &pago.moneda)
            .map_err(|e| format!("Excel write error: {}", e))?;
        worksheet
            .write(row, 4, pago.fecha_correo.format("%d/%m/%Y").to_string())
            .map_err(|e| format!("Excel write error: {}", e))?;

        let estado_str = match &pago.estado {
            PaymentStatus::Disponible => "disponible",
            PaymentStatus::Verificado => "verificado",
            PaymentStatus::Rechazado => "rechazado",
            PaymentStatus::PorRevisar => "por_revisar",
            PaymentStatus::Cuarentena => "cuarentena",
        };

        let estado_fmt = match &pago.estado {
            PaymentStatus::Verificado => &verificado_fmt,
            PaymentStatus::PorRevisar | PaymentStatus::Cuarentena => &por_revisar_fmt,
            _ => &disponible_fmt,
        };
        worksheet
            .write_with_format(row, 5, estado_str, estado_fmt)
            .map_err(|e| format!("Excel write error: {}", e))?;
    }

    worksheet.set_column_width(0, 12.0).ok();
    worksheet.set_column_width(1, 25.0).ok();
    worksheet.set_column_width(2, 15.0).ok();
    worksheet.set_column_width(3, 10.0).ok();
    worksheet.set_column_width(4, 30.0).ok();
    worksheet.set_column_width(5, 15.0).ok();

    let mut buf: Vec<u8> = Vec::new();
    // rust_xlsxwriter::Workbook can save to a writer
    // We need to save to Vec<u8> — rust_xlsxwriter saves to path, not writer directly
    // Workaround: save to temp path and read back
    use std::io::Write;
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!("reporte_{}.xlsx", Utc::now().timestamp()));
    workbook
        .save(tmp_path.to_str().unwrap())
        .map_err(|e| format!("Excel save error: {}", e))?;
    buf = std::fs::read(&tmp_path).map_err(|e| format!("Read temp xlsx error: {}", e))?;
    let _ = std::fs::remove_file(&tmp_path);

    Ok(buf)
}

// ===================== AUDIT =====================

async fn audit_log_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Query(query): Query<AuditQuery>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&auth) {
        return e.into_response();
    }

    let desde = query.desde.as_deref();
    let hasta = query.hasta.as_deref();

    match db::query_audit_log(&state.pool, query.action.as_deref(), query.user_id, desde, hasta)
        .await
    {
        Ok(entries) => (StatusCode::OK, Json(entries)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Error interno del servidor"})),
        )
            .into_response(),
    }
}

// ===================== QUARANTINE =====================

async fn list_quarantine_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&auth) {
        return e.into_response();
    }

    let status = params.get("status").map(|s| s.as_str());

    match db::list_quarantined(&state.pool, status).await {
        Ok(entries) => (StatusCode::OK, Json(entries)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Error interno del servidor"})),
        )
            .into_response(),
    }
}

async fn review_quarantine_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<QuarantineReview>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&auth) {
        return e.into_response();
    }

    let user_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();

    match db::review_quarantined(&state.pool, id, &req.decision, user_id).await {
        Ok(_) => {
            let _ = db::insert_audit_entry(
                &state.pool,
                Some(user_id),
                "review_quarantine",
                Some("quarantine"),
                Some(id),
                Some(&json!({"decision": req.decision})),
                None,
                None,
            )
            .await;
            (StatusCode::OK, Json(json!({"success": true}))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Error interno del servidor"})),
        )
            .into_response(),
    }
}

// ===================== USERS (Admin) =====================

async fn list_users_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&auth) {
        return e.into_response();
    }

    match db::list_users(&state.pool).await {
        Ok(users) => {
            let public: Vec<UserPublic> = users.into_iter().map(UserPublic::from).collect();
            (StatusCode::OK, Json(public)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Error interno del servidor"})),
        )
            .into_response(),
    }
}

async fn create_user_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(req): Json<CreateUserRequest>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&auth) {
        return e.into_response();
    }

    let hash = match auth::hash_password(&req.password) {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Error interno del servidor"})),
            )
                .into_response();
        }
    };

    let must_change_password = req.role == "cashier";

    match db::create_user(
        &state.pool,
        &req.username,
        &hash,
        &req.role,
        req.company_group.as_deref(),
        req.station_name.as_deref(),
        must_change_password,
    )
    .await
    {
        Ok(user) => {
            let user_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
            let _ = db::insert_audit_entry(
                &state.pool,
                Some(user_id),
                "create_user",
                Some("user"),
                Some(user.id),
                Some(&json!({"username": req.username, "role": req.role})),
                None,
                None,
            )
            .await;
            let public: UserPublic = user.into();
            (StatusCode::CREATED, Json(public)).into_response()
        }
        Err(e) => {
            let status = if e.contains("ya existe") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (status, Json(json!({"error": "Error interno del servidor"}))).into_response()
        }
    }
}

async fn update_user_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&auth) {
        return e.into_response();
    }

    match db::update_user(
        &state.pool,
        id,
        req.username.as_deref(),
        req.role.as_deref(),
        req.is_active,
        req.company_group.as_deref(),
        req.station_name.as_deref(),
        req.must_change_password,
    )
    .await
    {
        Ok(user) => {
            let admin_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
            let _ = db::insert_audit_entry(
                &state.pool,
                Some(admin_id),
                "update_user",
                Some("user"),
                Some(id),
                Some(&json!({"username": req.username, "role": req.role, "is_active": req.is_active})),
                None,
                None,
            )
            .await;
            let public: UserPublic = user.into();
            (StatusCode::OK, Json(public)).into_response()
        }
        Err(e) => {
            let status = if e.contains("ya existe") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (status, Json(json!({"error": "Error interno del servidor"}))).into_response()
        }
    }
}

async fn delete_user_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&auth) {
        return e.into_response();
    }

    match db::delete_user(&state.pool, id).await {
        Ok(_) => {
            let admin_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
            let _ = db::insert_audit_entry(
                &state.pool,
                Some(admin_id),
                "delete_user",
                Some("user"),
                Some(id),
                None,
                None,
                None,
            )
            .await;
            (StatusCode::OK, Json(json!({"success": true}))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Error interno del servidor"})),
        )
            .into_response(),
    }
}

async fn admin_reset_password_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<AdminResetPasswordRequest>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&auth) {
        return e.into_response();
    }

    if let Some(msg) = validate_password(&req.new_password) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": msg})),
        )
            .into_response();
    }

    let new_hash = match auth::hash_password(&req.new_password) {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Error interno del servidor"})),
            )
                .into_response();
        }
    };

    match db::admin_reset_password(&state.pool, id, &new_hash).await {
        Ok(_) => {
            let admin_id = Uuid::parse_str(&auth.user_id).unwrap_or_default();
            let _ = db::insert_audit_entry(
                &state.pool,
                Some(admin_id),
                "admin_reset_password",
                Some("user"),
                Some(id),
                Some(&json!({"reset_by": auth.user_id})),
                None,
                None,
            )
            .await;
            (StatusCode::OK, Json(json!({"success": true, "mensaje": "Contraseña reseteada correctamente"}))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Error interno del servidor"})),
        )
            .into_response(),
    }
}

async fn change_password_handler(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(req): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    let user_id = match Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "ID de usuario invalido"})),
            )
                .into_response();
        }
    };

    // Verify current password
    let user = match db::find_user_by_id(&state.pool, user_id).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Usuario no encontrado"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Error interno del servidor"})),
            )
                .into_response();
        }
    };

    // Validate current password
    match auth::verify_password(&req.current_password, &user.password_hash) {
        Ok(true) => {}
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "La contraseña actual no es correcta"})),
            )
                .into_response();
        }
    }

    // Validate new password complexity
    if let Some(msg) = validate_password(&req.new_password) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": msg})),
        )
            .into_response();
    }

    // Hash new password and update
    let new_hash = match auth::hash_password(&req.new_password) {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Error interno del servidor"})),
            )
                .into_response();
        }
    };

    match db::update_password(&state.pool, user_id, &new_hash).await {
        Ok(_) => {
            let _ = db::insert_audit_entry(
                &state.pool,
                Some(user_id),
                "change_password",
                Some("user"),
                Some(user_id),
                None,
                None,
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(json!({"success": true, "mensaje": "Contraseña actualizada correctamente"})),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Error interno del servidor"})),
        )
            .into_response(),
    }
}
