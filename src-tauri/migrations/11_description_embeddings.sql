CREATE VIRTUAL TABLE IF NOT EXISTS description_embeddings USING vec0(
    track_id INTEGER PRIMARY KEY,
    embedding float[384]
);
