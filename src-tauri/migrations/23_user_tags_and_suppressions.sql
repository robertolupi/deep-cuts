-- Tag suppressions (rescan-proof)
CREATE TABLE IF NOT EXISTS user_suppressed_tags (
    track_path TEXT NOT NULL,
    tag_name   TEXT NOT NULL,
    PRIMARY KEY (track_path, tag_name),
    FOREIGN KEY (track_path) REFERENCES tracks(path) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_user_suppressed_tags_path ON user_suppressed_tags(track_path);
