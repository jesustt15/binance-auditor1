#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod client_config;
mod db;
mod http_client;
mod imap;
mod settings;

use client_config::ClientModeConfig;
use db::PagoBinance;
use http_client::HttpClient;
use settings::AppSettings;
use serde::Serialize;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri_plugin_notification::NotificationExt;

struct AppState {
    db_conn: Mutex<rusqlite::Connection>,
    http_client: Mutex<Option<HttpClient>>,
    mode: String,
}

#[derive(Serialize)]
struct SyncResult {
    success: bool,
    mensajes_nuevos: i64,
    total_procesados: usize,
    error: Option<String>,
}

#[derive(Serialize)]
struct VerifyResult {
    verificado: bool,
    mensaje: String,
    data: Option<PagoBinance>,
}

// ---------- Mode detection ----------

#[tauri::command]
fn get_app_mode(state: tauri::State<AppState>) -> Result<String, String> {
    Ok(state.mode.clone())
}

// ---------- Client mode commands ----------

#[tauri::command]
async fn client_login(
    username: String,
    password: String,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    let result = client.login(&username, &password).await?;

    if let Some(token) = result.get("token").and_then(|t| t.as_str()) {
        let mut guard = state.http_client.lock().map_err(|e| e.to_string())?;
        if let Some(ref mut client) = *guard {
            client.set_token(token.to_string());
        }
    }

    Ok(result)
}

#[tauri::command]
async fn client_verify_payment(
    usuario_empresa: String,
    monto_empresa: f64,
    fecha_empresa: String,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    client.verify_payment(&usuario_empresa, monto_empresa, &fecha_empresa).await
}

#[tauri::command]
async fn client_list_payments(
    desde: Option<String>,
    hasta: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    client.list_payments(desde.as_deref(), hasta.as_deref()).await
}

#[tauri::command]
async fn client_trigger_sync(
    mode: Option<String>,
    since_date: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    client.trigger_sync(mode.as_deref(), since_date.as_deref()).await
}

#[tauri::command]
async fn client_get_imap_config(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    client.get("/api/config/imap").await
}

#[tauri::command]
async fn client_save_imap_config(
    email: String,
    password: String,
    host: String,
    port: i32,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    let body = serde_json::json!({
        "email": email,
        "password": password,
        "host": host,
        "port": port,
    });
    client.post_json_value("/api/config/imap", &body).await
}

#[tauri::command]
async fn client_audit_log(
    params: Option<std::collections::HashMap<String, String>>,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    let query = params
        .map(|p| {
            p.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&")
        })
        .unwrap_or_default();
    let path = if query.is_empty() {
        "/api/audit".to_string()
    } else {
        format!("/api/audit?{}", query)
    };
    client.get(&path).await
}

#[tauri::command]
async fn client_list_quarantine(
    status: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    let path = format!("/api/quarantine?status={}", status.unwrap_or_else(|| "pending".to_string()));
    client.get(&path).await
}

#[tauri::command]
async fn client_review_quarantine(
    id: String,
    decision: String,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    let body = serde_json::json!({ "decision": decision });
    client.post_json_value(&format!("/api/quarantine/{}/review", id), &body).await
}

#[tauri::command]
async fn client_quick_verify_pago(
    payment_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    client.quick_verify_payment(&payment_id).await
}

#[tauri::command]
async fn client_list_users(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    client.list_users().await
}

#[tauri::command]
async fn client_create_user(
    username: String,
    password: String,
    role: String,
    company_group: Option<String>,
    station_name: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    client.create_user(&username, &password, &role, company_group.as_deref(), station_name.as_deref()).await
}

#[tauri::command]
async fn client_change_password(
    current_password: String,
    new_password: String,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    client.change_password(&current_password, &new_password).await
}

#[tauri::command]
async fn client_update_user(
    user_id: String,
    updates: serde_json::Value,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    client.update_user(&user_id, &updates).await
}

#[tauri::command]
async fn client_delete_user(
    user_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    client.delete_user(&user_id).await
}

#[tauri::command]
async fn client_admin_reset_password(
    user_id: String,
    new_password: String,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = {
        let guard = state.http_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("Cliente HTTP no inicializado")?.clone()
    };
    client.admin_reset_password(&user_id, &new_password).await
}

// ---------- Original standalone commands (unchanged) ----------

#[tauri::command]
fn sync_emails(
    app: tauri::AppHandle,
) -> Result<SyncResult, String> {
    let settings = settings::load_settings();
    if settings.imap_user.is_empty() || settings.imap_password.is_empty() {
        return Ok(SyncResult {
            success: false,
            mensajes_nuevos: 0,
            total_procesados: 0,
            error: Some("Configura las credenciales de IMAP primero en la seccion de ajustes.".to_string()),
        });
    }

    let db_path = db::get_db_path();
    let conn = rusqlite::Connection::open(db_path).map_err(|e| format!("DB open error: {}", e))?;
    conn.busy_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;

    match imap::sync_emails(&settings.imap_user, &settings.imap_password, &conn, &app) {
        Ok(stats) => {
            if stats.nuevos > 0 {
                let _ = app
                    .notification()
                    .builder()
                    .title("Binance Auditor")
                    .body(&format!("{} pago(s) nuevo(s) detectado(s) y guardado(s).", stats.nuevos))
                    .show();
            }
            Ok(SyncResult {
                success: true,
                mensajes_nuevos: stats.nuevos,
                total_procesados: stats.total_procesados,
                error: None,
            })
        }
        Err(e) => {
            eprintln!("[IMAP ERROR] {}", e);
            Ok(SyncResult {
                success: false,
                mensajes_nuevos: 0,
                total_procesados: 0,
                error: Some(e),
            })
        },
    }
}

#[tauri::command]
fn verify_payment(
    usuario_empresa: String,
    monto_empresa: f64,
    fecha_empresa: String,
    verified_by_name: String,
    company_group: Option<String>,
    state: tauri::State<AppState>,
) -> Result<VerifyResult, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;

    let fecha_base = chrono::NaiveDate::parse_from_str(&fecha_empresa, "%Y-%m-%d")
        .map_err(|e| format!("Fecha invalida: {}", e))?;

    let inicio_rango = (fecha_base - chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap();
    let fin_rango = (fecha_base + chrono::Duration::days(1)).and_hms_opt(23, 59, 59).unwrap();

    let pago = db::find_pago_for_verification(
        &conn,
        &usuario_empresa,
        monto_empresa,
        &inicio_rango.to_string(),
        &fin_rango.to_string(),
    ).map_err(|e| format!("DB error: {}", e))?;

    match pago {
        Some(p) => {
            if p.tipo == "deposito" {
                return Ok(VerifyResult {
                    verificado: false,
                    mensaje: "Los depositos no pueden ser verificados automaticamente porque no tienen remitente.".to_string(),
                    data: None,
                });
            }

            let obs = "Conciliado automaticamente con reporte de empresa.".to_string();
            db::mark_as_verificado(&conn, p.id, &obs, &verified_by_name, company_group.as_deref())
                .map_err(|e| format!("DB update error: {}", e))?;

            let updated_pago = PagoBinance {
                estado: "verificado".to_string(),
                verificado_en: Some(chrono::Utc::now().to_rfc3339()),
                observaciones: Some(obs.clone()),
                verified_by_name: Some(verified_by_name),
                company_group: company_group.or(p.company_group.clone()),
                ..p.clone()
            };

            Ok(VerifyResult {
                verificado: true,
                mensaje: format!("Pago verificado y firmado con exito! ID Interno: {}", p.id),
                data: Some(updated_pago),
            })
        }
        None => {
            Ok(VerifyResult {
                verificado: false,
                mensaje: "No se encontro ningun correo coincidente o el pago ya fue reclamado previamente.".to_string(),
                data: None,
            })
        },
    }
}

#[tauri::command]
fn get_reports(
    desde: String,
    hasta: String,
    monto_exacto: Option<f64>,
    monto_min: Option<f64>,
    monto_max: Option<f64>,
    current_role: Option<String>,
    current_username: Option<String>,
    state: tauri::State<AppState>,
) -> Result<Vec<PagoBinance>, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;
    let inicio = format!("{}T00:00:00", desde);
    let fin = format!("{}T23:59:59", hasta);
    let pagos = db::get_reports(&conn, &inicio, &fin, monto_exacto, monto_min, monto_max)
        .map_err(|e| format!("DB error: {}", e))?;

    // Cashiers: only see non-verified payments + their own verified ones
    let is_cashier = current_role.as_deref() == Some("cashier");
    let username = current_username.as_deref();
    Ok(if is_cashier {
        pagos
            .into_iter()
            .filter(|p| {
                p.verified_by_name.is_none()
                    || p.verified_by_name.as_deref() == username
            })
            .collect()
    } else {
        pagos
    })
}

#[tauri::command]
fn get_reports_by_sender(
    desde: String,
    hasta: String,
    remitente: String,
    monto_exacto: Option<f64>,
    monto_min: Option<f64>,
    monto_max: Option<f64>,
    current_role: Option<String>,
    current_username: Option<String>,
    state: tauri::State<AppState>,
) -> Result<Vec<PagoBinance>, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;
    let inicio = format!("{}T00:00:00", desde);
    let fin = format!("{}T23:59:59", hasta);
    let pagos = db::get_reports_by_sender(&conn, &inicio, &fin, &remitente, monto_exacto, monto_min, monto_max)
        .map_err(|e| format!("DB error: {}", e))?;

    // Cashiers: only see non-verified payments + their own verified ones
    let is_cashier = current_role.as_deref() == Some("cashier");
    let username = current_username.as_deref();
    Ok(if is_cashier {
        pagos
            .into_iter()
            .filter(|p| {
                p.verified_by_name.is_none()
                    || p.verified_by_name.as_deref() == username
            })
            .collect()
    } else {
        pagos
    })
}

#[derive(Serialize)]
struct ExportResult {
    success: bool,
    file_path: String,
    mensaje: String,
}

#[tauri::command]
fn export_reports(
    desde: String,
    hasta: String,
    monto_exacto: Option<f64>,
    monto_min: Option<f64>,
    monto_max: Option<f64>,
    state: tauri::State<AppState>,
) -> Result<ExportResult, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;
    let inicio = format!("{}T00:00:00", desde);
    let fin = format!("{}T23:59:59", hasta);
    let pagos = db::get_reports(&conn, &inicio, &fin, monto_exacto, monto_min, monto_max)
        .map_err(|e| format!("DB error: {}", e))?;

    if pagos.is_empty() {
        return Ok(ExportResult {
            success: false,
            file_path: String::new(),
            mensaje: "No hay pagos en el rango seleccionado para exportar.".to_string(),
        });
    }

    let exports_dir = db::get_exports_dir();
    let filename = format!("reporte_binance_{}_{}.xlsx", desde, hasta);
    let file_path = exports_dir.join(&filename);
    let path_str = file_path.to_string_lossy().to_string();
    db::export_to_excel(&pagos, &path_str)?;

    Ok(ExportResult {
        success: true,
        file_path: path_str.clone(),
        mensaje: format!("Exportado exitosamente: {} registros.\nArchivo: {}", pagos.len(), path_str),
    })
}

#[tauri::command]
fn debug_listar_pagos(
    desde: Option<String>,
    hasta: Option<String>,
    monto_exacto: Option<f64>,
    monto_min: Option<f64>,
    monto_max: Option<f64>,
    current_role: Option<String>,
    current_username: Option<String>,
    state: tauri::State<AppState>,
) -> Result<Vec<PagoBinance>, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;
    let pagos = match (desde, hasta) {
        (Some(d), Some(h)) => {
            let inicio = format!("{}T00:00:00", d);
            let fin = format!("{}T23:59:59", h);
            db::get_reports(&conn, &inicio, &fin, monto_exacto, monto_min, monto_max).map_err(|e| format!("DB error: {}", e))?
        }
        _ => {
            db::get_all_pagos(&conn).map_err(|e| format!("DB error: {}", e))?
        }
    };

    // Cashiers: only see non-verified payments + their own verified ones
    let is_cashier = current_role.as_deref() == Some("cashier");
    let username = current_username.as_deref();
    Ok(if is_cashier {
        pagos
            .into_iter()
            .filter(|p| {
                p.verified_by_name.is_none() // non-verified (available)
                    || p.verified_by_name.as_deref() == username // own verified
            })
            .collect()
    } else {
        pagos
    })
}

#[derive(Serialize, Clone)]
struct ImportRowDetail {
    fila: usize,
    usuario: String,
    monto: f64,
    fecha: String,
    resultado: String,
}

#[derive(Serialize)]
struct ImportResult {
    total_filas: usize,
    verificados: usize,
    no_encontrados: usize,
    errores: usize,
    detalle: Vec<ImportRowDetail>,
}

#[tauri::command]
fn import_csv(
    contenido: String,
    username: String,
    state: tauri::State<AppState>,
) -> Result<ImportResult, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;
    let mut builder = csv::ReaderBuilder::new();
    builder.flexible(true);
    let mut reader = builder.from_reader(contenido.as_bytes());

    let headers = reader.headers()
        .map_err(|e| format!("Error leyendo headers del CSV: {}", e))?
        .clone();

    let col_usuario = find_column(&headers, &["usuario", "user", "remitente", "nombre"]);
    let col_monto = find_column(&headers, &["monto", "amount", "importe"]);
    let col_fecha = find_column(&headers, &["fecha", "date"]);

    let (col_u, col_m, col_f) = match (col_usuario, col_monto, col_fecha) {
        (Some(u), Some(m), Some(f)) => (u, m, f),
        _ => (0usize, 1usize, 2usize),
    };

    let mut resultados: Vec<ImportRowDetail> = Vec::new();
    let mut verificados = 0usize;
    let mut no_encontrados = 0usize;
    let mut errores = 0usize;

    for (i, record) in reader.records().enumerate() {
        let fila_num = i + 2;
        let record = match record {
            Ok(r) => r,
            Err(e) => {
                resultados.push(ImportRowDetail { fila: fila_num, usuario: String::new(), monto: 0.0, fecha: String::new(), resultado: format!("error: no se pudo leer fila: {}", e) });
                errores += 1;
                continue;
            }
        };

        let usuario = record.get(col_u).unwrap_or("").trim().to_string();
        let monto_str = record.get(col_m).unwrap_or("").trim().replace(",", "");
        let fecha = record.get(col_f).unwrap_or("").trim().to_string();

        if usuario.is_empty() || monto_str.is_empty() || fecha.is_empty() {
            resultados.push(ImportRowDetail { fila: fila_num, usuario, monto: 0.0, fecha, resultado: "error: campos vacios".to_string() });
            errores += 1;
            continue;
        }

        let monto: f64 = match monto_str.parse() {
            Ok(m) => m,
            Err(e) => {
                resultados.push(ImportRowDetail { fila: fila_num, usuario, monto: 0.0, fecha, resultado: format!("error: monto invalido '{}': {}", monto_str, e) });
                errores += 1;
                continue;
            }
        };

        let fecha_base = match chrono::NaiveDate::parse_from_str(&fecha, "%Y-%m-%d") {
            Ok(d) => d,
            Err(e) => {
                resultados.push(ImportRowDetail { fila: fila_num, usuario: usuario.clone(), monto, fecha: fecha.clone(), resultado: format!("error: fecha invalida (use YYYY-MM-DD): {}", e) });
                errores += 1;
                continue;
            }
        };

        let inicio_rango = (fecha_base - chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap();
        let fin_rango = (fecha_base + chrono::Duration::days(1)).and_hms_opt(23, 59, 59).unwrap();

        match db::find_pago_for_verification(&conn, &usuario, monto, &inicio_rango.to_string(), &fin_rango.to_string()) {
            Ok(Some(p)) => {
                let obs = format!("Conciliado via CSV (fila {}).", fila_num);
                let _ = db::mark_as_verificado(&conn, p.id, &obs, &username, None);
                resultados.push(ImportRowDetail { fila: fila_num, usuario, monto, fecha, resultado: "verificado".to_string() });
                verificados += 1;
            }
            Ok(None) => {
                resultados.push(ImportRowDetail { fila: fila_num, usuario, monto, fecha, resultado: "no_encontrado".to_string() });
                no_encontrados += 1;
            }
            Err(e) => {
                resultados.push(ImportRowDetail { fila: fila_num, usuario, monto, fecha, resultado: format!("error: {}", e) });
                errores += 1;
            }
        }
    }

    Ok(ImportResult { total_filas: resultados.len(), verificados, no_encontrados, errores, detalle: resultados })
}

fn find_column(headers: &csv::StringRecord, candidates: &[&str]) -> Option<usize> {
    for i in 0..headers.len() {
        if let Some(header) = headers.get(i) {
            let h = header.trim().to_lowercase();
            for &c in candidates {
                if h == c { return Some(i); }
            }
        }
    }
    None
}

#[tauri::command]
fn save_settings(imap_user: String, imap_password: String) -> Result<(), String> {
    let settings = AppSettings { imap_user, imap_password };
    settings::save_settings(&settings)
}

#[tauri::command]
fn get_settings() -> Result<AppSettings, String> {
    Ok(settings::load_settings())
}

#[tauri::command]
fn sync_historical(since_date: String, app: tauri::AppHandle) -> Result<SyncResult, String> {
    let settings = settings::load_settings();
    if settings.imap_user.is_empty() || settings.imap_password.is_empty() {
        return Ok(SyncResult { success: false, mensajes_nuevos: 0, total_procesados: 0, error: Some("Configura las credenciales de IMAP primero.".to_string()) });
    }

    let since_parsed = chrono::NaiveDate::parse_from_str(&since_date, "%Y-%m-%d")
        .map_err(|e| format!("Fecha invalida (use YYYY-MM-DD): {}", e))?;
    let since_imap = since_parsed.format("%d-%b-%Y").to_string();

    let db_path = db::get_db_path();
    let conn = rusqlite::Connection::open(db_path).map_err(|e| format!("DB open error: {}", e))?;
    conn.busy_timeout(Duration::from_secs(10)).map_err(|e| e.to_string())?;

    match imap::sync_emails_historical(&settings.imap_user, &settings.imap_password, &conn, &app, &since_imap) {
        Ok(stats) => {
            if stats.nuevos > 0 {
                let _ = app.notification().builder().title("Binance Auditor").body(&format!("Sync historico completado: {} pago(s) nuevo(s).", stats.nuevos)).show();
            }
            Ok(SyncResult { success: true, mensajes_nuevos: stats.nuevos, total_procesados: stats.total_procesados, error: None })
        }
        Err(e) => Ok(SyncResult { success: false, mensajes_nuevos: 0, total_procesados: 0, error: Some(e) }),
    }
}

#[tauri::command]
fn import_excel(file_path: String, username: String, state: tauri::State<AppState>) -> Result<ImportResult, String> {
    use calamine::{open_workbook_auto, Reader, DataType};
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;
    let mut workbook: calamine::Sheets<_> = open_workbook_auto(&file_path)
        .map_err(|e| format!("Error abriendo Excel '{}': {}", file_path, e))?;

    let range = workbook.worksheet_range_at(0).ok_or("El archivo Excel no tiene hojas".to_string())?.map_err(|e| format!("Error leyendo hoja: {}", e))?;

    let headers: Vec<String> = range.rows().next().ok_or("El archivo Excel esta vacio".to_string())?.iter().map(|c| c.as_string().unwrap_or_default().trim().to_lowercase()).collect();

    let find_col = |candidates: &[&str]| -> Option<usize> { headers.iter().position(|h| candidates.iter().any(|c| h.contains(c))) };
    let col_usuario = find_col(&["usuario", "user", "remitente", "nombre"]);
    let col_monto = find_col(&["monto", "amount", "importe"]);
    let col_fecha = find_col(&["fecha", "date"]);
    let (col_u, col_m, col_f) = match (col_usuario, col_monto, col_fecha) {
        (Some(u), Some(m), Some(f)) => (u, m, f),
        _ => (0usize, 1usize, 2usize),
    };

    let mut resultados: Vec<ImportRowDetail> = Vec::new();
    let mut verificados = 0usize;
    let mut no_encontrados = 0usize;
    let mut errores = 0usize;

    for (row_idx, row) in range.rows().enumerate().skip(1) {
        let fila_num = row_idx + 2;
        let usuario = row.get(col_u).and_then(|c| c.as_string()).unwrap_or_default().trim().to_string();
        let monto_str = row.get(col_m).and_then(|c| c.as_f64()).map(|v| v.to_string()).or_else(|| row.get(col_m).and_then(|c| c.as_string()).map(|s| s.replace(",", "").trim().to_string())).unwrap_or_default();
        let fecha = row.get(col_f).and_then(|c| c.as_string()).unwrap_or_default().trim().to_string();

        if usuario.is_empty() || monto_str.is_empty() || fecha.is_empty() {
            resultados.push(ImportRowDetail { fila: fila_num, usuario, monto: 0.0, fecha, resultado: "error: campos vacios".to_string() });
            errores += 1;
            continue;
        }

        let monto: f64 = match monto_str.parse() {
            Ok(m) => m,
            Err(e) => { resultados.push(ImportRowDetail { fila: fila_num, usuario, monto: 0.0, fecha, resultado: format!("error: monto invalido: {}", e) }); errores += 1; continue; }
        };

        let fecha_base = chrono::NaiveDate::parse_from_str(&fecha, "%Y-%m-%d").or_else(|_| chrono::NaiveDate::parse_from_str(&fecha, "%d/%m/%Y")).or_else(|_| chrono::NaiveDate::parse_from_str(&fecha, "%m/%d/%Y")).map_err(|e| format!("fecha invalida: {}", e));

        let fecha_base = match fecha_base {
            Ok(d) => d,
            Err(e) => { resultados.push(ImportRowDetail { fila: fila_num, usuario: usuario.clone(), monto, fecha: fecha.clone(), resultado: format!("error: {}", e) }); errores += 1; continue; }
        };

        let inicio_rango = (fecha_base - chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap();
        let fin_rango = (fecha_base + chrono::Duration::days(1)).and_hms_opt(23, 59, 59).unwrap();

        match db::find_pago_for_verification(&conn, &usuario, monto, &inicio_rango.to_string(), &fin_rango.to_string()) {
            Ok(Some(p)) => { let _ = db::mark_as_verificado(&conn, p.id, &format!("Conciliado via Excel (fila {}).", fila_num), &username, None); resultados.push(ImportRowDetail { fila: fila_num, usuario, monto, fecha, resultado: "verificado".to_string() }); verificados += 1; }
            Ok(None) => { resultados.push(ImportRowDetail { fila: fila_num, usuario, monto, fecha, resultado: "no_encontrado".to_string() }); no_encontrados += 1; }
            Err(e) => { resultados.push(ImportRowDetail { fila: fila_num, usuario, monto, fecha, resultado: format!("error: {}", e) }); errores += 1; }
        }
    }

    Ok(ImportResult { total_filas: resultados.len(), verificados, no_encontrados, errores, detalle: resultados })
}

#[tauri::command]
fn pick_and_import_excel(username: String, state: tauri::State<AppState>) -> Result<ImportResult, String> {
    let file = rfd::FileDialog::new().add_filter("Excel Files", &["xlsx", "xls"]).pick_file();
    let file_path = match file { Some(p) => p.to_string_lossy().to_string(), None => return Err("No se selecciono ningun archivo".to_string()) };
    import_excel(file_path, username, state)
}

fn main() {
    // Detectar modo
    let config = ClientModeConfig::detect();
    let is_client_mode = config.is_client_mode();
    eprintln!("[MODE] Modo detectado: {}", config.mode);

    let db_conn = db::init_db().expect("Failed to initialize database");

    let http_client = if is_client_mode {
        let server_url = config.server_url().unwrap_or("https://localhost:8443");
        eprintln!("[CLIENT] Conectando a servidor: {}", server_url);
        HttpClient::new(server_url).ok()
    } else {
        None
    };

    let app_mode = if is_client_mode { "client".to_string() } else { "standalone".to_string() };

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            // En modo standalone, corremos sync en background
            if !is_client_mode {
                thread::spawn(move || {
                    thread::sleep(Duration::from_secs(10));
                    loop {
                        let settings = settings::load_settings();
                        if !settings.imap_user.is_empty() && !settings.imap_password.is_empty() {
                            let db_path = db::get_db_path();
                            if let Ok(conn) = rusqlite::Connection::open(&db_path) {
                                if conn.busy_timeout(Duration::from_secs(5)).is_ok() {
                                    match imap::sync_emails(&settings.imap_user, &settings.imap_password, &conn, &app_handle) {
                                        Ok(stats) => {
                                            eprintln!("[BACKGROUND] Sync: {} nuevos", stats.nuevos);
                                            if stats.nuevos > 0 {
                                                let _ = app_handle.notification().builder().title("Binance Auditor").body(&format!("{} pago(s) nuevo(s).", stats.nuevos)).show();
                                            }
                                        }
                                        Err(e) => eprintln!("[BACKGROUND] Error: {}", e),
                                    }
                                }
                            }
                        }
                        thread::sleep(Duration::from_secs(5 * 60));
                    }
                });
            }
            Ok(())
        })
        .manage(AppState {
            db_conn: Mutex::new(db_conn),
            http_client: Mutex::new(http_client),
            mode: app_mode,
        })
        .invoke_handler(tauri::generate_handler![
            get_app_mode,
            // Client mode commands
            client_login,
            client_verify_payment,
            client_list_payments,
            client_trigger_sync,
            client_get_imap_config,
            client_save_imap_config,
            client_audit_log,
            client_list_quarantine,
            client_review_quarantine,
            client_quick_verify_pago,
            client_list_users,
            client_create_user,
            client_update_user,
            client_delete_user,
            client_admin_reset_password,
            client_change_password,
            // Standalone commands
            sync_emails,
            sync_historical,
            verify_payment,
            get_reports,
            get_reports_by_sender,
            export_reports,
            import_csv,
            import_excel,
            pick_and_import_excel,
            save_settings,
            get_settings,
            debug_listar_pagos,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
