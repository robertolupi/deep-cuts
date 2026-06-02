-- Migration 18: Add manual Playlists and dynamic Saved Searches (Smart Playlists) schema tables

CREATE TABLE IF NOT EXISTS playlists (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS playlist_tracks (
    playlist_id    INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id       INTEGER REFERENCES tracks(id)             ON DELETE SET NULL,
    position       INTEGER NOT NULL,
    cached_title   TEXT NOT NULL, -- Stored at insertion time for tombstoning
    cached_artist  TEXT NOT NULL, -- Stored at insertion time for tombstoning
    PRIMARY KEY (playlist_id, position)
);

CREATE INDEX IF NOT EXISTS idx_playlist_tracks_playlist_id ON playlist_tracks(playlist_id);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_track_id ON playlist_tracks(track_id);

CREATE TABLE IF NOT EXISTS saved_searches (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    name           TEXT NOT NULL,
    query_json     TEXT NOT NULL,   -- Serialized Svelte filter store JSON
    schema_version INTEGER NOT NULL DEFAULT 1,
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL
);
