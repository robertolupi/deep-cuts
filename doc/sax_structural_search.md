# SAX Structural Search — Design & Experiments

## Goal

A visual block composer UI that lets users search the library by song architecture ("find tracks with a quiet intro, then a chorus, then a drop") without writing regex. The user composes a named-block sequence; the backend finds tracks whose structural arc best matches it.

> **Architecture revision (post-experiment):** The backend is not a regex filter — it is a
> nearest-neighbour sequence matcher. Each track is quantized to a time-ordered label sequence
> (e.g. `[I][I][V][V][C][V][C][O]`). A user query is matched against these sequences using
> edit distance with centroid-weighted substitution costs. Tracks are returned sorted by match
> cost, not filtered by threshold. See Implementation Plan for details.

## Block → RLE mapping (proposed)

> **Updated after experiments** — see Experiment Results below. Energy alone is insufficient;
> sections are best characterised by two axes: **energy** and **repetition score**.

| User block | Energy level | RLE token | Notes |
|---|---|---|---|
| Intro | low | `L` (anchor `^`) | Must be at track start |
| Outro | any | `.*` (anchor `$`) | Often ends loud — don't assume quiet |
| Verse | low–mid | `[LM]` | Quiet but *repeated* |
| Pre-Chorus | mid | `M` | Mid-energy, highly repeated |
| Chorus | high | `H` | Loud and *repeated* |
| Drop | high→low→high | `HLH` | Loud / breakdown / loud |
| Bridge | mid–high | `[MH]` | Energetic, somewhat repeated, contextually unique |
| Break | low | `L` | Instrumental breakdown, quiet |
| Build | ascending | `L.*M.*H` | Monotonic ramp |
| Any | wildcard | `.*` | Skip / don't care |

Glue between blocks is always `.*` (order matters, gaps allowed). A "strict" mode with no glue is a future option.

## Validation dataset

174 Downspiral tracks in `~/Downloads/MP3 Songs/` each have a `lyrics.txt` with structured section labels:

```
[Intro]
[Verse 1]
[Pre-Chorus]
[Chorus]
[Verse 2]
[Bridge]
[Outro]
```

Aggregate label frequency across all 174 tracks:

| Label | Count |
|---|---|
| Chorus | 189 |
| Verse 2 | 85 |
| Verse 1 | 74 |
| Pre-Chorus | 60 |
| Bridge | 59 |
| Outro | 33 |
| Intro | 27 |
| Final Chorus | 14 |
| Verse | 15 |

These tracks are also in the Deep Cuts library (scanned from a watched folder), so we have both `waveform_sax` and lyrics labels for the same files — a rare alignment.

## Experiment Results (June 2025)

### Method

For each of 153 Downspiral tracks with `lyrics.txt` ground-truth labels:

1. Build a **self-similarity matrix (SSM)** from the 128-bin waveform using 8-bin sliding windows and cosine similarity.
2. Compute a **repetition score** per segment: mean cosine similarity to its k=3 nearest non-adjacent neighbours, normalised to [0,1] within each track.
3. Map each lyrics section label to its waveform position (line number / total lines) and read off the **SAX energy** and **repetition score** at that position.

### Two-feature centroids

| Section | Energy μ | Rep score μ | Energy σ | Rep σ | n |
|---|---|---|---|---|---|
| Intro | 0.016 | 0.466 | 0.072 | 0.345 | 93 |
| End | 0.237 | 0.275 | 0.344 | 0.324 | 38 |
| Verse | 0.338 | 0.706 | 0.332 | 0.317 | 308 |
| Pre-Chorus | 0.447 | 0.845 | 0.309 | 0.191 | 131 |
| Bridge | 0.556 | 0.797 | 0.343 | 0.248 | 130 |
| Outro | 0.617 | 0.588 | 0.398 | 0.401 | 103 |
| Chorus | 0.647 | 0.875 | 0.313 | 0.190 | 358 |

Energy scale: a=0, b=0.25, c=0.5, d=0.75, e=1.0 → averaged per section position.

### Key findings

1. **Intro is almost perfectly distinctive**: energy=0.016 (near silence), repetition=0.466 (not particularly repeated). The `^L` RLE pattern has near-100% recall.

2. **Energy alone is insufficient**: Chorus (0.647) and Outro (0.617) have nearly identical energy centroids. Pre-Chorus (0.447) and Bridge (0.556) overlap. You cannot distinguish them from SAX letters alone.

3. **Repetition score cleanly separates the rest**:
   - High-rep (>0.8): Pre-Chorus, Chorus, Bridge — these are the *hook* sections that recur
   - Mid-rep (0.5–0.8): Verse, Outro
   - Low-rep (<0.5): Intro, End — structurally unique moments

4. **Outro ends loud in Downspiral tracks**: energy=0.617, not quiet. The `L$` RLE pattern would miss most outros. Outro is better characterised by position (terminal) + mid repetition than by energy.

5. **Bridge vs Chorus are close** but bridge has slightly lower energy and slightly lower repetition. In practice, distinguishing them in a search UI may not be worth the complexity — users rarely search for "tracks with a bridge."

6. **Standard deviations are high** (especially Outro σ=0.40), meaning section character varies significantly between tracks. Any block search must be fuzzy, not exact.

### Implications for block composer

The block composer cannot rely on RLE regex alone. The backend needs to:

- Compute a **per-track repetition score vector** at query time (or cache it)
- Match blocks using **2D thresholds** (energy × repetition), not 1D letter matching
- Anchor Intro to `^` and terminal blocks to `$` by position, not energy

Alternatively: store the repetition-scored segment labels as a new column (e.g. `waveform_structure TEXT`) and match against that. This is a richer signal than `waveform_sax` alone.

### Energy-only recall (baseline for comparison)

| Pattern | Description | Ground truth | Recall | Precision |
|---|---|---|---|---|
| `^L` | Starts quietly | 91 | 100% | 5% |
| `H` | Has loud section | 147 | 100% | 8% |
| `L$` | Ends quietly | 98 | 83% | 6% |
| `^L.*H` | Intro + Chorus | 87 | 100% | 5% |
| `^L.*H.*L$` | Intro + Chorus + Outro | 80 | 80% | 5% |

Recall is high but precision is ~5% — almost every track matches. Combining with repetition score should improve precision dramatically.

## Experiments to run

### 1. Section label → energy level correlation

For each Downspiral track:
1. Parse `lyrics.txt` to extract the sequence of section labels and their order.
2. Estimate the *position* of each section as a fraction of track duration (line number / total lines as a proxy, or character count).
3. Map that position to the corresponding SAX letter (using `waveform_sax` and 32-segment grid).
4. Build a distribution: for each label type, what SAX letters appear at that position?

Expected hypothesis:
- `[Chorus]` → mostly `d` or `e` (high energy)
- `[Intro]` / `[Outro]` → mostly `a` or `b` (low energy)
- `[Verse]` → mostly `b`, `c` (low–mid)
- `[Bridge]` → mixed (contrast-dependent)

If the distributions are clean and separable, the block → RLE mapping above is well-founded. If not, we need to rethink (e.g. Verse and Chorus overlap too much).

### 2. RLE pattern recall

For each track, derive its RLE string (collapse consecutive SAX letters into L/M/H runs). Then check:
- What % of tracks with `[Intro]` label have `^L` in their RLE?
- What % of tracks with `[Chorus]` label have at least one `H` run?
- What % with both `[Intro]` and `[Chorus]` match `^L.*H`?

These are precision/recall numbers for the block patterns. If `^L.*H` recall is >70% for "intro + chorus" tracks, the pattern is usable.

### 3. False positive rate

Run `^L.*H` across the full library (not just Downspiral). How many non-Downspiral tracks match? Are they false positives (no quiet intro in reality) or true positives (we just lack labels for them)?

A spot-check of 10 false-positive candidates would reveal whether the pattern is too loose.

### 4. Collapsed SAX string fingerprints

Compute the RLE-collapsed SAX for all 174 tracks (drop consecutive duplicates, map to L/M/H). Look for the most common fingerprints:

```
LHLH   → clear verse/chorus alternation
LH     → simple intro-body
LHL    → intro / body / outro
HLH    → drop structure
LMHL   → build / peak / release
```

If a small number of fingerprints cover most tracks, those become the preset "template" patterns in the UI.

## UI concept

A horizontal block lane with a palette of named blocks:

```
[ Intro ]  [ Verse ]  [ Pre-Chorus ]  [ Chorus ]  [ Drop ]  [ Bridge ]  [ Outro ]  [ ··· ]
              ↓ user composes ↓
  ┌────────┬────────┬─────────┐
  │ Intro  │ Chorus │  Outro  │
  └────────┴────────┴─────────┘
  compiled regex: ^L.*H.*L$
  → 23 tracks match
```

**Key decisions (pending experiment results):**

- Are blocks repeatable? (`Chorus` + `Chorus` = must have two H peaks)
- Strict mode (no `.*` glue) for exact arc matching?
- Is this a filter (narrows track list) or a search (ranked by match quality)?
- Should the composer show a preview waveform color-coded by block assignment?

## Implementation plan (post-validation)

### Phase 0 — Pipeline consolidation (prerequisite, do first)

Merge SAX and repetition vector computation into the `audio_analysis` pass so all three
structural features are computed while the waveform is hot in memory and written atomically.

**Why:** `SaxPass` currently runs as Phase 1c — it reads `waveform_data` back from disk
immediately after `audio_analysis` wrote it. At library scale this is a needless I/O round-trip.
The fix eliminates an entire disk read per track.

**Schema addition:**
```sql
-- migrations/25_waveform_repetition.sql
ALTER TABLE tracks ADD COLUMN waveform_repetition TEXT; -- JSON array of 16 floats
```

**Implementation sketch (Rust):**
```rust
// Inside audio_analysis, after waveform_data is computed in memory:
let waveform_sax = sax::waveform_to_sax_from_bins(&waveform_bins);
let waveform_repetition = ssm::repetition_vector(&waveform_bins); // Vec<f32> len=16
// Write all three columns in one UPDATE
```

**On retiring SaxPass:** Rather than deleting `sax.rs`, keep `waveform_to_sax` and
`sax_mindist` as library functions called from `audio_analysis`. `SaxPass` itself can be
retired from the pipeline registry. This preserves independent re-runnability if the SAX
algorithm changes without requiring a full re-decode — just bump `audio_analysis` version
to trigger a re-run of all three columns together.

**What to store:** `waveform_repetition` as a JSON TEXT array (16 floats). Small enough
that BLOB offers no meaningful performance advantage, and TEXT is inspectable in the DB.

### Phase 1 — Segment quantization + sequence matching (prototype first)

The core of the backend, before any UI is built:

**Step 1 — Per-track label sequence**
For each track, use the 16 SSM segments and their (energy, repetition) coordinates to assign
the nearest centroid label to each segment. Result: a time-ordered label sequence, e.g.
`[I, I, V, V, C, V, C, O]` with per-segment confidence (distance to assigned centroid).

Centroid table (from 153-track experiment):

| Label | Energy μ | Rep μ |
|---|---|---|
| Intro | 0.016 | 0.466 |
| End | 0.237 | 0.275 |
| Verse | 0.338 | 0.706 |
| Pre-Chorus | 0.447 | 0.845 |
| Bridge | 0.556 | 0.797 |
| Outro | 0.617 | 0.588 |
| Chorus | 0.647 | 0.875 |

Store as `waveform_labels TEXT` (JSON array of 16 label strings) alongside
`waveform_repetition` in migration 25.

**Step 2 — Fuzzy sequence matching (edit distance)**

Match a user query sequence against all track label sequences using Levenshtein with
substitution costs weighted by 2D centroid distance:

```
cost(substitute A → B) = euclidean_distance(centroid_A, centroid_B) / max_dist
cost(insert)   = 0.5   # gaps in the track sequence are cheap (sections may repeat)
cost(delete)   = 1.0   # dropping a user-specified block is expensive
```

A segment at (0.5 energy, 0.8 rep) is near Bridge — substituting it for Bridge costs
less than substituting it for Intro. Tracks are returned sorted by total match cost.

**Step 3 — Position anchoring**

Before matching: force any `Intro` block in the query to only match segments within
the first 15% of the track (segments 0–2 of 16); `Outro` / `End` only within the last 15%
(segments 13–15). Score the middle freely. This is more reliable than energy for these labels.

**Minimal pattern language (for prototyping without UI):**
```
PATTERN   = [I, V, C, V, C, O]
TOLERANCE = 1 substitution, 0 deletions
ANCHOR    = first block → first 15%, last block → last 15%
```

**Prototype experiment results (June 2025, 1890 tracks):**

Ran two cost regimes against all tracks. Key findings:

*Label distribution after quantization:*
C (Chorus) 40%, P (Pre-Chorus) 29%, V (Verse) 14%, E (End) 7%, O (Outro) 5%, I (Intro) 5%, B (Bridge) 1%.
Pre-Chorus is over-represented — its centroid (0.447, 0.845) is very close to Chorus (0.647, 0.875),
causing many segments to bleed into P.

*Query results with strict costs (ins=1.5, del=2.0):*

| Query | Notable matches | Quality |
|---|---|---|
| [I, V, C, V, C, O] | Rammstein, Lou Reed, Céline Dion, Guns N' Roses | ✅ genre-diverse, structurally appropriate |
| [I, V, C] (build) | Vangelis, Nine Inch Nails, Elisa | ✅ genuinely building tracks |
| [C, V, C] (drop) | Bach Goldberg, Tchaikovsky, classical | ⚠️ false positives — loud→quiet→loud pattern in classical |
| [C, C, E] (ends quietly) | Madonna, Chuck D, Paul McCartney | ✅ reasonable |

*Remaining problems:*
1. **Costs quantize to discrete values** — many tracks tie at 2.50 or 6.50. Confidence is
   not effectively breaking ties. Need continuous cost or a soft scoring function.
2. **Pre-Chorus centroid too close to Chorus** — consider merging P into V/C or recalibrating
   on a broader, genre-diverse labeled set.
3. **Classical false positives on [C, V, C]** — loud orchestral movements look like choruses.
   A genre-confidence signal or minimum track length filter may help.

**Precision risk:** The 7 centroids were derived from 153 Downspiral tracks, which may be
more structurally regular than the broader library. EDM builds, classical movements, and
ambient tracks will produce noisy label sequences. A per-segment confidence score (distance
to assigned centroid) doubles as a track-level reliability signal — low mean confidence =
"this track's structure is ambiguous, deprioritize in results."

**IPC command (Phase 1b — after prototype validates):**
```typescript
search_by_structure(blocks: string[], tolerance: number) → { track: Track, cost: number }[]
```

### Phase 2 — Block composer UI

1. Svelte component: palette of named blocks + horizontal lane
2. Blocks are clickable to append, draggable to reorder, click-to-remove
3. Live result count + sorted result list (by match cost, not binary filter)
4. Waveform overlay: show inferred `[I][V][C]` labels on the waveform with SAX coloring
   so the user can see *why* a track matched
5. Integrate into filter sidebar alongside CLAP/semantic search

**Key decisions (still open):**
- Tolerance exposed to user (slider: exact → fuzzy) or fixed internally?
- Show match cost as a percentage score in the result list?
- Should repeated blocks (`[C, C]`) require two distinct high-rep peaks or allow overlap?

## Files

- `waveform_sax` column: `src-tauri/migrations/24_waveform_sax.sql`
- SAX computation: `src-tauri/src/analysis/sax.rs`
- MINDIST blending: `src-tauri/src/commands/map.rs` (`blended_embedding_distance`)
- Waveform coloring: `src/lib/components/TrackList.svelte`
- Related: `doc/sax_structure.md`, `doc/clap_window_selection.md`
