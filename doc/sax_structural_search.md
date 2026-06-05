# SAX Structural Search — Design & Experiments

## Goal

A visual block composer UI that lets users search the library by song architecture ("find tracks with a quiet intro, then a chorus, then a drop") without writing regex. Blocks map to named musical sections; the composer compiles them to an RLE regex over the `waveform_sax` column.

## Block → RLE mapping (proposed)

| User block | Energy level | RLE token | Notes |
|---|---|---|---|
| Intro | low | `L` (anchor `^`) | Must be at track start |
| Outro | low | `L` (anchor `$`) | Must be at track end |
| Verse | low–mid | `[LM]` | Verses are typically quieter than chorus |
| Pre-Chorus | mid | `M` | Build-up section |
| Chorus | high | `H` | Peak energy |
| Drop | high→low→high | `HLH` | Loud / breakdown / loud |
| Bridge | low–mid | `[LM]` | Contrast section, similar energy to verse |
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

1. Add `waveform_rle` derived column (or compute on the fly from `waveform_sax`)
2. Implement block composer Svelte component (palette + lane + live count)
3. IPC command: `search_by_structure(blocks: string[]) → Track[]`
4. Backend: compile block list to regex, run against `waveform_sax` via sqlite REGEXP or in-memory Rust scan
5. Integrate into filter sidebar alongside CLAP/semantic search

## Files

- `waveform_sax` column: `src-tauri/migrations/24_waveform_sax.sql`
- SAX computation: `src-tauri/src/analysis/sax.rs`
- MINDIST blending: `src-tauri/src/commands/map.rs` (`blended_embedding_distance`)
- Waveform coloring: `src/lib/components/TrackList.svelte`
- Related: `doc/sax_structure.md`, `doc/clap_window_selection.md`
