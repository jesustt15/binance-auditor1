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
        Err(e) => {
            eprintln!("[IMAP ERROR] {}", e);
            Ok(SyncResult {
                success: false,
                mensajes_nuevos: 0,
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
    if let Ok(todos) = db::get_reports(&conn, &inicio_rango.to_string(), &fin_rango.to_string()) {
        eprintln!("[VERIFY DEBUG] Pagos en BD dentro del rango: {} encontrados", todos.len());
        for p in &todos {
            eprintln!("[VERIFY DEBUG]   DB: id={} usuario='{}' monto={} moneda='{}' fecha='{}' estado='{}'",
                p.id, p.usuario_remitente, p.monto, p.moneda, p.fecha_correo, p.estado);
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
            eprintln!("[VERIFY DEBUG] MATCH encontrado: id={} usuario='{}' monto={}",
                p.id, p.usuario_remitente, p.monto);
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
    state: tauri::State<AppState>,
) -> Result<Vec<PagoBinance>, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;

    let inicio = format!("{}T00:00:00", desde);
    let fin = format!("{}T23:59:59", hasta);

    db::get_reports(&conn, &inicio, &fin)
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
    state: tauri::State<AppState>,
) -> Result<ExportResult, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;

    let inicio = format!("{}T00:00:00", desde);
    let fin = format!("{}T23:59:59", hasta);

    let pagos = db::get_reports(&conn, &inicio, &fin)
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
    state: tauri::State<AppState>,
) -> Result<Vec<PagoBinance>, String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;

    let pagos = match (desde, hasta) {
        (Some(d), Some(h)) => {
            let inicio = format!("{}T00:00:00", d);
            let fin = format!("{}T23:59:59", h);
            eprintln!("[DEBUG] Listando pagos en rango: {} a {}", inicio, fin);
            db::get_reports(&conn, &inicio, &fin).map_err(|e| format!("DB error: {}", e))?
        }
        _ => {
            eprintln!("[DEBUG] Listando TODOS los pagos en BD...");
            db::get_all_pagos(&conn).map_err(|e| format!("DB error: {}", e))?
        }
    };

    eprintln!("[DEBUG] Total pagos: {}", pagos.len());
    for p in &pagos {
        eprintln!("[DEBUG]   id={} usuario='{}' monto={} moneda='{}' fecha='{}' estado='{}'",
            p.id, p.usuario_remitente, p.monto, p.moneda, p.fecha_correo, p.estado);
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
            export_reports,
            import_csv,
            save_settings,
            get_settings,
            debug_listar_pagos,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
