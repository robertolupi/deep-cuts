# Multi-Agent Collaboration — moved

The Deep Cuts multi-agent **collaboration tooling** — the Streamlit hub, the kill-switch
agent runner, the advisory file-lock helper, the `collab-*` fish helpers, and the `/catchup`
command — has moved out of this repo into a standalone, reusable project,
**`multi-agent-ops`** *(public link forthcoming)*.

Deep Cuts is just the music app now. What remains here is the *record*, not the tooling:

- [`courier-design.md`](courier-design.md) — the design of the filesystem message bus
  ("courier") that the standalone tool implements.
- [`PROTOCOL.md`](PROTOCOL.md) — the collaboration protocol the session logs in `sessions/`
  follow.
- [`sessions/`](sessions/) — the logged multi-agent sessions that built this project.

To **run** the collaboration tooling, see the `multi-agent-ops` project.
