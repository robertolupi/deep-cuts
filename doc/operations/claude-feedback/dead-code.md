# Dead Code

Date: 2026-06-07

Unused code found while working on other tasks. Each item includes where it is and why it was likely left behind.

## `PipeOk` trait in `commands/chat.rs`

**Location:** [`src-tauri/src/commands/chat.rs:202`](../../../src-tauri/src/commands/chat.rs)

```rust
trait PipeOk<T> { fn pipe_ok(self) -> Result<T, String>; }
impl<T> PipeOk<T> for T { fn pipe_ok(self) -> Result<T, String> { Ok(self) } }
```

**Context:** `pipe_ok()` was used on three `collect::<Vec<_>>()` calls (`list_chat_sessions`, `get_chat_messages`, `search_chats`) to wrap the Vec in `Ok(...)`. During the SQLite error-swallowing fix (2026-06-07), those calls were replaced with `.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())`, which returns the `Result` directly. The trait is now dead.

**Fix:** Delete the trait and its impl (two lines). The compiler already warns about it (`trait PipeOk is never used`).
