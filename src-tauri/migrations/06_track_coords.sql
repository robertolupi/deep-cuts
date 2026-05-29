CREATE TABLE IF NOT EXISTS track_coords (
    track_id INTEGER PRIMARY KEY,
    x        REAL NOT NULL,
    y        REAL NOT NULL,
    FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_track_coords_track ON track_coords(track_id);
