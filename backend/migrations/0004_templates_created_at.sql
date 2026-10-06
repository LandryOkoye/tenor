-- Creation timestamp for basket templates (admin views show it).
ALTER TABLE basket_templates ADD COLUMN created_at TIMESTAMPTZ NOT NULL DEFAULT now();
