# Multi-Agent Collaboration — Human Manual

A practical guide for **Roberto** on running the multi-agent collaboration setup: the Collab
Hub, the agents (Claude + `agy`), the safety/kill-switch, and the conventions. This is the
"how do I actually use this" doc; `PROTOCOL.md` is the rules the agents follow.

---

## TL;DR

```fish
collab-hub      # launch the dashboard (chat + invoke agents + kill switch)
collab-kill     # 🛑 PANIC BUTTON — stop every running agent immediately
```

In the hub: type to Roberto's chat, click **Invoke Claude** (one constrained turn), watch the
reply land. If anything runs away, hit **🛑 Kill all agents** (or `collab-kill`).

---

## One-time setup (fish aliases)

Run each once; `alias --save` persists them to `~/.config/fish/functions/`:

```fish
alias --save collab-hub    'env CLAUDE_BIN=$HOME/.local/bin/claude /Users/rlupi/src/deep-cuts/tools/.venv/bin/streamlit run /Users/rlupi/src/deep-cuts/tools/collab_hub.py'
alias --save collab-kill   'python3 /Users/rlupi/src/deep-cuts/tools/collab_agent.py kill'
alias --save collab-claude 'python3 /Users/rlupi/src/deep-cuts/tools/collab_agent.py run claude'
alias --save collab-agy    'python3 /Users/rlupi/src/deep-cuts/tools/collab_agent.py run agy'
```

(No `cd` needed — the tools find the repo and active session from their own path.)

Binaries it expects: `claude` at `~/.local/bin/claude`, `agy` at `~/.local/bin/agy`. Override
with the `CLAUDE_BIN` / `AGY_BIN` env vars if they live elsewhere.

> **`agy`, not `gemini`.** `agy` is the current Gemini CLI (Antigravity). The old `gemini` CLI is
> deprecated — don't use it. In the chat, `agy` still posts under the participant handle **Gemini**.

---

## The pieces

| Thing | What it is |
|---|---|
| `tools/collab_hub.py` | Streamlit dashboard — chat, invoke agents, kill switch, live tasks |
| `tools/collab_agent.py` | The **only** way to run an agent headlessly — constraints + kill switch |
| `tools/file_lock.py` | Advisory "I own this file" lock for shared files |
| `doc/collab/sessions/<date>-<topic>/` | One folder per topic: `session.md` (curated) + `chat_log.jsonl` (live chat) |
| `doc/collab/tasks.md` | Shared TODO board (rendered live in the hub) |
| `.claude/commands/catchup.md` | The `/catchup` slash command for Claude Code |

---

## The daily loop

1. **Start the hub:** `collab-hub` → opens at http://localhost:8501.
2. **Pick the session** in the sidebar (defaults to the most recent active one).
3. **Chat:** type in the box; messages append to `chat_log.jsonl`.
4. **Get an agent turn:**
   - **From the hub:** click **Invoke Claude** — one constrained turn, reply appears in chat.
   - **From a terminal:** `collab-claude` or `collab-agy` (one turn on the active session).
   - **From Claude Code:** type `/catchup` (Claude reads the session and replies).
5. **`agy`/Meta can't write the hub** — for those, run `collab-agy`, or paste their reply into
   the chat box yourself.
6. **Promote** the keepers: click **Promote new messages** in the hub to append the new chat
   into `session.md` (the durable, curated record). The chat log is ephemeral; `session.md` is
   the audit trail.

---

## 🛑 The kill switch (read this)

Every headless agent runs through `tools/collab_agent.py`, in its own process group with a
pidfile. So you can always stop everything:

```fish
collab-kill                                 # or: python tools/collab_agent.py kill
```

It SIGKILLs every tracked agent **and its children**. There's also a **🛑 Kill all agents**
button in the hub sidebar, and `python tools/collab_agent.py status` to see what's running.

**Why you're safe from a runaway loop, by design:**
- Agents only run via the wrapper — never the raw CLI with `--dangerously-skip-permissions`
  (that flag is banned).
- Claude runs with **Bash disallowed** → it physically cannot shell out to invoke another agent,
  so there's no Claude↔agy ping-pong. `agy` runs `--sandbox`.
- **One click = one turn = stop.** Nothing auto-advances the turn; you are the clock.
- Hard wall-clock timeout (900s) per turn, plus the kill switch.

> ⚠️ Don't bypass the wrapper. Running `claude -p` or `agy -p` directly (e.g. the old
> `claude-catchup` fish snippets) skips all of the above — no constraints, no timeout, nothing the
> kill switch can stop. Always go through `collab_agent.py` / the `collab-*` aliases.

---

## `/catchup` (from a Claude Code terminal)

Type `/catchup` and Claude finds the active session, reads `chat_log.jsonl` + `session.md`,
tells you what's new, and replies. `/catchup agy` runs an `agy` turn instead (via the wrapper).

Note: a slash command added mid-session won't appear until you start a **fresh** Claude Code
session. The `collab-*` fish aliases work immediately regardless.

---

## Conventions (what the agents follow)

- **Archive a session** so agents stop working on it: drop an empty `ARCHIVED` file in its
  folder (`touch doc/collab/sessions/<id>/ARCHIVED`). Delete it to unarchive. Committed to git,
  so the archive state is shared. The hub never auto-selects archived sessions.
- **File locking:** before editing a *shared* file (`session.md`, `PROTOCOL.md`, …), agents take
  an advisory lock: `python tools/file_lock.py acquire <path> --owner claude` → edit → `release`.
  Append-only logs (`chat_log.jsonl`) skip the lock and use atomic appends instead.
- **Rich messages:** the hub's "📎 Attach artifact" accepts markdown, CSV/JSON (rendered as a
  table via pandas), and images. The file is saved under the session's `attachments/` and
  referenced by path in the chat — small log, real artifacts on disk.

---

## Honest caveats

- **It is not free.** The hub saves *your relay effort and context bloat* — it does **not** make
  agent turns free. Every invoke is a real agent run that costs tokens (a bootstrapped cold turn
  that reads `PROTOCOL.md` + `session.md` ran ~$0.70). Invoke when there's a turn worth taking.
- **There's no "watch for free."** An agent reacting to an update is an inference = tokens. A
  background watcher would spend *more*, not less. Human-paced invocation is the efficient design.

---

## Troubleshooting

| Symptom | Fix |
|---|---|
| `claude` / `agy` not found | `set -x CLAUDE_BIN /path/to/claude` (or `AGY_BIN`) before launching |
| Agent turn errors on a flag | The wrapper fails safe (no action). Check `claude --help`; tell Claude to fix the flag in `collab_agent.py` |
| Hub shows the wrong session | It picks most-recently-modified non-archived; pick explicitly in the sidebar, or archive the stale one |
| Something's running away | `collab-kill` (or the 🛑 button). Then `... status` to confirm it's clear |
| Chat not updating | The hub auto-refreshes every 2s; reload the page if needed |
