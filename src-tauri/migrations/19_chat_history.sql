CREATE TABLE IF NOT EXISTS chat_sessions (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id             INTEGER NOT NULL,
    title                TEXT    NOT NULL DEFAULT 'New Chat',
    window_start_secs    REAL,
    window_duration_secs REAL,
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL,
    FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE
);
CREATE INDEX idx_chat_sessions_track ON chat_sessions(track_id);

CREATE TABLE IF NOT EXISTS chat_messages (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL,
    role       TEXT    NOT NULL,
    content    TEXT    NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE
);
CREATE INDEX idx_chat_messages_session ON chat_messages(session_id);

-- FTS5 content table mirroring chat_messages.content
CREATE VIRTUAL TABLE IF NOT EXISTS chat_messages_fts USING fts5(
    content,
    content='chat_messages',
    content_rowid='id'
);

CREATE TRIGGER chat_messages_ai AFTER INSERT ON chat_messages BEGIN
    INSERT INTO chat_messages_fts(rowid, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER chat_messages_ad AFTER DELETE ON chat_messages BEGIN
    INSERT INTO chat_messages_fts(chat_messages_fts, rowid, content) VALUES ('delete', old.id, old.content);
END;

CREATE TRIGGER chat_messages_au AFTER UPDATE ON chat_messages BEGIN
    INSERT INTO chat_messages_fts(chat_messages_fts, rowid, content) VALUES ('delete', old.id, old.content);
    INSERT INTO chat_messages_fts(rowid, content) VALUES (new.id, new.content);
END;
