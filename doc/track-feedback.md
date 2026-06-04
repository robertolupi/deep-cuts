# Chat with Your Music

The Chat tab in the Track Detail Pane lets users ask open-ended questions about a track using Qwen2-Audio. It is implemented in `src/lib/components/ChatPanel.svelte` (frontend) and `src-tauri/src/commands/chat.rs` (backend).

## How it works

1. User selects a track and clicks the **Chat** tab.
2. llama-server starts in the background (spinner shown while booting).
3. A WaveSurfer timeline (peaks-only, no audio decode) shows a draggable region defaulting to `[0, min(duration, 180s)]`.
4. User types a question and presses Enter; the selected region bounds are sent to the backend.
5. The backend slices the audio to that region, attaches it to the first message, and streams the response back token by token via `chat_token` events.
6. Conversation history is kept in frontend state for the session; switching tracks clears it.

## IPC command

```
ask_qwen(track_id, question, window_start_secs, window_duration_secs, history) -> Result<String, String>
```

The audio attachment is only included on the first turn. Subsequent turns in `history` are text-only, keeping the payload small.

## Status

* **Implemented**: 
  - Chat tab UI (`ChatPanel.svelte`), back-end llama-server controller (`llama.rs`), and the `ask_qwen` IPC command (`chat.rs`).
  - Audio-slicing and token-by-token streaming response events (`chat_token`).
  - Conversation persistence via SQLite databases (`chat_sessions` and `chat_messages` tables via `19_chat_history.sql` migration).
  - Saved chat session lookups and message insertion via Rust IPC commands (`get_chat_messages`, `save_chat_message`, `create_chat_session`, etc.).

## Cross-References

- `src-tauri/src/commands/chat.rs` — `ask_qwen` implementation
- `src-tauri/src/llama.rs` — server lifecycle
- `src/lib/components/ChatPanel.svelte` — UI
- `doc/qwen_limitations.md` — token budget, window sizing
