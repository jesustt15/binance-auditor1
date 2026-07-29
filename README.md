# Binance Pay Auditor

Sistema de conciliación de pagos con detección de fraude para comercios que reciben pagos vía Binance Pay. Sincroniza correos de notificación de Binance vía IMAP (Gmail), aplica verificación anti-fraude (DKIM/SPF/DMARC), y permite a operadores conciliar pagos contra sus registros internos.

## Arquitectura

```
┌──────────────┐     ┌──────────────────┐     ┌──────────────┐
│  Tauri App   │────▶│  Rust API Server │────▶│  PostgreSQL   │
│  (React/TS)  │     │  (Axum 0.8)      │     │  16           │
│              │     │                   │     │               │
│  Modo local: │     │  /api/auth/*      │     │  users        │
│  SQLite      │     │  /api/payments/*  │     │  payments     │
│              │     │  /api/import/*    │     │  audit_log    │
│              │     │  /api/sync/*      │     │  quarantine   │
│              │     │  /health          │     │  imap_config  │
└──────────────┘     └────────┬─────────┘     └──────────────┘
                              │
                              ▼
                       ┌──────────────┐
                       │  Gmail IMAP  │
                       │  (TLS 993)   │
                       └──────────────┘
```

- **Frontend**: React 19 + TypeScript + Vite + TailwindCSS 4, empaquetado como app de escritorio con Tauri 2
- **Backend**: Rust (Axum), autenticación Argon2 + JWT, rate limiting por IP
- **Anti-fraude**: Verificación DKIM, SPF, DMARC de cada correo antes de persistir. Correos sospechosos van a cuarentena
- **Base de datos**: PostgreSQL 16 con migraciones automáticas

## Requisitos

- [Docker](https://docs.docker.com/get-docker/) y Docker Compose
- [Node.js](https://nodejs.org/) 20+ y [pnpm](https://pnpm.io/) (solo para desarrollo frontend)
- [Rust](https://rustup.rs/) (solo para desarrollo del servidor o Tauri)

## Despliegue rápido (Docker)

```bash
# 1. Configurar variables de entorno
cp server/.env.example .env
# Editar .env con contraseñas seguras (DB_PASSWORD, etc.)

# 2. Levantar servicios
docker compose up -d

# 3. Obtener la contraseña del admin (se genera aleatoriamente)
docker compose logs server | grep "ADMIN-PASSWORD"
# Ejemplo: [ADMIN-PASSWORD] Xk9#mP2$vL7nQ4!aB8cF

# 4. Abrir http://<ip-del-servidor>:8443
# Login: admin / <password del paso 3>
# El sistema fuerza cambio de contraseña en el primer login
```

## Variables de entorno

| Variable | Default | Descripción |
|----------|---------|-------------|
| `DB_USER` | `auditor` | Usuario PostgreSQL |
| `DB_PASSWORD` | — | Contraseña PostgreSQL (obligatorio) |
| `DB_NAME` | `auditor_db` | Nombre de la base de datos |
| `JWT_SECRET` | auto-generado | Secreto JWT. Si no se configura, se genera uno aleatorio y se persiste |
| `LISTEN_PORT` | `8443` | Puerto de la API |
| `CORS_ALLOWED_ORIGINS` | `http://localhost:1420,tauri://localhost` | Orígenes CORS permitidos (separados por coma) |
| `IMAP_POLL_INTERVAL_SECS` | `300` | Intervalo de sincronización IMAP |
| `RUST_LOG` | `binance_auditor_server=info` | Nivel de logs |
| `RUST_LOG_FORMAT` | `pretty` | `pretty` (texto coloreado) o `json` (para Loki/Elastic) |
| `TLS_CERT_PATH` | — | Ruta al certificado PEM para HTTPS |
| `TLS_KEY_PATH` | — | Ruta a la clave privada PEM para HTTPS |

## Endpoints principales

| Método | Ruta | Auth | Descripción |
|--------|------|------|-------------|
| `GET` | `/health` | No | Health check (DB ping) |
| `POST` | `/api/auth/login` | No | Login — rate limit: 5 req/30s por IP |
| `POST` | `/api/auth/refresh` | No | Refrescar token |
| `POST` | `/api/auth/change-password` | Sí | Cambiar contraseña (mín 8 chars, complejidad) |
| `GET` | `/api/payments` | Sí | Listar pagos con filtros por fecha |
| `POST` | `/api/payments/verify` | Sí | Conciliar pago (anti-fraude) |
| `POST` | `/api/import/csv` | Admin | Importar CSV |
| `POST` | `/api/import/excel` | Admin | Importar Excel (máx 20MB) |
| `GET` | `/api/export/xlsx` | Admin | Exportar Excel |
| `POST` | `/api/sync/trigger` | Admin | Forzar sincronización IMAP |
| `GET` | `/api/users` | Admin | Listar usuarios |
| `POST` | `/api/users` | Admin | Crear usuario |
| `PUT` | `/api/users/{id}` | Admin | Editar usuario |
| `GET` | `/api/audit` | Admin | Log de auditoría |
| `GET` | `/api/quarantine` | Admin | Correos en cuarentena |

## Desarrollo

```bash
# Backend (modo standalone con hot-reload)
cd server
cp .env.example .env
cargo run

# Frontend (modo dev Vite + Tauri)
pnpm install
pnpm tauri dev
```

### Tests

```bash
# Backend
cd server && cargo test

# Frontend
pnpm test
```

## Seguridad

- Contraseñas: Argon2id + mínimo 8 caracteres con mayúsculas, minúsculas, números y especiales
- Autenticación: JWT (access 15min, refresh 7 días)
- Rate limiting: 5 req/30s en login, 30 req/s en API general
- CORS: Configurable por variable de entorno, no `*`
- Body limit: 20MB máximo en uploads
- IMAP passwords: Encriptadas con age en la base de datos
- Errores: Nunca se exponen detalles de BD al cliente
- Docker: Ejecuta como usuario no-root
- Health check: Endpoint `/health` con verificación de conectividad a BD

## Licencia

Propietario. Uso interno.
