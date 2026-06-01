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

## Cross-References

- **Map layouts** (`map_layouts.md`) — the radar target profile should drive the map's mood contour overlay simultaneously. When the user adjusts the radar in the sidebar, the matching density contour lights up on the mood layout in real time — the two UIs are two views of the same query.
- **Saved searches** (`playlists_and_saved_searches.md`) — a named radar preset is a specialised saved search stored as `type: 'mood_profile'` in `query_json`. Presets appear as one-click sidebar shortcuts and can be used as track set inputs in the statistics page.
- **Statistics page** (`statistics_page.md`) — the radar overlay on the statistics summary KPIs (mood centroid thumbnail) uses the same component, making the mood profile of any track set immediately comparable at a glance.
