use rusqlite::{Connection, Result, params, OptionalExtension};
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use rust_xlsxwriter::{Format, Color, Workbook};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PagoBinance {
    pub id: i64,
    pub usuario_remitente: String,
    pub monto: f64,
    pub moneda: String,
    pub fecha_correo: String,
    pub estado: String,
    pub observaciones: Option<String>,
    pub verificado_en: Option<String>,
    pub creado_en: String,
}

fn get_app_dir() -> PathBuf {
    let app_data = std::env::var("APPDATA")
        .or_else(|_| std::env::var("LOCALAPPDATA"))
        .unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(app_data).join("binance-auditor");
    std::fs::create_dir_all(&dir).ok();
    dir
}

pub fn get_exports_dir() -> PathBuf {
    let dir = std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| get_app_dir())
        .join("Documents")
        .join("Binance Pay Auditor");
    std::fs::create_dir_all(&dir).ok();
    dir
}

pub fn get_db_path() -> PathBuf {
    get_app_dir().join("auditoria.db")
}

pub fn init_db() -> Result<Connection> {
    let db_path = get_db_path();
    let conn = Connection::open(db_path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS pagos_binance (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            usuario_remitente TEXT NOT NULL,
            monto REAL NOT NULL,
            moneda TEXT NOT NULL DEFAULT 'USDT',
            fecha_correo TEXT NOT NULL,
            estado TEXT NOT NULL DEFAULT 'disponible',
            observaciones TEXT,
            verificado_en TEXT,
            creado_en TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_pagos_busqueda ON pagos_binance(usuario_remitente, monto, estado)",
        [],
    )?;

    Ok(conn)
}

pub fn insert_pago(conn: &Connection, usuario: &str, monto: f64, moneda: &str, fecha: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO pagos_binance (usuario_remitente, monto, moneda, fecha_correo, estado)
         VALUES (?1, ?2, ?3, ?4, 'disponible')",
        params![usuario, monto, moneda, fecha],
    )?;
    Ok(())
}

pub fn pago_exists(conn: &Connection, usuario: &str, monto: f64, fecha: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pagos_binance
         WHERE usuario_remitente = ?1 AND monto = ?2 AND fecha_correo = ?3",
        params![usuario, monto, fecha],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn find_pago_for_verification(
    conn: &Connection,
    usuario: &str,
    monto: f64,
    fecha_inicio: &str,
    fecha_fin: &str,
) -> Result<Option<PagoBinance>> {
    let mut stmt = conn.prepare(
        "SELECT id, usuario_remitente, monto, moneda, fecha_correo, estado,
                observaciones, verificado_en, creado_en
         FROM pagos_binance
         WHERE usuario_remitente = ?1
           AND monto = ?2
           AND estado = 'disponible'
           AND fecha_correo >= ?3
           AND fecha_correo <= ?4
         LIMIT 1",
    )?;

    let pago = stmt.query_row(
        params![usuario, monto, fecha_inicio, fecha_fin],
        |row| {
            Ok(PagoBinance {
                id: row.get(0)?,
                usuario_remitente: row.get(1)?,
                monto: row.get(2)?,
                moneda: row.get(3)?,
                fecha_correo: row.get(4)?,
                estado: row.get(5)?,
                observaciones: row.get(6)?,
                verificado_en: row.get(7)?,
                creado_en: row.get(8)?,
            })
        },
    ).optional()?;

    Ok(pago)
}

pub fn mark_as_verificado(conn: &Connection, id: i64, observaciones: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE pagos_binance
         SET estado = 'verificado', verificado_en = ?1, observaciones = ?2
         WHERE id = ?3",
        params![now, observaciones, id],
    )?;
    Ok(())
}

pub fn get_all_pagos(conn: &Connection) -> Result<Vec<PagoBinance>> {
    let mut stmt = conn.prepare(
        "SELECT id, usuario_remitente, monto, moneda, fecha_correo, estado,
                observaciones, verificado_en, creado_en
         FROM pagos_binance
         ORDER BY fecha_correo DESC",
    )?;

    let pagos = stmt.query_map([], |row| {
        Ok(PagoBinance {
            id: row.get(0)?,
            usuario_remitente: row.get(1)?,
            monto: row.get(2)?,
            moneda: row.get(3)?,
            fecha_correo: row.get(4)?,
            estado: row.get(5)?,
            observaciones: row.get(6)?,
            verificado_en: row.get(7)?,
            creado_en: row.get(8)?,
        })
    })?.collect::<Result<Vec<_>>>()?;

    Ok(pagos)
}

pub fn get_reports(conn: &Connection, desde: &str, hasta: &str) -> Result<Vec<PagoBinance>> {
    let mut stmt = conn.prepare(
        "SELECT id, usuario_remitente, monto, moneda, fecha_correo, estado,
                observaciones, verificado_en, creado_en
         FROM pagos_binance
         WHERE fecha_correo >= ?1 AND fecha_correo <= ?2
         ORDER BY          fecha_correo DESC",
    )?;

    let pagos = stmt.query_map(params![desde, hasta], |row| {
        Ok(PagoBinance {
            id: row.get(0)?,
            usuario_remitente: row.get(1)?,
            monto: row.get(2)?,
            moneda: row.get(3)?,
            fecha_correo: row.get(4)?,
            estado: row.get(5)?,
            observaciones: row.get(6)?,
            verificado_en: row.get(7)?,
            creado_en: row.get(8)?,
        })
    })?.collect::<Result<Vec<_>>>()?;

    Ok(pagos)
}

pub fn export_to_excel(pagos: &[PagoBinance], file_path: &str) -> std::result::Result<String, String> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    // Headers — solo 3 columnas: Remitente, Monto, Fecha
    let headers = ["Usuario Remitente", "Monto", "Fecha Correo"];
    let header_format = Format::new().set_bold().set_background_color(Color::RGB(0x1E293B)).set_font_color(Color::RGB(0xFBBF24));

    for (col, header) in headers.iter().enumerate() {
        worksheet.write_with_format(0, col as u16, *header, &header_format)
            .map_err(|e| format!("Excel write error: {}", e))?;
    }

    // Data rows
    for (row_idx, pago) in pagos.iter().enumerate() {
        let row = (row_idx + 1) as u32;
        worksheet.write(row, 0, &pago.usuario_remitente).map_err(|e| format!("Excel write error: {}", e))?;
        worksheet.write(row, 1, pago.monto).map_err(|e| format!("Excel write error: {}", e))?;
        worksheet.write(row, 2, &pago.fecha_correo).map_err(|e| format!("Excel write error: {}", e))?;
    }

    // Column widths
    worksheet.set_column_width(0, 25.0).map_err(|e| format!("Excel column width error: {}", e))?;
    worksheet.set_column_width(1, 15.0).map_err(|e| format!("Excel column width error: {}", e))?;
    worksheet.set_column_width(2, 30.0).map_err(|e| format!("Excel column width error: {}", e))?;

    workbook.save(file_path).map_err(|e| format!("Excel save error: {}", e))?;

    Ok(file_path.to_string())
}
