-- Sortable volume on the markets table (brief §5.1: sort by volume).
-- Stored as BIGINT USDC base units, converted from Panta's human-readable
-- "volumeUsdc" string at sync time.
ALTER TABLE markets ADD COLUMN volume BIGINT;
