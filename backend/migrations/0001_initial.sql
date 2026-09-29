-- Initial schema (PROJECT_BRIEF.md §10).
-- Money columns are BIGINT USDC base units (6 decimals). Timestamps TIMESTAMPTZ UTC.

CREATE TABLE users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_pubkey TEXT NOT NULL UNIQUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at  TIMESTAMPTZ
);

CREATE TABLE markets (
    id             TEXT PRIMARY KEY,              -- Panta market id
    title          TEXT NOT NULL,
    category       TEXT,
    phase          TEXT,
    status         TEXT NOT NULL,
    close_time     TIMESTAMPTZ,
    resolve_time   TIMESTAMPTZ,
    resolved_at    TIMESTAMPTZ,
    outcome        TEXT,                          -- 'yes' | 'no' once final
    creator        TEXT,
    oracle         TEXT,
    rules_text     TEXT,
    image_url      TEXT,
    raw_json       JSONB NOT NULL DEFAULT '{}'::jsonb,
    first_seen_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX markets_category_idx ON markets (category);
CREATE INDEX markets_status_idx   ON markets (status);

CREATE TABLE market_snapshots (
    market_id TEXT NOT NULL REFERENCES markets (id) ON DELETE CASCADE,
    ts        TIMESTAMPTZ NOT NULL DEFAULT now(),
    yes_price DOUBLE PRECISION,
    no_price  DOUBLE PRECISION,
    volume    BIGINT
);
CREATE INDEX market_snapshots_market_ts_idx ON market_snapshots (market_id, ts);

CREATE TABLE market_events (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    market_id  TEXT NOT NULL REFERENCES markets (id) ON DELETE CASCADE,
    ts         TIMESTAMPTZ NOT NULL DEFAULT now(),
    event_type TEXT NOT NULL,                       -- created | trading_started | closed | resolution_proposed | resolved | claims_open | status_change
    payload    JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX market_events_market_ts_idx ON market_events (market_id, ts);

CREATE TABLE basket_templates (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug          TEXT NOT NULL UNIQUE,
    title         TEXT NOT NULL,
    description   TEXT NOT NULL DEFAULT '',
    category      TEXT,
    cover_url     TEXT,
    status        TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'published', 'closed', 'settled')),
    min_buy_usdc  BIGINT NOT NULL DEFAULT 1000000,
    max_buy_usdc  BIGINT NOT NULL DEFAULT 100000000,
    created_by    TEXT,
    published_at  TIMESTAMPTZ,
    closed_at     TIMESTAMPTZ
);

CREATE TABLE basket_legs (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id UUID NOT NULL REFERENCES basket_templates (id) ON DELETE CASCADE,
    market_id   TEXT NOT NULL REFERENCES markets (id),
    side        TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    weight_bps  INT  NOT NULL CHECK (weight_bps > 0),
    position    INT  NOT NULL DEFAULT 0
);
CREATE INDEX basket_legs_template_idx ON basket_legs (template_id);
CREATE INDEX basket_legs_market_idx   ON basket_legs (market_id);

CREATE TABLE basket_quotes (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id  UUID NOT NULL REFERENCES basket_templates (id),
    user_id      UUID REFERENCES users (id),
    amount_usdc  BIGINT NOT NULL,
    fee_usdc     BIGINT NOT NULL,
    slippage_bps INT NOT NULL,
    legs_json    JSONB NOT NULL,
    expires_at   TIMESTAMPTZ NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX basket_quotes_expiry_idx ON basket_quotes (expires_at);

CREATE TABLE basket_purchases (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id UUID NOT NULL REFERENCES basket_templates (id),
    user_id     UUID NOT NULL REFERENCES users (id),
    quote_id    UUID REFERENCES basket_quotes (id),
    amount_usdc BIGINT NOT NULL,
    fee_usdc    BIGINT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'quoted',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX basket_purchases_user_idx ON basket_purchases (user_id);

CREATE TABLE orders (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id            UUID NOT NULL REFERENCES users (id),
    basket_purchase_id UUID REFERENCES basket_purchases (id),
    market_id          TEXT NOT NULL REFERENCES markets (id),
    side               TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    amount_usdc        BIGINT NOT NULL,
    expected_shares    BIGINT,
    panta_quote_id     TEXT,
    panta_order_id     TEXT,
    tx_signature       TEXT,
    status             TEXT NOT NULL DEFAULT 'quoted',
    error_code         TEXT,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX orders_user_idx           ON orders (user_id);
CREATE INDEX orders_purchase_idx       ON orders (basket_purchase_id);
CREATE INDEX orders_panta_order_idx    ON orders (panta_order_id) WHERE panta_order_id IS NOT NULL;
CREATE INDEX orders_pending_status_idx ON orders (status) WHERE status IN ('submitted', 'confirmed');

CREATE TABLE transactions (
    signature    TEXT PRIMARY KEY,
    user_id      UUID REFERENCES users (id),
    kind         TEXT NOT NULL,                     -- order | basket | claim
    status       TEXT NOT NULL DEFAULT 'submitted', -- submitted | confirmed | finalized | failed
    slot         BIGINT,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    confirmed_at TIMESTAMPTZ,
    raw_error    TEXT
);
CREATE INDEX transactions_status_idx ON transactions (status) WHERE status IN ('submitted', 'confirmed');

CREATE TABLE claims (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID NOT NULL REFERENCES users (id),
    market_id    TEXT NOT NULL REFERENCES markets (id),
    tx_signature TEXT,
    amount_usdc  BIGINT,
    status       TEXT NOT NULL DEFAULT 'pending',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX claims_user_idx ON claims (user_id);

CREATE TABLE watchlist (
    user_id    UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    market_id  TEXT NOT NULL REFERENCES markets (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, market_id)
);

CREATE TABLE admin_users (
    wallet_pubkey TEXT PRIMARY KEY,
    role          TEXT NOT NULL DEFAULT 'admin'
);

CREATE TABLE auth_nonces (
    nonce      TEXT PRIMARY KEY,
    wallet     TEXT NOT NULL,
    message    TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ
);

-- Creator scorecard backing data for the Resolution Center (materialised by refresh job).
CREATE TABLE creator_stats (
    creator               TEXT PRIMARY KEY,
    markets_created       BIGINT NOT NULL DEFAULT 0,
    markets_resolved      BIGINT NOT NULL DEFAULT 0,
    pct_resolved_on_time  DOUBLE PRECISION,
    median_delay_hours    DOUBLE PRECISION,
    surprise_resolutions  BIGINT NOT NULL DEFAULT 0,
    total_volume_usdc     BIGINT NOT NULL DEFAULT 0,
    refreshed_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
