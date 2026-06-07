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
import { invoke } from "$lib/ipc";

const result = await invoke<MyReturnType>('my_command', { someArg: 'value' });
```

Argument names are converted from camelCase (TypeScript) to snake_case (Rust) automatically by Tauri. The return type generic is optional but recommended for type safety.

All app code should import `invoke` and `listen` from `$lib/ipc`, not directly from `@tauri-apps/api/core` or `@tauri-apps/api/event`. The wrapper owns local-debug mocks and is the right place to add typed command mappings.

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
import { listen } from "$lib/ipc";
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

**Rust**
- [ ] Handler decorated with `#[tauri::command]`
- [ ] Return type is `Result<T, String>` (or `Result<(), String>` for side-effects)
- [ ] Heavy logic extracted into a helper function, not inline in the handler
- [ ] Added to `tauri::generate_handler![]` in `lib.rs`
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` still passes

**`src/lib/ipc.ts` + `src/lib/mock-data.ts`** (required — do not skip)
- [ ] Entry added to `CommandMap` in `ipc.ts` with precise `args` and `result` types (use `Record<string, unknown>` / `unknown` with a `// TODO: tighten` comment only if the shape is truly unclear)
- [ ] Fixture data for new entity types added to `mock-data.ts` as typed exported constants
- [ ] Entry added to `MOCK_RESPONSES` in `ipc.ts` with a realistic return value for UI-affecting commands, or `null`/`undefined` for pure side-effects
- [ ] No direct `@tauri-apps/api` imports added to app code

**Frontend**
- [ ] `invoke` / `listen` imported from `$lib/ipc` (not from `@tauri-apps/api` directly)
- [ ] Command name string matches the Rust function name exactly (Tauri does not rename)
- [ ] For push events: payload type, event name, emit source, lifecycle, and unlisten ownership documented at the listen site
- [ ] Push-event `unlisten` stored and called in `onDestroy`
- [ ] Frontend test or store test added when the command drives user-visible state
