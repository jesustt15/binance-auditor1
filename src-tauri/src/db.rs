use rusqlite::{Connection, Result, params, OptionalExtension, types::Value};
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use rust_xlsxwriter::{Format, Color, Workbook};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PagoBinance {
    pub id: i64,
    pub tipo: String,
    pub usuario_remitente: Option<String>,
    pub monto: f64,
    pub moneda: String,
    pub fecha_correo: String,
    pub hora_correo: Option<String>,
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

    migrate_schema_v2(&conn)?;
    migrate_schema_v3(&conn)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS pagos_binance (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tipo TEXT NOT NULL DEFAULT 'pago',
            usuario_remitente TEXT,
            monto REAL NOT NULL,
            moneda TEXT NOT NULL DEFAULT 'USDT',
            fecha_correo TEXT NOT NULL,
            hora_correo TEXT,
            estado TEXT NOT NULL DEFAULT 'disponible',
            observaciones TEXT,
            verificado_en TEXT,
            creado_en TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_pagos_busqueda ON pagos_binance(tipo, usuario_remitente, monto, estado)",
        [],
    )?;

    let _ = backfill_hora_correo(&conn);

    Ok(conn)
}

fn extract_hora_from_stored_date(fecha: &str) -> Option<String> {
    // 1. Try RFC3339 (e.g., "2026-07-22T14:15:33+00:00")
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(fecha) {
        let utc_dt = dt.with_timezone(&chrono::Utc);
        let caracas_time = utc_dt.naive_utc() - chrono::Duration::hours(4);
        return Some(caracas_time.format("%H:%M:%S").to_string());
    }
    // 2. Try "%Y-%m-%d %H:%M:%S"
    let cleaned = fecha.replace("(UTC)", "").trim().to_string();
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&cleaned, "%Y-%m-%d %H:%M:%S") {
        let caracas_time = naive - chrono::Duration::hours(4);
        return Some(caracas_time.format("%H:%M:%S").to_string());
    }
    // 3. Try "%Y-%m-%dT%H:%M:%S"
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&cleaned, "%Y-%m-%dT%H:%M:%S") {
        let caracas_time = naive - chrono::Duration::hours(4);
        return Some(caracas_time.format("%H:%M:%S").to_string());
    }
    None
}

fn backfill_hora_correo(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("SELECT id, fecha_correo FROM pagos_binance WHERE hora_correo IS NULL OR hora_correo = ''")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?.collect::<Result<Vec<_>>>()?;

    for (id, fecha_correo) in rows {
        if let Some(hora) = extract_hora_from_stored_date(&fecha_correo) {
            conn.execute(
                "UPDATE pagos_binance SET hora_correo = ?1 WHERE id = ?2",
                params![hora, id],
            )?;
        }
    }
    Ok(())
}

fn migrate_schema_v2(conn: &Connection) -> Result<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version >= 2 {
        return Ok(());
    }

    // Check if old table exists
    let table_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='pagos_binance'",
        [],
        |row| row.get::<_, i64>(0),
    ).map(|c| c > 0)?;

    if !table_exists {
        // Fresh install — mark version so CREATE TABLE uses v2 schema
        conn.pragma_update(None, "user_version", 2)?;
        return Ok(());
    }

    // Migration: recreate table with updated schema
    conn.execute_batch("BEGIN TRANSACTION")?;

    conn.execute_batch(
        "CREATE TABLE pagos_binance_new (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tipo TEXT NOT NULL DEFAULT 'pago',
            usuario_remitente TEXT,
            monto REAL NOT NULL,
            moneda TEXT NOT NULL DEFAULT 'USDT',
            fecha_correo TEXT NOT NULL,
            estado TEXT NOT NULL DEFAULT 'disponible',
            observaciones TEXT,
            verificado_en TEXT,
            creado_en TEXT NOT NULL DEFAULT (datetime('now'))
        )"
    )?;

    conn.execute_batch(
        "INSERT INTO pagos_binance_new (usuario_remitente, monto, moneda, fecha_correo, estado, observaciones, verificado_en, creado_en)
         SELECT usuario_remitente, monto, moneda, fecha_correo, estado, observaciones, verificado_en, creado_en
         FROM pagos_binance"
    )?;

    conn.execute_batch("DROP TABLE pagos_binance")?;
    conn.execute_batch("ALTER TABLE pagos_binance_new RENAME TO pagos_binance")?;

    conn.execute_batch(
        "CREATE INDEX idx_pagos_busqueda ON pagos_binance(tipo, usuario_remitente, monto, estado)"
    )?;

    conn.pragma_update(None, "user_version", 2)?;

    conn.execute_batch("COMMIT")?;

    Ok(())
}

fn migrate_schema_v3(conn: &Connection) -> Result<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version >= 3 {
        return Ok(());
    }

    // Check if hora_correo column already exists (belt and suspenders)
    let has_column: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('pagos_binance') WHERE name='hora_correo'",
        [],
        |row| row.get::<_, i64>(0),
    ).map(|c| c > 0)?;

    if !has_column {
        conn.execute_batch("ALTER TABLE pagos_binance ADD COLUMN hora_correo TEXT")?;
    }

    conn.pragma_update(None, "user_version", 3)?;

    Ok(())
}

pub fn insert_pago(conn: &Connection, usuario: Option<&str>, monto: f64, moneda: &str, fecha: &str, hora: Option<&str>, tipo: &str) -> Result<()> {
    let estado = if tipo == "deposito" { "por_revisar" } else { "disponible" };
    conn.execute(
        "INSERT INTO pagos_binance (tipo, usuario_remitente, monto, moneda, fecha_correo, hora_correo, estado)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![tipo, usuario, monto, moneda, fecha, hora, estado],
    )?;
    Ok(())
}

pub fn pago_exists(conn: &Connection, usuario: Option<&str>, monto: f64, fecha: &str) -> Result<bool> {
    let count: i64 = match usuario {
        Some(u) => conn.query_row(
            "SELECT COUNT(*) FROM pagos_binance
             WHERE usuario_remitente = ?1 AND monto = ?2 AND fecha_correo = ?3",
            params![u, monto, fecha],
            |row| row.get(0),
        )?,
        None => conn.query_row(
            "SELECT COUNT(*) FROM pagos_binance
             WHERE usuario_remitente IS NULL AND monto = ?1 AND fecha_correo = ?2",
            params![monto, fecha],
            |row| row.get(0),
        )?,
    };
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
        "SELECT id, tipo, usuario_remitente, monto, moneda, fecha_correo, hora_correo, estado,
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
                tipo: row.get(1)?,
                usuario_remitente: row.get(2)?,
                monto: row.get(3)?,
                moneda: row.get(4)?,
                fecha_correo: row.get(5)?,
                hora_correo: row.get(6)?,
                estado: row.get(7)?,
                observaciones: row.get(8)?,
                verificado_en: row.get(9)?,
                creado_en: row.get(10)?,
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
        "SELECT id, tipo, usuario_remitente, monto, moneda, fecha_correo, hora_correo, estado,
                observaciones, verificado_en, creado_en
         FROM pagos_binance
         ORDER BY fecha_correo DESC",
    )?;

    let pagos = stmt.query_map([], |row| {
        Ok(PagoBinance {
            id: row.get(0)?,
            tipo: row.get(1)?,
            usuario_remitente: row.get(2)?,
            monto: row.get(3)?,
            moneda: row.get(4)?,
            fecha_correo: row.get(5)?,
            hora_correo: row.get(6)?,
            estado: row.get(7)?,
            observaciones: row.get(8)?,
            verificado_en: row.get(9)?,
            creado_en: row.get(10)?,
        })
    })?.collect::<Result<Vec<_>>>()?;

    Ok(pagos)
}

pub fn get_reports(
    conn: &Connection,
    desde: &str,
    hasta: &str,
    monto_exacto: Option<f64>,
    monto_min: Option<f64>,
    monto_max: Option<f64>,
) -> Result<Vec<PagoBinance>> {
    let mut sql = String::from(
        "SELECT id, tipo, usuario_remitente, monto, moneda, fecha_correo, hora_correo, estado,
                observaciones, verificado_en, creado_en
         FROM pagos_binance
         WHERE fecha_correo >= ?1 AND fecha_correo <= ?2",
    );

    let mut param_values: Vec<Value> = vec![
        Value::from(desde.to_string()),
        Value::from(hasta.to_string()),
    ];

    if let Some(exact) = monto_exacto {
        sql.push_str(&format!(" AND monto = ?{}", param_values.len() + 1));
        param_values.push(Value::from(exact));
    } else {
        if let Some(min) = monto_min {
            sql.push_str(&format!(" AND monto >= ?{}", param_values.len() + 1));
            param_values.push(Value::from(min));
        }
        if let Some(max) = monto_max {
            sql.push_str(&format!(" AND monto <= ?{}", param_values.len() + 1));
            param_values.push(Value::from(max));
        }
    }

    sql.push_str(" ORDER BY fecha_correo DESC");

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();

    let pagos = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(PagoBinance {
            id: row.get(0)?,
            tipo: row.get(1)?,
            usuario_remitente: row.get(2)?,
            monto: row.get(3)?,
            moneda: row.get(4)?,
            fecha_correo: row.get(5)?,
            hora_correo: row.get(6)?,
            estado: row.get(7)?,
            observaciones: row.get(8)?,
            verificado_en: row.get(9)?,
            creado_en: row.get(10)?,
        })
    })?.collect::<Result<Vec<_>>>()?;

    Ok(pagos)
}

pub fn get_reports_by_sender(
    conn: &Connection,
    desde: &str,
    hasta: &str,
    remitente: &str,
    monto_exacto: Option<f64>,
    monto_min: Option<f64>,
    monto_max: Option<f64>,
) -> Result<Vec<PagoBinance>> {
    let pattern = format!("%{}%", remitente);

    let mut sql = String::from(
        "SELECT id, tipo, usuario_remitente, monto, moneda, fecha_correo, hora_correo, estado,
                observaciones, verificado_en, creado_en
         FROM pagos_binance
         WHERE fecha_correo >= ?1 AND fecha_correo <= ?2
           AND usuario_remitente LIKE ?3",
    );

    let mut param_values: Vec<Value> = vec![
        Value::from(desde.to_string()),
        Value::from(hasta.to_string()),
        Value::from(pattern),
    ];

    if let Some(exact) = monto_exacto {
        sql.push_str(&format!(" AND monto = ?{}", param_values.len() + 1));
        param_values.push(Value::from(exact));
    } else {
        if let Some(min) = monto_min {
            sql.push_str(&format!(" AND monto >= ?{}", param_values.len() + 1));
            param_values.push(Value::from(min));
        }
        if let Some(max) = monto_max {
            sql.push_str(&format!(" AND monto <= ?{}", param_values.len() + 1));
            param_values.push(Value::from(max));
        }
    }

    sql.push_str(" ORDER BY fecha_correo DESC");

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();

    let pagos = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(PagoBinance {
            id: row.get(0)?,
            tipo: row.get(1)?,
            usuario_remitente: row.get(2)?,
            monto: row.get(3)?,
            moneda: row.get(4)?,
            fecha_correo: row.get(5)?,
            hora_correo: row.get(6)?,
            estado: row.get(7)?,
            observaciones: row.get(8)?,
            verificado_en: row.get(9)?,
            creado_en: row.get(10)?,
        })
    })?.collect::<Result<Vec<_>>>()?;

    Ok(pagos)
}

/// Formats an RFC3339 date string to DD/MM/YYYY for Excel display.
fn format_fecha_ddmmyyyy(rfc3339: &str) -> String {
    // Try parsing as RFC3339 first
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(rfc3339) {
        return dt.format("%d/%m/%Y").to_string();
    }
    // Fallback: if already DD/MM/YYYY or unknown format, return as-is
    rfc3339.to_string()
}

pub fn export_to_excel(pagos: &[PagoBinance], file_path: &str) -> std::result::Result<String, String> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    // Headers — Tipo, Remitente, Monto, Moneda, Fecha, Hora, Estado
    let headers = ["Tipo", "Usuario Remitente", "Monto", "Moneda", "Fecha Correo", "Hora", "Estado"];
    let header_format = Format::new().set_bold().set_background_color(Color::RGB(0x1E293B)).set_font_color(Color::RGB(0xFBBF24));

    for (col, header) in headers.iter().enumerate() {
        worksheet.write_with_format(0, col as u16, *header, &header_format)
            .map_err(|e| format!("Excel write error: {}", e))?;
    }

    // Status color coding
    let disponible_fmt = Format::new().set_background_color(Color::RGB(0xFEF3C7));
    let verificado_fmt = Format::new().set_background_color(Color::RGB(0xD1FAE5));
    let por_revisar_fmt = Format::new().set_background_color(Color::RGB(0xFDE68A));
    let rechazado_fmt = Format::new().set_background_color(Color::RGB(0xFEE2E2));

    // Data rows
    for (row_idx, pago) in pagos.iter().enumerate() {
        let row = (row_idx + 1) as u32;
        worksheet.write(row, 0, &pago.tipo).map_err(|e| format!("Excel write error: {}", e))?;
        worksheet.write(row, 1, pago.usuario_remitente.as_deref().unwrap_or("—")).map_err(|e| format!("Excel write error: {}", e))?;
        worksheet.write(row, 2, pago.monto).map_err(|e| format!("Excel write error: {}", e))?;
        worksheet.write(row, 3, &pago.moneda).map_err(|e| format!("Excel write error: {}", e))?;
        worksheet.write(row, 4, &format_fecha_ddmmyyyy(&pago.fecha_correo)).map_err(|e| format!("Excel write error: {}", e))?;
        worksheet.write(row, 5, pago.hora_correo.as_deref().unwrap_or("—")).map_err(|e| format!("Excel write error: {}", e))?;

        // Color-coded estado
        let estado_fmt = match pago.estado.as_str() {
            "verificado" => &verificado_fmt,
            "rechazado" => &rechazado_fmt,
            "por_revisar" => &por_revisar_fmt,
            _ => &disponible_fmt,
        };
        worksheet.write_with_format(row, 6, &pago.estado, estado_fmt)
            .map_err(|e| format!("Excel write error: {}", e))?;
    }

    // Column widths
    worksheet.set_column_width(0, 12.0).map_err(|e| format!("Excel column width error: {}", e))?;
    worksheet.set_column_width(1, 25.0).map_err(|e| format!("Excel column width error: {}", e))?;
    worksheet.set_column_width(2, 15.0).map_err(|e| format!("Excel column width error: {}", e))?;
    worksheet.set_column_width(3, 10.0).map_err(|e| format!("Excel column width error: {}", e))?;
    worksheet.set_column_width(4, 30.0).map_err(|e| format!("Excel column width error: {}", e))?;
    worksheet.set_column_width(5, 10.0).map_err(|e| format!("Excel column width error: {}", e))?;
    worksheet.set_column_width(6, 15.0).map_err(|e| format!("Excel column width error: {}", e))?;

    workbook.save(file_path).map_err(|e| format!("Excel save error: {}", e))?;

    Ok(file_path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE pagos_binance (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tipo TEXT NOT NULL DEFAULT 'pago',
                usuario_remitente TEXT,
                monto REAL NOT NULL,
                moneda TEXT NOT NULL DEFAULT 'USDT',
                fecha_correo TEXT NOT NULL,
                hora_correo TEXT,
                estado TEXT NOT NULL DEFAULT 'disponible',
                observaciones TEXT,
                verificado_en TEXT,
                creado_en TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX idx_pagos_busqueda ON pagos_binance(tipo, usuario_remitente, monto, estado);"
        ).unwrap();
        conn
    }

    fn seed_payment(conn: &Connection, usuario: Option<&str>, monto: f64, fecha: &str, tipo: &str) {
        let estado = if tipo == "deposito" { "por_revisar" } else { "disponible" };
        conn.execute(
            "INSERT INTO pagos_binance (tipo, usuario_remitente, monto, moneda, fecha_correo, estado)
             VALUES (?1, ?2, ?3, 'USDT', ?4, ?5)",
            params![tipo, usuario, monto, fecha, estado],
        ).unwrap();
    }

    // Task 4.4: Conditional pago_exists
    #[test]
    fn pago_exists_with_usuario_detects_duplicate() {
        let conn = setup_test_db();
        seed_payment(&conn, Some("alice"), 50.0, "2026-07-15", "pago");
        assert!(pago_exists(&conn, Some("alice"), 50.0, "2026-07-15").unwrap());
        assert!(!pago_exists(&conn, Some("alice"), 100.0, "2026-07-15").unwrap());
    }

    #[test]
    fn pago_exists_with_null_usuario_detects_deposit_duplicate() {
        let conn = setup_test_db();
        seed_payment(&conn, None, 100.0, "2026-07-15", "deposito");
        assert!(pago_exists(&conn, None, 100.0, "2026-07-15").unwrap());
        assert!(!pago_exists(&conn, None, 200.0, "2026-07-15").unwrap());
    }

    #[test]
    fn pago_exists_null_usuario_does_not_match_some_usuario() {
        let conn = setup_test_db();
        seed_payment(&conn, None, 100.0, "2026-07-15", "deposito");
        // A payment lookup with a non-null usuario should NOT match the deposit
        assert!(!pago_exists(&conn, Some("alice"), 100.0, "2026-07-15").unwrap());
    }

    // Task 4.5: Amount filter SQL
    #[test]
    fn amount_filter_exact_match() {
        let conn = setup_test_db();
        seed_payment(&conn, Some("a"), 50.0, "2026-07-10T12:00:00+00:00", "pago");
        seed_payment(&conn, Some("b"), 100.0, "2026-07-10T12:00:00+00:00", "pago");
        seed_payment(&conn, Some("c"), 150.0, "2026-07-10T12:00:00+00:00", "pago");

        let results = get_reports(&conn, "2026-01-01T00:00:00", "2026-12-31T23:59:59", Some(100.0), None, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].monto, 100.0);
    }

    #[test]
    fn amount_filter_range_min_max() {
        let conn = setup_test_db();
        seed_payment(&conn, Some("a"), 30.0, "2026-07-10T12:00:00+00:00", "pago");
        seed_payment(&conn, Some("b"), 50.0, "2026-07-10T12:00:00+00:00", "pago");
        seed_payment(&conn, Some("c"), 100.0, "2026-07-10T12:00:00+00:00", "pago");
        seed_payment(&conn, Some("d"), 200.0, "2026-07-10T12:00:00+00:00", "pago");

        let results = get_reports(&conn, "2026-01-01T00:00:00", "2026-12-31T23:59:59", None, Some(40.0), Some(150.0)).unwrap();
        assert_eq!(results.len(), 2);
        let amounts: Vec<f64> = results.iter().map(|r| r.monto).collect();
        assert!(amounts.contains(&50.0));
        assert!(amounts.contains(&100.0));
    }

    #[test]
    fn amount_filter_min_only() {
        let conn = setup_test_db();
        seed_payment(&conn, Some("a"), 30.0, "2026-07-10T12:00:00+00:00", "pago");
        seed_payment(&conn, Some("b"), 50.0, "2026-07-10T12:00:00+00:00", "pago");
        seed_payment(&conn, Some("c"), 100.0, "2026-07-10T12:00:00+00:00", "pago");

        let results = get_reports(&conn, "2026-01-01T00:00:00", "2026-12-31T23:59:59", None, Some(50.0), None).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn amount_filter_max_only() {
        let conn = setup_test_db();
        seed_payment(&conn, Some("a"), 30.0, "2026-07-10T12:00:00+00:00", "pago");
        seed_payment(&conn, Some("b"), 50.0, "2026-07-10T12:00:00+00:00", "pago");
        seed_payment(&conn, Some("c"), 100.0, "2026-07-10T12:00:00+00:00", "pago");

        let results = get_reports(&conn, "2026-01-01T00:00:00", "2026-12-31T23:59:59", None, None, Some(50.0)).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn amount_filter_no_filter_returns_all_in_range() {
        let conn = setup_test_db();
        seed_payment(&conn, Some("a"), 50.0, "2026-07-10T12:00:00+00:00", "pago");
        seed_payment(&conn, Some("b"), 100.0, "2026-07-10T12:00:00+00:00", "pago");

        let results = get_reports(&conn, "2026-01-01T00:00:00", "2026-12-31T23:59:59", None, None, None).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn insert_pago_sets_estado_por_revisar_for_deposito() {
        let conn = setup_test_db();
        insert_pago(&conn, None, 100.0, "USDT", "2026-07-15", None, "deposito").unwrap();

        let mut stmt = conn.prepare("SELECT tipo, usuario_remitente, estado FROM pagos_binance").unwrap();
        let row = stmt.query_row([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?))
        }).unwrap();
        assert_eq!(row.0, "deposito");
        assert_eq!(row.1, None);
        assert_eq!(row.2, "por_revisar");
    }

    #[test]
    fn insert_pago_sets_estado_disponible_for_pago() {
        let conn = setup_test_db();
        insert_pago(&conn, Some("alice"), 50.0, "USDT", "2026-07-15", None, "pago").unwrap();

        let mut stmt = conn.prepare("SELECT tipo, usuario_remitente, estado FROM pagos_binance").unwrap();
        let row = stmt.query_row([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?))
        }).unwrap();
        assert_eq!(row.0, "pago");
        assert_eq!(row.1.as_deref(), Some("alice"));
        assert_eq!(row.2, "disponible");
    }

    // Task 4.6: Integration test — v1→v2 migration data preservation
    #[test]
    fn migrate_v1_to_v2_preserves_data() {
        let conn = Connection::open_in_memory().unwrap();
        // Simulate v1 schema
        conn.execute_batch(
            "CREATE TABLE pagos_binance (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                usuario_remitente TEXT NOT NULL,
                monto REAL NOT NULL,
                moneda TEXT NOT NULL DEFAULT 'USDT',
                fecha_correo TEXT NOT NULL,
                estado TEXT NOT NULL DEFAULT 'disponible',
                observaciones TEXT,
                verificado_en TEXT,
                creado_en TEXT NOT NULL DEFAULT (datetime('now'))
            );
            INSERT INTO pagos_binance (usuario_remitente, monto, moneda, fecha_correo, estado)
             VALUES ('alice', 50.0, 'USDT', '2026-07-10', 'disponible');
            INSERT INTO pagos_binance (usuario_remitente, monto, moneda, fecha_correo, estado)
             VALUES ('bob', 100.0, 'USDC', '2026-07-11', 'verificado');
            PRAGMA user_version = 1;"
        ).unwrap();

        // Run migration
        migrate_schema_v2(&conn).unwrap();

        // Verify data preserved and tipo defaults to 'pago'
        let mut stmt = conn.prepare("SELECT tipo, usuario_remitente, monto, moneda, fecha_correo, estado FROM pagos_binance ORDER BY id").unwrap();
        let rows: Vec<(String, Option<String>, f64, String, String, String)> = stmt.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        }).unwrap().map(|r| r.unwrap()).collect();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "pago"); // tipo defaults to pago
        assert_eq!(rows[0].1.as_deref(), Some("alice"));
        assert_eq!(rows[0].2, 50.0);
        assert_eq!(rows[1].0, "pago");
        assert_eq!(rows[1].1.as_deref(), Some("bob"));
        assert_eq!(rows[1].2, 100.0);

        // Verify user_version
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
        assert_eq!(version, 2);
    }

    #[test]
    fn test_backfill_hora_correo() {
        let conn = setup_test_db();
        // Insert a record with NULL hora_correo and a fecha_correo
        conn.execute(
            "INSERT INTO pagos_binance (tipo, usuario_remitente, monto, moneda, fecha_correo, hora_correo, estado)
             VALUES ('pago', 'alice', 50.0, 'USDT', '2026-07-22T14:15:33+00:00', NULL, 'disponible')",
            [],
        ).unwrap();

        // Run the backfill
        backfill_hora_correo(&conn).unwrap();

        // Verify the hour was populated (Caracas time UTC-4: 14:15:33 -> 10:15:33)
        let hora: String = conn.query_row(
            "SELECT hora_correo FROM pagos_binance WHERE usuario_remitente = 'alice'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(hora, "10:15:33");
    }
}
