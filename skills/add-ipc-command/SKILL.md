---
name: add-ipc-command
description: Pattern for adding a new Tauri IPC command (request/response or push event) in the deep-cuts monorepo
---

# Adding a New Tauri IPC Command

IPC is the only channel between the Svelte frontend and the Rust backend. There are two patterns: **request/response** (`invoke`) and **push events** (`emit`/`listen`). Most commands use request/response; push events are used for long-running background work (scan updates, analysis progress).

---

## Pattern A — Request/response (`invoke`)

### 1. Write the command handler

Commands live in `src-tauri/src/commands/` (one file per domain — `library.rs`, `playlists.rs`, `analysis.rs`, etc.). Add your handler to the appropriate file or create a new one and expose it via `commands/mod.rs`.

```rust
#[tauri::command]
pub fn my_command(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    some_arg: String,
) -> Result<MyReturnType, String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    // ... do work ...
    Ok(result)
}
```

- Return `Result<T, String>` — the `Err` string surfaces as a rejected Promise in the frontend.
- Access the DB via `conn_state: tauri::State<'_, Mutex<Connection>>` (registered at startup as `app.manage(Mutex::new(conn))`).
- Access the app handle (for emitting events) via `app: tauri::AppHandle`.
- Keep handlers thin; push heavy logic into helper functions so they're testable.

### 2. Register the command

Find the `tauri::generate_handler![]` macro in `lib.rs` and add your command:

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    my_command,
])
```

Forgetting this step compiles cleanly but the frontend call will throw at runtime.

### 3. Call it from the frontend

```typescript
import { invoke } from '@tauri-apps/api/core';

const result = await invoke<MyReturnType>('my_command', { someArg: 'value' });
```

Argument names are converted from camelCase (TypeScript) to snake_case (Rust) automatically by Tauri. The return type generic is optional but recommended for type safety.

---

## Pattern B — Push events from a background thread

Use this when the Rust side needs to notify the frontend proactively (e.g. scan progress, analysis completion).

### Rust side — emit from a spawned thread

```rust
#[tauri::command]
fn start_long_task(app: tauri::AppHandle) -> Result<(), String> {
    std::thread::spawn(move || {
        for i in 0..100 {
            // ... do work ...
            app.emit("my-task-progress", serde_json::json!({ "percent": i })).ok();
        }
        app.emit("my-task-complete", ()).ok();
    });
    Ok(())
}
```

`app.emit()` broadcasts to all WebView windows. The payload is serialized to JSON.

### Frontend side — listen for events

```typescript
import { listen } from '@tauri-apps/api/event';
import { onDestroy } from 'svelte';

const unlisten = await listen<{ percent: number }>('my-task-progress', (event) => {
    progress = event.payload.percent;
});

// Clean up on component destroy to avoid listener leaks
onDestroy(() => unlisten());
```

Always store and call the unlisten function in `onDestroy` — leaked listeners accumulate across hot-reloads in dev mode.

---

## Managed state

`DbManager` is already registered as managed state in `lib.rs`. To add new shared state (e.g. a cache, a flag), register it before `.run()`:

```rust
.manage(MyState::new())
```

Then receive it in your command:

```rust
fn my_command(my_state: tauri::State<'_, MyState>) -> Result<(), String> { ... }
```

---

## Checklist

- [ ] Handler decorated with `#[tauri::command]`
- [ ] Added to `tauri::generate_handler![]`
- [ ] Frontend call uses the correct snake_case command name as a string literal
- [ ] `Result<T, String>` return type (or `()` for fire-and-forget)
- [ ] Push-event listeners cleaned up with `onDestroy` (if applicable)
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` still passes
