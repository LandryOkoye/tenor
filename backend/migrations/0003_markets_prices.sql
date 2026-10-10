-- Current spot prices on the market row itself (denormalised from the last
-- sync/detail fetch) so list/detail endpoints can serve prices without
-- joining the snapshot history. Worker keeps them fresh; history stays in
-- market_snapshots.
ALTER TABLE markets ADD COLUMN yes_price TEXT;
ALTER TABLE markets ADD COLUMN no_price TEXT;
