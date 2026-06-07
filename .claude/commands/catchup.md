---
description: Catch up on the active collab session (chat_log.jsonl + session.md) and respond (pass 'gemini' to invoke Gemini)
---
Catch up on the active multi-agent collaboration session.

If $ARGUMENTS contains "gemini" or "agy":
1. Find the most recently modified **non-archived** session directory under `doc/collab/sessions/` (excluding directories containing an `ARCHIVED` file).
2. Run Gemini headlessly **through the kill-switch wrapper** (never the raw `agy` CLI — the
   wrapper enforces `--sandbox` and records a pidfile so `python tools/collab_agent.py kill`
   can stop it):
   ```bash
   python tools/collab_agent.py run agy --session <active_session_name>
   ```
   (The wrapper supplies the bootstrap "read protocol, append one reply" prompt; pass a custom
   prompt as the next argument if needed.)
3. Verify that the command completed successfully and a new message was written to `chat_log.jsonl`.

Otherwise (default / Claude mode):
1. Find the most recently modified **non-archived** session directory under `doc/collab/sessions/` (excluding directories containing an `ARCHIVED` file).
2. Read its `chat_log.jsonl` (the live chat) and the tail of `session.md` (the curated record).
3. Tell me what's new since the last Claude turn, then respond to the latest message(s) directed at me.
4. If I ask you to reply *in the chat*: acquire the advisory lock first (`python tools/file_lock.py acquire <path> --owner claude`), append exactly one JSON line to `chat_log.jsonl` as `{"timestamp": <ISO8601 UTC>, "sender": "Claude", "type": "markdown", "content": <reply>}`, then release the lock. Do not invoke other agents.

Follow `doc/collab/PROTOCOL.md`.

