---
status: proposed
owner: Roberto
last_verified: 2026-06-07
implemented_by:
superseded_by:
related_code: tools/collab_agent.py, tools/collab_hub.py, tools/file_lock.py
related_skills: bot-collab
---

# The Collab Courier — a filesystem message bus for human + AI collaboration

> **Status: proposed (design agreed, not yet built).** A small program that lets a human
> (Roberto) and two or more AI agents (Claude, `agy`) hold a real, multi-party conversation —
> with datasets and images attached — over a plain filesystem, with no server, no broker, and a
> clean, GitHub-reviewable transcript as the only thing that ever gets committed.

## TL;DR

Three peers — **Roberto, Claude, `agy`** — exchange messages addressed with **To / CC / From**,
just like email. A single script, the **courier**, routes those messages through a **spool that
lives outside the git repo** (transient), **pings** each agent with the new content (so warm
agents never re-read the whole session), and writes a **clean Markdown transcript + attachment
files into the repo** (durable, reviewable on GitHub). No mail server, no IMAP, no standing UI.

```
                 spool/  (OUTSIDE the repo — transient: per-peer inboxes + doorbell FIFOs)
   Roberto ──┐                                                  ┌── reads inbox / transcript directly,
   Claude  ──┼──►   courier   (routes by To/CC)  ──► pings ────►┤    sends via `courier send`
   agy     ──┘             └──► writes session.md + attachments/ (IN the repo — clean Markdown)
```

## Problem & constraints

We run multi-agent sessions (see [`PROTOCOL.md`](PROTOCOL.md)). Relaying every message by hand is
slow and burns tokens. A shared bus has to satisfy five hard constraints:

1. **Sandboxed agents.** `agy` is workspace-locked and may not open network sockets. The bus must
   work over the **filesystem only** — no broker, no server the agents must reach.
2. **Token efficiency.** A cold `claude -p` re-reads `PROTOCOL.md` + the session every turn
   (~$0.70). A warm, long-lived session that is *handed the new message* pays only for that
   message. The bus must support **warm, incremental delivery**.
3. **Multi-party.** Not hub-and-spoke through one agent. Every peer can address every other; an
   agent can **ask the human a question** and the human answers, and vice-versa.
4. **GitHub-reviewable record.** Reviewers read the repo on GitHub, which renders **Markdown, not
   `.eml`**. The committed artifact must be a clean Markdown transcript plus ordinary attachment
   files.
5. **No standing infrastructure.** No daemon to babysit, nothing to leave running, and a hard
   **kill-switch** for runaway agents (already built: [`tools/collab_agent.py`](../../tools/collab_agent.py)).

## Design at a glance

Three ideas do all the work:

- **An outside-repo spool** carries the live, transient traffic (one little file per message, plus
  read-state and a wakeup FIFO per agent). Keeping it *outside* git means the repo never fills with
  mail noise.
- **A clean-Markdown transcript** is the only thing committed. The courier appends a readable block
  per message to `session.md` and drops attachments into `attachments/`. That is the GitHub artifact.
- **A doorbell ping carries the message content.** Agents block on a FIFO (a free kernel wait — no
  polling, no tokens) and are woken with the new message already in hand, so they never re-read the
  session. This is what makes warm agents cheap.

## Why this shape — the road we didn't take

This design is the survivor of a fast, honest brainstorm. The rejected branches are instructive:

| We considered… | Why we dropped it |
|---|---|
| **Streamlit hub + in-repo JSONL** (built first) | A custom UI and renderer we'd have to maintain; the JSONL clutters git; cold per-click invocations are expensive. |
| **A real message broker** (Redis / NATS / RabbitMQ / MQTT) | A daemon + network sockets the **sandbox blocks** — it reintroduces exactly the problem the filesystem bus solves. |
| **A real mail server** (postfix / dovecot + maildir + Apple Mail over IMAP) | Avoids the broker's network-to-the-agents problem but is a brittle, macOS-specific sysadmin project, and **`.eml` does not render on GitHub** — so it fails the review constraint. |
| **MIME `.eml` as the committed record** | Same: great for a mail client, unreadable as a repo artifact. |
| **A standing dashboard UI at all** | Unnecessary. The human reads the transcript directly and asks the agents to talk (`/cc agy …`); a *throwaway* Streamlit view can be hacked up on demand when a dataset or image needs eyes. |

Around the maildir step we joked that "we're rebuilding unix mail." That was the signal the design
was *right* (convergent evolution onto a 40-year-old solution) — and also the cue to **stop before
the daemon**. We keep mail's good ideas (To/CC/From addressing, a durable spool, ack-by-move) and
drop its infrastructure. The line is: **real format and concepts, no server.**

## Participants & addressing

Every participant is a first-class peer with an inbox:

| Peer | Reads | Writes | Wakeup |
|---|---|---|---|
| **Roberto** | transcript / `courier inbox roberto` | `courier send` (or asks Claude to) | a light notification |
| **Claude** | doorbell ping (content) | `courier send` | blocking FIFO read |
| **`agy`** | doorbell ping (content) | `courier send` | blocking FIFO read |
| **Meta** *(optional)* | manual (can't write files) | Roberto pastes on its behalf | — |

Messages carry `From`, `To`, and `CC`. The courier delivers a copy to each recipient's inbox and
pings the agent recipients. `agy → To: roberto, CC: claude "which split should I use?"` lands in
Roberto's inbox **and** Claude's; Roberto replies `From: roberto, To: agy`. No one sits in the middle.

## Components

### 1. The spool (outside the repo, transient)

```
$COLLAB_SPOOL/<session>/         # e.g. ~/.deep-cuts-collab/<session>/  (gitignored location, outside repo)
  <peer>/new/   <peer>/cur/      # maildir-style: new = unread, move to cur = read/ACK
  <peer>/doorbell                # FIFO; pinged when a message lands for <peer>
```

Maildir semantics give durability and reliability for free: a message file sits in `new/` until the
recipient processes it and **moves it to `cur/` (that move *is* the ACK)**. Crash mid-process? The
file is still in `new/` → redelivered. The "offset" is implicit (what remains in `new/`). Built on
Python's stdlib `mailbox` + `email`, so To/CC/From/threading/MIME-attachments come from the library,
not from us.

### 2. The courier program — `tools/collab_courier.py`

```
courier send --from X --to Y[,Z] [--cc …] [--subject S] (--body … | --body-file f) [--attach file …]
courier inbox <peer>           # list unread
courier read <id>              # show one message
courier loop <agent>           # block on doorbell → hand over new content → (agent replies) → re-block
```

`send` does three things atomically-ish: writes the MIME message into each recipient's `new/`, pings
their doorbells, **and** appends a clean Markdown block to the repo's `session.md` + copies any
attachments into the repo's `attachments/`.

### 3. The committed record (in the repo)

The only thing git ever sees is the human-readable transcript:

```markdown
## [agy → roberto, cc claude · 14:02]
Which validation split should I use for the SSM sweep?
📎 attachment: `attachments/split-candidates.csv`
```

Plus `attachments/` (CSV, PNG, …). Reviewers on GitHub read this like any other doc. The transient
maildir never enters git.

### 4. The doorbell / warm loop

An agent runs `courier loop`: it **blocks** reading its `doorbell` FIFO (a free wait — verified: a
foreground blocking read costs nothing until a write arrives, then wakes once), gets the new message
content, handles it with **warm context** (no re-reading `PROTOCOL.md`/`session.md`), replies via
`courier send`, moves the processed file to `cur/`, and re-blocks. `__STOP__` ends the loop. The
human is woken by a lighter notification (or simply reads the transcript) and replies on their own clock.

## Reliability & safety

- **Delivery:** maildir `new/ → cur/` = durable spool + ACK + crash redelivery, standardized.
- **No runaway loops:** an agent handles one delivered message and re-blocks; it does not auto-invoke
  a peer. The existing kill-switch ([`tools/collab_agent.py kill`](../../tools/collab_agent.py))
  SIGKILLs every running agent's process group; agents stay constrained (Claude: no general `Bash`;
  `agy`: `--sandbox`).
- **Shared-file edits:** the repo `session.md` is append-only here; for any concurrent mutable-file
  edit the advisory lock still applies ([`tools/file_lock.py`](../../tools/file_lock.py),
  [`PROTOCOL.md`](PROTOCOL.md)).
- **Locality:** everything is local files; nothing leaves the machine.

## What this deliberately is *not*

- **Not a mail server.** No SMTP/IMAP daemon, no Apple Mail account, no ports.
- **Not a broker.** No Redis/NATS/socket.
- **Not a standing UI.** The transcript is the default surface; ad-hoc Streamlit only when a dataset
  or image needs rendering.
- **Not `.eml` in git.** Mail format on the wire; clean Markdown in the repo.

The minimalism is the point — it is the smallest thing that satisfies all five constraints.

## Relationship to what exists

- **Supersedes** the human-reading role of the Streamlit hub ([`tools/collab_hub.py`](../../tools/collab_hub.py));
  the hub may be retired or kept as an occasional throwaway viewer.
- **Keeps** `tools/collab_agent.py` (kill-switch, constrained headless runs) and
  `tools/file_lock.py` (advisory locking).
- **Extends** the [`PROTOCOL.md`](PROTOCOL.md) conventions (ARCHIVED tombstones, locking, ACK
  logging) — the courier is the transport; the protocol is the etiquette.

## Open questions

1. **The make-or-break: can `agy` stay warm?** The symmetric warm-loop assumes `agy` (the app, not
   one-shot `--print`) can block on the doorbell and loop within `--sandbox --add-dir`. If it can't,
   `agy` runs cold-but-routed (re-reads per message) while Claude stays warm — the design still works,
   just asymmetrically. *Owner: Gemini to confirm.*
2. **The clean-log generator.** Roberto plans to write a nicer transcript renderer later; the first
   cut keeps it minimal (append a Markdown block per message).
3. **Human notification.** Start with a terminal print / bell; a macOS notification is a later nicety.
4. **Meta's lane.** Meta can't write files; Roberto pastes its messages into the spool on its behalf.

## Decision log

- **2026-06-07** — Chose filesystem-as-bus over a broker/server (sandbox constraint). *(Gemini, Claude)*
- **2026-06-07** — Adopted mail *format + concepts* (maildir, To/CC, ack-by-move) via Python stdlib;
  rejected running an actual mail daemon. *(Roberto, Claude)*
- **2026-06-07** — Committed record is clean Markdown + attachments, **not** `.eml` (GitHub
  rendering). Spool moved *outside* the repo. *(Roberto)*
- **2026-06-07** — No standing UI; human reads the transcript and drives via the agents; throwaway
  Streamlit on demand. N-way peer addressing (everyone can ask/answer everyone). *(Roberto)*
