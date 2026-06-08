# CCREP — Known Issues

Found in live use during the `knowledge-manager-design-review` CCREP session (2026-06-08).
Both are now **fixed**; kept here as a resolved log. Each entry: symptom → root cause → fix.

## 1. Split-brain ledger: code default DB path ≠ launcher default — ✅ FIXED (2026-06-08)

**Symptom.** A proposer submitted to one ledger DB; reviewers' `run_evaluation` /
`compute_consensus` returned `unknown proposal` / `reduce_task requires at least one event`. The
proposal was invisible to everyone not using the same entry path.

**How it surfaced.** Codex's first proposal `20f5748d39674222b36095a8c4115dc1` landed in
`scratch/ccrep/ccrep.db`; reviewer servers (via the launcher) read `scratch/ccrep.db`.
Re-submitting through the launcher fixed it at the time.

**Root cause.** Two disagreeing defaults for `CCREP_DB`:
- `tools/ccrep/ledger.py` — `DEFAULT_DB = "scratch/ccrep/ccrep.db"`, used CWD-relative when
  `CCREP_DB` was unset (also wrongly documented in `server.py`, `README.md`, `SKILL.md`).
- `tools/run_ccrep_mcp.py` — `CCREP_DB = <git-common-dir parent>/scratch/ccrep.db`.

So invoking ccrep directly (not via `run_ccrep_mcp.py`) wrote to a different ledger than reviewers
read; worse, it was CWD-relative so even sibling worktrees diverged.

**Fix.** `ledger.db_path()` now resolves the default canonically: `CCREP_DB` wins if set;
otherwise `_canonical_repo_root() / "scratch/ccrep.db"`, mirroring the launcher's
git-common-dir logic. All worktrees now share one ledger regardless of CWD or entry path.
Docstrings/docs corrected in `ledger.py`, `server.py`, `README.md`, `skills/ccrep/SKILL.md`.
Tests: `test_ledger.py::test_db_path_*` (env override wins; default is canonical + CWD-independent).

## 2. Provenance check false-negative: unsourced numeric claim not flagged — ✅ FIXED (2026-06-08)

**Symptom.** The `provenance_warnings` hard check reported "no unreferenced numeric claims" and
passed, even though `codebase-knowledge-manager.md` contained "a simple interpreter run takes
less than 15ms…" — an unsourced, load-bearing numeric claim. A human critique (C6) had to catch it.

**Root cause.** The detector was `_TWO_DECIMAL = re.compile(r"\b\d+\.\d{2,}%?\b")`, which only
matches figures with 2+ decimal places. `15ms` (zero decimals, unit-suffixed) slipped through.

**Fix.** Extracted a testable `executor._numeric_claims(line)` helper that flags three shapes:
multi-decimal figures, **unit-suffixed numbers** (`15ms`, `6.3 GB`, `92%`, `3x`, `2s`), and
**comparator-prefixed numbers** (`<15`, `~6.3`). A trailing `(?![A-Za-z0-9])` boundary plus a
curated unit list keep false positives low (verified: zero spurious hits across the two real
proposal docs; the lone hit was a genuine `0.87` similarity value). Still WARN-only — it lists
findings, never fails the gate. Tests: `test_executor.py` (regression case `15ms`, the old regex's
miss, new positive forms, non-measurement negatives, dedup/order).

---
*Recorded by Claude (Reviewer 1) during the knowledge-manager-design-review CCREP session.
Fixes verified by subagents claude1 (ledger path) and claude2 (provenance), 2026-06-08.*
