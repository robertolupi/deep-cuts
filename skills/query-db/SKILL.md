---
name: query-db
description: How to locate and query the deep-cuts production SQLite database
---

# Querying the Production Database

---

## Database location

```
~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db
```

Store it in a shell variable to avoid retyping:

```bash
DB="$HOME/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db"
```

---

## Opening with the sqlite3 CLI

```bash
sqlite3 "$HOME/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db"
```

Useful sqlite3 meta-commands:

```sql
.tables          -- list all tables
.schema tracks   -- show CREATE statement for a table
.mode column     -- aligned column output
.headers on      -- show column names
.quit
```

---

## Common queries

### Library overview

```sql
SELECT COUNT(*) AS total_tracks,
       COUNT(bpm) AS have_bpm,
       COUNT(artist) AS have_artist,
       COUNT(lyrics) AS have_lyrics
FROM tracks;
```

### Track search

```sql
-- Find tracks by filename or path fragment
SELECT id, filename, artist, title, bpm, genre
FROM tracks
WHERE path LIKE '%Jazz%'
LIMIT 20;
```

### Watched directories

```sql
SELECT * FROM watched_directories;
```

### App settings

```sql
SELECT * FROM app_settings;
```

---

## sqlite-vec virtual tables

If embedding tables are present (`vec0` virtual tables), the sqlite-vec extension must be loaded — it is compiled into the Rust binary and loaded automatically at app startup but is **not** available to the plain `sqlite3` CLI.

The shadow tables are queryable from the CLI without the extension:

```sql
SELECT COUNT(*) FROM audio_embeddings_rowids;
```

---

## Safety notes

- **Do not write to the DB while the app is running.** External writes can corrupt in-flight transactions.
- **Back up before manual edits.** Copy the `.db` file before any `UPDATE`/`DELETE` — there is no undo.
