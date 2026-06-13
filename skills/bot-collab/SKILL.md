---
name: bot-collab
description: Pattern for multi-agent collaboration sessions in the deep-cuts fam — IRC-first coordination over the botfam substrate (channels, nicks, FIFO line protocol, wake loop), plus session-log conventions
---

# Multi-Agent Collaboration Skill

Use this skill as the launcher for structured collaborative sessions between Roberto and multiple
agentic coding assistants in the **deep-cuts fam**.

The canonical protocol lives in the **botfam repo**:
`~/src/fams/botfam/main/doc/collab/PROTOCOL.md` (coordination rules) and
`~/src/fams/botfam/main/doc/collab/IRC-OPS.md` (server ops, credentials, recovery, client/wake
recipes). If this skill and the protocol disagree, follow the protocol and update this skill later.

> **Coordination vs. quality ratchet — two layers.** The IRC substrate (this skill) is the
> *coordination transport*. When the goal is not just to coordinate but to make a specific artifact
> *provably better* — evaluate a change, gather admissible peer critiques, and merge only when the
> consensus gate passes — use **[CCREP](../ccrep/SKILL.md)**; its proposals and votes run as bang
> commands on `#dc-ccrep`. They compose: coordinate here, ratchet there.

## The IRC substrate (first choice)

- **Server**: ergo at `localhost:6667` (Docker-hosted; IRC is down whenever Docker Desktop is
  down — see IRC-OPS §1).
- **Identity**: per-fam IRC nick is `<actor>-dc` (`claude-dc`, `agy-dc`); the per-worktree git
  `user.name` stays the bare actor name (`claude`, `agy`). NickServ password file:
  `~/.botfam/irc-pass-dc-<actor>` (mode 600). **Never keep credentials in `scratch/`** — it is
  treated as `/tmp` (IRC-OPS §2).
- **Actor/nick mismatch**: the client's runtime dir is keyed by nick (`scratch/irc/<nick>/`). If
  tooling looks it up by actor name, symlink `scratch/irc/<actor>` → `scratch/irc/<nick>` rather
  than changing identity configs.
- **Channels** derive from `slug = "dc"` in the fam.toml: `#dc` (discussion) and `#dc-ccrep`
  (proposals & votes). Older notes saying `#deep-cuts-ccrep` are stale. Cross-fam channels (e.g.
  `#party`) are joined on request.
- **Client** (background task; use the absolute path, `~/bin` may not be on PATH):
  `~/bin/botfam irc-client <nick> --pass-file ~/.botfam/irc-pass-dc-<actor>`.
  The client does **not** auto-reconnect — restart it after any server downtime.
- **Sending — FIFO line protocol** (`scratch/irc/<nick>/in`):
  - a bare line is sent as text to the main channel;
  - `/msg <target> <text>` targets another channel or nick;
  - `/raw <IRC command>` for anything else (e.g. `/raw JOIN #party`).
  Messages over 400 bytes auto-split. **Sends can fail silently** (e.g. `broken pipe` after a
  connection drop) — verify your message echoed into `scratch/irc/<nick>/log` before assuming
  delivery.
- **Wake loop**: run `~/bin/botfam irc-wait --nick <nick>` as a background task and **re-arm it
  after every wake** — an unarmed watcher is the number-one cause of silently unresponsive agents
  (IRC-OPS §3).
- **Replay-on-join**: scribe-dc logs all channel events to
  `~/src/fams/deep-cuts/dc-collab/history.jsonl` — read it before acting so you never reply to a
  stale conversation. On-channel, `!tally id=<proposal_id>` asks the scribe for a consensus tally.
- **MCP adapters**: when the botfam MCP tools (`irc_write`/`irc_read`/`irc_wait`) are registered in
  your harness, prefer them — they are thin wrappers over the same FIFO/log contract. The FIFO and
  log file are the canonical interfaces and the always-available fallback.

## Worktree Topology

The fam uses the unified multi-fam layout: `/Users/rlupi/src/fams/deep-cuts/main` is the shared
merge target; each agent works in its own worktree (`wt-claude`, `wt-agy`, …) on an
`agent/<actor>` branch.

- Worktree branches are for deliverables; other actors' worktrees are **read-only** (PROTOCOL.md
  §4) — to update one, message the owner on `#dc`.
- Live coordination happens on IRC, not via committed files. Use CCREP/git commits only when
  reviewing or integrating a concrete deliverable.

## Legacy transports (retiring)

The Python `collab` MCP (UDS mailboxes and task queue under `scratch/coordination/`) and the FIFO
baton ([collab](../collab/SKILL.md)) are the pre-IRC coordination layer. Per the migration note,
they retire after the first successful ccrep merge over IRC. Reach for them only if the IRC
substrate is down and the work cannot wait — and say so on the session record.

## Startup Checklist

When the user mentions a multi-agent collaboration session or asks you to join the fam:

1. Read the replay (`~/src/fams/deep-cuts/dc-collab/history.jsonl`) before acting.
2. Launch the IRC client and the `irc-wait` watcher as background tasks (commands above).
3. Announce yourself on `#dc`; verify the join and your message echo in `scratch/irc/<nick>/log`.
4. Find or create the session directory under `doc/collab/sessions/YYYY-MM-DD-topic-slug/` when the
   work needs a durable session log; read the full `session.md`, not only the tail.
5. After every wake: read the new log lines, act, re-arm `irc-wait`.

## Session Files

New sessions use this path shape:

```text
doc/collab/sessions/YYYY-MM-DD-topic-slug/session.md
```

Session logs are working records. Durable decisions must be promoted into normal `doc/` files,
`skills/` files, or code comments. The IRC history is the live record; the session log is the
curated one.

---

## Handoff Format

End collaborative turns with the structured handoff required by `PROTOCOL.md`:

```markdown
**→ Handoff:**
**Task:** [what to do]
**Context:** [files, data, or prior decisions needed]
**Deliverable:** [expected artifact]
```

On IRC, post the same Task/Context/Deliverable triple to `#dc` (auto-split handles length).

## Documenting Roberto's Contributions

When Roberto makes a direct contribution (e.g., providing design direction, running commands,
writing code, or giving explicit feedback that is not just relaying an agent's handoff), the active
agent must document it in the session log:
* Insert a `## [Roberto, HH:MM]` block outlining his feedback, code changes, or decisions, OR
* Explicitly credit and outline his steering decisions within the agent's own turn.
This keeps the session log as the single source of truth for the collaborative path.

## Recording Acknowledgements (ACKs)

Agreement is signal worth keeping, not just handoffs and disagreements. When you endorse, confirm,
verify, or accept another participant's work, record it in the session log — not only on IRC:
* A short ACK line inside your turn ("ACK: verified Gemini's mir_eval numbers, consistent on re-run"), OR
* A one-line `## [X, HH:MM]` block when relaying someone else's ACK.

This makes consensus — and who reached it — part of the durable record.
