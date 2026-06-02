-- Migration 17: Add columns for AcoustID fingerprint matching, MusicBrainz MBID metadata cache, and album cover artwork.

ALTER TABLE tracks ADD COLUMN acoustid_status TEXT;
ALTER TABLE tracks ADD COLUMN musicbrainz_id TEXT;
ALTER TABLE tracks ADD COLUMN enriched_metadata TEXT;
ALTER TABLE tracks ADD COLUMN cover_art BLOB;

-- Global network toggle: 'silent' (default, auto-update) or 'never' (disabled lookup)
INSERT OR IGNORE INTO app_settings (key, value) VALUES ('acoustid_enrichment_enabled', 'silent');
