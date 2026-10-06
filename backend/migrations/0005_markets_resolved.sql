-- Resolved flag on markets (mirror of Panta's `resolved` catalog field).
-- Kept as an explicit boolean next to phase for cheap filtering in jobs.
ALTER TABLE markets ADD COLUMN resolved BOOLEAN NOT NULL DEFAULT false;
