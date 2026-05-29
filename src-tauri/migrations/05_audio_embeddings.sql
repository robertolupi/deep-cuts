CREATE VIRTUAL TABLE IF NOT EXISTS audio_embeddings USING vec0(
    track_id INTEGER PRIMARY KEY,
    embedding float[512]
);
