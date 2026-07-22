-- Migration 002: Add hora_correo column to payments table
-- hora_correo stores the email time in UTC-4 Caracas (HH:MM:SS), extracted from the email Date header.
-- Column is nullable to accommodate existing rows.

ALTER TABLE payments ADD COLUMN IF NOT EXISTS hora_correo TIME NULL;
