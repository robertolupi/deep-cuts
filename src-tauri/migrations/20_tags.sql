CREATE TABLE IF NOT EXISTS tags (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL UNIQUE,
    normalized_name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS track_tags (
    track_id INTEGER NOT NULL,
    tag_id   INTEGER NOT NULL,
    source   TEXT NOT NULL,
    PRIMARY KEY (track_id, tag_id),
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id)   REFERENCES tags(id)   ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_track_tags_tag ON track_tags(tag_id);

CREATE TABLE IF NOT EXISTS tag_diagnostic_logs (
    track_id        INTEGER PRIMARY KEY,
    raw_suggestions TEXT,
    cleanup_outcome TEXT,
    updated_at      TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS tag_synonym_cache (
    tag_a        TEXT NOT NULL,
    tag_b        TEXT NOT NULL,
    is_synonym   INTEGER NOT NULL,
    evaluated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tag_a, tag_b)
);
