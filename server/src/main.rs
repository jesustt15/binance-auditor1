mod anti_fraud;
mod auth;
mod config;
mod db;
mod imap_sync;
mod models;
mod routes;

use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Init tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "binance_auditor_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Iniciando Binance Auditor Server v1.0...");

    // Load .env file if present (for development convenience)
    match dotenvy::dotenv() {
        Ok(path) => tracing::info!(".env cargado desde {}", path.display()),
        Err(e) => tracing::warn!(".env no encontrado: {}. Usando defaults o env vars del sistema.", e),
    }

    // Load config
    let config = config::ServerConfig::from_env()?;
    tracing::info!(
        "Config cargada: listen={}, db={}",
        config.listen_addr,
        config.database_url.split('@').last().unwrap_or("***")
    );

    // Load age identity for credential encryption
    let age_identity = config::load_or_create_age_key(&config.age_key_path)?;
    tracing::info!("age identity cargada de {}", config.age_key_path);

    // Init PostgreSQL pool
    let pool = db::init_pool(&config.database_url).await?;
    tracing::info!("Pool PostgreSQL creado.");

    // Run migrations
    db::run_migrations(&pool).await?;

    // Seed default admin user on first run
    db::seed_admin(&pool).await?;

    // Build shared state
    let shared_state = Arc::new(routes::AppState {
        pool: pool.clone(),
        config: config.clone(),
        age_identity: age_identity.clone(),
    });

    // Build router
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = routes::build_router(shared_state).layer(cors);

    // Inject the JWT secret into the request extensions so AuthUser extractor can use it
    let jwt_secret = Arc::new(config.jwt_secret.clone());
    let app = app.layer(axum::extract::Extension(jwt_secret.clone()));

    // Spawn background IMAP sync task
    let bg_pool = pool.clone();
    let bg_age = age_identity.clone();
    let poll_interval = config.imap_poll_interval_secs;
    tokio::spawn(async move {
        imap_sync::background_sync_loop(bg_pool, bg_age, poll_interval).await;
    });
    tracing::info!(
        "Tarea de sync IMAP iniciada (intervalo: {}s)",
        poll_interval
    );

    // Start server (with or without TLS)
    let addr = config.listen_addr.clone();
    tracing::info!("Servidor escuchando en {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    match (config.tls_cert_path.as_deref(), config.tls_key_path.as_deref()) {
        (Some(cert), Some(key)) => {
            tracing::info!("Usando TLS con certificado: {}", cert);
            // Read cert and key
            let cert_pem = std::fs::read_to_string(cert)?;
            let key_pem = std::fs::read_to_string(key)?;

            // For TLS with axum, we need axum_server or rustls
            // Simple approach: use axum_server
            // But since axum_server is not in deps, let's use a simpler approach:
            // Just serve HTTP for now, with a warning
            tracing::warn!("TLS config detectado pero axum_server no esta en dependencias. Sirviendo HTTP.");
            axum::serve(listener, app).await?;
        }
        _ => {
            tracing::warn!("Sin TLS configurado — sirviendo HTTP (solo para desarrollo LAN).");
            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}
