use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;
use std::time::Duration;
use uuid::Uuid;

use crate::models::*;

/// Inicializa el pool de conexiones a PostgreSQL
pub async fn init_pool(database_url: &str) -> Result<PgPool, String> {
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(300))
        .connect(database_url)
        .await
        .map_err(|e| format!("DB pool error: {}", e))
}

/// Ejecuta las migraciones al iniciar el servidor.
/// PostgreSQL no soporta multi-statement en una sola query via sqlx,
/// asi que partimos el SQL por ';' y ejecutamos cada statement individualmente.
pub async fn run_migrations(pool: &PgPool) -> Result<(), String> {
    let migration_sql = include_str!("../migrations/001_init.sql");

    for statement in migration_sql.split(';') {
        let trimmed = statement.trim();
        // Saltar comentarios y lineas vacias
        if trimmed.is_empty() || trimmed.lines().all(|l| l.trim().starts_with("--") || l.trim().is_empty()) {
            continue;
        }
        sqlx::query(trimmed)
            .execute(pool)
            .await
            .map_err(|e| {
                let preview: String = trimmed.chars().take(60).collect();
                format!("Migration error at '{}...': {}", preview, e)
            })?;
    }

    tracing::info!("Migraciones ejecutadas correctamente.");
    Ok(())
}

/// Crea el usuario admin por defecto si no existe (solo para primer arranque).
pub async fn seed_admin(pool: &PgPool) -> Result<(), String> {
    use crate::auth;
    let exists: (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM users WHERE username = 'admin')")
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Error verificando admin: {}", e))?;

    if !exists.0 {
        let hash = auth::hash_password("admin123")
            .map_err(|e| format!("Error hasheando password: {}", e))?;
        sqlx::query("INSERT INTO users (username, password_hash, role) VALUES ('admin', $1, 'admin')")
            .bind(&hash)
            .execute(pool)
            .await
            .map_err(|e| format!("Error creando admin: {}", e))?;
        tracing::info!("Usuario admin por defecto creado (admin / admin123). CAMBIALO en produccion!");
    }

    Ok(())
}

// =========================================================================
// Users CRUD
// =========================================================================

pub async fn create_user(
    pool: &PgPool,
    username: &str,
    password_hash: &str,
    role: &str,
    station_name: Option<&str>,
) -> Result<User, String> {
    let row = sqlx::query_as::<_, User>(
        "INSERT INTO users (username, password_hash, role, station_name)
         VALUES ($1, $2, $3::text, $4)
         RETURNING id, username, password_hash, role, station_name, is_active, created_at, updated_at",
    )
    .bind(username)
    .bind(password_hash)
    .bind(role)
    .bind(station_name)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("unique") {
            format!("El usuario '{}' ya existe", username)
        } else {
            format!("DB error al crear usuario: {}", e)
        }
    })?;
    Ok(row)
}

pub async fn find_user_by_username(pool: &PgPool, username: &str) -> Result<Option<User>, String> {
    let row = sqlx::query_as::<_, User>(
        "SELECT id, username, password_hash, role, station_name, is_active, created_at, updated_at
         FROM users WHERE username = $1",
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB error al buscar usuario: {}", e))?;
    Ok(row)
}

pub async fn find_user_by_id(pool: &PgPool, user_id: Uuid) -> Result<Option<User>, String> {
    let row = sqlx::query_as::<_, User>(
        "SELECT id, username, password_hash, role, station_name, is_active, created_at, updated_at
         FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB error al buscar usuario: {}", e))?;
    Ok(row)
}

pub async fn list_users(pool: &PgPool) -> Result<Vec<User>, String> {
    let rows = sqlx::query_as::<_, User>(
        "SELECT id, username, password_hash, role, station_name, is_active, created_at, updated_at
         FROM users ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error al listar usuarios: {}", e))?;
    Ok(rows)
}

pub async fn update_user(
    pool: &PgPool,
    user_id: Uuid,
    role: Option<&str>,
    is_active: Option<bool>,
    station_name: Option<&str>,
) -> Result<User, String> {
    let row = sqlx::query_as::<_, User>(
        "UPDATE users
         SET role = COALESCE($2::text, role),
             is_active = COALESCE($3, is_active),
             station_name = COALESCE($4, station_name),
             updated_at = now()
         WHERE id = $1
         RETURNING id, username, password_hash, role, station_name, is_active, created_at, updated_at",
    )
    .bind(user_id)
    .bind(role)
    .bind(is_active)
    .bind(station_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB error al actualizar usuario: {}", e))?
    .ok_or("Usuario no encontrado".to_string())?;
    Ok(row)
}

// =========================================================================
// Payments CRUD
// =========================================================================

pub async fn list_payments(
    pool: &PgPool,
    desde: &str,
    hasta: &str,
    monto_exacto: Option<f64>,
    monto_min: Option<f64>,
    monto_max: Option<f64>,
) -> Result<Vec<Payment>, String> {
    let mut sql = String::from(
        "SELECT id, tipo, usuario_remitente, monto, moneda, fecha_correo, estado,
                observaciones, verificado_en, verified_by, fraud_verdict, fraud_details,
                email_uid, raw_headers, created_at, updated_at
         FROM payments
         WHERE fecha_correo >= $1::timestamptz AND fecha_correo <= $2::timestamptz",
    );

    // Build dynamic query manually since sqlx doesn't support dynamic query! easily
    if monto_exacto.is_some() {
        sql.push_str(" AND monto = $3::numeric");
    } else {
        if monto_min.is_some() {
            sql.push_str(" AND monto >= $3::numeric");
        }
        if monto_max.is_some() {
            sql.push_str(" AND monto <= $4::numeric");
        }
    }
    sql.push_str(" ORDER BY fecha_correo DESC");

    // Use a simpler approach: fetch all within date range and filter in-memory
    // for the amount filters. This avoids complex dynamic query building.
    let mut query = sqlx::query_as::<_, Payment>(
        "SELECT id, tipo, usuario_remitente, monto, moneda, fecha_correo, estado,
                observaciones, verificado_en, verified_by, fraud_verdict, fraud_details,
                email_uid, raw_headers, created_at, updated_at
         FROM payments
         WHERE fecha_correo >= $1::timestamptz AND fecha_correo <= $2::timestamptz
         ORDER BY fecha_correo DESC",
    )
    .bind(desde)
    .bind(hasta);

    let mut rows: Vec<Payment> = query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("DB error al listar pagos: {}", e))?;

    // Apply amount filters in-memory (simplifies dynamic SQL)
    if let Some(exact) = monto_exacto {
        rows.retain(|p| {
            let m: f64 = p.monto.to_string().parse().unwrap_or(0.0);
            (m - exact).abs() < 0.001
        });
    } else {
        if let Some(min) = monto_min {
            rows.retain(|p| {
                let m: f64 = p.monto.to_string().parse().unwrap_or(0.0);
                m >= min - 0.001
            });
        }
        if let Some(max) = monto_max {
            rows.retain(|p| {
                let m: f64 = p.monto.to_string().parse().unwrap_or(0.0);
                m <= max + 0.001
            });
        }
    }

    Ok(rows)
}

pub async fn list_payments_by_sender(
    pool: &PgPool,
    desde: &str,
    hasta: &str,
    remitente: &str,
    monto_exacto: Option<f64>,
    monto_min: Option<f64>,
    monto_max: Option<f64>,
) -> Result<Vec<Payment>, String> {
    let pattern = format!("%{}%", remitente);

    let mut query = sqlx::query_as::<_, Payment>(
        "SELECT id, tipo, usuario_remitente, monto, moneda, fecha_correo, estado,
                observaciones, verificado_en, verified_by, fraud_verdict, fraud_details,
                email_uid, raw_headers, created_at, updated_at
         FROM payments
         WHERE fecha_correo >= $1::timestamptz
           AND fecha_correo <= $2::timestamptz
           AND usuario_remitente ILIKE $3
         ORDER BY fecha_correo DESC",
    )
    .bind(desde)
    .bind(hasta)
    .bind(&pattern);

    let mut rows: Vec<Payment> = query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("DB error al buscar por remitente: {}", e))?;

    // Apply amount filters in-memory
    if let Some(exact) = monto_exacto {
        rows.retain(|p| {
            let m: f64 = p.monto.to_string().parse().unwrap_or(0.0);
            (m - exact).abs() < 0.001
        });
    } else {
        if let Some(min) = monto_min {
            rows.retain(|p| {
                let m: f64 = p.monto.to_string().parse().unwrap_or(0.0);
                m >= min - 0.001
            });
        }
        if let Some(max) = monto_max {
            rows.retain(|p| {
                let m: f64 = p.monto.to_string().parse().unwrap_or(0.0);
                m <= max + 0.001
            });
        }
    }

    Ok(rows)
}

pub async fn find_payment_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Payment>, String> {
    let row = sqlx::query_as::<_, Payment>(
        "SELECT id, tipo, usuario_remitente, monto, moneda, fecha_correo, estado,
                observaciones, verificado_en, verified_by, fraud_verdict, fraud_details,
                email_uid, raw_headers, created_at, updated_at
         FROM payments WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB error al buscar pago: {}", e))?;
    Ok(row)
}

pub async fn find_payment_for_verification(
    pool: &PgPool,
    usuario: &str,
    monto: f64,
    fecha_inicio: &str,
    fecha_fin: &str,
) -> Result<Option<Payment>, String> {
    let row = sqlx::query_as::<_, Payment>(
        "SELECT id, tipo, usuario_remitente, monto, moneda, fecha_correo, estado,
                observaciones, verificado_en, verified_by, fraud_verdict, fraud_details,
                email_uid, raw_headers, created_at, updated_at
         FROM payments
         WHERE usuario_remitente = $1
           AND monto = $2::numeric
           AND estado = 'disponible'
           AND fecha_correo >= $3::timestamptz
           AND fecha_correo <= $4::timestamptz
         LIMIT 1",
    )
    .bind(usuario)
    .bind(monto.to_string())
    .bind(fecha_inicio)
    .bind(fecha_fin)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB error al buscar pago para verificacion: {}", e))?;
    Ok(row)
}

pub async fn verify_payment(
    pool: &PgPool,
    payment_id: Uuid,
    verified_by: Uuid,
    observaciones: &str,
) -> Result<(), String> {
    let now = Utc::now();
    sqlx::query(
        "UPDATE payments
         SET estado = 'verificado',
             verificado_en = $1,
             verified_by = $2,
             observaciones = $3,
             updated_at = now()
         WHERE id = $4",
    )
    .bind(now)
    .bind(verified_by)
    .bind(observaciones)
    .bind(payment_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error al verificar pago: {}", e))?;
    Ok(())
}

pub async fn insert_payment(
    pool: &PgPool,
    usuario: Option<&str>,
    monto: f64,
    moneda: &str,
    fecha: &str,
    tipo: &str,
    email_uid: Option<i32>,
    fraud_verdict: Option<&str>,
    fraud_details: Option<&serde_json::Value>,
    raw_headers: Option<&serde_json::Value>,
) -> Result<Uuid, String> {
    let estado = if tipo == "deposito" { "por_revisar" } else { "disponible" };

    let row = sqlx::query(
        "INSERT INTO payments (tipo, usuario_remitente, monto, moneda, fecha_correo, estado,
                               email_uid, fraud_verdict, fraud_details, raw_headers)
         VALUES ($1, $2, $3::numeric, $4, $5::timestamptz, $6::text, $7, $8, $9, $10)
         RETURNING id",
    )
    .bind(tipo)
    .bind(usuario)
    .bind(monto.to_string())
    .bind(moneda)
    .bind(fecha)
    .bind(estado)
    .bind(email_uid)
    .bind(fraud_verdict)
    .bind(fraud_details)
    .bind(raw_headers)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("DB error al insertar pago: {}", e))?;

    Ok(row.get("id"))
}

pub async fn payment_exists_by_uid(
    pool: &PgPool,
    email_uid: i32,
) -> Result<bool, String> {
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM payments WHERE email_uid = $1",
    )
    .bind(email_uid)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("DB error al chequear duplicado: {}", e))?;
    Ok(count.0 > 0)
}

pub async fn payment_exists_by_fields(
    pool: &PgPool,
    usuario: Option<&str>,
    monto: f64,
    fecha: &str,
) -> Result<bool, String> {
    let count: (i64,) = match usuario {
        Some(u) => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM payments
                 WHERE usuario_remitente = $1 AND monto = $2::numeric AND fecha_correo = $3::timestamptz",
            )
            .bind(u)
            .bind(monto.to_string())
            .bind(fecha)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("DB error al chequear duplicado: {}", e))?
        }
        None => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM payments
                 WHERE usuario_remitente IS NULL AND monto = $1::numeric AND fecha_correo = $2::timestamptz",
            )
            .bind(monto.to_string())
            .bind(fecha)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("DB error al chequear duplicado: {}", e))?
        }
    };
    Ok(count.0 > 0)
}

// =========================================================================
// Audit Log
// =========================================================================

pub async fn insert_audit_entry(
    pool: &PgPool,
    user_id: Option<Uuid>,
    action: &str,
    resource_type: Option<&str>,
    resource_id: Option<Uuid>,
    details: Option<&serde_json::Value>,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO audit_log (user_id, action, resource_type, resource_id, details, ip_address, user_agent)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(user_id)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(details)
    .bind(ip_address)
    .bind(user_agent)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error al insertar audit: {}", e))?;
    Ok(())
}

pub async fn query_audit_log(
    pool: &PgPool,
    action: Option<&str>,
    user_id: Option<Uuid>,
    desde: Option<&str>,
    hasta: Option<&str>,
) -> Result<Vec<AuditEntry>, String> {
    let mut sql = String::from(
        "SELECT id, user_id, action, resource_type, resource_id, details, ip_address::text, user_agent, created_at
         FROM audit_log WHERE 1=1",
    );

    // Dynamic build — but for simplicity, use a base query
    let rows = sqlx::query_as::<_, AuditEntry>(
        "SELECT id, user_id, action, resource_type, resource_id, details,
                COALESCE(host(ip_address), '') as ip_address, user_agent, created_at
         FROM audit_log
         WHERE ($1::text IS NULL OR action = $1)
           AND ($2::uuid IS NULL OR user_id = $2)
           AND ($3::timestamptz IS NULL OR created_at >= $3)
           AND ($4::timestamptz IS NULL OR created_at <= $4)
         ORDER BY created_at DESC
         LIMIT 500",
    )
    .bind(action)
    .bind(user_id)
    .bind(desde)
    .bind(hasta)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error al consultar audit: {}", e))?;

    Ok(rows)
}

// =========================================================================
// Quarantine
// =========================================================================

pub async fn insert_quarantined_email(
    pool: &PgPool,
    raw_email: &str,
    subject: Option<&str>,
    from_address: Option<&str>,
    failure_reason: &str,
    dkim_result: Option<&str>,
    spf_result: Option<&str>,
    dmarc_result: Option<&str>,
    parsed_data: Option<&serde_json::Value>,
) -> Result<Uuid, String> {
    let row = sqlx::query(
        "INSERT INTO quarantined_emails (raw_email, subject, from_address, failure_reason,
                                         dkim_result, spf_result, dmarc_result, parsed_data)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id",
    )
    .bind(raw_email)
    .bind(subject)
    .bind(from_address)
    .bind(failure_reason)
    .bind(dkim_result)
    .bind(spf_result)
    .bind(dmarc_result)
    .bind(parsed_data)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("DB error al insertar en cuarentena: {}", e))?;
    Ok(row.get("id"))
}

pub async fn list_quarantined(
    pool: &PgPool,
    status: Option<&str>,
) -> Result<Vec<QuarantinedEmail>, String> {
    let rows = sqlx::query_as::<_, QuarantinedEmail>(
        "SELECT id, raw_email, subject, from_address, failure_reason, dkim_result, spf_result,
                dmarc_result, parsed_data, reviewed_by, reviewed_at, review_decision, created_at
         FROM quarantined_emails
         WHERE ($1::text IS NULL OR review_decision = $1)
         ORDER BY created_at DESC
         LIMIT 200",
    )
    .bind(status)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error al listar cuarentena: {}", e))?;
    Ok(rows)
}

pub async fn review_quarantined(
    pool: &PgPool,
    quarantine_id: Uuid,
    decision: &str,
    reviewed_by: Uuid,
) -> Result<(), String> {
    let now = Utc::now();
    sqlx::query(
        "UPDATE quarantined_emails
         SET review_decision = $1, reviewed_by = $2, reviewed_at = $3
         WHERE id = $4",
    )
    .bind(decision)
    .bind(reviewed_by)
    .bind(now)
    .bind(quarantine_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error al revisar cuarentena: {}", e))?;
    Ok(())
}

// =========================================================================
// IMAP Credentials
// =========================================================================

pub async fn get_imap_credentials(pool: &PgPool) -> Result<Option<ImapCredentials>, String> {
    let row = sqlx::query_as::<_, ImapCredentials>(
        "SELECT id, email, encrypted_password, imap_host, imap_port, last_sync_at, created_at, updated_at
         FROM imap_credentials ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB error al obtener credenciales: {}", e))?;
    Ok(row)
}

pub async fn upsert_imap_credentials(
    pool: &PgPool,
    email: &str,
    encrypted_password: &str,
    host: &str,
    port: i32,
) -> Result<(), String> {
    // Delete existing and insert new (single-credential model)
    sqlx::query("DELETE FROM imap_credentials")
        .execute(pool)
        .await
        .map_err(|e| format!("DB error al limpiar credenciales: {}", e))?;

    sqlx::query(
        "INSERT INTO imap_credentials (email, encrypted_password, imap_host, imap_port)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(email)
    .bind(encrypted_password)
    .bind(host)
    .bind(port)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error al guardar credenciales: {}", e))?;
    Ok(())
}

pub async fn update_last_sync(pool: &PgPool, credential_id: Uuid) -> Result<(), String> {
    let now = Utc::now();
    sqlx::query(
        "UPDATE imap_credentials SET last_sync_at = $1, updated_at = now() WHERE id = $2",
    )
    .bind(now)
    .bind(credential_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error al actualizar last_sync: {}", e))?;
    Ok(())
}

// =========================================================================
// Imports
// =========================================================================

pub async fn create_import(
    pool: &PgPool,
    filename: &str,
    file_type: &str,
    total_rows: i32,
    uploaded_by: Uuid,
) -> Result<Uuid, String> {
    let row = sqlx::query(
        "INSERT INTO imports (filename, file_type, total_rows, uploaded_by)
         VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(filename)
    .bind(file_type)
    .bind(total_rows)
    .bind(uploaded_by)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("DB error al crear import: {}", e))?;
    Ok(row.get("id"))
}

pub async fn insert_import_row(
    pool: &PgPool,
    import_id: Uuid,
    row_number: i32,
    usuario: Option<&str>,
    monto: Option<f64>,
    fecha: Option<&str>,
    result: &str,
    error_message: Option<&str>,
    matched_payment_id: Option<Uuid>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO import_rows (import_id, row_number, usuario, monto, fecha, result, error_message, matched_payment_id)
         VALUES ($1, $2, $3, $4::numeric, $5, $6, $7, $8)",
    )
    .bind(import_id)
    .bind(row_number)
    .bind(usuario)
    .bind(monto.map(|m| m.to_string()))
    .bind(fecha)
    .bind(result)
    .bind(error_message)
    .bind(matched_payment_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error al insertar import row: {}", e))?;
    Ok(())
}

pub async fn update_import_counts(
    pool: &PgPool,
    import_id: Uuid,
    verified_rows: i32,
    failed_rows: i32,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE imports SET verified_rows = $1, failed_rows = $2 WHERE id = $3",
    )
    .bind(verified_rows)
    .bind(failed_rows)
    .bind(import_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error al actualizar import: {}", e))?;
    Ok(())
}

use chrono::Utc;
