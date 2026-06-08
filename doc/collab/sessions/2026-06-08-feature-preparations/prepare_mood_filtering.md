# Preparation Plan: Mood Filtering — Fuzzy Logic & Radar UI

## 1. Goal & Requirements
The goal of this feature is to replace the binary "in/out" hard thresholds of the current mood filters with a continuous **Fuzzy Logic Filtering and Ranking Model**. The user interface will center around an interactive **Mood Radar Chart** that allows dragging vertices to shape a target mood profile, rather than setting independent sliders.

### Core Requirements
1. **Interactive Vertex Dragging on Radar**: Enhance the existing `MoodRadar.svelte` component to support dragging vertices on mouse move, providing real-time visual feedback.
2. **Fuzzy Logic Membership calculation**: 
   - A symmetric trapezoidal membership function mapping raw Essentia mood scores (0.0–1.0) to a membership degree (0.0–1.0).
   - Core region (100% membership): $|x - T| \le \text{tolerance} \times 0.5$
   - Shoulder region (decaying to 0%): $\text{tolerance} \times 0.5 < |x - T| < \text{tolerance}$
   - Out-of-bounds (0% membership): $|x - T| \ge \text{tolerance}$
   - Unspecified axes (center at 0.0) are excluded from matching.
3. **Fuzzy Strictness Slider**: Interpolate aggregate match score:
   `final_score = (1 - strictness) * weighted_mean + strictness * strict_minimum`
   where strictness ranges from 0.0 (compensatory average) to 1.0 (strict fuzzy AND).
4. **Ranking and Cutoff**: Rank tracks by fuzzy score. Filter by a "Cutoff Limit" slider (e.g. top 20% matches) instead of a binary threshold.
5. **Saved Presets**: Save and load target radar shapes as named presets stored in the database as saved searches (`type: 'mood_profile'`).
6. **Map Integration ("Mood Match" Color Mode)**:
   - A new map color mode, **Mood Match**, coloring points based on their fuzzy score against the current radar profile using a gray-to-neon gradient.
   - Synchronizes across layouts (Acoustic, Semantic, Mood).

---

## 2. Semantic Hit Rate
The following semantic queries were run to find relevant components and modules:

1. **Query**: `"MoodRadar"` (Similarity: 0.5430)
   - *Match*: `doc/proposals/mood_filtering_ideas.md:Status`
   - *Insight*: Points directly to the existing `MoodRadar.svelte` component (currently read-only) and the need to extend it for interactive dragging.
2. **Query**: `"filters.svelte.ts"` (Similarity: 0.6612)
   - *Match*: `doc/operations/codex-feedback/item-F2-split-svelte-components.md:Goal`
   - *Insight*: Confirms `filters.svelte.ts` as the central state store where the new filter mode, strictness, and cutoff properties must reside.
3. **Query**: `"fuzzy filtering model"` (Similarity: 0.4775)
   - *Match*: `doc/proposals/mood_filtering_ideas.md:Fuzzy Filtering Model`
   - *Insight*: Provides the mathematical details for aggregation and interpolation (AND vs. OR).
4. **Query**: `"Mood Match map coloring"` (Similarity: 0.6710)
   - *Match*: `doc/proposals/mood_filtering_ideas.md:Map Integration`
   - *Insight*: Links the radar filter profile with the interactive canvas overlays in `MusicMap.svelte` and color helpers in `mapMath.ts`.

---

## 3. Impact Assessment

### Database / Schema Changes
- **No migrations needed**: Saved mood profiles can be stored directly within the existing `saved_searches` table. The `query_json` field will serialize the state of the active profile:
  ```json
  {
    "type": "mood_profile",
    "profile": {
      "happy": 0.8,
      "sad": null,
      "aggressive": 0.2,
      "relaxed": 0.6,
      "party": 0.9,
      "acoustic": null,
      "electronic": 0.8
    },
    "tolerance": 0.30,
    "strictness": 0.50,
    "cutoff": 0.20
  }
  ```

### Rust Backend
- **No changes required**: Since all Essentia mood features (`mood_happy`, etc.) are already loaded from the database into the track list on the client side, similarity computations and rankings can be done entirely in the frontend. This maintains 60fps UI performance while avoiding backend IPC round-trips.

### Frontend Svelte 5 / TS
- **`src/lib/stores/filters.svelte.ts`**:
  - Add state variables:
    - `moodFilterMode: 'hard' | 'fuzzy'` (default `'hard'`)
    - `fuzzyStrictness: number` (0.0 to 1.0, default `0.5`)
    - `fuzzyCutoff: number` (0.0 to 1.0, default `0.25`)
    - `moodProfile: MoodValues` (target coordinates per axis)
  - Implement a derived map of fuzzy match scores for all tracks: `moodMatchScores = $derived(Map<number, number>)`.
  - Update `filteredTracks` to rank and slice tracks when `moodFilterMode === 'fuzzy'`.
- **`src/lib/components/MoodRadar.svelte`**:
  - Implement mouse drag tracking in the D3 canvas. Track the closest axis during pointer drag and update the threshold value continuously.
  - Expose a new event `onchange?: (profile: MoodValues) => void`.
- **`src/lib/components/MoodSection.svelte`**:
  - Replace the static sliders layout with a toggle: **Hard Bounds** vs **Fuzzy Match**.
  - Add sliders for **Fuzzy Strictness** and **Match Limit** (Cutoff %).
  - Add a **Presets Dropdown** connected to `saved_searches` to load/save named profiles.
- **`src/lib/components/MusicMap.svelte` & `src/lib/utils/mapMath.ts`**:
  - Add `'mood_match'` to `colorCoding`.
  - Implement `resolveTrackColor` for `'mood_match'` using the reactive fuzzy match score.

---

## 4. Implementation Checklist

### Step 1: Extend the Filters Store (`src/lib/stores/filters.svelte.ts`)
- [ ] Add the following state properties to the store:
  - `moodFilterMode` (`'hard' | 'fuzzy'`)
  - `fuzzyStrictness` (`number`)
  - `fuzzyCutoff` (`number` representing the fraction of tracks to retain, e.g. `0.20` for top 20%)
  - `moodProfile` (`MoodValues` containing target numeric value or `null` per axis)
- [ ] Implement the trapezoidal membership function `getFuzzyMembership(value, target, tolerance)`.
- [ ] Implement a `$derived` helper `trackFuzzyScores` mapping each track ID to its aggregate score based on active dimensions:
  - If no dimensions are active (all `null` or `0`), score is `1.0`.
  - For active dimensions, compute trapezoidal memberships.
  - Compute `weighted_mean` and `strict_minimum`.
  - Apply `final_score = (1 - strictness) * weighted_mean + strictness * strict_minimum`.
- [ ] Integrate fuzzy scoring into `filteredTracks`:
  - If `moodFilterMode === 'fuzzy'`:
    - Apply all non-mood filters.
    - Calculate and attach/sort candidates by their fuzzy score.
    - Filter candidates: keep only those with a score $> 0$ and within the top `fuzzyCutoff` percentile of matches.
    - Sort the final list by fuzzy score descending.
- [ ] Extend `clearAll` to reset the fuzzy filter settings.

### Step 2: Implement Interactive Vertex Dragging in `MoodRadar.svelte`
- [ ] Refactor the D3 interaction handlers in `MoodRadar.svelte` to support mouse dragging:
  - On `mousedown`: check if clicking close to a vertex or an axis. Lock onto that axis.
  - On `mousemove` (while mouse is down): project mouse coordinates onto the locked axis angle to calculate value in range `[0.0, 1.0]`.
  - Emit updates via a new `onchange` or existing `onAxisClick` callback.
  - On `mouseup` / `mouseleave`: release the drag lock.
- [ ] Add touch support for mobile/trackpad swipe gestures.

### Step 3: Upgrade Sidebar UI (`MoodSection.svelte`)
- [ ] Add a Segmented Control/Switch at the top: `Filters Mode: Hard | Fuzzy`.
- [ ] Conditional UI block for `moodFilterMode === 'fuzzy'`:
  - Show the interactive `MoodRadar` connected to `filters.moodProfile` and `filters.moodTolerance`.
  - Add a **Strictness Slider** (`0.0` - "Compensatory OR" to `1.0` - "Strict AND").
  - Add a **Match Limit Slider** (`5%` to `100%` top matches).
  - Add a **Presets Selector**:
    - Query saved searches with `type === 'mood_profile'`.
    - Provide a button to "Save current as Preset" triggering `create_saved_search`.
    - Provide a delete button for custom presets.
- [ ] Keep the existing range sliders/histogram sidebar for `moodFilterMode === 'hard'`.

### Step 4: Map Visualisation & Coloring (`MusicMap.svelte` & `mapMath.ts`)
- [ ] In `src/lib/utils/mapMath.ts`, update `MappedTrackPoint` and `resolveTrackColor` to accept a `'mood_match'` coding.
- [ ] Implement the coloring math: map fuzzy scores (0.0 to 1.0) to a visual gradient.
  - E.g. $0.0 \rightarrow$ theme background/outline gray, $1.0 \rightarrow$ saturated theme primary color.
- [ ] Add `"mood_match"` option to the toolbar under `COLOR` in `MusicMap.svelte` when a fuzzy profile is active.
- [ ] Ensure point colors update dynamically in real time on the map as the user drags the radar vertex.
