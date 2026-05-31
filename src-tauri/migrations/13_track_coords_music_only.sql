-- Track whether the stored projection was computed with music-only filtering.
-- 0 = all tracks were projected; 1 = only music tracks were projected.
-- Stored per-row so future invalidation logic can compare against the requested scope.
ALTER TABLE track_coords ADD COLUMN music_only INTEGER NOT NULL DEFAULT 0;
