# Session: CCREP Synthesis — Implementation Split (code vs. skill/rule)

## Participants
* Roberto (Human)
* Claude (AI) — actor `claude`
* Antigravity (AI) — actor `agy`

**Goal:** Amend [doc/proposals/ccrep-synthesis.md](../../../proposals/ccrep-synthesis.md) to say
*what part of CCREP is code, what part is skill or rule*, and make sure the protocol works for
three usage modes: **independent development**, **code reviews**, and **design docs**. Reach
consensus, then write the amended doc.

---

## [Claude, 05:57]

Opening the discussion. Here is my proposed framing for the amendment; pushback wanted on all of it.

### A. The code/rule split

My claim: CCREP's *code surface is small and is exactly the set of invariants that agents cannot be
trusted to self-police.* Everything that is judgment goes in skills/rules. Concretely:

| Concern | Where it lives | Why |
|---|---|---|
| Append-only `event_log` + materialized views (proposals, critiques, votes, merge_records) | **Code** (MCP server) | Storage must be deterministic; this is the blackboard. |
| Reduce `ConsensusState` from the log; *no self-approval*; *votes expire on new commit*; *N independent approvals* | **Code** (reducer) | These are the ratchet invariants. If an agent can satisfy them by asserting them, the ratchet is fake. |
| Resolve branch → immutable `commit_sha`; content-addressed eval cache keyed `(commit_sha, eval_suite_hash, dataset_hash, env_hash)` | **Code** | Reproducibility primitive; pure mechanism. |
| Worktree lifecycle + run the eval suite → `EvaluationReport` | **Code** (executor) | But *which* suite runs is task config, not server logic. |
| Physical revision gates (tree-sitter AST checks, max_files / max_changed_lines on the diff) | **Code** | "Physical gates on the diff" only mean anything if the server enforces them. |
| Critique **structure** (severity class present, evidence link present, file:line present) | **Code** (schema validation) | Presence of fields is mechanically checkable. |
| Critique **quality** / admissibility ("specific + actionable", is the evidence real) | **Skill/Rule** (`bot-collab` / a review rule) | This is reviewer judgment. The schema can demand a field; only a reviewer can tell if it's substantive. |
| Reviewer independence beyond `author != reviewer` (e.g. *different model family*) | **Code** enforces the checkable part; **Rule** for who-reviews-what convention | Model-family is a field; assignment is social. |
| Provenance hygiene (claims sourced or marked speculative) | **Rule** (mirrors critique-admissibility) | Judgment; can't be unit-tested. The session's own confabulated-benchmark problem is the motivating case. |
| Plateau / edit-war stop | **Code** detects the metric/Levenshtein condition; **Skill** decides to actually stop or escalate | Detection is mechanical; the call to halt is judgment. |
| Human-gate categories (public API, destructive migration, model/dataset change, arch) | **Code** enforces the block; **Rule** defines what counts | The block must be unbypassable; classification is judgment. |

Net: the buildable Phase-1 server is ~7 MCP tools + a reducer + a worktree executor. Everything
softer is taught through the **existing** skills (`bot-collab`, `how-to-experiment`, `write-docs`)
rather than new code. I'd state this explicitly in the doc so nobody builds an "admissibility
classifier" service.

### B. The part the current doc under-specifies: it assumes every proposal has an eval suite

The doc's worked example is a boundary-threshold tweak with `cargo test` + golden metrics. But
Roberto wants this to also cover **code reviews** and **design docs** — and *a design doc has no
`cargo test`.* You cannot run a golden-metric regression gate on a Markdown proposal. So I think the
key amendment is to make CCREP **artifact-type-aware**: the *server stays generic*, and each task
declares an **artifact profile** that selects which gate components apply.

| Artifact profile | Automated gate (the "eval suite") | Peer gate | Human gate |
|---|---|---|---|
| **Code change** (independent dev) | build + test + lint + fmt; no golden-metric regression | 1 implementation approval + 1 independent approval; no open blocking critiques | public-API / migration / model-or-dataset / large-arch |
| **Code review** (reviewing an existing/external diff) | build + test on the PR head (no metric gate unless the PR touches the pipeline) | ≥1 admissible structured critique + 1 independent approval | merge stays human — it's a review verdict, not an auto-merge |
| **Design doc** | `lint_collab.py` + link-check + skill-index consistency; **no metric gate**, **revision AST/line gates disabled** (they're meaningless on prose) | 1 independent approval + admissible critiques + **provenance rule** | **always human** |

Two things fall out of this nicely:

1. **The doc `status:` frontmatter *is* the consensus state for design docs.** `need-human-review`
   ≈ APPROVED-pending-human; `accepted` ≈ MERGED; `superseded` ≈ a merge that retired prior values.
   CCREP for docs is mostly formalizing a lifecycle we already run by hand — this very doc is at
   `need-human-review` in the body but `accepted` in the frontmatter, which is exactly the kind of
   drift a reducer would catch.
2. The revision gates from §3 (forbid `new_function_defs`, `max_changed_lines`) are **code-change-only**
   and must be declared as such, or they'll nonsensically fire on a doc edit.

### C. What this means for the three modes, concretely

- **Independent development:** CCREP is the eval+review harness. One agent proposes on a branch, the
  server evals in a worktree, a *different-family* agent critiques, gate = green checks + 1 independent
  approval. This is Phase 1 verbatim — no change needed.
- **Code reviews:** same machinery, "eval suite" = build/test on the head, deliverable = the
  admissible `Critique` set + an approval/block verdict. The ratchet value here is the *structured,
  evidence-linked critique*, not auto-merge.
- **Design docs:** same machinery, "eval suite" = the doc linters, gate leans entirely on
  admissible critiques + provenance + human sign-off. No metric math. The `write-docs` skill already
  owns doc quality; CCREP just adds the evidence ledger + lifecycle reducer around it.

### Questions for you (agy)

1. Do you buy the **"code = invariants only, judgment = skills"** line, or is there something soft
   you think *must* be code (I'm least sure about critique-admissibility — schema-enforced fields
   vs. a real check)?
2. Is **artifact profiles** the right generalization, or would you rather keep three separate flows?
   I prefer one generic server + a profile field; tell me if that's over-abstracting.
3. For design docs — agree that mapping `status:` ↔ consensus state is worth stating, or is that
    conflating a human-owned frontmatter field with machine-derived state in a way that'll bite us?

**→ Handoff:**
**Task:** React to A (code/rule split), B (artifact profiles), and C (the three modes). Mark what you
agree with, push back where you disagree, and propose the concrete section structure for the
amendment to `ccrep-synthesis.md`.
**Context:** [doc/proposals/ccrep-synthesis.md](../../../proposals/ccrep-synthesis.md);
this session.md; the four source designs under `doc/collab/sessions/2026-06-08-multi-agent-collaboration-research/`.
**Deliverable:** Your turn appended below + a handoff back, so we can converge on the section list
before either of us writes the doc.

---

## [Gemini, 06:05]

Quoting the handoff I picked up:
> **Task:** React to A (code/rule split), B (artifact profiles), and C (the three modes). Mark what you agree with, push back where you disagree, and propose the concrete section structure for the amendment to `ccrep-synthesis.md`.
> **Context:** [doc/proposals/ccrep-synthesis.md](../../../proposals/ccrep-synthesis.md); this session.md; the four source designs under `doc/collab/sessions/2026-06-08-multi-agent-collaboration-research/`.
> **Deliverable:** Your turn appended below + a handoff back, so we can converge on the section list before either of us writes the doc.

### A. The Code/Rule Split
I fully endorse your core claim: **code owns the invariants we cannot trust agents to self-police**, and judgment/conventions live in skills and rules.

Here are my refinements:
1. **Critique Admissibility (Verify Evidence Links in Code):** While checking if a critique's logic is "specific + actionable" requires human or LLM reviewer judgment (Skill), the server **can and should programmatically verify evidence links** (Code). The schema checks that the critique contains a file:line or line range, and the worktree executor verifies that this path exists and the line numbers are valid within the proposed `commit_sha`. If the link is dead, the critique is rejected as malformed before it ever hits a reviewer or proposer.
2. **Plateau / Edit-War Escalation:** The detection logic is code. I propose the *result* of the edit-war or plateau condition is also code-enforced: the server automatically shifts the proposal to an `ESCALATED` state, locks further commits on that branch, and posts a task to a third agent or flags it for Roberto. This keeps the ratchet invariants strict.
3. **Provenance Rule:** The rule itself is a convention (Skill/Rule), but we can add a simple linter check in the automated gate for design docs that flags any raw, unreferenced two-decimal metrics (e.g. `99.27%`) if they aren't marked as speculative or backed by a file reference. This programmatically pushes back against the confabulation problem observed in the research session.

### B. Artifact Profiles
Introducing **Artifact Profiles** is the perfect abstraction. It prevents over-complicating the CCREP server by decoupling the core ledger and state machine from task-specific rules.

Addressing your third question: **Mapping `status:` frontmatter to consensus state is not only worth stating, it should be an invariant.**
We should define:
- If a document's frontmatter has `status: need-human-review`, the server enforces that the proposal must be in the `APPROVED` (pending human) state.
- If it has `status: accepted`, the server verifies the proposal has been `MERGED`.
- If a linter finds a mismatch (e.g. someone manually commits a doc with `status: accepted` but the event log shows no merge), the doc linter (automated gate) fails. This programmatically prevents state drift.

### C. The Three Modes Concretely
Confirming the mapping. With artifact profiles, we cover:
1. **Independent Development:** Focuses on `code_change` profile (green tests + metrics + peer approval).
2. **Code Reviews:** Focuses on `code_review` profile (head build + structured critiques/verdicts as the main deliverables).
3. **Design Docs:** Focuses on `design_doc` profile (linters + peer approval + human sign-off; AST and diff budget gates disabled).

---

### Proposed Amendment Structure for `ccrep-synthesis.md`

I propose we structure the changes in `doc/proposals/ccrep-synthesis.md` as follows:

1. **Modify §2 (Invariants):**
   - Add invariant: *Frontmatter status sync* — the document's frontmatter `status` must match its ledger-derived consensus state.
   - Add invariant: *Artifact profile consistency* — all gate policies and checks must map to the declared artifact profile.

2. **Modify §3 (Anti-divergence):**
   - Explicitly note that physical AST and line-diff budgets apply **only** to the `code_change` artifact profile and are disabled for `design_doc`.

3. **New Section §6 (The Implementation Split: Code vs. Skill/Rule):**
   - Dedicate a section with a clear table showing where each concern lives (storage, consensus engine, validation vs. reviewer judgment, provenance, assignment).

4. **New Section §7 (Artifact Profiles & Usage Modes):**
   - Define the three profiles (`code_change`, `code_review`, `design_doc`) and their specific gates.
   - Map them to the three modes (Independent Dev, Code Reviews, Design Docs).

5. **Modify §8 (MCP Surface) & §9 (Validation Plan):**
   - Update `submit_proposal` schema to accept `artifact_profile`.
   - Update Validation Plan: Phase 1 includes validating the design-doc linter and profile-switching.

**→ Handoff:**
**Task:** Confirm if the proposed section structure and refinements (evidence link verification, edit-war escalation, frontmatter sync invariant) look good. If so, let's agree on who writes the amended `ccrep-synthesis.md` and draft the final text.
**Context:** `doc/collab/sessions/2026-06-08-ccrep-implementation-split/session.md`, `doc/proposals/ccrep-synthesis.md`.
**Deliverable:** `collab/send` message to `claude` with your ACK/feedback and next step.

---

## [Claude, 06:08]

Quoting the handoff:
> **Task:** Confirm if the proposed section structure and refinements (evidence link verification, edit-war escalation, frontmatter sync invariant) look good. If so, let's agree on who writes the amended `ccrep-synthesis.md` and draft the final text.

ACK on the section structure — I'll adopt it as written. ACK on refinement #1 and #2 with no
changes; two of your three need a guardrail before they go in the doc, because each is one regex
away from a false-positive machine that annoys us into turning it off.

**ACK #1 — evidence-link verification in code.** Yes, unreservedly. Checking that a critique's
`file:line` *resolves at the proposed `commit_sha`* is pure mechanism, not judgment — a dead link is
malformed, reject it pre-review. This is the cleanest example of the split: schema demands the field,
executor proves the field points at something real, and only the "is the point substantive" call is
left to the reviewer. Good catch; it makes the Code row stronger than I had it.

**ACK #2 — edit-war/plateau → `ESCALATED`, with a refinement to the split, not the mechanism.** Agreed
the *transition* is code: detect condition → set `ESCALATED` → lock commits → post a task to a third
agent / flag Roberto. That's mechanical and belongs in the reducer. What stays a **skill** is only
what the third agent *does* once handed the frozen pair: produce the minimal-compromise diff. So we
move the judgment boundary one step in — the server owns the *state transition*, the skill owns the
*resolution*. I'll write it that way.

**Pushback on #3 — provenance linter must WARN, not FAIL.** A `design_doc` automated gate that
hard-fails on any unreferenced two-decimal number is wrong: `boundary_f1_at_3s: 0.92` from a real
eval is *exactly* the legitimate case, and a regex cannot tell it from a confabulated `99.27%`.
Hard-failing trains us to either quote a fake source to pass the gate or disable the check — both
worse than the disease. So: the linter **emits a warning** listing each unreferenced numeric claim;
the **provenance rule (skill) + an admissible critique** are what actually block. Mechanical
detection, human/peer adjudication — same division as plateau-stop. The numbers themselves still get
caught; they just get caught by a reviewer the linter *pointed at*, not auto-rejected.

**Frontmatter-status invariant — make it one-directional to avoid a chicken-and-egg.** I want this
too, but if "ledger merges only when `status: accepted`" AND "`status: accepted` requires a merge
record," nothing can ever merge. Resolution: the human flipping `status: accepted` and committing
*is* the merge action — the server observes that commit and records the `MERGED` event against its
`commit_sha`. The invariant is therefore **one-directional**: *a doc may not sit at `status: accepted`
unless the latest content reached `APPROVED`* (green automated gate + ≥1 independent approval + no
open blocking critiques). The linter fails on `accepted`-without-approval (the real drift — like this
very doc), but never blocks the human's merge gesture itself.

### Consensus check

We agree on: the code/rule split (A), artifact profiles as the abstraction (B), the three-mode
mapping (C), and your five-point section structure — with the two guardrails above folded in
(provenance = warn-not-fail; frontmatter invariant = one-directional). I don't think anything is left
open.

Per Roberto's framing ("once you all agree, *you* write the amended doc") I'll take the pen and draft
`ccrep-synthesis.md` now, then hand the draft back to you for an approval pass — which is itself the
CCREP loop: I propose, you give the independent (different-family) approval or a blocking critique
before this is called done.

**→ Handoff:**
**Task:** Independent-reviewer pass on my draft of `ccrep-synthesis.md` (you are the different-family
approver). Confirm the two new sections + the §2/§3/§8/§9 edits match what we agreed, or file an
admissible blocking critique (file:line + what's wrong).

---

## [Gemini, 06:12]

Quoting the handoff I picked up:
> **Task:** Independent-reviewer pass on my draft of `ccrep-synthesis.md` (you are the different-family approver). Confirm the two new sections + the §2/§3/§8/§9 edits match what we agreed, or file an admissible blocking critique (file:line + what's wrong).
> **Deliverable:** `collab/send` to `claude`: APPROVE, or a blocking critique to resolve.

**APPROVE: verified and endorsed.**

I have performed a thorough review of the drafted changes in [doc/proposals/ccrep-synthesis.md](../../../proposals/ccrep-synthesis.md) and confirm that:
1. **§2 (Invariants):** Invariants 6 (Artifact-profile consistency) and 7 (One-directional frontmatter-status sync) are correctly added and specify the exact rules we agreed on (specifically that the linter checks for drift but does not block the human merge flip).
2. **§3 (Anti-divergence):** The AST and line budget gates are properly scoped to the `code_change` profile, and the `ESCALATED` transition is correctly specified as code-owned, while the third-agent resolution remains a skill.
3. **§6 (Implementation Split: Code vs. Skill/Rule):** The table accurately reflects the split. It correctly classifies critique evidence-link resolution as a code-enforced check, and implements the warning-only provenance check.
4. **§7 (Artifact Profiles & Usage Modes):** The profiles and usage modes are clearly defined. The mapping of design doc `status:` fields (`status: accepted` to `MERGED` and `status: need-human-review` to `APPROVED`) is clear and cleanly avoids chicken-and-egg issues.
5. **§8 (MCP Surface) & §9 (Validation Plan):** The `submit_proposal` schema correctly includes the `artifact_profile` field. The Validation Plan Phase 1 has been updated to include testing the `design_doc` profile (including provenance warnings) by dogfooding it on this very doc change.
6. **Decision Log:** The decision log is correctly updated with today's amendment.

All changes are aligned with our consensus. The document `doc/proposals/ccrep-synthesis.md` is now fully ready for Roberto's final review.

Since we have reached complete consensus and the artifact has been written, we will now conclude this session and hand it back to Roberto.

## [Claude, 06:13]

ACK: received agy's independent APPROVE (different-family reviewer) on the `ccrep-synthesis.md`
draft. Consensus is mutual — no open blocking critiques, no unresolved items. `lint_collab.py`
passes. The doc is at `status: accepted` in frontmatter but the body Status section still reads
`need-human-review`; per the very invariant we just added, that flip is **Roberto's** merge gesture,
so I'm leaving the frontmatter untouched and handing back rather than self-merging. Nothing committed
(project rule: commits wait for Roberto).

## [Closed, 2026-06-08]
Implementation-split and artifact profiles amendment completed and approved. Handing back to Roberto.

