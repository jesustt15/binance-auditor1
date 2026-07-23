-- Migration 001: Initial schema for multi-user server
-- 7 tables: users, imap_credentials, payments, quarantined_emails, audit_log, imports, import_rows

-- Users and auth
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,  -- argon2id
    role TEXT NOT NULL CHECK (role IN ('admin', 'cashier')),
    company_group TEXT CHECK (company_group IN ('ferreteria_principal', 'pintatodo', 'herramientas_brink')),
    station_name VARCHAR(100),
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- IMAP credentials (encrypted with age, admin-only)
CREATE TABLE IF NOT EXISTS imap_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) NOT NULL,
    encrypted_password TEXT NOT NULL,  -- age-encrypted
    imap_host VARCHAR(255) NOT NULL DEFAULT 'imap.gmail.com',
    imap_port INTEGER NOT NULL DEFAULT 993,
    last_sync_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Payments (replaces pagos_binance from SQLite)
CREATE TABLE IF NOT EXISTS payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tipo VARCHAR(20) NOT NULL DEFAULT 'pago' CHECK (tipo IN ('pago', 'deposito')),
    usuario_remitente VARCHAR(255),
    monto DECIMAL(15, 2) NOT NULL,
    moneda VARCHAR(10) NOT NULL DEFAULT 'USDT',
    fecha_correo TIMESTAMPTZ NOT NULL,
    estado TEXT NOT NULL DEFAULT 'disponible'
        CHECK (estado IN ('disponible', 'verificado', 'rechazado', 'por_revisar', 'cuarentena')),
    observaciones TEXT,
    verificado_en TIMESTAMPTZ,
    verified_by UUID REFERENCES users(id),
    company_group TEXT CHECK (company_group IN ('ferreteria_principal', 'pintatodo', 'herramientas_brink')),
    fraud_verdict TEXT DEFAULT 'clean'
        CHECK (fraud_verdict IN ('clean', 'dkim_fail', 'spf_fail', 'dmarc_fail', 'duplicate', 'amount_tamper')),
    fraud_details JSONB,
    email_uid INTEGER,  -- IMAP UID for dedup
    raw_headers JSONB,  -- DKIM/SPF/DMARC results
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Quarantined emails (failed anti-fraud, pending admin review)
CREATE TABLE IF NOT EXISTS quarantined_emails (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    raw_email TEXT NOT NULL,
    subject VARCHAR(500),
    from_address VARCHAR(255),
    failure_reason VARCHAR(100) NOT NULL,  -- dkim_fail, spf_fail, dmarc_fail
    dkim_result VARCHAR(50),
    spf_result VARCHAR(50),
    dmarc_result VARCHAR(50),
    parsed_data JSONB,  -- what extract_binance_data would have produced
    reviewed_by UUID REFERENCES users(id),
    reviewed_at TIMESTAMPTZ,
    review_decision TEXT DEFAULT 'pending' CHECK (review_decision IN ('accept', 'reject', 'pending')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Append-only audit log
CREATE TABLE IF NOT EXISTS audit_log (
    id BIGSERIAL PRIMARY KEY,
    user_id UUID REFERENCES users(id),
    action VARCHAR(50) NOT NULL,  -- login, verify_payment, import_csv, config_change, etc.
    resource_type VARCHAR(50),    -- payment, user, imap_config, import
    resource_id UUID,
    details JSONB,                -- structured action details
    ip_address INET,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- CSV/Excel imports
CREATE TABLE IF NOT EXISTS imports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    filename VARCHAR(500) NOT NULL,
    file_type VARCHAR(10) NOT NULL CHECK (file_type IN ('csv', 'xlsx', 'xls')),
    total_rows INTEGER NOT NULL,
    verified_rows INTEGER NOT NULL DEFAULT 0,
    failed_rows INTEGER NOT NULL DEFAULT 0,
    uploaded_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS import_rows (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    import_id UUID NOT NULL REFERENCES imports(id) ON DELETE CASCADE,
    row_number INTEGER NOT NULL,
    usuario VARCHAR(255),
    monto DECIMAL(15, 2),
    fecha VARCHAR(50),
    result VARCHAR(50) NOT NULL,  -- verified, not_found, error
    error_message TEXT,
    matched_payment_id UUID REFERENCES payments(id)
);

-- Indexes for query performance
CREATE INDEX IF NOT EXISTS idx_payments_fecha ON payments(fecha_correo DESC);
CREATE INDEX IF NOT EXISTS idx_payments_estado ON payments(estado);
CREATE INDEX IF NOT EXISTS idx_payments_usuario ON payments(usuario_remitente);
CREATE INDEX IF NOT EXISTS idx_payments_fraud ON payments(fraud_verdict);
CREATE INDEX IF NOT EXISTS idx_payments_email_uid ON payments(email_uid);
CREATE INDEX IF NOT EXISTS idx_audit_user ON audit_log(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_log(action);
CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_log(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_quarantine_decision ON quarantined_emails(review_decision);
CREATE INDEX IF NOT EXISTS idx_import_rows_import ON import_rows(import_id);
