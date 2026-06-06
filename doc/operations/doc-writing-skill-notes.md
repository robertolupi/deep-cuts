# Notes Toward a Documentation Skill

Date: 2026-06-06

These are working notes captured during the docs reorganization. They should be consolidated into a `skills/write-docs/SKILL.md` or similar if the pattern keeps paying off.

## Why a Skill Would Help

The repository accumulated many docs in six days across multiple agents. The main failure mode is not bad writing; it is unclear lifecycle state. Brainstorms, implemented architecture, research notes, collaboration logs, and public-facing docs can look equally authoritative when they sit side-by-side in `doc/`.

A docs skill would help future agents decide whether to:

- preserve a brainstorming note as historical context;
- promote a section into architecture docs;
- mark a proposal as `need-human-review`;
- update links after moving files;
- avoid touching protected public-link paths.

## Rules the Skill Should Enforce

1. Read [doc/INDEX.md](../INDEX.md) before reorganizing docs.
2. Do not move protected paths without explicit approval:
   - `doc/collab/PROTOCOL.md`
   - `doc/collab/sessions/2026-06-06-sax-transformer/session.md`
   - `skills/bot-collab/SKILL.md`
   - `models/manifest.json`
3. Treat `doc/private/` as private context; do not rewrite or reorganize it without explicit direction.
4. For unimplemented or mixed brainstorms, use `need-human-review` instead of inventing priority.
5. Keep collaboration `session.md` files as working logs unless the user asks for summarization.
6. After moving docs, update relative links and run a local link check.
7. When implementation differs from the proposal, add an "Implemented outcome" note rather than deleting the old rationale.

## Recommended Doc Shapes

Architecture/current docs:

```markdown
# Feature / Subsystem

## Current State
## Code Map
## Data Model / IPC
## Operational Notes
## Open Follow-Ups
```

Proposal docs:

```markdown
# Proposal

## Status
## Problem
## Proposed UX / Behavior
## Data Model / IPC Impact
## Validation Plan
## Product-Fit Notes
## Decision Log
```

Research docs:

```markdown
# Experiment / Research Topic

## Question
## Method
## Results
## Interpretation
## What Changed in the Product
## Remaining Unknowns
```

## Product-Fit Observations From This Pass

- The strongest docs are the ones that clearly separate observations from decisions, for example Qwen limitations and statistics page notes.
- The riskiest docs are broad roadmap matrices that imply priority from excitement scores. These should stay `need-human-review` until Roberto explicitly picks them up.
- SAX/structure docs are valuable but dense. They probably need a single current architecture summary plus separate historical experiment logs.
- Map and playlist proposals are plausible, but several ideas are UI-heavy and can easily expand the app surface area faster than the core library-management loop. A docs skill should ask for acceptance criteria before implementation.
