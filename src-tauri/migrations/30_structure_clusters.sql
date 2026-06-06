CREATE TABLE structure_clusters (
    id          INTEGER PRIMARY KEY,
    label       TEXT NOT NULL,
    regex       TEXT NOT NULL,
    track_count INTEGER NOT NULL DEFAULT 0
);
