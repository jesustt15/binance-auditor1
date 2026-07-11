#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod imap;
mod settings;

use db::PagoBinance;
use settings::AppSettings;
use serde::Serialize;
use std::sync::Mutex;

struct AppState {
    db_conn: Mutex<rusqlite::Connection>,
}

#[derive(Serialize)]
struct SyncResult {
    success: bool,
    mensajes_nuevos: i64,
    error: Option<String>,
}

#[derive(Serialize)]
struct VerifyResult {
    verificado: bool,
    mensaje: String,
    data: Option<PagoBinance>,
}

#[tauri::command]
fn sync_emails(state: tauri::State<AppState>) -> Result<SyncResult, String> {
    let settings = settings::load_settings();
    if settings.imap_user.is_empty() || settings.imap_password.is_empty() {
        return Ok(SyncResult {
            success: false,
            mensajes_nuevos: 0,
            error: Some("Configura las credenciales de IMAP primero en la seccion de ajustes.".to_string()),
        });
    }

    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;
    match imap::sync_emails(&settings.imap_user, &settings.imap_password, &conn) {
        Ok(count) => Ok(SyncResult {
            success: true,
            mensajes_nuevos: count,
            error: None,
        }),
        Err(e) => Ok(SyncResult {
            success: false,
            mensajes_nuevos: 0,
            error: Some(e),
        }),
    }
}

#[tauri::command]
fn verify_payment(
    usuario_empresa: String,
    monto_empresa: f64,
    fecha_empresa: String,
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
            let obs = "Conciliado automaticamente con reporte de empresa.".to_string();
            db::mark_as_verificado(&conn, p.id, &obs)
                .map_err(|e| format!("DB update error: {}", e))?;

            let updated_pago = PagoBinance {
                estado: "verificado".to_string(),
                verificado_en: Some(chrono::Utc::now().to_rfc3339()),
                observaciones: Some(obs.clone()),
                ..p.clone()
            };

            Ok(VerifyResult {
                verificado: true,
                mensaje: format!("Pago verificado y firmado con exito! ID Interno: {}", p.id),
                data: Some(updated_pago),
            })
        }
        None => Ok(VerifyResult {
            verificado: false,
            mensaje: "No se encontro ningun correo coincidente o el pago ya fue reclamado previamente.".to_string(),
            data: None,
        }),
    }
}

#[tauri::command]
fn get_reports(
    desde: String,
    hasta: String,
    state: tauri::State<AppState>,
) -> Result<Vec<PagoBinance>, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;

    let inicio = format!("{}T00:00:00", desde);
    let fin = format!("{}T23:59:59", hasta);

    db::get_reports(&conn, &inicio, &fin)
        .map_err(|e| format!("DB error: {}", e))
}

#[tauri::command]
fn save_settings(imap_user: String, imap_password: String) -> Result<(), String> {
    let settings = AppSettings {
        imap_user,
        imap_password,
    };
    settings::save_settings(&settings)
}

#[tauri::command]
fn get_settings() -> Result<AppSettings, String> {
    Ok(settings::load_settings())
}

fn main() {
    let db_conn = db::init_db().expect("Failed to initialize database");

    tauri::Builder::default()
        .manage(AppState {
            db_conn: Mutex::new(db_conn),
        })
        .invoke_handler(tauri::generate_handler![
            sync_emails,
            verify_payment,
            get_reports,
            save_settings,
            get_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
