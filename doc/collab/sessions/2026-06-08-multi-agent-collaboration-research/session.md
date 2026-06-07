# Session: Multi-Agent Collaboration and Consensus Model Research

## Participants
* Roberto (Human)
* Gemini (AI)

---

## [Roberto, 00:15]

Claude lamented during the last session that there was contention on the `session.md` log during collaboration, making it unable to write successfully. 

I want to do a deep research session on a more robust, local-first collaboration model between SoTA AI agents to avoid write contention and turn multi-agent coordination into a shared asset (consensus, peer review, voting, amendments).

Gemini, please formulate a detailed prompt/brief for Google DeepThink that references our repository files directly using GitHub URLs, so it can read them and provide a complete design.

---

## [Gemini, 00:16]

Here is the research brief and query prompt designed for Google DeepThink, pointing directly to the relevant files in the [`robertolupi/deep-cuts`](https://github.com/robertolupi/deep-cuts) repository.

***

# Prompt for Google DeepThink: Local-First, Multi-Agent Collaboration and Consensus Protocol

## 1. Context & Architecture
We are developing a local-first workspace where a human developer and multiple heterogenous, state-of-the-art (SoTA) AI coding/advisory agents (such as Claude, Gemini, and OpenAI Codex) collaborate on a shared Git repository ([`robertolupi/deep-cuts`](https://github.com/robertolupi/deep-cuts)).

Our current multi-agent collaboration and communication transport is a daemonless, filesystem-based, maildir-style asynchronous message bus (referred to as the "Collab Courier"):
* **Design Doc**: See [courier-design.md](https://github.com/robertolupi/deep-cuts/blob/main/doc/collab/courier-design.md)
* **The Spool**: Lives outside the Git repository (transient) to avoid commit noise. It maintains per-actor directories (`new/`, `cur/`) and doorbell named pipes (FIFOs) or directory watchers.
* **Delivery**: Sending a message is an atomic `write` to a temporary directory followed by an OS-level `rename()` to the recipient's `new/` inbox. Waking up an agent is event-driven via a blocking read on its doorbell.
* **The Git Record**: The only thing committed to Git is the unified session log (`session.md`) representing the chronological conversation, plus any attachments (e.g., code snippets, CSVs, charts). See [PROTOCOL.md](https://github.com/robertolupi/deep-cuts/blob/main/doc/collab/PROTOCOL.md).

For simple serial turn-taking, we also have a Unix named-pipe FIFO baton protocol (where one actor blocks on `cat` while the other works, then passes a `NEXT` token).
* **Baton Protocol**: See [fifo-handoff-design.md](https://github.com/robertolupi/deep-cuts/blob/main/doc/collab/fifo-handoff-design.md).

## 2. The Failure Mode: Write Contention on the Session Log
When multiple agents run in parallel or participate in concurrent brainstorming, they must log their turns, ACKs, or results in the shared `session.md` file. Because agent execution is concurrent, they suffer from **write-write races**:
* Multiple agents try to read, modify, and write the same `session.md` file concurrently.
* File-write tools error out with "file has been modified since read" or silently overwrite each other's turns, leading to out-of-order logs, lost turns, or corrupted Markdown.
* An advisory locking file mechanism (`session.md.lock`) either introduces severe sequential blocking (defeating parallel execution) or is bypassed by client tool failures.
* **Real-World Contention Example**: See [sessions/2026-06-07-salami-eval-followup/session.md](https://github.com/robertolupi/deep-cuts/blob/main/doc/collab/sessions/2026-06-07-salami-eval-followup/session.md) for a log where Claude, Gemini/agy, and Codex competed for writes, resulting in out-of-order back-filling.

We want to move away from a simple "baton-passing" sequential lock and design a coordination protocol that **turns concurrent collaboration into an asset** (leveraging multi-agent consensus, peer review, voting, and amendments).

## 3. Design Constraints
* **Local-First & Daemonless**: No standing external databases (e.g., PostgreSQL, Redis) or message brokers. Everything must run on the local filesystem using files, standard system calls, lightweight IPC (FIFOs, watchers), or SQLite (WAL).
* **Sandbox-Safe**: Agents are workspace-bound and cannot run arbitrary background network daemons.
* **Reviewable Output**: The final output of the session must be a clean, readable, linear Markdown log (`session.md`) committed to the Git repository.

## 4. Candidate Paradigms to Research & Synthesize
Please research and analyze how to structure a protocol covering the following ideas, evaluating their tradeoffs in complexity, token consumption, latency, and reliability:

### A. Atomic Broadcast and Spool-and-Reassemble
* Instead of writing directly to `session.md`, agents write their turns as individual JSON/Markdown files to their own spool directories (e.g. `turns/<actor>_timestamp.json`).
* A host-side coordinator or a post-commit hook aggregates and reassembles them into the linear `session.md` ordered by timestamp.
* **Question**: How do we handle state broadcasting? If Agent A appends a turn, how do we efficiently notify and update the local context of Agent B and Agent C so they are not working off stale assumptions?

### B. Scribe Models & Rotating Batons
* **Dedicated Scribe**: A lightweight, single-writer agent acts as the sole editor of the shared `session.md`. Other agents send structured messages to the Scribe, which acts as the thread compiler.
* **Rotating Scribe**: The privilege of writing to `session.md` is tied to a "baton." The agent holding the baton compiles the current turn and passes it. 
* **Question**: How do we design an election/rotation protocol that runs locally and handles crashes (e.g. an agent crashes while holding the baton or while processing a turn)?

### C. Consensus-Driven Amendments and Voting
* Instead of letting agents freely append to the roadmap or design docs, we treat shared artifacts as "proposals" subject to **Amendments** and **Voting**.
* If Agent A wants to modify a design doc, it writes a structured "Amendment Proposal".
* Peer agents evaluate the amendment and vote (using a consensus protocol, such as Condorcet voting or simple majority).
* Once consensus is reached, the amendment is applied to the master document.
* **Question**: How can we design a lightweight, file-based voting and amendment protocol that works asynchronously and without a central coordinator server?

### D. Lightweight CRDTs or Git-Native Merge Loops
* Can conflict-free replicated data types (CRDTs) or native Git branches/merge-conflict resolvers be simplified so that LLM agents can resolve document conflicts programmatically?

## 5. Requested Deliverables
1. **Architectural Evaluation Matrix**: Compare the 4 paradigms (Atomic Spooling, Dedicated Scribe, Rotating Scribe, Voting/Amendments) across Complexity, Latency, Token Cost, Resilience, and Consensus Quality.
2. **The Unified Synthesis**: Propose a unified protocol design. Can we use a Maildir/spool-and-reassemble model for the *conversational transcript* (append-only log), while using a Scribe-coordinated Amendment/Voting protocol for the *durable design documents* (mutable plans)?
3. **Envelope and Schema Specifications**: Provide the JSON message schemas for envelopes (`id`, `from`, `to`, `type`, `payload`) matching these operations:
   * `proposal`: proposing a change or a turn.
   * `vote`: voting on a proposal.
   * `ack`: acknowledging receipt/consensus.
   * `commit`: applying an approved proposal.
4. **State Machine Diagram**: Map out the life-cycle of a parallel multi-agent session (e.g. IDLE, WORKING, PROPOSING, VOTING, RECONCILING).
5. **FastMCP Integration Blueprint**: Explain how to extend our current Python-based MCP server (see [claude-mcp-adapter.md](https://github.com/robertolupi/deep-cuts/blob/main/doc/collab/claude-mcp-adapter.md) and [coordination-protocol.md](https://github.com/robertolupi/deep-cuts/blob/main/doc/collab/coordination-protocol.md)) to natively support these consensus and serialization operations.
