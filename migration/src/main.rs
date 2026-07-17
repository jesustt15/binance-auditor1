use anyhow::{Context, Result};
use rusqlite::Connection as SqliteConn;
use sqlx::postgres::PgPoolOptions;
use std::path::PathBuf;
use uuid::Uuid;

/// Estructura que representa un pago en SQLite (schema existente)
#[derive(Debug)]
struct PagoSqlite {
    id: i64,
    tipo: String,
    usuario_remitente: Option<String>,
    monto: f64,
    moneda: String,
    fecha_correo: String,
    estado: String,
    observaciones: Option<String>,
    verificado_en: Option<String>,
    creado_en: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 5 || args[1] != "--from" || args[3] != "--to" {
        eprintln!("Uso: migrate --from sqlite --path <ruta.db> --to postgres --conn <connection_string>");
        eprintln!();
        eprintln!("Ejemplo:");     
        eprintln!("  migrate --from sqlite --path \"C:\\\\Users\\\\...\\\\auditoria.db\" --to postgres --conn \"postgres://auditor:pass@localhost/auditor_db\"");
        std::process::exit(1);
    }

    let mut db_path = String::new();
    let mut pg_conn = String::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--path" if i + 1 < args.len() => {
                db_path = args[i + 1].clone();
                i += 2;
            }
            "--conn" if i + 1 < args.len() => {
                pg_conn = args[i + 1].clone();
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }

    if db_path.is_empty() {
        anyhow::bail!("Se requiere --path <ruta al .db SQLite>");
    }
    if pg_conn.is_empty() {
        anyhow::bail!("Se requiere --conn <connection string PostgreSQL>");
    }

    println!("============================================");
    println!(" Migracion SQLite -> PostgreSQL");
    println!("============================================");
    println!(" Origen:  {}", db_path);
    println!(" Destino: {}", pg_conn.split('@').last().unwrap_or("***"));
    println!();

    // 1. Leer SQLite
    let sqlite = SqliteConn::open(&db_path)
        .context("No se pudo abrir la base SQLite")?;

    let pagos = read_sqlite_pagos(&sqlite)?;
    println!("Leidos {} registros de SQLite.", pagos.len());

    if pagos.is_empty() {
        println!("No hay datos para migrar.");
        return Ok(());
    }

    // 2. Conectar a PostgreSQL
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&pg_conn)
        .await
        .context("No se pudo conectar a PostgreSQL")?;

    println!("Conexion a PostgreSQL establecida.");

    // 3. Insertar en PostgreSQL
    let mut migrados = 0usize;
    let mut duplicados = 0usize;
    let mut errores = 0usize;

    for pago in &pagos {
        // Generar UUID para el nuevo registro
        let new_id = Uuid::new_v4();

        let result = sqlx::query(
            "INSERT INTO payments (id, tipo, usuario_remitente, monto, moneda, fecha_correo, estado,
                                   observaciones, verificado_en, created_at)
             VALUES ($1, $2, $3, $4::numeric, $5, $6::timestamptz, $7::text, $8, $9::timestamptz, $10::timestamptz)",
        )
        .bind(new_id)
        .bind(&pago.tipo)
        .bind(&pago.usuario_remitente)
        .bind(format!("{:.2}", pago.monto))
        .bind(&pago.moneda)
        .bind(&pago.fecha_correo)
        .bind(&pago.estado)
        .bind(&pago.observaciones)
        .bind(&pago.verificado_en)
        .bind(&pago.creado_en)
        .execute(&pool)
        .await;

        match result {
            Ok(_) => {
                migrados += 1;
                if migrados % 100 == 0 {
                    println!("  {} registros migrados...", migrados);
                }
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("unique") || err_str.contains("duplicate") {
                    duplicados += 1;
                } else {
                    errores += 1;
                    eprintln!("  ERROR en registro id={}: {}", pago.id, err_str);
                }
            }
        }
    }

    // 4. Reporte final
    println!();
    println!("============================================");
    println!(" MIGRACION COMPLETADA");
    println!("============================================");
    println!(" Total SQLite:    {}", pagos.len());
    println!(" Migrados:        {}", migrados);
    println!(" Duplicados:      {}", duplicados);
    println!(" Errores:         {}", errores);
    println!();
    println!("NOTA: Los IDs originales (INTEGER) se perdieron en la migracion.");
    println!("      Se generaron nuevos UUIDs para cada registro.");
    println!("      Los datos originales en SQLite NO fueron modificados.");

    Ok(())
}

fn read_sqlite_pagos(conn: &SqliteConn) -> Result<Vec<PagoSqlite>> {
    let mut stmt = conn.prepare(
        "SELECT id, tipo, usuario_remitente, monto, moneda, fecha_correo, estado,
                observaciones, verificado_en, creado_en
         FROM pagos_binance
         ORDER BY id",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(PagoSqlite {
            id: row.get(0)?,
            tipo: row.get(1)?,
            usuario_remitente: row.get(2)?,
            monto: row.get(3)?,
            moneda: row.get(4)?,
            fecha_correo: row.get(5)?,
            estado: row.get(6)?,
            observaciones: row.get(7)?,
            verificado_en: row.get(8)?,
            creado_en: row.get(9)?,
        })
    })?;

    let mut pagos = Vec::new();
    for row in rows {
        pagos.push(row?);
    }

    Ok(pagos)
}
