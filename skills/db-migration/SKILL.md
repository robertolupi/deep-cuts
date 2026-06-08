---
name: db-migration
description: Safe pattern for adding SQLite schema migrations in the deep-cuts Rust/rusqlite_migration stack
---

# Adding a DB Schema Migration

Schema changes are managed by `rusqlite_migration`. The crate applies migrations in order and tracks the applied version in an internal `__migrations` table. Once a migration is shipped it is immutable — the only safe path forward is a new migration.

---

## Rules

- **Never edit an existing migration.** Changing past migrations will corrupt databases that have already applied them.
- **Never reorder migrations.** Index position is the version number.
- **Always add at the end** of the array returned by `get_migrations()` in `src-tauri/src/database.rs`.
- **Use External SQL Files**: All migration SQL scripts must reside in `src-tauri/migrations/` as zero-padded, index-prefixed `.sql` files (e.g. `07_my_new_column.sql`) and be loaded via `include_str!()`.
- **Avoid Inline SQL in Handlers**: Database interactions (queries, updates) should be encapsulated inside repository methods on the database structs (`Track`, `WatchedDirectory`) rather than written inlined inside Tauri command handlers.

---

## Adding a migration

1. **Create the SQL file** in `src-tauri/migrations/` using zero-padded prefixes. For example, `src-tauri/migrations/07_my_new_column.sql`:

   ```sql
   ALTER TABLE tracks ADD COLUMN <new_column> TEXT;
   ```

2. **Register it at the end** of the `get_migrations()` array in `src-tauri/src/database.rs`:

   ```rust
   M::up(include_str!("../migrations/07_my_new_column.sql")),
   ```

For a new table, create `src-tauri/migrations/08_my_new_table.sql`:

```sql
CREATE TABLE IF NOT EXISTS my_new_table (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER NOT NULL,
    value    REAL,
    FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE
);
CREATE INDEX idx_my_new_table_track ON my_new_table(track_id);
```

Register it in `database.rs`:

```rust
M::up(include_str!("../migrations/08_my_new_table.sql")),
```

---

## Encapsulating Database Code (Repository Pattern)

Tauri commands should remain extremely thin and decoupled from raw SQL queries. 
*   **Do not** execute prepare statements or map query rows inside `lib.rs` handlers.
*   **Instead**, implement clean CRUD repository methods directly on the domain structs in `src-tauri/src/database.rs`:

```rust
impl Track {
    pub fn find_all(conn: &Connection) -> Result<Vec<Self>, rusqlite::Error> {
        // Build the SELECT from the canonical column list; map rows by name.
        let sql = format!(
            "SELECT {} FROM tracks ORDER BY artist ASC",
            Self::COLUMN_LIST.join(", "),
        );
        let mut stmt = conn.prepare(&sql)?;
        stmt.query_map([], Self::from_row)?.collect()
    }
}
```

`Track::COLUMN_LIST` and `Track::from_row` are **generated** by the
`db_row_mapping!(Track { ... })` macro in `database.rs` from a single field
list. Rows are mapped by column **name** (`row.get("title")`), so the `SELECT`
order is irrelevant and there is no positional `row.get(N)` to keep aligned.
New `SELECT * FROM tracks`-style queries should reuse `Track::COLUMN_LIST`
rather than hand-writing the column list again.

*   Call them from your Tauri command:

```rust
#[tauri::command]
fn get_tracks(conn_state: tauri::State<'_, Mutex<Connection>>) -> Result<Vec<Track>, String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    Track::find_all(&conn).map_err(|e| e.to_string())
}
```

---

## After adding the migration

1. **Update Rust structs** — if the new column is read anywhere (e.g. the `Track` struct in `database.rs`), add the field with `Option<T>` to handle pre-migration rows. For `Track`, also add the column name to the `db_row_mapping!(Track { ... })` list directly below the struct — the field name must equal the column name. The compiler enforces that the struct and the macro list stay in sync: a name in only one place fails to build, so there is no silent drift. (You do **not** need to touch `find_all`/`find` — they read from `COLUMN_LIST` automatically.)

2. **Update sidecar structs** — if the new column holds ML-derived data that should survive a library rescan, wire it into `src-tauri/src/scanner/sidecar.rs` in three places:
   - Add the field to `SidecarMlMetadata` (with `Option<T>`)
   - Add the column to the `SELECT` in `save()` and assign it in the struct literal
   - Add a `SET <column> = ?` to the `UPDATE` statement in `restore()`

3. **Run the test suite** — the in-memory test DB exercises every migration on each run:
   ```bash
   cargo test --manifest-path src-tauri/Cargo.toml
   ```
   A failing migration test means your SQL is malformed or conflicts with an earlier migration. The `database::tests` migration-invariant tests also assert that expected tables, indexes, virtual tables, and every `Track`-mapped column exist after all migrations — so a dropped or renamed mapped column fails here rather than at runtime.

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
