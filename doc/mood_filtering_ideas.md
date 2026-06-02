# Mood Filtering — Fuzzy Logic & Radar UI

## Motivation

Current mood filters apply hard thresholds to continuous Essentia scores. A track scoring 0.68 on "happy" and 0.65 on "relaxed" is indistinguishable from one scoring 0.95 and 0.95 — both pass or fail the same binary test. This loses information and creates jarring cut-offs at filter boundaries.

Fuzzy logic treats mood membership as a matter of degree, which matches how we actually think about music mood. The UI surface for this is a **radar chart** where the user draws a target mood profile rather than setting N independent sliders.

---

## Fuzzy Filtering Model

### Membership functions

Each mood dimension gets a fuzzy membership function mapping a raw Essentia score (0.0–1.0) to a membership degree (0.0–1.0). A trapezoidal function works well:

```
membership
1.0  |     ___________
     |    /           \
0.0  |___/             \___
         low  core  high
```

The user sets the "core" region (full membership) and the shoulders (partial membership) via the radar UI. Tracks in the core are fully in; tracks in the shoulders are partially in; tracks outside are out.

### Track-to-profile similarity

Given a target profile (one membership value per mood dimension) and a track's mood vector, the match score is the fuzzy AND across all dimensions — i.e. the minimum (or weighted average) of per-dimension membership values:

```
score(track, profile) = weighted_mean(membership_d(track.mood_d) for d in dimensions)
```

Dimensions the user leaves at zero (centre of radar) are excluded from scoring — they don't penalise tracks that score high on an "unspecified" mood.

### Ranking, not filtering

Fuzzy matching produces a continuous score per track. Rather than a hard in/out filter, tracks are **ranked** by match score. A cutoff slider controls how many tracks appear (e.g. "show top 20% matches"). This avoids the empty-results problem common with conjunctive hard filters.

### Fuzzy Strictness Slider (AND vs. OR Interpolation)
To give users granular control over how multi-dimensional moods aggregate, a **Fuzzy Strictness Slider** is introduced. This slider interpolates between two classic fuzzy logic aggregation functions:
- **Strict Minimum (Fuzzy AND)**: The track's overall match score is determined solely by its lowest score across the selected dimensions:
  `score = min(membership_d(track.mood_d))`
  This is strict; a track must satisfy *every* single target dimension perfectly to rank high.
- **Weighted Mean (Compensatory OR)**: The track's overall score is the weighted average:
  `score = sum(w_d * membership_d) / sum(w_d)`
  This is compensatory; a track that matches "happy" at 100% but is slightly less "relaxed" than requested can still score very highly because the happy dimension compensates for the relaxed shortfall.
- **Interpolation Formula**: The user can slide continuously between strict (0.0) and compensatory (1.0). The resulting score is computed as:
  `final_score = (1 - strictness) * weighted_mean + strictness * strict_minimum`

---

## Radar Chart UI

A spider/radar chart with one axis per mood dimension. The user drags each vertex to set the target value for that dimension.

```
              happy
               |
    acoustic --+-- party
      /        |        \
 relaxed    [centre]  electronic
      \        |        /
    acoustic --+-- aggressive
               |
              sad
```

### Interactions

- **Drag a vertex** — sets the target membership for that dimension
- **Click centre** — resets all dimensions (no mood preference)
- **Named profiles** — save the current radar shape as a named preset (e.g. "late night", "workout", "focus"). Presets appear as a dropdown above the radar.
- **Opacity ring** — a shaded polygon shows the current track's mood profile overlaid on the target, so you can see at a glance how close it is.

### Alternative UI Profiles
To avoid the visual clutter and readability issues common with multi-axis radar charts (especially for users unfamiliar with polar coordinates), two alternative UI profiles are provided:
- **Split-Bar Dashboard**: A vertical list of clean, dual-slider horizontal trackbars (displaying both raw values and fuzzy transition ranges) that dynamically update match percentages as they are dragged.
- **Parallel Coordinates Sidebar**: A series of parallel vertical axes where a single line representing the currently highlighted track passes through each scale. Users draw "gate" boundaries directly on the vertical lines to filter out tracks that deviate too far from their desired thresholds.

### Dimensions

Initial set from Essentia (7 dims):

| Axis | Essentia column |
|---|---|
| Happy | `mood_happy` |
| Sad | `mood_sad` |
| Aggressive | `mood_aggressive` |
| Relaxed | `mood_relaxed` |
| Party | `mood_party` |
| Acoustic | `mood_acoustic` |
| Electronic | `mood_electronic` |

Qwen2-derived soft dimensions could be added later (e.g. "intimate", "intense", "nostalgic") once a reliable extraction pipeline exists — see `private/acousticbrainz-exploration.md` for the compact embedding ideas that could surface these.

---

## Map Integration

The radar target profile can drive map colouring as a new colour mode: **Mood Match**.

Each dot is coloured on a gradient from grey (no match) to a saturated hue (strong match) based on its score against the current radar profile. The user would literally see the mood cluster they're targeting light up on the map, with surrounding similar tracks visible in softer colour.

This is a natural complement to the radar: define a profile in the sidebar, see where it lives on the map, click a dot to explore neighbours.

### Cross-Layout Color Synergies
The visual feedback of the "Mood Match" gradient is not restricted to the Mood layout. Because the mood match score is computed independently of the layout coordinate space, users can apply the Mood Match color overlay to:
- **Acoustic Similarity Layout**: Visualizes how acoustic clusters (e.g., electronic, guitar-driven rock) correlate with specific mood profiles (e.g., energetic/aggressive vs. melancholic/ambient).
- **Semantic / Vibe Layout**: Allows the user to verify if their Qwen2-Audio free-text clusters align with standard Essentia mood categories. For example, a cluster representing "dusty cinematic vinyl" should light up when the "acoustic" and "sad/relaxed" mood profile is active.
The active color overlay dynamically synchronizes its gradients across layout transitions to maintain a continuous, coherent visual comparison.

---

## Named Mood Profiles (Smart Playlists)

Saved radar profiles are essentially mood-based smart playlists. They could:

- Appear in the filter sidebar as one-click presets
- Be associated with a user tag (tracks that score above threshold automatically get the tag, updated on re-analysis)
- Be exported/imported as simple JSON

This bridges the fuzzy mood system with the tagging system described in `tagging_ideas.md`.

---

## Open Questions

1. **Aggregation function** — weighted mean vs minimum (t-norm) for combining per-dimension membership. Minimum is stricter (all dimensions must match); weighted mean is more forgiving. Expose as a toggle ("strict" / "loose" mode)?

2. **Radar shape** — standard equal-angle radar vs user-reorderable axes? Reorderable is more flexible but complex to implement.

3. **Qwen2 integration** — Qwen2 mood descriptions are free text today. To add them as radar dimensions we need reliable extraction into scalar scores. This may require the compact embedding model from `private/acousticbrainz-exploration.md` to be in place first.

4. **BPM and energy as pseudo-mood axes** — BPM and loudness are not mood dimensions per se but correlate with perceived energy. Worth including on the radar as optional axes, or keep the radar strictly for Essentia mood classifiers?

---

## Agreed Implementation Plan (2026-06-02)

### Decision: sliders for filtering, radar for display only

The radar is **not** used as a filter input — dragging 7 axes simultaneously is too much cognitive load and the 0–1 raw scale is misleading without knowing the library distribution. Instead:

- **Filtering** uses per-mood `RangeSlider` components with a histogram background showing the actual distribution of values in the library. Same pattern as the existing BPM filter.
- **Display** uses a read-only radar in `TrackDetailPane` to replace the current flat mood bars — richer at-a-glance fingerprint.
- **Statistics** keeps the existing radar for set comparison (already works).

### Step 1 — Extract `MoodRadar.svelte`

The working D3 radar lives in `src/lib/components/StatisticsPanel.svelte`, function `renderMoodRadar` (~lines 225–271). Extract it into `src/lib/components/MoodRadar.svelte` with props:

```typescript
moodA: MoodValues;           // primary polygon
moodB?: MoodValues;          // optional overlay
```

Where `MoodValues = { happy, sad, aggressive, relaxed, party, acoustic, electronic: number | null }`.

Replace the inline call in `StatisticsPanel` with `<MoodRadar moodA={...} moodB={...} />`.
Add `<MoodRadar moodA={trackMood} />` to `TrackDetailPane` replacing the flat mood bars.

### Step 2 — Add mood filter state

In `src/lib/stores/filters.svelte.ts`, add 7 optional range pairs (all default to `[0, 1]`):

```typescript
moodHappy:      [number, number] = [0, 1];
moodSad:        [number, number] = [0, 1];
moodAggressive: [number, number] = [0, 1];
moodRelaxed:    [number, number] = [0, 1];
moodParty:      [number, number] = [0, 1];
moodAcoustic:   [number, number] = [0, 1];
moodElectronic: [number, number] = [0, 1];
```

Wire them into the `filteredTracks` derived computation (same pattern as `minBpm`/`maxBpm`).

### Step 3 — Extend `RangeSlider.svelte` with histogram

`src/lib/components/RangeSlider.svelte` already has `min`, `max`, `step`, `minValue`, `maxValue`, `unit`. Add an optional `distribution` prop:

```typescript
distribution?: number[];  // normalised bucket heights 0–1, length = number of bins
```

Render thin semi-transparent bars behind the track fill. Derive the histogram from `library.tracks` on the frontend — no backend changes needed.

### Step 4 — Add mood sliders to `FilterSidebar.svelte`

Add a collapsible "MOOD" section after the BPM section in `src/lib/components/FilterSidebar.svelte`. One `RangeSlider` per mood dimension, each showing the library distribution behind it. Only show when at least one track in the library has mood data (`coverage_mood > 0`).

Also retrofit the existing BPM slider to use the histogram background once Step 3 is done.

### Key files

| File | Role |
|------|------|
| `src/lib/components/StatisticsPanel.svelte` | Source of `renderMoodRadar` to extract |
| `src/lib/components/TrackDetailPane.svelte` | Add read-only `MoodRadar` replacing flat bars |
| `src/lib/components/RangeSlider.svelte` | Extend with `distribution` prop |
| `src/lib/components/FilterSidebar.svelte` | Add mood sliders section |
| `src/lib/stores/filters.svelte.ts` | Add mood range state + filtering logic |
| `src/lib/stores/library.svelte.ts` | Source of `library.tracks` for histogram data |

No backend changes, no new IPC commands, no DB migrations needed.

---

## Implementation Note — Shared MoodRadar Component

`StatisticsPanel.svelte` already contains a working D3 radar implementation (`renderMoodRadar`) that renders one or two mood polygons on a fixed 7-axis chart. Before building the filtering UI, this should be extracted into a reusable `MoodRadar.svelte` component accepting:

```typescript
// props
moodA: MoodValues;          // primary polygon (e.g. track or set A)
moodB?: MoodValues;         // optional overlay polygon (set B or target profile)
interactive?: boolean;      // if true, vertices are draggable (for filter mode)
onchange?: (v: MoodValues) => void;  // emitted when user drags a vertex
```

`TrackDetailPane.svelte` would use it in read-only mode (`interactive=false`) to replace the current flat mood bars with a compact radar, giving a much richer at-a-glance mood fingerprint.

`StatisticsPanel.svelte` would replace its inline `renderMoodRadar` call with the same component in read-only mode with two polygons (set A + set B).

The filter sidebar would use it in interactive mode, with `onchange` driving `filters.moodProfile`.

This refactor should happen before adding mood filter state, to avoid duplicating the rendering logic a third time.

---

## Cross-References

- **Map layouts** (`map_layouts.md`) — the radar target profile should drive the map's mood contour overlay simultaneously. When the user adjusts the radar in the sidebar, the matching density contour lights up on the mood layout in real time — the two UIs are two views of the same query.
- **Saved searches** (`playlists_and_saved_searches.md`) — a named radar preset is a specialised saved search stored as `type: 'mood_profile'` in `query_json`. Presets appear as one-click sidebar shortcuts and can be used as track set inputs in the statistics page.
- **Statistics page** (`statistics_page.md`) — the radar overlay on the statistics summary KPIs (mood centroid thumbnail) uses the same component, making the mood profile of any track set immediately comparable at a glance.
