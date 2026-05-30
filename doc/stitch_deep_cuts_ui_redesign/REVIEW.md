# Stitch Mockup Review

Review of the three Google Stitch mockups against the proposals in `doc/ui_ideas.md`.
Overall the mockups are well-aligned with the four-zone layout and add significant
production-ready detail, particularly in the design system.

---

## Library Table View

**Strong.** The four-zone layout is correct — filter sidebar left, content center,
detail pane right, player bar bottom.

**What works well:**
- The filter sidebar shows live genre chips, a Camelot key grid, BPM slider, and an
  "Atmospheres" mood axis. The **Camelot key grid** is a materially better affordance
  than the dropdown proposed in the doc — it maps directly to how DJs think about keys
  and enables harmonic compatibility at a glance.
- The track detail pane is well-structured: vinyl graphic, AI description prose, mood
  bars, and technical specs all visible without scrolling. The `44.1 kHz / 24 bit /
  320 kbps / Stereo` block correctly uses the JetBrains Mono monospace treatment from
  the Sonic Glitch design system.
- The player bar is compact and clean. The waveform sits in a dark recessed
  `waveform-bg` — the right visual separation from the glass UI panels above it.

**Gaps:**
- The **"Sounds vs. Feels" similarity slider** is absent from the detail pane. It
  should appear below the AI description prose when a track is selected, with a ranked
  list of similar tracks beneath it.
- The **spectrogram toggle** is not shown in this view. Per `doc/ui_ideas.md` it
  should sit in the player bar as an icon button that expands the bar downward when
  enabled.

---

## Music Map View

**The strongest of the three.** The floating map toolbar with Density Contours,
Instrument Picker, and Pathfinding toggles matches the map-specific controls proposed
for `doc/ui_ideas.md` exactly.

**What works well:**
- The floating selected-track card on the map surface (showing **Dissonance** and
  **Tempo Stability** readouts) is a nice addition not in the original proposal. Both
  fields are good candidates once Essentia data is complete for the full library.
- The right detail pane showing **Acoustic Proximity** results ("Symphony of Psalms",
  "Internal Space") confirms that the similarity list belongs in the detail pane, not
  as a separate panel.
- Dimming out-of-filter tracks rather than hiding them is shown correctly — the doc
  specifies this explicitly to preserve spatial context on the map.

**Gaps:**
- The map itself looks sparse and cramped — this is the **UMAP normalization bug**
  already diagnosed (`standardize_to_100` using absolute min/max rather than
  percentile clipping), not a design problem. It will resolve once the normalization
  fix and energy-based CLAP window selection land.
- **No outlier satellite region** is shown. Expected — that feature is still at
  proposal stage in `doc/music_map_improvements.md`.
- The Sounds vs. Feels slider is again absent from the detail pane on this view.

---

## Analysis View

**Good structure, some fictional content.**

**What works well:**
- The per-pass progress bars with track counts (e.g. "1,946 / 3,086") make the
  pipeline feel live rather than a background black box. This is a meaningful UX
  upgrade over the current scan progress indicator.
- The right pane showing the currently-processing track with real-time BPM, activity
  mode, and mood percentage bars is excellent — it gives the analysis session a sense
  of presence.
- The estimated total remaining time ("30m 44s") is a good touch that the current
  implementation does not compute but should.

**Gaps / corrections needed:**
- The pass names in the mockup are fictional and will need to map to the real pipeline
  passes: `audio_analysis`, `bpm_correction`, `bpm_refinement`, `clap`, `essentia`,
  `qwen`, `description_embed`. "Data Cognitive Training" and "Lifs & Dynamics" should
  become "AI Description (Qwen)" and "Loudness & Dynamics (Essentia)".
- The pass count shown ("3,086 tracks") is higher than the actual library size —
  likely a Stitch placeholder. The real count is 1,886 tracks currently.

---

## Design System — Sonic Glitch

**Production-ready.** The token structure, typography scale, and elevation hierarchy
are complete and can be adopted directly.

**Typography:**
- Inter for all UI chrome and prose — correct.
- JetBrains Mono for technical metadata (BPM, Key, sample rate, file paths) — the
  tabular-numbers feature of JetBrains Mono ensures vertical digit alignment in the
  track table, which the current app lacks.

**Color tokens:**
- The existing app uses `--color-accent-cyan: #00f2fe`. Sonic Glitch defines
  `primary-container: #00f0ff` — essentially the same hue (2nm difference), so
  migration cost is very low.
- The addition of **Studio Pink** (`#ff00ff`) as the secondary accent for AI-generated
  insights and "Feels" metadata is a good semantic distinction — cyan = acoustic /
  technical, pink = AI / semantic.

**Spacing tokens** can go directly into CSS custom properties:
```
--sidebar-width: 260px
--detail-pane-width: 320px
--player-bar-height: 80px
```

**Elevation model** (glassmorphism tonal layering) is cleaner than the current
approach and eliminates the need for drop shadows that look heavy against dark
backgrounds.

---

## Settings / Navigation

The mockup keeps Settings as a top-level navigation tab. The proposal in
`doc/ui_ideas.md` moved it to a native macOS/Windows menu bar item to free up
nav space. The mockup approach is simpler to implement and still functional —
the native menu bar migration can be deferred as a later polish pass rather than
a launch blocker.

---

## Gap Summary

| Proposal | In mockups? | Action |
|---|---|---|
| Four-zone layout (sidebar / content / detail / player) | ✓ | Implement |
| Camelot key grid in filter sidebar | ✓ (better than proposed) | Implement |
| Table / Map / Analysis view toggle | ✓ | Implement |
| Track detail pane with AI description + mood bars | ✓ | Implement |
| Sonic Glitch design system tokens | ✓ | Adopt directly |
| "Sounds vs. Feels" slider in detail pane | ✗ | Add to mockup, then implement |
| Spectrogram toggle in player bar | ✗ (mentioned in brief) | Add to mockup |
| Map floating toolbar (contours / instruments / pathfinding) | ✓ | Implement |
| Outlier satellite region on map | ✗ | Still at proposal stage |
| Native menu bar (Settings out of nav) | ✗ | Defer — low priority |
| Real pass names in Analysis view | ✗ | Fix in mockup before implementation |
| Remaining-time estimate in Analysis view | ✗ (shown but not implemented) | Implement in backend |
