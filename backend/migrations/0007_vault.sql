-- Tokenized baskets (ADR 0002): one on-chain basket_vault per template.

CREATE TABLE vault_baskets (
    template_id    UUID PRIMARY KEY REFERENCES basket_templates (id),
    onchain_id     BIGINT NOT NULL UNIQUE,      -- basket_vault `id` (PDA seed)
    basket_pda     TEXT NOT NULL,
    vault_pda      TEXT NOT NULL,
    mint           TEXT NOT NULL,               -- basket token mint
    panta_fee_usdc TEXT NOT NULL,               -- Panta protocol fee account
    -- [{market_id, event, authority, market_vault, side, weight_bps}]
    legs_json      JSONB NOT NULL,
    status         TEXT NOT NULL DEFAULT 'pending', -- pending | live | closed | settled
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Every vault transaction we build (admin create, user deposit/redeem).
CREATE TABLE vault_txs (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id                 UUID NOT NULL REFERENCES users (id),
    template_id             UUID NOT NULL REFERENCES vault_baskets (template_id),
    kind                    TEXT NOT NULL,      -- create | deposit | redeem
    amount_usdc             BIGINT,             -- deposit amount (base units)
    tokens                  BIGINT,             -- expected (deposit) / burned (redeem)
    built_message           BYTEA NOT NULL,
    last_valid_block_height BIGINT,
    tx_signature            TEXT,
    status                  TEXT NOT NULL DEFAULT 'built', -- built | submitted | confirmed | failed
    error_code              TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX vault_txs_user_idx ON vault_txs (user_id);
