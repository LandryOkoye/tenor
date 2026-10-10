-- Transaction integrity + recovery.
--
-- built_message: the exact wire bytes of the message we validated and handed
-- to the wallet. Submit only accepts a signed tx whose message is
-- byte-identical, so nothing (fee, accounts, amounts) can be swapped out.
--
-- last_valid_block_height: when the tx's blockhash expires. The tx_confirm
-- job uses it to declare an unseen signature dead instead of waiting forever.

ALTER TABLE orders ADD COLUMN built_message BYTEA;
ALTER TABLE orders ADD COLUMN last_valid_block_height BIGINT;

ALTER TABLE basket_purchases ADD COLUMN built_message BYTEA;
ALTER TABLE basket_purchases ADD COLUMN last_valid_block_height BIGINT;
ALTER TABLE basket_purchases ADD COLUMN tx_signature TEXT;

-- Which record a broadcast signature belongs to (order | basket purchase |
-- claim batch id), so the confirmation job can finish the right flow.
ALTER TABLE transactions ADD COLUMN ref_id UUID;
ALTER TABLE transactions ADD COLUMN last_valid_block_height BIGINT;

-- One row per claim transaction we build ("claim" or a chunk of "claim all").
CREATE TABLE claim_batches (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id                 UUID NOT NULL REFERENCES users (id),
    market_ids              TEXT[] NOT NULL,
    built_message           BYTEA NOT NULL,
    last_valid_block_height BIGINT,
    tx_signature            TEXT,
    status                  TEXT NOT NULL DEFAULT 'built', -- built | submitted | confirmed | failed
    error_code              TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX claim_batches_user_idx ON claim_batches (user_id);
