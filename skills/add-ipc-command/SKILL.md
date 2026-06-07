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

### 3. Update `src/lib/ipc.ts`

**This step is required for every new command.** `$lib/ipc` is the single import point for all frontend IPC — it owns local-debug mocks and the command type boundary.

#### 3a. Add a mock response

The mock system has two files:

- **`src/lib/mock-data.ts`** — typed fixture data exported as constants (`MOCK_TRACKS`, `MOCK_PLAYLISTS`, etc.). Add new entity arrays or objects here when the command introduces a new return type.
- **`src/lib/ipc.ts`** — the `MOCK_RESPONSES` map wires command names to handlers that use data from `mock-data.ts`.

For a simple query command:

```typescript
// In src/lib/mock-data.ts — add fixture data if introducing a new type
export const MOCK_MY_THINGS: MyThing[] = [
  { id: 1, name: "Example Thing", value: 42 },
];

// In src/lib/ipc.ts — wire the command
import { MOCK_MY_THINGS } from "$lib/mock-data";

const MOCK_RESPONSES: Record<string, (args?: any) => unknown> = {
  // ... existing entries ...
  get_my_things: () => MOCK_MY_THINGS,
  get_my_thing: ({ id }: { id: number }) => MOCK_MY_THINGS.find(t => t.id === id) ?? null,
  save_my_thing: () => null,   // side-effect only — still add entry to suppress warning
};
```

Rules:
- If the command returns data that affects visible UI, provide a realistic mock value so `?local_debug=1` dev mode works without Tauri. Reference the `ui-debug` skill for how to verify this.
- If the command is a side-effect only (e.g. `save_*`, `delete_*`), return `null` or `undefined` — still add the entry so the console warning for unhandled commands is suppressed.
- If the command result depends on args (e.g. `get_track`), pattern-match on `args` to return something plausible.
- Rich structured data (new entity types, lists of items) belongs in `mock-data.ts`; inline lambdas in `ipc.ts` are for simple derivations from that data.

#### 3b. Keep the import boundary clean

```typescript
// ✓ correct
import { invoke, listen } from "$lib/ipc";

// ✗ wrong — bypasses mocks and type map
import { invoke } from "@tauri-apps/api/core";
```

Never import `invoke` or `listen` directly from `@tauri-apps/api` in app code. Only `$lib/ipc.ts` itself imports from the Tauri package.

### 4. Call it from the frontend

```typescript
import { invoke } from "$lib/ipc";

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
import { listen } from "$lib/ipc";
import { onDestroy } from 'svelte';

const unlisten = await listen<{ percent: number }>('my-task-progress', (event) => {
    progress = event.payload.percent;
});

// Clean up on component destroy to avoid listener leaks
onDestroy(() => unlisten());
```

Always store and call the unlisten function in `onDestroy` — leaked listeners accumulate across hot-reloads in dev mode.

### Document push event metadata

Add a comment near the `listen` call (or in the relevant store) that documents:

```typescript
// Event: 'my-task-progress'
// Payload: { percent: number }
// Emitted by: start_long_task (commands/your_domain.rs)
// Lifecycle: emitted 0–N times between 'my-task-start' and 'my-task-complete'
// Unlisten: owned by ThisComponent, cleaned up in onDestroy
```

This prevents the event name, payload shape, and lifecycle from becoming implicit tribal knowledge. Push events are harder to discover than commands — document them at the listen site.

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

---

## Common mistakes

| Mistake | Symptom | Fix |
|---------|---------|-----|
| Forgetting `tauri::generate_handler![]` | Frontend call throws `"command not found"` at runtime; Rust compiles fine | Add the command to `generate_handler![]` in `lib.rs` |
| Importing `invoke` directly from `@tauri-apps/api/core` | `?local_debug=1` mock mode silently falls through to real Tauri and errors | Import from `$lib/ipc` only |
| No `MOCK_RESPONSES` entry for a UI-affecting command | `?local_debug=1` logs a console warning and resolves `undefined`; UI breaks in ways that are hard to debug | Add a realistic mock to `MOCK_RESPONSES` |
| Missing `CommandMap` entry | `invoke` call is untyped; TypeScript won't catch wrong args or return type | Add entry to `CommandMap` in `ipc.ts` |
| Mismatched command name string | Frontend call throws `"command not found"`; Rust compiles fine | The string must match the Rust function name exactly — Tauri does not rename handlers |
| Push-event listener not cleaned up | Duplicate handlers accumulate across hot-reloads; events fire multiple times | Store the `unlisten` fn and call it in `onDestroy` |
| Push-event payload type not documented | Future callers guess the shape from runtime logs | Add payload type, event name, emit source, lifecycle, and unlisten ownership as a comment at the listen site |
