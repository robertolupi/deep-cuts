---
name: git-commits
description: Rules for writing commit messages in the deep-cuts repository — subject line format, body content, scope naming, and what to omit
---

# Git Commit Messages

Every commit message is permanent project history. Write it for the person (or agent) reading `git log` six months from now, not for the person who just reviewed the diff.

---

## Subject line

Format: `type(scope): imperative verb phrase`

```
fix(ipc): route all Tauri imports through $lib/ipc
feat(analysis): add sleep prevention during pipeline run
docs(feedback): archive C1, F1a, F1b completed items
test(analysis): add pass invariant tests
```

**Type** signals the category of change:

| Type | When to use |
|------|-------------|
| `feat` | New user-visible capability |
| `fix` | Corrects a bug or wrong behavior |
| `refactor` | Internal restructuring, no behavior change |
| `test` | Adding or fixing tests only |
| `docs` | Documentation only |
| `perf` | Performance improvement |
| `tooling` | Dev tools, scripts, lint |
| `chore` | Deps, config, build — nothing a user sees |

**Scope** is the subsystem or domain: `ipc`, `analysis`, `scanner`, `frontend`, `stores`, `db`, `css`, `skills`, `feedback`, `design`. Keep it to one word. If you can't name the scope, the commit may contain multiple logical changes.

**Verb** is imperative, present tense: `add`, `fix`, `replace`, `remove`, `extract`, not `added`, `fixes`, `replacing`.

**Length**: subject under 72 characters. If you can't fit it, the commit is probably too large.

---

## Body

Include a body when the *why* is not obvious from the subject and diff. The diff already shows *what* changed — the body explains the decision, the problem, or the constraint.

```
fix(rust): replace silent filter_map(ok) with visible error handling

filter_map(|r| r.ok()) was swallowing SQLite row-mapping errors across
multiple query paths, making schema drift invisible at runtime. Each
call site now logs the error and the query continues — no behavior
change for callers, but failures are now observable in logs.
```

Good reasons to add a body:
- The fix is non-obvious and a future reader might revert it thinking it's wrong.
- A design decision was made between two plausible alternatives.
- The change has a known limitation or follow-up.
- The commit implements a tracked backlog item — name the item ID.

Bad reasons to add a body:
- Restating what the diff already shows ("changed X to Y in file Z").
- Describing the task context ("as part of the F1 campaign we decided to...").
- Trailing noise ("per review", "as discussed", "various cleanup").

---

## Scope and backlog references

When a commit implements a tracked item from the operations backlog, reference the item ID in the subject or body:

```
feat(ipc): add typed CommandMap with 86 commands (F1b)
fix(stores): make library.init() idempotent and add dispose() (F3)
```

This creates a navigable link between `git log` and `doc/operations/`.

---

## What to omit

- **Do not explain the code.** Well-named identifiers do that. Never write "added a new function called X that does Y".
- **Do not reference the PR, session, or conversation.** These are ephemeral. The commit stands alone.
- **Do not add "various", "misc", "cleanup", or "small fixes"** as the entire subject. Name the specific thing that changed.
- **Do not use past tense.** "Fixed" → "fix", "Added" → "add".
- **Do not pad.** A short, precise subject is better than a long vague one.

---

## One logical change per commit

The subject line is a forcing function. If you cannot write a single clear subject, the commit contains multiple logical changes and should be split.

A commit that touches the same *concern* in multiple files is fine (e.g. "fix(css): replace hardcoded colors with --sg-* tokens in 18 components"). A commit that touches two *unrelated concerns* in the same session should be two commits.

---

## Examples

```
feat(analysis): add HDBSCAN structure clustering pass
fix(download): structured error events, spawn_blocking, group key validation
refactor(backend): modularize commands, extract repositories, decouple coordinators
test(analysis): add pass invariant tests for lifecycle, order, and reset
docs(architecture): add data flow and IPC domain map (D5)
tooling(skills): auto-inject skill table into agent instruction files
chore: add tools/ export scripts and models/ directory scaffold
```
