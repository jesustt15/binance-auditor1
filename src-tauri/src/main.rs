#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod imap;
mod settings;

use db::PagoBinance;
use settings::AppSettings;
use serde::Serialize;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri_plugin_notification::NotificationExt;

struct AppState {
    db_conn: Mutex<rusqlite::Connection>,
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
            // Trigger desktop notification for new payments
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
    state: tauri::State<AppState>,
) -> Result<VerifyResult, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;

    let fecha_base = chrono::NaiveDate::parse_from_str(&fecha_empresa, "%Y-%m-%d")
        .map_err(|e| format!("Fecha invalida: {}", e))?;

    let inicio_rango = (fecha_base - chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap();
    let fin_rango = (fecha_base + chrono::Duration::days(1)).and_hms_opt(23, 59, 59).unwrap();

    eprintln!("[VERIFY DEBUG] Buscando: usuario='{}' monto={} fecha_base='{}'",
        usuario_empresa, monto_empresa, fecha_empresa);
    eprintln!("[VERIFY DEBUG] Rango: '{}' hasta '{}'",
        inicio_rango, fin_rango);

    // Log ALL payments in range for diagnostics
    if let Ok(todos) = db::get_reports(&conn, &inicio_rango.to_string(), &fin_rango.to_string(), None, None, None) {
        eprintln!("[VERIFY DEBUG] Pagos en BD dentro del rango: {} encontrados", todos.len());
        for p in &todos {
            eprintln!("[VERIFY DEBUG]   DB: id={} tipo='{}' usuario='{}' monto={} moneda='{}' fecha='{}' estado='{}'",
                p.id, p.tipo, p.usuario_remitente.as_deref().unwrap_or("N/A"), p.monto, p.moneda, p.fecha_correo, p.estado);
        }
    }

    let pago = db::find_pago_for_verification(
        &conn,
        &usuario_empresa,
        monto_empresa,
        &inicio_rango.to_string(),
        &fin_rango.to_string(),
    ).map_err(|e| format!("DB error: {}", e))?;

    match pago {
        Some(p) => {
            // Guard: skip deposits — they have no sender to match
            if p.tipo == "deposito" {
                return Ok(VerifyResult {
                    verificado: false,
                    mensaje: "Los depositos no pueden ser verificados automaticamente porque no tienen remitente.".to_string(),
                    data: None,
                });
            }

            eprintln!("[VERIFY DEBUG] MATCH encontrado: id={} tipo='{}' usuario='{}' monto={}",
                p.id, p.tipo, p.usuario_remitente.as_deref().unwrap_or("N/A"), p.monto);
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
        None => {
            eprintln!("[VERIFY DEBUG] NO MATCH: verifica que usuario, monto, y fecha coincidan exactamente.");
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
    state: tauri::State<AppState>,
) -> Result<Vec<PagoBinance>, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;

    let inicio = format!("{}T00:00:00", desde);
    let fin = format!("{}T23:59:59", hasta);

    db::get_reports(&conn, &inicio, &fin, monto_exacto, monto_min, monto_max)
        .map_err(|e| format!("DB error: {}", e))
}

#[tauri::command]
fn get_reports_by_sender(
    desde: String,
    hasta: String,
    remitente: String,
    monto_exacto: Option<f64>,
    monto_min: Option<f64>,
    monto_max: Option<f64>,
    state: tauri::State<AppState>,
) -> Result<Vec<PagoBinance>, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;

    let inicio = format!("{}T00:00:00", desde);
    let fin = format!("{}T23:59:59", hasta);

    db::get_reports_by_sender(&conn, &inicio, &fin, &remitente, monto_exacto, monto_min, monto_max)
        .map_err(|e| format!("DB error: {}", e))
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

    eprintln!("[EXPORT] Archivo generado: {} ({} registros)", path_str, pagos.len());

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
    state: tauri::State<AppState>,
) -> Result<Vec<PagoBinance>, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;

    let pagos = match (desde, hasta) {
        (Some(d), Some(h)) => {
            let inicio = format!("{}T00:00:00", d);
            let fin = format!("{}T23:59:59", h);
            eprintln!("[DEBUG] Listando pagos en rango: {} a {}", inicio, fin);
            db::get_reports(&conn, &inicio, &fin, monto_exacto, monto_min, monto_max).map_err(|e| format!("DB error: {}", e))?
        }
        _ => {
            eprintln!("[DEBUG] Listando TODOS los pagos en BD...");
            db::get_all_pagos(&conn).map_err(|e| format!("DB error: {}", e))?
        }
    };

    eprintln!("[DEBUG] Total pagos: {}", pagos.len());
    for p in &pagos {
        eprintln!("[DEBUG]   id={} tipo='{}' usuario='{}' monto={} moneda='{}' fecha='{}' estado='{}'",
            p.id, p.tipo, p.usuario_remitente.as_deref().unwrap_or("N/A"), p.monto, p.moneda, p.fecha_correo, p.estado);
    }
    Ok(pagos)
}

#[derive(Serialize, Clone)]
struct ImportRowDetail {
    fila: usize,
    usuario: String,
    monto: f64,
    fecha: String,
    resultado: String, // "verificado", "no_encontrado", "error: ..."
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
    state: tauri::State<AppState>,
) -> Result<ImportResult, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;

    let mut builder = csv::ReaderBuilder::new();
    builder.flexible(true);
    let mut reader = builder.from_reader(contenido.as_bytes());

    // Auto-detect column indices by header names
    let headers = reader.headers()
        .map_err(|e| format!("Error leyendo headers del CSV: {}", e))?
        .clone();

    let col_usuario = find_column(&headers, &["usuario", "user", "remitente", "nombre"]);
    let col_monto = find_column(&headers, &["monto", "amount", "importe"]);
    let col_fecha = find_column(&headers, &["fecha", "date"]);

    let (col_u, col_m, col_f) = match (col_usuario, col_monto, col_fecha) {
        (Some(u), Some(m), Some(f)) => (u, m, f),
        _ => {
            // Fallback: usar posiciones fijas (0=usuario, 1=monto, 2=fecha)
            eprintln!("[CSV] Headers no detectados, usando posiciones fijas. Headers: {:?}", headers);
            (0usize, 1usize, 2usize)
        }
    };

    let mut resultados: Vec<ImportRowDetail> = Vec::new();
    let mut verificados = 0usize;
    let mut no_encontrados = 0usize;
    let mut errores = 0usize;

    for (i, record) in reader.records().enumerate() {
        let fila_num = i + 2; // +2: 1-indexed + header row

        let record = match record {
            Ok(r) => r,
            Err(e) => {
                resultados.push(ImportRowDetail {
                    fila: fila_num,
                    usuario: String::new(),
                    monto: 0.0,
                    fecha: String::new(),
                    resultado: format!("error: no se pudo leer fila: {}", e),
                });
                errores += 1;
                continue;
            }
        };

        let usuario = record.get(col_u).unwrap_or("").trim().to_string();
        let monto_str = record.get(col_m).unwrap_or("").trim().replace(",", "");
        let fecha = record.get(col_f).unwrap_or("").trim().to_string();

        if usuario.is_empty() || monto_str.is_empty() || fecha.is_empty() {
            resultados.push(ImportRowDetail {
                fila: fila_num,
                usuario,
                monto: 0.0,
                fecha,
                resultado: "error: campos vacios".to_string(),
            });
            errores += 1;
            continue;
        }

        let monto: f64 = match monto_str.parse() {
            Ok(m) => m,
            Err(e) => {
                resultados.push(ImportRowDetail {
                    fila: fila_num,
                    usuario,
                    monto: 0.0,
                    fecha,
                    resultado: format!("error: monto invalido '{}': {}", monto_str, e),
                });
                errores += 1;
                continue;
            }
        };

        // Run the same verification logic as verify_payment
        let fecha_base = match chrono::NaiveDate::parse_from_str(&fecha, "%Y-%m-%d") {
            Ok(d) => d,
            Err(e) => {
                resultados.push(ImportRowDetail {
                    fila: fila_num,
                    usuario: usuario.clone(),
                    monto,
                    fecha: fecha.clone(),
                    resultado: format!("error: fecha invalida (use YYYY-MM-DD): {}", e),
                });
                errores += 1;
                continue;
            }
        };

        let inicio_rango = (fecha_base - chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap();
        let fin_rango = (fecha_base + chrono::Duration::days(1)).and_hms_opt(23, 59, 59).unwrap();

        match db::find_pago_for_verification(
            &conn,
            &usuario,
            monto,
            &inicio_rango.to_string(),
            &fin_rango.to_string(),
        ) {
            Ok(Some(p)) => {
                let obs = format!("Conciliado via CSV (fila {}).", fila_num);
                let _ = db::mark_as_verificado(&conn, p.id, &obs);
                resultados.push(ImportRowDetail {
                    fila: fila_num,
                    usuario,
                    monto,
                    fecha,
                    resultado: "verificado".to_string(),
                });
                verificados += 1;
            }
            Ok(None) => {
                resultados.push(ImportRowDetail {
                    fila: fila_num,
                    usuario,
                    monto,
                    fecha,
                    resultado: "no_encontrado".to_string(),
                });
                no_encontrados += 1;
            }
            Err(e) => {
                resultados.push(ImportRowDetail {
                    fila: fila_num,
                    usuario,
                    monto,
                    fecha,
                    resultado: format!("error: {}", e),
                });
                errores += 1;
            }
        }
    }

    eprintln!("[CSV] Importado: {} filas | {} verificados | {} no encontrados | {} errores",
        resultados.len(), verificados, no_encontrados, errores);

    Ok(ImportResult {
        total_filas: resultados.len(),
        verificados,
        no_encontrados,
        errores,
        detalle: resultados,
    })
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

#[tauri::command]
fn sync_historical(
    since_date: String,
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

    // Parse and validate the date (expected: YYYY-MM-DD)
    let since_parsed = chrono::NaiveDate::parse_from_str(&since_date, "%Y-%m-%d")
        .map_err(|e| format!("Fecha invalida (use YYYY-MM-DD): {}", e))?;
    let since_imap = since_parsed.format("%d-%b-%Y").to_string();

    let db_path = db::get_db_path();
    let conn = rusqlite::Connection::open(db_path).map_err(|e| format!("DB open error: {}", e))?;
    conn.busy_timeout(Duration::from_secs(10)).map_err(|e| e.to_string())?;

    eprintln!("[HISTORICAL] Sync historico desde: {} (IMAP: {})", since_date, since_imap);

    match imap::sync_emails_historical(&settings.imap_user, &settings.imap_password, &conn, &app, &since_imap) {
        Ok(stats) => {
            if stats.nuevos > 0 {
                let _ = app
                    .notification()
                    .builder()
                    .title("Binance Auditor")
                    .body(&format!(
                        "Sync historico completado: {} pago(s) nuevo(s) registrado(s).",
                        stats.nuevos
                    ))
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
            eprintln!("[HISTORICAL] IMAP error: {}", e);
            Ok(SyncResult {
                success: false,
                mensajes_nuevos: 0,
                total_procesados: 0,
                error: Some(e),
            })
        }
    }
}

#[tauri::command]
fn import_excel(
    file_path: String,
    state: tauri::State<AppState>,
) -> Result<ImportResult, String> {
    use calamine::{open_workbook_auto, Reader, DataType};

    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;

    let mut workbook: calamine::Sheets<_> = open_workbook_auto(&file_path)
        .map_err(|e| format!("Error abriendo Excel '{}': {}", file_path, e))?;

    // Read the first sheet
    let range = workbook
        .worksheet_range_at(0)
        .ok_or("El archivo Excel no tiene hojas".to_string())?
        .map_err(|e| format!("Error leyendo hoja: {}", e))?;

    // Auto-detect column indices from header row
    let headers: Vec<String> = range.rows()
        .next()
        .ok_or("El archivo Excel esta vacio".to_string())?
        .iter()
        .map(|c| c.as_string().unwrap_or_default().trim().to_lowercase())
        .collect();

    let find_col = |candidates: &[&str]| -> Option<usize> {
        headers.iter().position(|h| candidates.iter().any(|c| h.contains(c)))
    };

    let col_usuario = find_col(&["usuario", "user", "remitente", "nombre"]);
    let col_monto = find_col(&["monto", "amount", "importe"]);
    let col_fecha = find_col(&["fecha", "date"]);

    let (col_u, col_m, col_f) = match (col_usuario, col_monto, col_fecha) {
        (Some(u), Some(m), Some(f)) => (u, m, f),
        _ => {
            eprintln!("[XLSX] Headers no detectados, usando posiciones fijas (0,1,2). Headers: {:?}", headers);
            (0usize, 1usize, 2usize)
        }
    };

    let mut resultados: Vec<ImportRowDetail> = Vec::new();
    let mut verificados = 0usize;
    let mut no_encontrados = 0usize;
    let mut errores = 0usize;

    // Skip header row (row index 0)
    for (row_idx, row) in range.rows().enumerate().skip(1) {
        let fila_num = row_idx + 2; // 1-indexed + header

        let usuario = row.get(col_u).and_then(|c| c.as_string()).unwrap_or_default().trim().to_string();
        let monto_str = row.get(col_m)
            .and_then(|c| c.as_f64())
            .map(|v| v.to_string())
            .or_else(|| row.get(col_m).and_then(|c| c.as_string()).map(|s| s.replace(",", "").trim().to_string()))
            .unwrap_or_default();
        let fecha = row.get(col_f).and_then(|c| c.as_string()).unwrap_or_default().trim().to_string();

        if usuario.is_empty() || monto_str.is_empty() || fecha.is_empty() {
            resultados.push(ImportRowDetail {
                fila: fila_num,
                usuario,
                monto: 0.0,
                fecha,
                resultado: "error: campos vacios".to_string(),
            });
            errores += 1;
            continue;
        }

        let monto: f64 = match monto_str.parse() {
            Ok(m) => m,
            Err(e) => {
                resultados.push(ImportRowDetail {
                    fila: fila_num,
                    usuario,
                    monto: 0.0,
                    fecha,
                    resultado: format!("error: monto invalido '{}': {}", monto_str, e),
                });
                errores += 1;
                continue;
            }
        };

        // Try multiple date formats
        let fecha_base = chrono::NaiveDate::parse_from_str(&fecha, "%Y-%m-%d")
            .or_else(|_| chrono::NaiveDate::parse_from_str(&fecha, "%d/%m/%Y"))
            .or_else(|_| chrono::NaiveDate::parse_from_str(&fecha, "%m/%d/%Y"))
            .map_err(|e| format!("fecha invalida '{}': {}", fecha, e));

        let fecha_base = match fecha_base {
            Ok(d) => d,
            Err(e) => {
                resultados.push(ImportRowDetail {
                    fila: fila_num,
                    usuario: usuario.clone(),
                    monto,
                    fecha: fecha.clone(),
                    resultado: format!("error: {}", e),
                });
                errores += 1;
                continue;
            }
        };

        let inicio_rango = (fecha_base - chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap();
        let fin_rango = (fecha_base + chrono::Duration::days(1)).and_hms_opt(23, 59, 59).unwrap();

        match db::find_pago_for_verification(
            &conn,
            &usuario,
            monto,
            &inicio_rango.to_string(),
            &fin_rango.to_string(),
        ) {
            Ok(Some(p)) => {
                let obs = format!("Conciliado via Excel (fila {}).", fila_num);
                let _ = db::mark_as_verificado(&conn, p.id, &obs);
                resultados.push(ImportRowDetail {
                    fila: fila_num,
                    usuario,
                    monto,
                    fecha,
                    resultado: "verificado".to_string(),
                });
                verificados += 1;
            }
            Ok(None) => {
                resultados.push(ImportRowDetail {
                    fila: fila_num,
                    usuario,
                    monto,
                    fecha,
                    resultado: "no_encontrado".to_string(),
                });
                no_encontrados += 1;
            }
            Err(e) => {
                resultados.push(ImportRowDetail {
                    fila: fila_num,
                    usuario,
                    monto,
                    fecha,
                    resultado: format!("error: {}", e),
                });
                errores += 1;
            }
        }
    }

    eprintln!("[XLSX] Importado: {} filas | {} verificados | {} no encontrados | {} errores",
        resultados.len(), verificados, no_encontrados, errores);

    Ok(ImportResult {
        total_filas: resultados.len(),
        verificados,
        no_encontrados,
        errores,
        detalle: resultados,
    })
}

/// Opens a native file dialog and imports the selected Excel file.
#[tauri::command]
fn pick_and_import_excel(
    state: tauri::State<AppState>,
) -> Result<ImportResult, String> {
    let file = rfd::FileDialog::new()
        .add_filter("Excel Files", &["xlsx", "xls"])
        .pick_file();

    let file_path = match file {
        Some(p) => p.to_string_lossy().to_string(),
        None => return Err("No se selecciono ningun archivo".to_string()),
    };

    import_excel(file_path, state)
}

fn main() {
    let db_conn = db::init_db().expect("Failed to initialize database");

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // Background thread: sync IMAP every 5 minutes
            let app_handle = app.handle().clone();
            thread::spawn(move || {
                // Give the app a moment to fully start before first sync
                thread::sleep(Duration::from_secs(10));

                loop {
                    eprintln!("[BACKGROUND] Iniciando sync automatico IMAP...");
                    let settings = settings::load_settings();

                    if !settings.imap_user.is_empty() && !settings.imap_password.is_empty() {
                        let db_path = db::get_db_path();
                        match rusqlite::Connection::open(&db_path) {
                            Ok(conn) => {
                                if conn.busy_timeout(Duration::from_secs(5)).is_ok() {
                                    match imap::sync_emails(
                                        &settings.imap_user,
                                        &settings.imap_password,
                                        &conn,
                                        &app_handle,
                                    ) {
                                        Ok(stats) => {
                                            eprintln!(
                                                "[BACKGROUND] Sync completado: {} nuevos, {} procesados",
                                                stats.nuevos, stats.total_procesados
                                            );
                                            // Trigger desktop notification for new payments
                                            if stats.nuevos > 0 {
                                                let _ = app_handle
                                                    .notification()
                                                    .builder()
                                                    .title("Binance Auditor")
                                                    .body(&format!(
                                                        "{} pago(s) nuevo(s) detectado(s) y guardado(s).",
                                                        stats.nuevos
                                                    ))
                                                    .show();
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("[BACKGROUND] IMAP sync error: {}", e);
                                        }
                                    }
                                } else {
                                    eprintln!("[BACKGROUND] DB busy_timeout error");
                                }
                            }
                            Err(e) => {
                                eprintln!("[BACKGROUND] DB open error: {}", e);
                            }
                        }
                    } else {
                        eprintln!("[BACKGROUND] Credenciales IMAP no configuradas, saltando sync.");
                    }

                    // Sleep 5 minutes before next iteration
                    thread::sleep(Duration::from_secs(5 * 60));
                }
            });

            Ok(())
        })
        .manage(AppState {
            db_conn: Mutex::new(db_conn),
        })
        .invoke_handler(tauri::generate_handler![
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
