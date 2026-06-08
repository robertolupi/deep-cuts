<!-- claude's private log (memtable). Append timestamped entries here ONLY.
     Never edit session.md directly — run tools/merge_sessions.py to compact. -->

## [claude, 2026-06-08T07:58:00+02:00]
Kicking off the `session-management` collab (claude, agy, codex). The motivating
pain: three agents appending to one `session.md` race each other — exactly the
RESOLVED note in `doc/collab/PROTOCOL.md` (write collisions, lost turns,
back-filled steering). Roberto's idea: borrow from **log-structured merge trees**.

I built a first cut and dogfooded it in this very session:

- **Per-bot memtables.** Each agent appends only to `session.<bot>.md`
  (`session.claude.md`, `session.agy.md`, `session.codex.md`). No agent ever
  writes a file another agent writes, so the write-race disappears by
  construction — no locks, no baton needed for the *log*.
- **Compaction.** `tools/merge_session.py <session-dir>` reads every
  `session.*.md`, parses `## [handle, ISO-8601]` entries, and merges them into a
  generated `session.md` sorted by timestamp. This is the SSTable-merge step.
- **Standardized entry key.** Header is `## [<handle>, <ISO-8601 timestamp>]`.
  ISO-8601 sorts lexicographically == chronologically, so the merge is a stable
  k-way merge keyed on (timestamp, handle, source, order) — deterministic.
- **`--check` mode** for pre-commit: exits non-zero if `session.md` is stale vs
  the per-bot files, so the canonical log can't drift from its inputs.
- **`session.md` is GENERATED**: banner at top, do-not-edit. Source of truth is
  the per-bot files.

Design notes / open questions for the proposal:
1. **Timestamp granularity.** I used second precision. Two entries in the same
   second tie-break by handle — fine for human reading, but if we ever need true
   causal order we'd want a logical clock (Lamport) or a monotonic seq. For a
   turn-taking log where the collab MCP already serializes handoffs, wall-clock
   seconds is plenty. Flagging it, not solving it.
2. **Relationship to the collab MCP.** The MCP already gives single-writer
   handoffs (PROTOCOL rule 0). So is this redundant? No — they're complementary.
   The MCP serializes the *turn*; the split files make the *log* robust even when
   agents work in parallel worktrees, or when an agent edits out-of-band, or when
   the MCP is down. The merge is the durable reconciliation regardless of how
   coordination happened. Worth stating crisply in the proposal.
3. **Handoffs.** Handoff lines live inside entries as today (`**→ Handoff:**`),
   so they merge through untouched.
4. **Consensus loop.** Per Roberto: after contributing, each of us listens for
   the others (collab/recv) until we converge — ccrep-like but lightweight, no
   formal gate. I'll drive that as turn-holder.

Next: hand off to agy and codex to (a) write their own `session.<bot>.md` entries
using this format, (b) stress-test the merge (ordering, ties, malformed
headers), and (c) push back on the design. One of us then writes
`doc/proposal/session-coordination.md`.

**→ Handoff:**
**Task:** Append your own `session.<bot>.md` entry in this dir using the
`## [<handle>, <ISO-8601>]` format, run `tools/merge_session.py` on the dir to
confirm your entry merges in order, and critique the design (timestamp ties,
merge edge cases, redundancy vs collab MCP, who writes the proposal).
**Context:** `tools/merge_session.py`, this session dir
(`doc/collab/sessions/2026-06-08-session-management/`), and the RESOLVED
write-race note in `doc/collab/PROTOCOL.md`.
**Deliverable:** Your `session.<bot>.md` file + critique; we converge via
collab/recv, then one of us writes `doc/proposal/session-coordination.md`.

## [claude, 2026-06-08T08:01:00+02:00]
Delivered `tools/merge_sessions.py` (renamed to the plural name you both used).
I read your ACKs — we're converged. I folded your two admissible critiques into
the script:

1. **UTC normalization (codex's catch).** My first cut sorted timestamp strings
   lexicographically, which is *wrong* once offsets differ: `08:00+02:00`
   (06:00Z) must sort before `07:30Z`. Fixed — `to_utc()` parses RFC3339
   (tolerates trailing `Z`), assumes local tz for naive stamps, normalizes to
   UTC, and unparseable stamps sort last instead of crashing. Verified:
   cross-tz ordering, naive parse, malformed-sorts-last all pass.
2. **Tie-break = (utc-instant, source-file, original-order).** Exactly codex's
   `(timestamp, source, ordinal)`; dropped handle from the key since the source
   filename already encodes the author. Stable and deterministic.

ACKs on the open decisions — no objections:
- **Path:** `doc/proposals/session-coordination.md` (plural). Confirmed the repo
  has `doc/proposals/` and no `doc/proposal/`. Roberto typed the singular form,
  so the proposal should open with a one-line note that we followed the existing
  taxonomy; trivial for Roberto to override.
- **Roberto's contributions:** `session.roberto.md` when he writes directly;
  "Relayed Roberto feedback" inside an agent's file when an agent documents chat
  steering. The merger treats `session.roberto.md` like any other source.
- **Verbatim bodies:** the parser only splits on `## [handle, timestamp]`
  headers and never interprets handoffs/ACKs/code — confirmed.
- **Pre-commit:** `--check` verifies freshness and exits non-zero; it never
  mutates during commit. The hook runs the same command with `--check`.

One thing the proposal should call out explicitly (my design note #2): this
**composes with, not replaces, the collab MCP**. The MCP serializes the *turn*
(single-writer handoffs); split files make the *log* race-free even under
parallel worktrees or out-of-band edits, and the merge reconciles regardless of
how coordination happened. The MCP is the control plane; split-files + merge is
the data plane for the log.

Script is done and dogfooded — this entire session.md was produced by it.
Handing the proposal to **agy** as agreed (codex reviews the script in parallel).

**→ Handoff:**
**Task:** agy — draft `doc/proposals/session-coordination.md` capturing the
agreed design (per-bot memtables, RFC3339 entry key, UTC-normalized k-way merge,
`session.md` as generated artifact, `--check` pre-commit, `session.roberto.md`,
compose-with-collab-MCP framing). codex — review `tools/merge_sessions.py` and
flag any parser edge cases or lint issues.
**Context:** `tools/merge_sessions.py` (final), this session dir, the RESOLVED
write-race note in `doc/collab/PROTOCOL.md`, and the taxonomy note (use plural
`doc/proposals/`).
**Deliverable:** `doc/proposals/session-coordination.md` from agy; review notes
in `session.codex.md` from codex.

## [claude, 2026-06-08T08:03:00+02:00]
ACK agy's `doc/proposals/session-coordination.md` — accurate and complete. It
captures the design we converged on: split memtables, RFC3339 entry key,
UTC-normalized k-way merge with `(utc_instant, source_file, original_order)`
tie-break, generated `session.md`, `--check` pre-commit, `session.roberto.md`,
and the control-plane/data-plane framing vs the collab MCP. The taxonomy note up
top correctly records why we used plural `doc/proposals/`.

Two tiny fixes I made directly (unambiguous facts, no design change):
- Corrected the quoted banner in the proposal from `merge_session.py` (singular)
  to the actual plural `merge_sessions.py`.
- Harmonized the script's top docstring with the naive-timestamp behavior a peer
  set during review (naive == UTC, not local).

No remaining objections from me. I consider the artifact set complete:
`tools/merge_sessions.py` + `doc/proposals/session-coordination.md`, both
dogfooded against this session. Waiting on codex's script review to close.

## [claude, 2026-06-08T08:05:00+02:00]
Final ACK and proposed closeout. Consensus reached across claude, agy, codex.

**Accepted decisions:**
- Per-bot memtables (`session.<bot>.md`), single writer each — write-races gone by
  construction.
- `tools/merge_sessions.py` compacts all `session.*.md` into a generated
  `session.md` via UTC-normalized k-way merge, tie-break
  `(utc_instant, source_file, original_order)`, verbatim bodies.
- Entry key: `## [handle, RFC3339-with-offset]`. Offsets mandatory (codex's
  determinism point — naive stamps can't be ordered across machines).
- `session.md` is a generated artifact (banner, do-not-edit); `--check`
  gates freshness for a pre-commit hook (no silent mutation).
- `session.roberto.md` for direct human input; "Relayed Roberto feedback" inside
  an agent file otherwise.
- Composes with, does not replace, the collab MCP (control plane vs data plane).
- Proposal lives at `doc/proposals/session-coordination.md` (plural, per repo
  taxonomy; Roberto typed the singular form — trivial to override).

**Dogfooded:** this entire session's `session.md` was produced by the merge
script; `lint_collab.py` passes; `--check` is green.

**Non-blocking finding for the proposal:** a `## [Closed, …]` marker heading
parses as a pseudo-participant and would pollute the auto-derived Participants
list — so I'm recording this closeout *inside* a normal claude entry rather than
as a standalone `## [Closed]` block. The proposal should either reserve such
handles (Closed, etc.) in the merger or specify that closeouts live inside an
agent entry. Same applies to any non-roster heading.

Handing back to **Roberto** for review. Per PROTOCOL we do not create the
`ARCHIVED` tombstone — that is Roberto's call. Session stays active and resumable.
