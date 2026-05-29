ALTER TABLE tracks ADD COLUMN waveform_data TEXT;
ALTER TABLE tracks ADD COLUMN key TEXT;
ALTER TABLE tracks ADD COLUMN scale TEXT;
ALTER TABLE tracks ADD COLUMN key_strength REAL;
ALTER TABLE tracks ADD COLUMN loudness_lufs REAL;
ALTER TABLE tracks ADD COLUMN loudness_range REAL;
CREATE TABLE IF NOT EXISTS track_passes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER NOT NULL,
    pass_name TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    status INTEGER NOT NULL DEFAULT 0,
    log TEXT,
    result TEXT,
    duration_ms INTEGER,
    last_run_at TEXT,
    FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE,
    UNIQUE(track_id, pass_name)
);
CREATE INDEX IF NOT EXISTS idx_track_passes_status ON track_passes(status);
CREATE INDEX IF NOT EXISTS idx_track_passes_track ON track_passes(track_id);
