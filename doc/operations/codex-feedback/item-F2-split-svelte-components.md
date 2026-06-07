# F2: Split Oversized Svelte Components

Source: [codebase-improvements.md](codebase-improvements.md)

## Problem

The largest Svelte files mix data fetching, derivation, rendering, canvas/SVG logic, styling, and IPC in a single file. This makes them hard to test and reason about.

Candidates (largest first, approximate):
- `TrackDetailPane.svelte`
- `MusicMap.svelte`
- `FilterSidebar.svelte`
- `AnalysisPanel.svelte`
- `ChatPanel.svelte`
- `StatisticsPanel.svelte`

## Goal

Start with `FilterSidebar` and `filters.svelte.ts`. Extract pure modules for:

- filter application logic
- saved-search serialization
- structure matching
- sorting
- semantic/CLAP result reduction

Pure modules are cheaper to test and reduce Svelte rune coupling.

## Approach

1. Identify all logic in `FilterSidebar.svelte` that has no DOM/rune dependencies.
2. Extract each chunk to `src/lib/utils/filters/` with unit tests.
3. Replace inline logic in the component with imports from the new modules.
4. Repeat for the next largest component once `FilterSidebar` is stable.

## Files to touch

- `src/lib/components/FilterSidebar.svelte`
- `src/lib/stores/filters.svelte.ts`
- `src/lib/utils/filters/` (new directory)
- `src/lib/components/TrackDetailPane.svelte` (next after FilterSidebar)

## Notes

This is the highest-risk item because `FilterSidebar` is central to the main app flow. Add tests before refactoring, not after. Do not merge until the golden path (filter + sort + semantic search) is manually verified.
