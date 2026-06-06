# Deep Cuts Docs

This directory holds design notes, feature proposals, research logs, and collaboration records for Deep Cuts.

## Proposal Lifecycle

Feature proposal docs should start with frontmatter so future contributors and agents can tell whether the document is current:

```yaml
---
status: proposed | active | implemented | superseded | deferred | rejected
owner: Roberto
last_verified: YYYY-MM-DD
implemented_by:
superseded_by:
related_code:
related_skills:
---
```

Use these statuses consistently:

- `proposed`: plausible, not committed to implementation.
- `active`: current working direction.
- `implemented`: shipped or merged into the app; include code references.
- `superseded`: replaced by a newer doc or implementation approach.
- `deferred`: intentionally parked.
- `rejected`: evaluated and intentionally not pursued.

## Recommended Proposal Shape

1. Current decision or recommendation.
2. User-visible behavior.
3. Data model, migration, and sidecar impact.
4. IPC commands/events and frontend type impact.
5. Testing and verification plan.
6. Rejected alternatives.
7. Historical experiment notes.

Research notes can stay looser, but once a doc starts guiding implementation it should be promoted into this shape.

## Skill Discovery

Project skills live under `skills/*/SKILL.md`. The discoverable index is [skills/INDEX.md](../skills/INDEX.md), generated from each skill's frontmatter:

```bash
tools/.venv/bin/python tools/generate_skill_index.py
```

Keep skill `name` and `description` frontmatter specific enough for agents to match the right workflow before editing code or docs.

## Collaboration Logs

Multi-agent collaboration sessions live under `doc/collab/sessions/`. Session logs are working records, not durable architecture docs. When a session produces a lasting decision, promote that decision into a normal `doc/` file, a `skills/` file, or code comments as appropriate.
