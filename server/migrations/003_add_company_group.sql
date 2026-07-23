-- Migration 003: Add company_group to users and payments
-- company_group identifies which business entity a user belongs to.
-- Allowed values: ferreteria_principal, pintatodo, herramientas_brink
-- Required for cashiers, optional for admins.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS company_group TEXT
    CHECK (company_group IN ('ferreteria_principal', 'pintatodo', 'herramientas_brink'));

-- Store which company group verified each payment, for traceability.
ALTER TABLE payments
    ADD COLUMN IF NOT EXISTS company_group TEXT
    CHECK (company_group IN ('ferreteria_principal', 'pintatodo', 'herramientas_brink'));

CREATE INDEX IF NOT EXISTS idx_users_company_group ON users(company_group);
CREATE INDEX IF NOT EXISTS idx_payments_company_group ON payments(company_group);
