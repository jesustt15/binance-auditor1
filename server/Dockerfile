# =============================================================================
# Binance Pay Auditor — Server Dockerfile
# =============================================================================
# Multi-stage build: compila en Rust, ejecuta en Debian slim (~30 MB final).
#
# Los SQL de migración se incrustan en el binario con include_str!(),
# así que el runtime no necesita acceso a los archivos de migración.
# =============================================================================

# ---- Stage 1: Build ----
FROM rust:1.88-slim-bookworm AS builder

WORKDIR /app

# Instalar solo lo mínimo para compilar (ring crate necesita cc)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copiar manifests primero para cachear dependencias
COPY Cargo.toml Cargo.lock ./

# Crear un src/main.rs dummy para que cargo pueda resolver dependencias
# sin tener todo el código fuente todavía
RUN mkdir -p src && \
    echo 'fn main() { println!("dummy"); }' > src/main.rs && \
    mkdir -p migrations && \
    echo '' > migrations/001_init.sql

# Descargar y compilar dependencias (esta capa se cachea mientras no cambien
# Cargo.toml ni Cargo.lock)
RUN cargo build --release 2>&1; \
    rm -rf src migrations

# Copiar el código fuente real
COPY src/ ./src/
COPY migrations/ ./migrations/

# Truco: tocar main.rs para forzar recompilación solo de nuestro código
# (las dependencias ya están compiladas y cacheadas)
RUN touch src/main.rs

# Compilar el binario final
RUN cargo build --release

# ---- Stage 2: Runtime ----
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Crear usuario no-root para ejecutar el servidor
RUN groupadd -r auditor && useradd -r -g auditor -d /app -s /sbin/nologin auditor

WORKDIR /app

# Crear directorio para datos persistentes (age_key, jwt_secret, temp files)
RUN mkdir -p /app/data && chown -R auditor:auditor /app

# Copiar solo el binario compilado
COPY --from=builder --chown=auditor:auditor /app/target/release/binance-auditor-server .

# Cambiar a usuario no-root
USER auditor

EXPOSE 8443

# El binario:
# 1. Lee DATABASE_URL del entorno
# 2. Ejecuta migraciones (CREATE TABLE IF NOT EXISTS)
# 3. Crea admin con password aleatoria si no existe
# 4. Genera JWT_SECRET aleatorio si no está configurado
# 5. Escucha en LISTEN_ADDR
CMD ["./binance-auditor-server"]
