---
name: db-migration
description: Safe pattern for adding SQLite schema migrations in the deep-cuts Rust/rusqlite_migration stack
---

# Adding a DB Schema Migration

Schema changes are managed by `rusqlite_migration`. The crate applies migrations in order and tracks the applied version in an internal `__migrations` table. Once a migration is shipped it is immutable — the only safe path forward is a new migration.

---

## Rules

- **Never edit an existing `M::up(...)` call.** Changing past migrations will corrupt databases that have already applied them.
- **Never reorder migrations.** Index position is the version number.
- **Always add at the end** of the array returned by `get_migrations()` in `src-tauri/src/database.rs`.
- SQLite's `ALTER TABLE ... ADD COLUMN` is the only DDL allowed mid-migration without rebuilding the table. Adding `NOT NULL` columns without a default requires recreating the table.

---

## Adding a migration

Open `src-tauri/src/database.rs` and append to the `migrations![]` array:

```rust
M::up("
    ALTER TABLE tracks ADD COLUMN <new_column> TEXT;
"),
```

For multiple changes that belong together:

```rust
M::up("
    ALTER TABLE tracks ADD COLUMN detected_energy REAL;
    ALTER TABLE tracks ADD COLUMN detected_valence REAL;
"),
```

For a new table:

```rust
M::up("
    CREATE TABLE IF NOT EXISTS my_new_table (
        id       INTEGER PRIMARY KEY AUTOINCREMENT,
        track_id INTEGER NOT NULL,
        value    REAL,
        FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE
    );
    CREATE INDEX idx_my_new_table_track ON my_new_table(track_id);
"),
```

---

## After adding the migration

1. **Update Rust structs** — if the new column is read anywhere (e.g. the `Track` struct in `database.rs`), add the field with `Option<T>` to handle pre-migration rows.

2. **Update sidecar structs** — if the new column holds ML-derived data that should survive a library rescan, wire it into `src-tauri/src/scanner/sidecar.rs` in three places:
   - Add the field to `SidecarMlMetadata` (with `Option<T>`)
   - Add the column to the `SELECT` in `save()` and assign it in the struct literal
   - Add a `SET <column> = ?` to the `UPDATE` statement in `restore()`

3. **Run the test suite** — the in-memory test DB exercises every migration on each run:
   ```bash
   cargo test --manifest-path src-tauri/Cargo.toml
   ```
   A failing migration test means your SQL is malformed or conflicts with an earlier migration.

3. **Verify from scratch** — wipe the dev DB to confirm all migrations apply cleanly:
   ```bash
   npm run tauri dev
   ```
   (or delete `~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db` first)

---

## sqlite-vec virtual tables

If adding an embedding table, use `vec0` syntax:

```rust
M::up("
    CREATE VIRTUAL TABLE IF NOT EXISTS my_embeddings USING vec0(
        track_id INTEGER PRIMARY KEY,
        embedding float[<dim>]
    );
"),
```

The sqlite-vec extension is loaded at app startup via `DbManager`. Do not remove or reorder those load calls.

---

## Production database location

```
~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db
```

Back it up before testing destructive migrations manually.
