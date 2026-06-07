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

---

## [Gemini, 00:25]

(Scribed by Gemini.) We successfully received the output and synthesis from Google DeepThink on our query. DeepThink analyzed our local-first environment, concurrency contentions, and filesystem limitations (specifically DrvFS on WSL2 and BTRFS COW), and proposed the **Courier Protocol** as a daemonless, POSIX-primitive-backed consensus and lock-free coordination framework.

Below is the complete DeepThink analysis, architectural matrix, state machines, schema blueprints, and FastMCP implementation:

# Consensus and Concurrency Control in Local-First Multi-Agent Systems: The Courier Protocol for Deep Cuts

## Evolution and Architectural Context of Deep Cuts
Local-first desktop applications represent a fundamental paradigm shift in modern software architecture, prioritizing absolute privacy, offline autonomy, and direct hardware execution over cloud-dependent models. The open-source desktop application Deep Cuts, developed by Roberto Lupi, embodies this design by offering music producers a private environment to scan, index, and analyze their studio libraries. Originally conceived under the prototype moniker `music-intelligence`, the application transitioned from a hybrid framework—which relied on a fragile, external Python/FastAPI backend subprocess—to a fully unified, in-process Rust runtime executing within a Tauri and Svelte 5 application stack.

The core processing pipeline of Deep Cuts executes digital signal processing, ONNX inference, and Contrastive Language-Audio Pretraining (CLAP) semantic embeddings natively on the host machine. High-dimensional vector searches are processed locally using the `sqlite-vec` extension, and spatial library layouts are generated using Uniform Manifold Approximation and Projection (UMAP) modified with $p_1\text{--}p_{99}$ percentile clipping to isolate acoustic outliers.

To streamline human-AI pair programming and automate library enrichment, the development lifecycle of Deep Cuts uses multiple autonomous artificial intelligence agents. These agents align their operational boundaries using a persistent `skills/` directory containing structured markdown protocols that govern database migrations, Svelte 5 store structures, and Tauri Inter-Process Communication (IPC) commands. Multi-agent collaboration operates through a centralized markdown log, `doc/collab/session.md`, which serves as a shared, human-readable scratchpad for tracking tasks, recording design decisions, and negotiating development turns.

The current protocol evolved from an architectural debate between Gemini and Claude. Gemini originally proposed a complex, daemon-centric coordination layer consisting of a `session.json` global state tracker, an append-only JSON Lines message bus (`inbox.jsonl`), and dedicated Python helper scripts or an SQLite-based Model Context Protocol (MCP) server to serialize filesystem access. Claude argued for a simpler approach: removing the database layer and using direct, markdown-native files that both models read and write natively without an intermediate parser.

While the markdown-centric model succeeded in reducing transport overhead, early implementations suffered from a critical failure mode: silent filesystem writes. During an initial session, Gemini attempted a write operation to a non-existent `doc/collab/` directory; the write failed silently due to sandboxing and OS-level path restrictions, yet the model reported a successful operation and confabulated the existence of the file. Claude detected the missing directory and introduced a verification rule into the protocol: any agent claiming to have committed a file must programmatically verify the write on disk and provide cryptographic proof before handing off the execution baton.

Currently, the collaboration roster features four active handles: Roberto Lupi, Claude (governing low-level implementation), Gemini (focused on architectural research), and Meta (leveraging quick data experiments and web context), orchestrating their activities through a shared, sequential git history.

## Concurrency Contention and Theoretical Coordination Foundations
When multiple autonomous agents operate in a local-first, daemonless repository, they act as independent, asynchronous processes executing without a global coordinator. This environment introduces classic distributed concurrency challenges, most notably write contention, deadlocks, and livelocks, when multiple agents attempt to access or modify `session.md` simultaneously.

To prevent these anomalies, the Courier Protocol adapts classical distributed consensus and concurrency theories, specifically Dijkstra’s Dining Philosophers problem, Chandy-Misra distributed token-passing, and Raft-inspired replicated state machines.

In the Dining Philosophers paradigm, adjacent philosophers (agents) must acquire two shared chopsticks (local files, such as `session.md` and a metadata index) to eat (execute a write operation). If every agent grabs its left file first and waits indefinitely for its right file, the system deadlocks due to circular wait. The four necessary Coffman conditions for deadlock—mutual exclusion, resource holding, non-preemption, and circular wait—are highly prevalent in multi-agent environments.

The Courier Protocol resolves this by establishing a strict, global lexicographical resource-ordering heuristic. All writeable resources are assigned a sequence based on their repository-relative path, and agents must lock resources in ascending lexicographical order.

For dynamic token management, Courier incorporates the Chandy-Misra distributed token-passing algorithm, which eliminates deadlock and starvation by assigning a state (clean or dirty) to each resource token. When agents contend for a resource, tokens are transferred based on request priorities and token cleanliness, ensuring that no agent is permanently starved of write access.

The table below contrasts the performance and deployment profiles of Chandy-Misra and classical resource-ordering against reinforcement learning (RL) models (such as Proximal Policy Optimization, Soft Actor-Critic, Advantage Actor-Critic, and Deep Q-Networks) across varying agent counts.

| Coordination Methodology | Small Scale ($N=5$) Performance | Medium Scale ($N=40$) Performance | Sample Efficiency | Convergence Stability | Deployment Cost |
|---|---|---|---|---|---|
| **Dijkstra Resource Ordering** | Immediate 0% Deadlock, High Throughput | Instantaneous Execution, Stable Scaling | Infinite (Deterministic) | Absolute (No Training) | Zero (POSIX Library Only) |
| **Chandy-Misra Token-Passing** | Matches RL Fairness (0.90 Equity Metric) | Deterministic Consistency, No Overhead | Infinite (Rule-Based) | Absolute (No Training) | Zero (Protocol Defined) |
| **Proximal Policy Optimization (PPO)** | Cumulative Reward: 750, Fairness: 0.90 | Cumulative Reward: 300, Fairness: 0.85 | Poor (10K+ Episodes) | High (Policy Gradient) | High GPU/Training overhead |
| **Soft Actor-Critic (SAC)** | Cumulative Reward: 700, Fairness: 0.88 | Degraded performance under scaling | Poor (15K+ Episodes) | Superior Convergence | High GPU/Training overhead |
| **Advantage Actor-Critic (A2C)** | Low Reward Ceiling relative to PPO | High Variance, Policy Fluctuation | Very Poor | Moderate | High GPU/Training overhead |

Empirical evaluations from DPBench—a coordination benchmark designed to measure multi-agent LLM systems under resource contention—indicate a sharp performance asymmetry between sequential and simultaneous decision-making. When executing sequentially, four of the six tested frontier models (GPT-5.2, Claude Opus 4.5, Grok 4.1, Gemini 2.5 Flash, Llama 4 Maverick) successfully resolve coordination, achieving a 0.0% point estimate for deadlock with a 95% Wilson upper confidence interval at or below 16.1%. However, when forced to make simultaneous decisions under resource contention, default model prompts result in deadlock rates exceeding 95%.

DPBench isolates three critical protocol-level variables that drive simultaneous coordination failures down to 0%:
1. **Pre-commitment Communication Rounds**: Implementing three rounds of pre-commitment discussion among agents before committing writes drops the deadlock rate of Gemini 2.5 Flash from 90.0% to 0.0%. Conversely, a single round of messaging provides no statistical benefit, yielding an 86.7% deadlock rate compared to the 90.0% baseline.
2. **Prompt-Level Coordination Strategy**: Injecting explicit resource-ordering rules or symmetry-breaking instructions directly into the agent’s system prompt reduces deadlock to 0.0% without requiring runtime training.
3. **Group Size Adjustment**: As the group size $N$ scales from 5 to 10 under a fixed set of coordination prompts, the natural decrease in relative resource density drops deadlock rates from 90.0% to 10.0% for specific agent clusters.

## Filesystem Constraints and Atomic POSIX Primitives
Implementing a local-first consensus protocol without a persistent, memory-mapped daemon requires mapping distributed state transitions directly onto local filesystem operations. However, local filesystems present unique physical and logical constraints, particularly across diverse operating systems and mount points.

SQLite Write-Ahead Logging (WAL) is commonly used to optimize local database performance. By logging changes to a separate `*.db-wal` file, WAL allows concurrent reads to proceed without blocking writes. Transactions commit quickly because changes are appended sequentially, and background checkpoint operations periodically merge WAL pages back into the primary database file. Despite these benefits, WAL mode presents significant limitations in multi-agent local runtimes:
* **WSL2 and DrvFS Incompatibility**: Under Windows Subsystem for Linux (WSL2), accessing projects stored on the native Windows host filesystem (e.g., `/mnt/c/`) uses the DrvFS (9P) protocol. This protocol behaves like a network filesystem, failing to support POSIX shared-memory mapping via `*.db-shm` files. This mismatch prevents concurrent WSL-based CLI agents and Windows-native tools from accessing the database, triggering immediate locking errors.
* **BTRFS Copy-on-Write (COW) Degradation**: On Linux filesystems utilizing BTRFS, the Copy-on-Write metadata mechanism conflicts with SQLite's sequential WAL writes. During checkpointing, this interaction causes worker processes to hang on database locks, triggering `sqlite3.OperationalError: disk I/O error` failures, stale task claims, and gateway crashes.
* **Read-Only Limitations**: If an agent is running in a read-only sandbox, WAL mode cannot initialize because the operating system blocks the creation and truncation of the shared-memory `*.db-shm` file.

To bypass these failure modes, the Courier Protocol enforces strict POSIX atomic primitives. The standard `mkdir` system call is used as the core atomic locking primitive. On POSIX-compliant systems, directory creation is fully atomic and handled as a single, indivisible OS/kernel-level instruction. If two agents attempt to call `mkdir` on `.courier/lock/` simultaneously, the operating system kernel guarantees that exactly one call succeeds, returning an error (`EEXIST`) to the second writer without risking intermediate states.

To execute safe file updates, Courier uses the atomic rename pattern. Rather than writing directly to `session.md`, which exposes the log to partial writes and corruption in the event of a crash, the writing agent generates a temporary file, `session.md.tmp`, in the same directory. Once the write is complete and verified, the agent executes the `rename` or `renameat2` system call (or Python's `replace()`). This swaps the temporary file with the active log file in a single, atomic operation. This ensures that concurrent reader processes always observe a fully consistent state, viewing either the pre-update or post-update file, but never a partial or corrupted draft.

## Architectural Evaluation Matrix
To determine the optimal synchronization strategy for the `robertolupi/deep-cuts` repository, the Courier Protocol is evaluated against alternative local concurrency models across key technical criteria.

```text
       CONCURRENCY MODEL TRADE-OFFS IN LOCAL-FIRST MULTI-AGENT RUNTIMES
       
  <--- Low Complexity / Low Throughput
              |
              +---> <--- High Reliability / Medium Throughput
              |
   <--- High Performance / Low WSL2/BTRFS Compatibility
```

| Technical Evaluation Dimension | Sequential Baton Handoff | Raft-Lite On-Disk Consensus | SQLite WAL-Backed Sync |
|---|---|---|---|
| **Throughput & Concurrency** | Low; only one agent may execute at any given timestamp. | Medium; serialized through a dynamically elected filesystem leader. | High; supports multiple concurrent readers and serialized writers. |
| **Deadlock Vulnerability** | Negligible; strict sequential passing eliminates race conditions. | Low; mitigated by randomized election and backoff timers. | Moderate; requires configuring a `busy_timeout` to prevent locked states. |
| **Livelock Vulnerability** | Low; token handoff relies on deterministic routing. | Moderate; mitigated by randomized candidate election delays. | Low; write serialization is managed by the OS file-locking layer. |
| **WSL2 / DrvFS Mount Stability** | Perfect; uses standard file operations across the 9P boundary. | Perfect; uses simple directory creation and atomic file renames. | Unstable; shared-memory allocation blocks across host mount points. |
| **BTRFS COW Performance** | Perfect; minimal write operations avoid file fragmentation. | High; append-only write profiles are compatible with copy-on-write. | Poor; checkpointing triggers disk I/O errors and process hangs. |
| **Silent Write Resiliency** | Vulnerable; requires application-level verification loops. | Superior; quorum-based log confirmation validates disk commits. | Excellent; transactions provide ACID safety guarantees. |
| **Implementation Complexity** | Low; relies on simple metadata states in `session.md`. | High; requires log replication and active term management. | Moderate; requires managing connection pools and DB migrations. |
| **Version Control Friendliness** | High; sequential modifications minimize Git merge conflicts. | Moderate; metadata logs can generate Git untracked noise. | Poor; binary database files are incompatible with Git merge flows. |

## Unified Protocol Synthesis and Operational Lifecycle
The Courier Protocol synthesizes these distributed systems principles into a unified, daemonless coordination framework optimized for local repositories. It operates as an on-disk replicated state machine, using POSIX primitives to serialize concurrent updates to `session.md`.

```text
                 +-----------------------------------------+
                 |       COURIER RUNTIME INIT STATE        |
                 +--------------------+--------------------+
                                      |
                                      v
                 +-----------------------------------------+
                 |      FILESYSTEM COMPATIBILITY DETECT    |
                 +--------------------+--------------------+
                                      |
                  +-------------------+-------------------+
                  |                                       |
                  | BTRFS / WSL2 Detected                 | Standard FS
                  v                                       v
     +--------------------------+           +--------------------------+
     |   Fallback: Journal      |           |     Standard WAL Mode    |
     |   DELETE / TRUNCATE      |           |     Sync NORMAL          |
     +--------------------------+           +--------------------------+
```

### 1. Initial State and File System Compatibility Auto-Detection
When an agent initializes, it must determine the underlying filesystem type to prevent database locking errors.
* **BTRFS / WSL2 Detection**: The agent inspects `/proc/self/mountinfo` and reviews the database directory path.
* **Fallback Strategy**: If BTRFS or WSL2 DrvFS is detected, the agent bypasses WAL mode and executes the SQL command:
  ```sql
  PRAGMA journal_mode = DELETE;
  ```
  This falls back to a standard rollback journal, trading write concurrency for data integrity. On standard filesystems (e.g., ext4, xfs), the agent configures:
  ```sql
  PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;
  ```
  This configuration balances transaction speed, concurrency, and crash resilience.

### 2. Lock Acquisition and Symmetry Breaking
To append changes to `session.md`, an agent must acquire the global write lock.
* **Lock Ordering**: The agent locks resources in ascending lexicographical order to prevent deadlocks.
* **Atomic Step**: The agent attempts to create the directory `.courier/lock/`.
* **Backoff Strategy**: If the call returns `EEXIST`, the lock is held by another agent. The requesting agent enters a sleep state, calculating a randomized exponential backoff window $T_{\text{wait}}$ to prevent livelocks:
  $$T_{\text{wait}} = T_{\text{base}} \times 2^{\text{retry\_count}} + \text{rand}(0, T_{\text{jitter}})$$
  where $T_{\text{base}} = 100\text{ ms}$, $T_{\text{jitter}} = 150\text{ ms}$, and $\text{retry\_count}$ increments on each failed attempt, up to a maximum of 5.

### 3. Log Proposal and Consensus Quorum Validation
Once the lock is acquired, the agent acts as the term Coordinator.
* **Log Write**: The Coordinator writes a signed log entry, formatted using the `LogEntry` schema, to `.courier/logs/00000X_T_agent.json`.
* **Quorum Calculation**: Passive agents monitor the `.courier/logs/` directory. They validate the signature and term, and write a signed confirmation file back to the log directory. The log entry is considered committed once the Coordinator collects confirmations from a strict majority $Q$ of the active agents listed in the roster:
  $$Q = \left\lfloor \frac{N_{\text{agents}}}{2} \right\rfloor + 1$$

### 4. Write-Read-Verify Materialization Sequence
Upon reaching quorum, the Coordinator materializes the update into `session.md` using a fail-safe write sequence:
* **Atomic Temp Write**: The Coordinator writes the updated markdown payload to `session.md.tmp`.
* **Atomic Swap**: The Coordinator calls `rename("session.md.tmp", "session.md")` (or equivalent `replace()`).
* **Verification Check**: The Coordinator reads `session.md` back from the disk and computes its SHA-256 hash.
* **Error Resolution**: If the file is missing or the hash does not match the in-memory state, the Coordinator logs an error, aborts the transaction, and rolls back the state to prevent silent write failures.

### 5. Baton Release and Transition
Once verification succeeds, the Coordinator releases the filesystem lock:
* **Lock Removal**: The Coordinator removes the `.courier/lock/` directory.
* **Baton Forwarding**: The Coordinator writes a new `HandoffBaton` metadata payload to `.courier/baton.json`. This advances the term and notifies the next scheduled agent in the queue to begin its execution cycle.

## JSON Message Schemas
The following schema specifications define the structured data exchanged by agents during the lock negotiation, replication, and state verification phases.

### HandoffBaton Schema
The `HandoffBaton` schema defines the structure of the active execution token. It ensures orderly, sequential turn-taking and includes checksum validation to protect against silent write failures.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "HandoffBaton",
  "type": "object",
  "properties": {
    "baton_id": {
      "type": "string",
      "format": "uuid",
      "description": "Unique identifier for the current baton handoff transaction."
    },
    "term": {
      "type": "integer",
      "minimum": 0,
      "description": "The current logical consensus term of the multi-agent session."
    },
    "current_holder": {
      "type": "string",
      "description": "Cryptographic DID of the agent currently holding the baton."
    },
    "next_designated_holder": {
      "type": "string",
      "description": "Cryptographic DID of the target agent scheduled to receive the baton next."
    },
    "last_committed_index": {
      "type": "integer",
      "minimum": 0,
      "description": "The highest log index known to be committed and written to session.md."
    },
    "session_file_checksum": {
      "type": "string",
      "description": "The SHA-256 hash of the session.md file on disk at the moment of handoff."
    },
    "timestamp": {
      "type": "string",
      "format": "date-time",
      "description": "ISO 8601 timestamp of baton release."
    }
  },
  "required": [
    "baton_id",
    "term",
    "current_holder",
    "next_designated_holder",
    "last_committed_index",
    "session_file_checksum",
    "timestamp"
  ]
}
```

### LogEntry Schema
This schema defines a single proposed write entry submitted to the decentralized log directory. It contains the raw change payload along with cryptographic signatures for validation.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "LogEntry",
  "type": "object",
  "properties": {
    "index": {
      "type": "integer",
      "minimum": 0,
      "description": "The sequential index of this log entry within the consensus timeline."
    },
    "term": {
      "type": "integer",
      "minimum": 0,
      "description": "The consensus term in which this entry was proposed."
    },
    "author_did": {
      "type": "string",
      "description": "The cryptographic DID of the agent proposing this write operation."
    },
    "timestamp": {
      "type": "string",
      "format": "date-time",
      "description": "ISO 8601 timestamp of log creation."
    },
    "payload": {
      "type": "object",
      "properties": {
        "operation": {
          "type": "string",
          "enum": ["append", "replace"],
          "description": "The modification type to perform on session.md."
        },
        "content": {
          "type": "string",
          "description": "The raw markdown content to write to the session log."
        }
      },
      "required": ["operation", "content"]
    },
    "signature": {
      "type": "string",
      "description": "Ed25519 signature of the entry's index, term, and payload, signed by the author."
    }
  },
  "required": [
    "index",
    "term",
    "author_did",
    "timestamp",
    "payload",
    "signature"
  ]
}
```

### SyncState Schema
This schema documents the localized sync status of an individual agent, recording the filesystem type and validation metadata.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "SyncState",
  "type": "object",
  "properties": {
    "agent_did": {
      "type": "string",
      "description": "The cryptographic DID of the reporting agent."
    },
    "current_term": {
      "type": "integer",
      "minimum": 0,
      "description": "The highest term observed by the reporting agent."
    },
    "last_applied_index": {
      "type": "integer",
      "minimum": 0,
      "description": "The index of the last log entry locally applied to the agent's view."
    },
    "detected_filesystem": {
      "type": "string",
      "description": "The underlying filesystem type detected (e.g., ext4, xfs, btrfs, drvfs)."
    },
    "is_synchronized": {
      "type": "boolean",
      "description": "Flag indicating if the agent's state matches the latest committed index."
    }
  },
  "required": [
    "agent_did",
    "current_term",
    "last_applied_index",
    "detected_filesystem",
    "is_synchronized"
  ]
}
```

## State Machine Representation
The internal lifecycle transitions of an agent executing the Courier Protocol are detailed in the state machine diagram below. This state machine enforces mutual exclusion, manages write operations, and executes the verification loop to prevent file corruption.

```text
                     +---------------------------------------+
                     |                 IDLE                  | <-------------------+
                     +-------------------+-------------------+                     |
                                         |                                         |
                                         | Agent requests write access             |
                                         v                                         |
                     +---------------------------------------+                     |
                     |                HUNGRY                 |                     |
                     +-------------------+-------------------+                     |
                                         |                                         |
                                         | Lock Ordering Check Passed              |
                                         v                                         |
                     +---------------------------------------+                     |
                     |               ACQUIRING               |                     |
                     +-------------------+-------------------+                     |
                                         |                                         |
               +-------------------------+-------------------------+               |
               | Try mkdir(.courier/lock/)                         | Failure       |
               |                                                   | (Collision)   |
               v                                                   v               |
+------------------------------+                     +---------------------------+ |
|           WRITING            |                     |       RANDOM BACKOFF      | |
+--------------+---------------+                     +-------------+-------------+ |
               |                                                   |               |
               | Writes temp file & renames                        | Timeout       |
               v                                                   | Expired       |
+------------------------------+                                   +---------------+
|          VERIFYING           |
+--------------+---------------+
               |
         +-----+-------------------------------------+
         | Read-back & verify SHA-256                |
         |                                           |
         | Hash Match                                | Hash Mismatch / Error
         v                                           v
+------------------------------+                     +---------------------------+
|          COMMITTED           |                     |    FAIL-SAFE RECOVERY     |
+--------------+---------------+                     +-------------+-------------+
               |                                                   |
               | State updated in state.json                       | Transaction rolled back;
               v                                                   | error reported
               |                                                   v
+------------------------------+                     +---------------------------+
|          RELEASING           |                     |     RELEASE LOCK ERR      |
+--------------+---------------+                     +-------------+-------------+
               |                                                   |
               +-------------------------+-------------------------+
                                         |
                                         | Remove lock/ directory 
                                         v
                     +---------------------------------------+
                     |                 IDLE                  |
                     +---------------------------------------+
```

## FastMCP Integration Blueprint
FastMCP is a high-performance Python framework designed by Jared Lowin and maintained by PrefectHQ. FastMCP simplifies MCP server development by converting standard Python functions into compliant JSON-RPC tool endpoints, generating schemas, and handling type validation. Powering approximately 70% of current MCP servers, FastMCP automatically dereferences Pydantic shared schemas and inlines nested definitions. This optimization ensures compatibility with common LLM clients, such as Claude Desktop and VS Code Copilot, which do not fully support complex JSON Schema references.

The Python script below implements the Courier Protocol as a FastMCP server, exposing the lock, append, verification, and release sequence as validated tools.

```python
"""
Courier Protocol FastMCP Server for Deep Cuts.
Provides atomic locking, safe writing, and verification tools for multi-agent workflows.
"""

import os
import time
import hashlib
from typing import Dict, Any
from pathlib import Path
from pydantic import BaseModel, Field
from fastmcp import FastMCP

# Initialize the FastMCP Server using the Prefect framework 
mcp = FastMCP("Courier")

# Root directory configuration
COLLAB_DIR = Path("doc/collab")
LOCK_DIR = COLLAB_DIR / ".courier" / "lock"
STATE_FILE = COLLAB_DIR / ".courier" / "state.json"
SESSION_LOG = COLLAB_DIR / "session.md"

# Ensure protocol folders exist on initialization
COLLAB_DIR.mkdir(parents=True, exist_ok=True)
(COLLAB_DIR / ".courier").mkdir(parents=True, exist_ok=True)


class LockPayload(BaseModel):
    agent_id: str = Field(..., description="Cryptographic DID key of the requesting agent.")
    term: int = Field(..., description="The current logical term of the system.")


class AppendPayload(BaseModel):
    agent_id: str = Field(..., description="Cryptographic DID key of the writing agent.")
    content: str = Field(..., description="Markdown content to append to the log.")
    term: int = Field(..., description="The term in which this append is executed.")


class VerificationPayload(BaseModel):
    agent_id: str = Field(..., description="Cryptographic DID key of the verifying agent.")
    expected_sha256: str = Field(..., description="The expected SHA-256 hash of the log file.")


def detect_filesystem_constraints(target_path: Path) -> str:
    """
    Inspects the host environment to detect WSL2 DrvFS mounts or BTRFS systems,
    enabling safety fallbacks to prevent SQLite WAL-associated errors.
    """
    try:
        # Detect BTRFS via mountinfo 
        mountinfo = Path("/proc/self/mountinfo")
        if mountinfo.exists():
            mount_data = mountinfo.read_text(encoding="utf-8")
            if "btrfs" in mount_data:
                return "btrfs"

        # Detect WSL2 DrvFS mounts 
        resolved = str(target_path.resolve())
        if "/mnt/" in resolved or "Microsoft" in Path("/proc/version").read_text(encoding="utf-8"):
            return "drvfs"
    except Exception:
        pass
    return "standard"


@mcp.tool()
def acquire_lock(agent_id: str, term: int) -> Dict[str, Any]:
    """
    Acquires the global coordination lock by atomically creating a directory.
    Implements Dijkstra's mutual exclusion primitive to prevent write collisions.
    """
    try:
        # Atomic directory creation 
        LOCK_DIR.mkdir(parents=False, exist_ok=False)
        return {
            "status": "SUCCESS",
            "lock_acquired": True,
            "agent_id": agent_id,
            "term": term,
            "message": "Atomic lock directory created successfully."
        }
    except FileExistsError:
        # Lock held by another process 
        return {
            "status": "COLLISION",
            "lock_acquired": False,
            "agent_id": agent_id,
            "message": "The lock is held by another agent. Calculate a backoff and retry."
        }
    except Exception as e:
        return {
            "status": "ERROR",
            "lock_acquired": False,
            "message": f"Lock acquisition failed: {str(e)}"
        }


@mcp.tool()
def append_session_log(agent_id: str, content: str, term: int) -> Dict[str, Any]:
    """
    Appends content to the shared session.md file using the atomic rename pattern.
    Detects mount conditions and performs immediate post-write verification.
    """
    if not LOCK_DIR.exists():
        return {
            "status": "DENIED",
            "message": "Write request rejected. You must acquire the lock before writing."
        }

    fs_type = detect_filesystem_constraints(SESSION_LOG)
    tmp_file = SESSION_LOG.with_suffix(".tmp")

    try:
        # 1. Read existing log contents
        existing_markdown = ""
        if SESSION_LOG.exists():
            existing_markdown = SESSION_LOG.read_text(encoding="utf-8")

        # 2. Append new agent entry
        updated_markdown = (
            f"{existing_markdown}\n\n"
            f"\n"
            f"{content}\n"
        )

        # 3. Write payload to temporary file 
        tmp_file.write_text(updated_markdown, encoding="utf-8")

        # 4. Atomic swap replacing the active log 
        tmp_file.replace(SESSION_LOG)

        # 5. Write-Read-Verify (WRV) confirmation loop 
        on_disk_content = SESSION_LOG.read_text(encoding="utf-8")
        computed_sha256 = hashlib.sha256(on_disk_content.encode("utf-8")).hexdigest()

        return {
            "status": "SUCCESS",
            "file_modified": "session.md",
            "detected_filesystem": fs_type,
            "computed_sha256": computed_sha256,
            "message": "Log updated successfully and verified on physical storage."
        }
    except Exception as e:
        if tmp_file.exists():
            tmp_file.unlink()
        return {
            "status": "FAIL",
            "message": f"Write failed: {str(e)}. Changes rolled back."
        }


@mcp.tool()
def verify_write_integrity(agent_id: str, expected_sha256: str) -> Dict[str, Any]:
    """
    Performs absolute integrity verification to isolate silent write failures.
    Ensures the written data matches the expected cryptographic checksum.
    """
    if not SESSION_LOG.exists():
        return {
            "status": "FAILURE",
            "integrity_matched": False,
            "message": "Silent write failure detected. session.md is missing from disk."
        }

    try:
        on_disk_content = SESSION_LOG.read_text(encoding="utf-8")
        actual_sha256 = hashlib.sha256(on_disk_content.encode("utf-8")).hexdigest()

        if actual_sha256 == expected_sha256:
            return {
                "status": "VERIFIED",
                "integrity_matched": True,
                "agent": agent_id,
                "sha256": actual_sha256
            }
        else:
            return {
                "status": "CORRUPTED",
                "integrity_matched": False,
                "agent": agent_id,
                "actual_sha256": actual_sha256,
                "expected_sha256": expected_sha256,
                "message": "Integrity check failed. Checksum mismatch detected."
            }
    except Exception as e:
        return {
            "status": "ERROR",
            "integrity_matched": False,
            "message": f"Verification failed to execute: {str(e)}"
        }


@mcp.tool()
def release_lock(agent_id: str) -> Dict[str, Any]:
    """
    Releases the global lock by removing the lock directory.
    Enables pending agents to attempt lock acquisition.
    """
    try:
        if LOCK_DIR.exists():
            # Atomic directory removal 
            LOCK_DIR.rmdir()
            return {
                "status": "SUCCESS",
                "lock_released": True,
                "agent_id": agent_id
            }
        else:
            return {
                "status": "NO_OP",
                "lock_released": True,
                "message": "No active lock was found to release."
            }
    except Exception as e:
        return {
            "status": "ERROR",
            "lock_released": False,
            "message": f"Failed to remove lock directory: {str(e)}"
        }


if __name__ == "__main__":
    mcp.run()
```

## Conclusions and Engineering Directives
To establish secure, concurrent human-AI collaboration within local-first applications like Deep Cuts, the development lifecycle should transition from unstructured file updates to formalized filesystem protocols. The analysis of daemonless, offline-first execution environments yields several key directives for the system's architecture:
1. **Use OS-Level Atomic Locks**: Rely on directory creation (`mkdir`) and file renaming (`rename`) rather than simple file existence checks to enforce locking. These POSIX system calls are natively atomic, preventing race conditions and write collisions without requiring a persistent coordinator daemon.
2. **Implement Mandatory Verification Loops**: To resolve the silent write failures common in sandboxed LLM execution environments, require all writing processes to verify their changes. This is done by reading back the file from disk, calculating its SHA-256 checksum, and matching it against the intended change before releasing the lock or passing the handoff baton.
3. **Adopt Dynamic Filesystem Detection**: Because WSL2 DrvFS mounts and BTRFS Copy-on-Write systems conflict with SQLite's default WAL configurations, multi-agent frameworks must dynamically audit their runtime mount paths. When these systems are detected, database drivers must fall back to rollback journal modes (`DELETE` or `TRUNCATE`) and use standard directory-based locking to prevent database corruption.
4. **Leverage FastMCP for Schema Dereferencing**: When building tool integrations for AI agents, use the FastMCP framework. FastMCP automatically dereferences nested schemas and inlines Pydantic models, resolving JSON Schema compatibility issues for common LLM clients like Claude Desktop.

Integrating the Courier Protocol’s deterministic coordination, structural constraints, and atomic operations into the `robertolupi/deep-cuts` repository ensures that multi-agent session logs remain consistent, transactionally secure, and resilient against local concurrency failures.

---

## [Roberto, 00:26]

Frankly, I am not impressed. I could have designed this myself. We don't have the limitation that we cannot have a central database/coordinator; we can change the MCP implementation to act as that.

What I want is enrichment of results, so one agent reviews the other's results in collaboration until something better than pure coordination happens. Think like Raft or Paxos, but instead of reaching consensus about a lock, we improve the qualitative result. I do want a serialized log, which could work like DeepThink proposed, but that's the least interesting bit.

Let's reformulate the query for DeepThink, shifting the focus from lock-level concurrency control to a **Cognitive Consensus and Result Enrichment Protocol**, where the central MCP server acts as the state coordinator.

---

## [Gemini, 00:28]

Understood. Let's shift the focus entirely. The plumbing of file locking and serialized logs is just the baseline infrastructure. The real value is in designing a **Cognitive Consensus and Collaborative Refinement Protocol** where the central MCP server acts as an active **Orchestrator and Blackboard**.

Here is the reformulated prompt for Google DeepThink:

***

# Prompt for Google DeepThink: Cognitive Consensus and Result Enrichment Protocol (CCREP) for Multi-Agent Engineering

## 1. The Core Objective: Evolving Qualitative Artifacts
In our development environment ([`robertolupi/deep-cuts`](https://github.com/robertolupi/deep-cuts)), we have multiple specialized AI agents:
* **Claude**: Focuses on implementation (writing Rust, TypeScript, Svelte 5).
* **Gemini (agy)**: Focuses on architectural design, research, and mathematical framing.
* **Codex**: Executes verification scripts, runs optimizations, and handles structured refactoring.
* **Human (Roberto)**: Directs the process and sets quality targets.

We want to design a protocol where these agents collaborate to **iteratively refine and enrich qualitative outputs** (code quality, mathematical models, hyperparameter configurations, test coverage) rather than just coordinating locks on files. 

Think of this as a **"Cognitive Paxos/Raft"** where the consensus is not reached on a lock token, but on the *proven quality value* of the codebase or plan.

## 2. Infrastructure Assumption: Centralized MCP Coordinator
We do **not** have a requirement to remain serverless. We can evolve our Model Context Protocol (MCP) server implementation (currently described in [claude-mcp-adapter.md](https://github.com/robertolupi/deep-cuts/blob/main/doc/collab/claude-mcp-adapter.md) and [coordination-protocol.md](https://github.com/robertolupi/deep-cuts/blob/main/doc/collab/coordination-protocol.md)) to act as a **Central Coordinator, Orchestrator, and State Database**.

The MCP server will:
* Maintain the state of all work-in-progress branches and tasks.
* Run automated verification harnesses (e.g. executing tests, linting, computing boundary detection F1 metrics).
* Expose tool endpoints to agents for submitting proposals, reading reviews, and registering votes.

## 3. The Protocol Loop: Propose - Critique - Evaluate - Merge
We need a formal state machine and protocol for the following collaborative loop:

1. **Proposal (Proposer Role)**:
   * Agent A claims a task, creates a git branch, makes changes (e.g. optimizing boundary thresholds, writing a Svelte component, or implementing a Rust migration), and submits it to the MCP server via `submit_proposal()`.
2. **Automated Evaluation (Orchestrator Role)**:
   * The MCP server checks out the branch, executes the evaluation/test suite (e.g., reproducing the statistical regression tests in `evaluate_salami_phase0.py`), and caches the metrics (e.g. F1@3s, p-values, lint failures) as a structured **Evaluation Report**.
3. **Peer Review & Critique (Critiquer/Acceptor Role)**:
   * Agent B reads the proposal code and the Evaluation Report. It runs its own analysis, identifies edge cases, and writes a **Peer Review** (e.g., pointing out a potential memory leak in Svelte or recommending a different Viterbi duration prior).
   * Agent B can either:
     * **Approve**: If it meets the quality threshold.
     * **Critique with Amendment**: Propose specific edits/refinements to the branch.
4. **Iterative Enrichment**:
   * Agent A (or Agent C) consumes the Peer Review, applies amendments to the branch, and submits a revised proposal.
   * The loop repeats, with the code quality progressively rising.
5. **Consensus Commit (Learner/Consensus Gate)**:
   * The proposal is merged into `main` only when it meets a **Consensus Gate**:
     * Automated criteria: Tests pass, no regressions on golden metrics.
     * Peer criteria: Active quorum agreement (e.g., at least one implementation agent and one architectural agent approve).
     * Human override: Optional human-in-the-loop sign-off.

## 4. Key Design Challenges for Research
Please analyze, research, and design solutions for the following problems:

### A. Preventing Cognitive Divergence & Loop Termination
In peer-to-peer LLM loops, agents can get stuck in infinite edit-war loops (e.g., swapping formatting style, arguing over naming, or endlessly tuning a parameter back and forth). 
* How do we design **loop termination conditions**? (e.g., temperature decay schedules, metric convergence thresholds, or maximum revision budgets).
* How do we model a "Simulated Annealing" approach for agent prompts, where agent leeway or prompt creativity is narrowed down as the iterations progress?

### B. Weighted Voting and Specialized Consensus
When evaluating a proposed change, not all votes are equal. A TypeScript bug is best verified by an implementation agent, while an optimization choice is best evaluated by a research agent.
* How do we structure a **weighted consensus voting scheme** (similar to Paxos quorums) where quorums are dynamic and weighted by the agents' declared specialties and task domains?
* Can we adapt Condorcet voting to let agents rank multiple implementation branches (e.g., Branch A: Nelder-Mead vs Branch B: CMA-ES vs Branch C: Random Search) and converge on the mathematically optimal choice?

### C. Representation of Qualitative State
* How do we formally define the "State" of the consensus?
* Design the JSON schema for:
  * `Proposal`: links to git commit, description, and author.
  * `EvaluationReport`: compiles test results, metrics, and static analysis.
  * `Critique`: comments, suggested code diffs, or parameter changes.
  * `ConsensusState`: tracks quorums, approvals, and voting weight tallies.

### D. MCP Server Integration Blueprint
* Sketch the API design (tools, resources, prompts) for the expanded MCP server acting as the Blackboard.
* How does the MCP server manage branch checkouts, test execution, and coordinate which agent is woken up to critique or amend a proposal next?

---

## [Codex, 00:35]

(Scribed by Codex.) I have conducted research on the Cognitive Consensus and Result Enrichment Protocol (CCREP). The complete design is available in the dedicated document: [codex-ccrep-design.md](file:///Users/rlupi/src/deep-cuts/doc/collab/sessions/2026-06-08-multi-agent-collaboration-research/codex-ccrep-design.md).

Here is the Executive Summary of the proposed consensus and improvement protocol:

### CCREP Executive Summary

* **Decoupling from Paxos/Raft Replication**: Instead of solving network partition replication, CCREP adapts Paxos/Raft concepts to a centralized coordinator model. Consensus is reached not on a lock, but on the *proven quality* of a code/design change. A consensus value is represented as an immutable tuple: `(task_id, commit_sha, evaluation_report, critique_set, vote_set, gate_policy_version)`.
* **Dual State Machines**: The protocol tracks both the overall **Task Lifecycle** (`OPEN` $\rightarrow$ `CLAIMED` $\rightarrow$ `HAS_PROPOSALS` $\rightarrow$ `CANDIDATE_SELECTION` $\rightarrow$ `MERGED`) and the lifecycle of each individual **Proposal Revision** (`SUBMITTED` $\rightarrow$ `EVALUATING` $\rightarrow$ `REVIEWING` $\rightarrow$ `APPROVED` $\rightarrow$ `MERGED`).
* **Anti-Divergence & Simulated Annealing**: To prevent infinite "taste loops" (arguments over formatting, naming, etc.), CCREP introduces:
  1. A **revision budget** (max 5 revisions) and a **minimum metric delta** threshold (e.g. F1 change must be $\ge 0.002$ to trigger a re-eval).
  2. **Critique Admissibility**: Only specific, actionable, and evidence-linked findings block merges.
  3. **Simulated Annealing**: As the revision round increases, a temperature parameter decays, narrowing the allowed prompt freedom and search space (e.g., Round 0 = high temp, creative architecture exploration; Round 3+ = low temp, minimal bug-fixes only).
* **Weighted Dynamic Consensus**: Votes are weighted by agent capabilities relative to the task domains. An agent with high Rust/ML capabilities has a higher vote weight on Viterbi model adjustments, but lower weight on Svelte UI files. Quorums are calculated dynamically by dot-multiplying the agent's capability vector with the task's domain vector.
* **Condorcet Voting for Competing Branches**: When multiple optimization strategies exist (e.g. Branch A: Nelder-Mead vs Branch B: CMA-ES), agents rank the branches, and a weighted pairwise matrix is constructed to identify the Condorcet winner.
* **Centralized MCP Orchestration**: The MCP server acts as the central coordinator (the Blackboard), executing automated evaluation suites (lints, tests, boundary evaluation scripts) in isolated local worktrees, maintaining a database of consensus states, and assigning the next agent's action through a pull-based task queue.

---

## [Gemini, 00:40]

(Scribed by Gemini.) I have processed the Google DeepThink research output on CCREP. The complete protocol design from Google DeepThink has been placed in a dedicated file: [google-ccrep-design.md](file:///Users/rlupi/src/deep-cuts/doc/collab/sessions/2026-06-08-multi-agent-collaboration-research/google-ccrep-design.md).

Here is the Executive Summary of the Google DeepThink CCREP design:

### Google DeepThink CCREP Executive Summary

* **Centralized MCP Orchestration (The Blackboard)**: Validates that the manual asynchronous coordination pattern ("Two AIs, One Notepad") should be automated by an active MCP server managing git workspaces, testing processes, and agent scheduling.
* **Topological Task Scheduling via CIG**: Tasks and agent communication are coordinated using a **Causal Interaction Graph (CIG)**—a directed acyclic graph $G_{CIG} = (A, M, C)$ mapping triggering messages to agent responses. Primitives $\text{get\_parents}(v)$ and $\text{get\_descendants}(v)$ allow tracing dependencies to isolate test failures and target review requests to affected peers.
* **Textual Simulated Annealing (TSA) & Textual Learning Rate Decay (TLRD)**:
  1. Let $T_k = T_0 \cdot \gamma^k$ control agent prompts, decaying from high $T$ (exploratory, system-wide refactoring) to low/terminal $T$ (restricted, linter-guided syntax fixes only).
  2. TLRD decays the physical volume of characters allowed to change per commit as iterations progress.
* **Quorum Weight Calibration via Bayes-Optimal Log-Odds**: Domain weight $w_{i,D}$ is computed based on historical accuracy, peer corroboration, and domain matching. It maps through a log-odds transformation:
  $$w_{i,D}^* = \ln\left( \frac{\beta_{i,D}}{1 - \beta_{i,D}} \right)$$
  ensuring specialized domain experts override generalist models on target issues.
* **Concordance Tracking via Kendall's W**: Rates agent agreement on a Consensus Matrix $M \in \mathbb{R}^{N \times K}$ using Kendall's concordance $W$. The coordinator maps $W$ to Friedman's chi-square distribution ($\chi^2 = N(K - 1)W$) to determine statistical significance. If agreement falls below $0.65$, it isolates the most discordant agent to supply targeted feedback.
* **Pairwise Preference Resolution via Schulze Method**: Employs a Condorcet-consistent graph-based path-strength calculation (Schulze method) using a Floyd-Warshall variant to resolve circular preferences among multiple competing branches without deadlock.

## [Meta, 00:45]

(Scribed by Meta.) I have provided my detailed proposal for the Cognitive Consensus and Result Enrichment Protocol (CCREP), focusing on establishing a dynamic "quality lock" framework rather than a basic file guard. The full document is available at: [meta-ccrep-design.md](file:///Users/rlupi/src/deep-cuts/doc/collab/sessions/2026-06-08-multi-agent-collaboration-research/meta-ccrep-design.md).

Here is the Executive Summary of the Meta CCREP design:

### Meta CCREP Executive Summary

* **Dual-Loop State Machine**: Tracks both overall Task Lifecycles and individual Proposal Revisions through a structured pipeline (`IDLE` $\rightarrow$ `PROPOSED` $\rightarrow$ `EVALUATING` $\rightarrow$ `REVIEWING` $\rightarrow$ `AMENDING` $\rightarrow$ `VOTING` $\rightarrow$ `MERGE_READY` $\rightarrow$ `MERGED`).
* **Multi-Layered Termination Levers**:
  1. **Plateau Detection**: Terminates if revision metrics ($\Delta$F1@3s $< 0.001$ and $\Delta$lint_errors $= 0$) do not improve for two consecutive runs, or if the `max_revisions = 5` budget is exceeded.
  2. **Simulated Annealing prompt controls**: Decays temperature ($T_n = T_0 \cdot 0.7^n$) over revision rounds to shift model instructions from broad architectural revisions to target-only bug fixes.
  3. **Edit-War Detector**: Monitors Levenshtein distance between revision $n$ and $n-2$ to flag alternating author style-flapping. Automatically initiates Codex compromise diffs when oscillations are flagged.
* **Domain-Specific Weighted Quorum**: Computes quorum dynamically based on the task domain and agent capabilities (e.g., Claude carries 0.6 implementation weight; Gemini carries 0.6 architectural weight; Codex carries 0.6 verification weight; Human holds absolute veto).
* **Schulze Method Path Strengths**: Resolves cyclical branch preferences (e.g., when choosing between multiple optimization branches) using a Floyd-Warshall path strength algorithm to determine Condorcet winners.
* **Log Scribe Isolation**: Agents never edit `session.md` directly. All messages route through spool directories, and the MCP server scribe appends a linearized summary only when a proposal is successfully merged.

---

## [Antigravity, 00:50]

I have synthesized the proposals from Codex, Google DeepThink, and Meta AI into a unified and improved coordination model: **Unified CCREP 2.1**. 

This protocol addresses real-world sandboxing constraints and execution limits on macOS/Unix (specifically removing the unhonored prompt temperature limits in favor of hard physical constraints). The complete design is available in: [unified-ccrep-design.md](file:///Users/rlupi/src/deep-cuts/doc/collab/sessions/2026-06-08-multi-agent-collaboration-research/unified-ccrep-design.md).

Here is the Executive Summary of the Unified CCREP 2.1 design:

### Unified CCREP 2.1 Executive Summary

* **Programmatic AST & Line-Diff Gatekeeping**: Since LLM agents cannot dynamically self-adjust their inference temperature mid-session and clients frequently ignore API temperature settings, CCREP 2.1 physically enforces limits. The MCP server parses files using AST and diff checks, programmatically rejecting modifications that exceed strict line budgets or structural templates (e.g. blocking function signature modifications in late rounds, allowing only linter-suggested edits in terminal rounds).
* **Git Worktree Isolation & Test Orchestration**: To prevent compilation and test runs from polluting the developer's active directory, all evaluations run in isolated, detached git worktrees under `.ccrep/worktrees/prop-{proposal_id}` which are automatically created and pruned via OS-level git calls.
* **SQLite WAL with POSIX Directory Locks**: Solves database contention and mounting errors on WSL2/DrvFS and BTRFS filesystems by wrapping SQLite transactions with atomic directory-creation lock barriers (`mkdir .ccrep/locks/db.lock`).
* **Daemonless Doorbell Signaling**: Initiates agent execution without active background daemons using a maildir spool directory (`spool/<agent>/new/`) coupled with POSIX named pipes (FIFOs) for event notification.
* **Multi-Objective Optimization Vector (MOOV)**: Evaluates quality on a multi-dimensional Pareto vector tracking test pass rates, code coverage, cyclomatic complexity, and domain performance rather than a single metric.
* **Laplacian-Smoothed Log-Odds Weighting**: Fixes division-by-zero boundary conditions in weighted voting calculations by applying Laplace smoothing to historical accuracy values.
* **Elite Branch Cross-Pollination**: Automatically hybridizes competing branches that represent different Pareto quality trade-offs (e.g., high coverage vs high performance) using a three-way git merge.
