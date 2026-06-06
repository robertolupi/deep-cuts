# Documentation Index

Last updated: 2026-06-06

This index is the documentation map after the 2026-06-06 cleanup. It classifies existing documents by inferred status from the docs themselves, code, and git history. Ambiguous or unimplemented ideas are marked `need-human-review`.

## Status Legend

- `implemented`: the feature or workflow exists in code or project process.
- `partially-implemented`: some core pieces exist, but the doc still contains unimplemented proposal material.
- `active-research`: recent research direction or experiment trail, not necessarily product commitment.
- `need-human-review`: brainstorming, unimplemented, mixed, stale, or strategically ambiguous.
- `protected`: keep stable because external/public links depend on the file path/content.
- `private-not-reviewed`: private/blog material left untouched in this pass.

## Protected Paths

Do not reorganize these in phase 2 without an explicit redirect/compatibility plan:

- [doc/collab/PROTOCOL.md](collab/PROTOCOL.md) — linked from public writing.
- [doc/collab/sessions/2026-06-06-sax-transformer/session.md](collab/sessions/2026-06-06-sax-transformer/session.md) — linked from public writing / LinkedIn context.
- [../skills/bot-collab/SKILL.md](../skills/bot-collab/SKILL.md) — protocol companion for public collaboration workflow.
- [../models/manifest.json](../models/manifest.json) — app update/model manifest, not a documentation artifact.

## Current Implementation / Architecture Docs

| Doc | Status | Confidence | Evidence / Notes |
|---|---|---:|---|
| [autotagging.md](architecture/autotagging.md) | implemented | High | Matches implemented `clap`, `essentia`, and `qwen` passes plus `track_tags` behavior. Still useful as system overview. |
| [clap_window_selection.md](research/clap_window_selection.md) | partially-implemented | High | Git history includes adaptive CLAP window selection; later sections propose future per-section CLAP embeddings. |
| [dev-inspector.md](architecture/dev-inspector.md) | implemented | High | `DevDrawer`, `DevHud`, debug IPC, and dev-only enrichment are present. |
| [metrics_monitoring.md](architecture/metrics_monitoring.md) | partially-implemented | Medium | Metrics DB and Metrics Inspector exist; Prometheus-style/export ideas remain proposal material. |
| [qwen_limitations.md](architecture/qwen_limitations.md) | implemented | High | Documents constraints reflected in `analysis/qwen.rs` and chat behavior. Keep near Qwen/chat architecture docs. |
| [statistics_page.md](architecture/statistics_page.md) | implemented | High | Doc explicitly states `StatisticsPanel.svelte` and backend `statistics.rs` are functional. |
| [track-feedback.md](architecture/track-feedback.md) | implemented | High | Chat tab, `ask_qwen`, sessions, and Qwen cross-links exist. |

## Structural Analysis / SAX Research

| Doc | Status | Confidence | Evidence / Notes |
|---|---|---:|---|
| [sax_structure.md](research/sax_structure.md) | partially-implemented | High | `waveform_sax`, SAX pass, structural filters, and map/detail UI exist; `waveform_fingerprint` material is superseded by later migrations dropping it. |
| [sax_structural_search.md](research/sax_structural_search.md) | active-research | Medium | Strong research trail. Some sequence/alignment ideas landed; block composer, repetition vectors, and learned/Viterbi flow need review. |
| [sax_structure_learning.md](research/sax_structure_learning.md) | active-research | Medium | Model options and training experiments are recent; implementation path should be reviewed before more work. |
| [waveform_envelope_analysis.md](research/waveform_envelope_analysis.md) | need-human-review | Medium | Overlaps with SAX/structure search and map projection ideas. Keep as brainstorm until merged into current architecture docs. |
| [clap_window_selection.md](research/clap_window_selection.md) | partially-implemented | High | Also belongs here because future direction depends on structural labels. |

## Feature Proposals / Brainstorms

| Doc | Status | Confidence | Evidence / Notes |
|---|---|---:|---|
| [feature_feasibility_analysis.md](proposals/feature_feasibility_analysis.md) | need-human-review | High | Mixed matrix: one item marked done, others still proposals. Good candidate for phase-2 split into implemented notes vs backlog. |
| [library_maintenance_utilities.md](proposals/library_maintenance_utilities.md) | need-human-review | Medium | Short brainstorm; unclear commitment or implementation status. |
| [map_layouts.md](proposals/map_layouts.md) | partially-implemented | High | Projection modes and transitions are implemented; persisted layout catalog, staleness tracking, and coordinate-stability ideas still need review. |
| [mood_filtering_ideas.md](proposals/mood_filtering_ideas.md) | partially-implemented | Medium | Mood filter/radar UI exists, but fuzzy ranking/radar interaction details should be reviewed against current UX. |
| [music_map_improvements.md](proposals/music_map_improvements.md) | need-human-review | Medium | Map improvements are plausible but mixed with expensive projection/outlier ideas. Feasibility should be reconsidered against current code and performance. |
| [playlist_view_enhancements.md](proposals/playlist_view_enhancements.md) | partially-implemented | High | Playlist ordering and saved-search naming exist; drag UI polish, transition visualizer, optimizer, and recommendation ideas still need review. |
| [roadmap_ideas.md](proposals/roadmap_ideas.md) | need-human-review | High | Explicitly deferred/wishful brainstorming. Keep as backlog source, not commitment. |
| [semantic_feature_brainstorm.md](proposals/semantic_feature_brainstorm.md) | need-human-review | High | Central brainstorm with phased roadmap language; should not drive implementation without human review. |
| [track_comparison_design.md](proposals/track_comparison_design.md) | need-human-review | Medium | Design brainstorm; some supporting primitives exist, but no clear current implementation commitment. |
| [user_edit_song.md](proposals/user_edit_song.md) | partially-implemented | High | User tags and automatic tag suppression exist; broad field-level metadata override schema/UX remains unimplemented proposal material. |

## Model Evaluation / External Research

| Doc | Status | Confidence | Evidence / Notes |
|---|---|---:|---|
| [gemma4_evaluation.md](research/gemma4_evaluation.md) | need-human-review | Medium | Evaluation plan and model comparison. Current app still has Qwen/GGUF paths; Gemma direction needs human decision. |
| [qwen_limitations.md](architecture/qwen_limitations.md) | implemented | High | Keep as current model constraint doc until the model strategy changes. |
| [qwen_eval_results.json](research/qwen_eval_results.json) | active-research | Medium | Raw evaluation artifact. Keep near Qwen/Gemma evaluation material in phase 2. |

## Operations / Process Docs

| Doc | Status | Confidence | Evidence / Notes |
|---|---|---:|---|
| [README.md](README.md) | implemented | High | This doc's local overview and conventions are current. |
| [codex-feedback/README.md](operations/codex-feedback/README.md) | implemented | High | Review output from Codex cleanup pass. |
| [codex-feedback/codebase-improvements.md](operations/codex-feedback/codebase-improvements.md) | need-human-review | High | Recommendation backlog, not implementation commitment. |
| [codex-feedback/completed-improvements.md](operations/codex-feedback/completed-improvements.md) | implemented | High | Completion log for feedback items moved out of active backlog docs. |
| [codex-feedback/docs-approach-improvements.md](operations/codex-feedback/docs-approach-improvements.md) | partially-implemented | High | This index implements part of the recommendation; file reorganization remains phase 2. |
| [codex-feedback/project-opinion-swot.md](operations/codex-feedback/project-opinion-swot.md) | implemented | High | High-level project assessment, SWOT, and product-spine recommendation from the Codex review. |
| [codex-feedback/skills-improvements.md](operations/codex-feedback/skills-improvements.md) | partially-implemented | High | Active skills/process backlog after completed items were moved to `completed-improvements.md`. |
| [collab/PROTOCOL.md](collab/PROTOCOL.md) | protected | High | Publicly linked protocol. Do not move casually. |

## Private / Blog / Outreach Notes

Private docs are intentionally not reorganized in phase 1. They may include audience context that is not reconstructable from code.

| Doc | Status |
|---|---|
| [private/acousticbrainz-exploration.md](private/acousticbrainz-exploration.md) | private-not-reviewed |
| [private/analysis-data-retention.md](private/analysis-data-retention.md) | private-not-reviewed |
| [private/blog-release-announcement.md](private/blog-release-announcement.md) | private-not-reviewed |
| [private/blog_draft_multiagent_collab.md](private/blog_draft_multiagent_collab.md) | private-not-reviewed |
| [private/blog_draft_sax_structure.md](private/blog_draft_sax_structure.md) | private-not-reviewed |
| [private/blog_post_multiagent_collab.md](private/blog_post_multiagent_collab.md) | private-not-reviewed |
| [private/blog_post_sax_structure.md](private/blog_post_sax_structure.md) | private-not-reviewed |
| [private/outreach.md](private/outreach.md) | private-not-reviewed |
| [private/prototyping_strategy.md](private/prototyping_strategy.md) | private-not-reviewed |

## Phase 2 Reorganization Plan

The first physical reorganization is complete:

- `doc/architecture/` for implemented/current technical docs.
- `doc/research/` for SAX, model, CLAP, and experiment notes.
- `doc/proposals/` for `need-human-review` feature brainstorms.
- `doc/operations/` for process and maintenance docs.
- `doc/private/` stays private and should be reorganized only with explicit approval.

Future cleanup should focus on content consolidation rather than more path churn:

1. Review `need-human-review` docs and either promote, defer, supersede, or reject them.
2. Leave protected paths in place.
3. Consider stubs only if old doc paths turn out to be externally referenced.
4. Run a markdown link checker or a local script over `doc/`, `docs/`, `README.md`, `AGENTS.md`, `CLAUDE.md`, and `GEMINI.md` after each docs move.
