---
name: write-docs
description: Guidelines for creating, updating, reorganizing, and reviewing Deep Cuts documentation, including doc taxonomy, lifecycle status, protected public-link paths, proposal handling, and link verification.
---

# Writing and Organizing Docs

Use this skill whenever you create, edit, move, classify, or review files under `doc/`, `docs/`, root docs such as `README.md`, or documentation-facing agent instructions.

## Start Here

1. Read `doc/INDEX.md` before reorganizing or classifying docs.
2. Read `doc/README.md` for the current lifecycle vocabulary.
3. If the task touches multi-agent collaboration docs, also read `skills/bot-collab/SKILL.md` and `doc/collab/PROTOCOL.md`.
4. If the task touches public website docs under `docs/`, verify whether `docs/` and `doc/` both need updates.

## Documentation Taxonomy

Use the current folders consistently:

| Folder | Purpose |
|---|---|
| `doc/architecture/` | Implemented/current technical docs and subsystem behavior. |
| `doc/research/` | Experiments, model evaluations, analysis notes, and research trails. |
| `doc/proposals/` | Brainstorms and unimplemented or review-needed product/technical ideas. |
| `doc/operations/` | Process docs, reviews, cleanup notes, and maintenance guidance. |
| `doc/collab/` | Multi-agent collaboration protocol and session logs. |
| `doc/private/` | Private/blog/outreach notes; do not reorganize without explicit direction. |

When a doc mixes categories, prefer preserving it and adding status/context over splitting it immediately.

For partially implemented docs, classify at the smallest useful feature slice. A single proposal can contain implemented storage, partially implemented backend commands, and unimplemented UI. Prefer a compact status table near the top over creating parallel `*_implemented.md` files.

## Lifecycle Status

Use these statuses from `doc/README.md`:

- `implemented`: shipped or merged into the app; include code references.
- `partially-implemented`: meaningful parts exist, but proposal material remains.
- `active-research`: recent research direction or experiment trail, not product commitment.
- `need-human-review`: brainstorming, unimplemented, stale, mixed, or strategic enough that Roberto should decide.
- `superseded`: replaced by a newer doc or implementation approach.
- `deferred`: intentionally parked.
- `rejected`: evaluated and intentionally not pursued.

For unimplemented or ambiguous ideas, use `need-human-review`. Do not invent product priority.

## Protected Paths

Do not move or rewrite these without explicit approval:

- `doc/collab/PROTOCOL.md`
- `doc/collab/sessions/2026-06-06-sax-transformer/session.md`
- `skills/bot-collab/SKILL.md`
- `models/manifest.json`

`models/manifest.json` is app data, not a docs artifact.

Keep collaboration `session.md` files as working logs unless the user explicitly asks for summarization or migration.

## Writing Patterns

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

## Brainstorm Handling

Many docs are intentionally exploratory. Preserve useful history, but make current state obvious:

- Add a "Current State" or "Status" section when a doc still matters.
- Add "Implemented outcome" when code diverged from the original proposal.
- Add "Superseded by" when another doc or implementation replaced the idea.
- Use `superseded` only when code or git history shows replacement/removal; use `need-human-review` for unfinished or strategically ambiguous ideas.
- Leave broad roadmap matrices as `need-human-review` unless Roberto explicitly promotes them.
- If you have product-fit or feasibility concerns, add them as clearly labeled notes, not as hidden rewrites.

## Link and Path Rules

After moving or renaming docs:

1. Update relative markdown links in moved files and files that link to them.
2. Search for plain-text old paths such as `doc/old_name.md`.
3. Leave historical references inside collaboration session logs unchanged unless asked.
4. Run a local link check.

Use this lightweight checker from the repo root:

```bash
tools/.venv/bin/python - <<'PY'
from pathlib import Path
import re
roots=[Path('doc'), Path('README.md'), Path('AGENTS.md'), Path('CLAUDE.md'), Path('GEMINI.md')]
files=[]
for r in roots:
    if r.is_file():
        files.append(r)
    elif r.exists():
        files.extend(r.rglob('*.md'))
missing=[]
link_re=re.compile(r'(?<!!)\[([^\]]+)\]\(([^)]+)\)')
for f in files:
    text=f.read_text(encoding='utf-8')
    for label,target in link_re.findall(text):
        if target.startswith(('http://','https://','mailto:','file://','#')):
            continue
        path=target.split('#',1)[0]
        if not path:
            continue
        if not (f.parent/path).resolve().exists():
            missing.append((str(f), label, target))
print('missing links:', len(missing))
for item in missing:
    print(item)
PY
```

## Skill Index

If you add, remove, rename, or edit frontmatter for a skill, regenerate the skill index:

```bash
tools/.venv/bin/python tools/generate_skill_index.py
```

Then verify `skills/INDEX.md` changed as expected.
