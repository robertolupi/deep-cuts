# Chat with Your Music

## Motivation

Deep Cuts already runs Qwen2-Audio over every track to extract genre, mood, and a short description. The same model — already downloaded, already loadable via `llama-server` — can answer open-ended production questions: "why does this sound muddy?", "what's the arrangement structure?", "how does this compare to a typical club track?". A dedicated Chat tab in the Track Detail Pane makes that capability available interactively, without leaving the app.

The `tools/feedback.sh` script proved the idea: the model gives useful, specific answers when asked directly about an audio file. This design formalises that into a first-class UI feature.

---

## User Flow

1. User selects a track in the library.
2. User clicks the **Chat** tab in the Track Detail Pane.
3. llama-server starts in the background (if not already running). A spinner with "Loading model…" is shown while it boots.
4. Once the server is healthy, a chat input appears. The track audio is implicitly in context for every message.
5. User types a question and presses Enter (or clicks Send).
6. The response streams in below the input, token by token.
7. The conversation accumulates in the panel for the session. Switching to a different track clears the history and re-anchors to the new audio.

---

## UI Layout

The Chat tab lives alongside the existing content in `TrackDetailPane.svelte`. The tab bar is already implicit (AI description, mood bars, metadata are stacked sections); a tab strip at the top of the pane is the natural entry point.

```
┌──────────────────────────────────────┐
│  [Info]  [Chat]                      │  ← tab strip
├──────────────────────────────────────┤
│  ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~    │  ← WaveSurfer waveform
│  ▶  0:42 ─────────●──────── 3:54    │     (replaces main PlayerBar)
│             [Use this section]        │  ← anchors the 30s window
├──────────────────────────────────────┤
│  Loading model…  ⠋                   │  ← spinner (first open only)
├──────────────────────────────────────┤
│  You: Why does the low end feel      │
│  muddy on this track?                │
│                                      │
│  Qwen: The track has a prominent     │
│  sub-bass around 60–80 Hz that       │
│  competes with the kick drum…        │
│                                      │
│  You: How would you fix that?        │
│                                      │
│  Qwen: ▌  (streaming)               │
│                                      │
├──────────────────────────────────────┤
│  [Ask something about this track…]   │  ← textarea + Enter to send
└──────────────────────────────────────┘
```

The WaveSurfer instance is mounted when the Chat tab opens and destroyed (main PlayerBar restored) when leaving. For tracks under ~5 minutes the full waveform is shown with no region constraint. For longer tracks a draggable region selector appears (WaveSurfer regions plugin) defaulting to the first 3 minutes; the selected region is what gets sent with each question. The chat area scrolls independently. The input is disabled while a response is streaming or the model is loading.

---

## Backend: New IPC Command

A single new Tauri command handles the interactive use case:

```
ask_qwen(track_id: i64, question: String, window_start_secs: Option<f64>, window_duration_secs: Option<f64>, history: Vec<(String, String)>) -> Result<String, String>
```

- `window_start_secs` — start of the audio region to send, in seconds. `None` means send the full track.
- `window_duration_secs` — duration of the region in seconds. `None` means full track. Ignored if `window_start_secs` is `None`.
- `history` — prior `(user, assistant)` turn pairs for multi-turn context.

For tracks under ~5 minutes both values are `None` and the full audio is passed; llama.cpp chunks it automatically. For longer tracks the frontend passes the region the user selected in WaveSurfer.

**What it does:**

1. Looks up the track path from the DB by `track_id`.
2. Calls `ensure_llama_server_running` (idempotent — no-op if already running).
3. Reads and decodes the audio file, resamples to 16 kHz mono. If a window is specified, slices to that region; otherwise passes the full audio.
4. Builds the `messages` array: the **first** user turn includes the audio attachment; subsequent turns from `history` are text-only. The new question is appended as the final user message.
5. Sends the chat completion to `llama-server` with no structured output format.
6. Returns the raw text response.

Conversation history is owned by the frontend; the backend is stateless.

### Server Lifecycle

`ensure_llama_server_running` is already written and handles boot, health polling, and the case where the server is already up from a pipeline run. The Chat tab just calls it on mount (when the tab becomes visible) and shows a spinner until `GET /health` returns 200.

The server stays running until the app quits or a pipeline run's teardown kills it. There is no dedicated teardown for the Chat tab — the server is cheap to leave running once loaded.

---

## Frontend State

A lightweight Svelte store (or local `$state` in `TrackDetailPane`) tracks:

```ts
type Message = { role: 'user' | 'assistant'; content: string };

let messages = $state<Message[]>([]);       // loaded from DB on tab open, cleared on track change
let streaming = $state(false);
let modelReady = $state(false);             // true once llama-server /health returns 200
let windowStart = $state<number | null>(null);    // null = full track; set when user drags region
let windowDuration = $state<number | null>(null); // null = full track
```

When `player.selectedTrack` changes, `messages` is cleared (or reloaded from DB if persistence is implemented) and `modelReady` is re-checked — the server may still be up from a previous session.

### Streaming

`llama-server` supports SSE streaming via `stream: true` in the completions payload. The Tauri command can either:

- **Stream via Tauri events** — emit `feedback_token` events from Rust as tokens arrive; the frontend appends them. This gives real-time streaming but requires a small event-emitting loop in Rust.
- **Return full response** — simpler, no streaming; user sees the complete answer appear at once after a few seconds.

For v1, returning the full response is sufficient. Streaming is a follow-up.

---

## DSP Reuse

The audio decoding and windowing logic already exists in `qwen.rs` (`process_job`). It should be extracted into a shared helper in `dsp.rs` or a new `qwen_audio.rs` so both the pipeline pass and the interactive command can call it without duplication.

---

## Implementation Checklist

**Backend**
- [ ] Extract audio-window helper from `qwen.rs::process_job` into a shared function (accepting position, not always midpoint)
- [ ] Add `ask_qwen(track_id, question, window_secs, history)` IPC command in a new `src-tauri/src/commands/chat.rs`
- [ ] Register the command in `lib.rs`
- [ ] Add `chat_history` table migration `(id, track_id, role, content, created_at)` (low priority)
- [ ] Add `get_chat_history(track_id)` and `save_chat_message(track_id, role, content)` IPC commands (low priority)

**Frontend**
- [ ] Add a tab strip to `TrackDetailPane.svelte` (Info / Chat)
- [ ] Mount a WaveSurfer instance on Chat tab open; destroy it and restore main PlayerBar on tab close
- [ ] "Use this section" button that captures playhead position into `windowSecs`
- [ ] Boot spinner: call `ask_qwen` (or a dedicated `ensure_llama_ready` command) on tab open; show spinner until ready
- [ ] Chat history display with user/assistant bubbles and auto-scroll
- [ ] Input textarea: Enter to send, disabled while streaming or model loading
- [ ] Wire `selectedTrack` change to reload/clear history
- [ ] Error state if server fails to boot

---

## Decisions

1. **Multi-turn context** — yes, text-only. Prior turns are included in the `messages` array sent to `llama-server` on each request (user/assistant alternating). The audio attachment is only sent once — on the first message of the session. Subsequent turns are text-only, which keeps payloads small and avoids re-encoding the audio for every question. The backend command signature becomes `ask_qwen(track_id, question, history)` where `history` is the prior turns.

2. **Which 30 seconds?** — for most tracks, the full audio is passed and llama.cpp chunks it automatically (see `qwen_limitations.md` for token budget details). The Chat tab includes a **WaveSurfer player** replacing (or hiding) the main PlayerBar while the tab is active. For tracks longer than ~5 minutes (where the full audio would exhaust the 8,192-token context), the user can drag a region selector to pick which section to analyse; the window duration is configurable and defaults to 3 minutes. The main PlayerBar resumes when the user leaves the Chat tab.

3. **Persisting conversations** — conversations are saved to the DB per track (low priority, implement after core chat works). A new `chat_history` table stores `(track_id, role, content, created_at)`. On opening the Chat tab the prior conversation is reloaded, and the user can clear it with a button.

4. **Server boot trigger** — llama-server is started **on tab click only**, never eagerly. Given its RAM footprint this is the correct default. A spinner is shown while it loads; the input is locked until `GET /health` returns 200. The server is left running for the app session once started — no teardown on tab close, since reloading weights is expensive.

---

## Cross-References

- `src-tauri/src/analysis/qwen.rs` — existing pipeline pass; audio windowing logic to extract
- `src-tauri/src/llama.rs` — server lifecycle management (`ensure_llama_server_running`, `terminate_llama_server`)
- `src/lib/components/TrackDetailPane.svelte` — UI entry point
- `tools/feedback.sh` — CLI prototype that validated the concept
