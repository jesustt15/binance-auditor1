mod anti_fraud;
mod auth;
mod config;
mod db;
mod imap_sync;
mod models;
mod routes;

use std::net::SocketAddr;
use std::sync::Arc;
use axum::extract::DefaultBodyLimit;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
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

    // Build CORS layer from configured allowed origins
    let allowed_origins = config.cors_allowed_origins.clone();
    let origins: Vec<http::HeaderValue> = allowed_origins
        .iter()
        .map(|o| o.parse().expect("Invalid CORS origin"))
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods(Any)
        .allow_headers(Any);

    tracing::info!("CORS allowed origins: {:?}", allowed_origins);

    // Global body limit: 20MB para prevenir DoS por uploads enormes
    let app = routes::build_router(shared_state)
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024))
        .layer(cors);

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

    // Graceful shutdown signal (SIGTERM / Ctrl+C)
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        tracing::info!("Señal de apagado recibida. Cerrando servidor gracefulmente...");
    };

    match (config.tls_cert_path.as_deref(), config.tls_key_path.as_deref()) {
        (Some(cert_path), Some(key_path)) => {
            tracing::info!("TLS habilitado — certificado: {}", cert_path);
            let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(
                cert_path.to_string(),
                key_path.to_string(),
            )
            .await?;

            let handle = axum_server::Handle::new();
            let shutdown_handle = handle.clone();

            // Spawn shutdown listener
            tokio::spawn(async move {
                tokio::signal::ctrl_c().await.ok();
                tracing::info!("Apagando servidor TLS...");
                shutdown_handle.shutdown();
            });

            let socket = tokio::net::TcpListener::bind(&addr).await?;
            axum_server::from_tcp_rustls(socket.into_std()?, tls_config)
                .handle(handle)
                .serve(app.into_make_service_with_connect_info::<SocketAddr>())
                .await?;
        }
        _ => {
            tracing::warn!("Sin TLS configurado — sirviendo HTTP (solo para desarrollo LAN).");
            tracing::info!(
                "Para habilitar TLS, configurá TLS_CERT_PATH y TLS_KEY_PATH con rutas a archivos PEM."
            );
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown_signal)
            .await?;
        }
    }

    tracing::info!("Servidor detenido.");
    Ok(())
}
