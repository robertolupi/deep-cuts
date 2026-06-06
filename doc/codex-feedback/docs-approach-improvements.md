# Docs and Proposal Improvements

Date: 2026-06-06

## 1. Add proposal lifecycle metadata

Many docs are strong research notes but weak lifecycle records. Some have clear status, while others read like current direction without saying whether they are active, implemented, superseded, or exploratory.

Add frontmatter to proposal docs:

```yaml
---
status: proposed | active | implemented | superseded | deferred | rejected
owner: Roberto
last_verified: 2026-06-06
implemented_by:
superseded_by:
related_code:
related_skills:
---
```

This is especially useful for docs such as `sax_structure_learning.md`, `sax_structural_search.md`, `music_map_improvements.md`, `playlist_view_enhancements.md`, and `feature_feasibility_analysis.md`.

## 2. Separate research results from implementation decisions

Several docs contain experiments, revised architecture, implementation sketches, and future ideas in one file. That is fine during exploration, but later agents need a clear "current decision" section.

Recommended document shape:

1. Status/frontmatter.
2. Current decision.
3. Accepted constraints.
4. Rejected alternatives and why.
5. Implementation plan.
6. Validation plan.
7. Historical experiment notes.

This keeps valuable research while preventing stale early sections from driving implementation.

## 3. Add acceptance criteria to feature docs

Before implementing proposal docs, require an "Acceptance Criteria" section covering:

- user-visible behavior;
- data model and migration impact;
- IPC commands/events and typed frontend boundary changes;
- tests required;
- local-debug/browser verification steps;
- theme/accessibility requirements for UI work;
- migration/sidecar/reset behavior for analysis features.

Good candidates: `music_map_improvements.md`, `playlist_view_enhancements.md`, `statistics_page.md`, `user_edit_song.md`, and structural search docs.

## 4. Normalize collaboration session storage

`doc/collab/PROTOCOL.md` says each session should live in a directory with `session.md`, but `doc/collab/sessions/` still has older flat session markdown files.

Move or archive old flat sessions into directories, or update the protocol to explicitly support both. The directory convention is better for sessions with artifacts, sample data, and scripts.

## 5. Promote durable decisions out of session logs

The collaboration logs are useful, but they are not maintained design docs. Add a closing step to collaboration sessions:

- summarize accepted decisions;
- list rejected alternatives;
- link implementation PR/commits;
- promote durable instructions into `doc/`, `skills/`, or code comments;
- mark the session archived.

This would reduce repeated rediscovery in future agent runs.

## 6. Keep docs synchronized with implementation

Recent history shows docs and implementation moving fast around SAX/structure analysis. Add a lightweight "doc sync" checklist for feature commits:

- If a migration changed, update related design docs.
- If an analysis pass changed, update `skills/add-analysis-pass`.
- If an IPC command changed, update typed frontend command docs/mocks.
- If a proposal was implemented differently than planned, add an "Implemented outcome" note.
- If a feature was removed, mark the proposal or old approach as superseded.

## 7. Make command examples executable from one place

Command examples should state the working directory and avoid mixed `cd` plus repo-root paths. Prefer repo-root examples unless a subdirectory is truly required.

Fix examples that do:

```bash
cd tools
python tools/script.py
```

Prefer:

```bash
tools/.venv/bin/python tools/script.py
```

This matches the skills guidance and is more reliable for agents.

## 8. Add a short architecture map

The repo would benefit from a maintained architecture map under `doc/` or `docs/tech.md` that connects:

- scanner;
- main DB and metrics DB;
- analysis pass registry and execution order;
- model manifest/download flow;
- frontend stores and major surfaces;
- IPC command domains;
- sidecar export/restore.

This would make future reviews and agent work faster, especially when changes span Rust, Svelte, migrations, and docs.
