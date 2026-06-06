# SAX-Based Track Structure Analysis

## Current State

SAX moved from experiment into the app, but several early schema and query ideas were superseded by later structure-cluster work.

| Area | Status | Evidence / Notes |
| :--- | :--- | :--- |
| Waveform SAX extraction | Implemented | The app stores and computes SAX strings from waveform envelopes. |
| SAX alignment and segment storage | Implemented | Later migrations added alignment and alignment-segment tables used by structure views and filters. |
| Structure clusters | Implemented | Structure cluster analysis and UI/filter integration are present. |
| `waveform_fingerprint` column | Superseded | A later migration drops this field; treat fingerprint-specific sections as historical unless revived. |
| Two-stage structural search | Partially implemented | The data substrate exists, but block sketch/query and full blended DTW+CLAP search UX remain design/research material. |

---

## Background

SAX (Symbolic Aggregate approXimation, Lin et al. 2003) converts a time series into a short string over a small alphabet by:

1. **PAA** (Piecewise Aggregate Approximation) — divide the series into equal-length segments and take the mean of each. Our 128-bin RMS waveform envelope is essentially PAA already computed by the scanner.
2. **z-normalisation** — subtract mean, divide by standard deviation, so the shape is compared independently of absolute loudness.
3. **Quantization** — map each segment mean to a letter using breakpoints derived from a Gaussian distribution, so each letter is equally probable. With a 5-letter alphabet (a–e): a = very quiet, b = quiet, c = medium, d = loud, e = very loud.

The result is a ~32-character string per track that encodes the *structural shape* of the energy envelope.

## Why it's useful

CLAP embeddings capture timbral/acoustic similarity ("sounds like"). SAX captures architectural similarity ("is built like"). Two tracks can sound completely different but share the same structural pattern — e.g. a jazz ballad and an electronic track both following `quiet intro → build → loud body → fade`. These are complementary signals.

Additionally the strings are human-readable, queryable with regex, and cheap to compute and store.

## Prototype (June 2025)

Script: `tools/waveform_explorer.py` (Streamlit)  
Implementation: `scripts/compare_clap_windows.py`

### Alphabet & segmentation

- 5-letter alphabet (a–e)
- 32 segments over the 128-bin waveform (each segment = 4 bins ≈ 1/32 of track duration)
- After SAX, collapse to Run-Length Encoded L/M/H string for pattern matching:
  - `a–b` → L (low)
  - `c` → M (mid)
  - `d–e` → H (high)

### A note on compression

The waveform envelope is already a lossy fixed-length representation: 128 bins regardless of track duration. A 3-minute track and a 10-minute track both produce a 128-bin waveform. The SAX string compresses this further to 32 segments, so each character represents ~1/32 of the track's duration — not a fixed time window.

This means the *length* of a run in the SAX string carries no absolute temporal information, only relative proportion. Consecutive-duplicate dropping (e.g. `aabbccdd` → `abcd`) is therefore lossless in the meaningful sense: the sequence of structural events is preserved, but the already-meaningless run lengths are discarded. This makes the compressed form well-suited for structural matching — two tracks with the same arc but different tempos or lengths will produce the same or similar collapsed strings.

### Structural patterns detected via regex on RLE

| Pattern | Regex | Meaning |
|---|---|---|
| Verse/chorus | `(LH){2,}` or `(HL){2,}` | Alternating quiet/loud sections |
| Drop | `H+L+H+` | Loud → breakdown → loud |
| Quiet intro | `^L{2,}` | Track opens quietly |
| Quiet outro | `L{2,}$` | Track fades out quietly |
| Ramp up | monotonic increase across checkpoints | Build throughout |
| Ramp down | monotonic decrease | Fades/deconstructs |
| Flat | all same letter | Brickwall mastered or silence |

### Library breakdown (1891 tracks, tags non-exclusive)

| Tag | Count | % |
|---|---|---|
| verse-chorus | 831 | 43% |
| drop | 811 | 42% |
| quiet-intro | 761 | 40% |
| distributed | 340 | 17% |
| quiet-outro | 235 | 12% |
| ramp-up | 214 | 11% |
| ramp-down | 121 | 6% |
| flat | 1 | 0% |

Note: tags overlap (most tracks match 2–3 patterns). The primary use is in combination.

### Example SAX strings

```
LHLHLHLH  →  clear verse/chorus (e.g. pop, rock with regular structure)
LMHL      →  quiet intro / build / loud body / outro
HMHL      →  drop structure (loud / breakdown / loud)
LMHMHMHL  →  multiple chorus peaks with verses between
```

## Application to window selection

The SAX pattern directly informs which 10-second windows are most representative for CLAP embedding:

| Pattern | Window strategy |
|---|---|
| flat | temporal spread (0.15 / 0.50 / 0.85) |
| ramp-up / ramp-down | temporal spread (energy tercile degenerates to temporal anyway) |
| quiet-intro only | skip first L-run, then tercile on remainder |
| quiet-outro only | skip last L-run, then tercile on remainder |
| verse-chorus / drop | tercile (low=verse, high=chorus/drop) |
| distributed | tercile |

The `quiet-intro` and `quiet-outro` tags are particularly valuable — they let us explicitly exclude fade-ins and fade-outs from window candidates, which the current CV-based approach misses.

## Pairwise Levenshtein experiment (June 2026)

Script: `tools/sax_levenshtein_histogram.py`

Computed all 1,785,105 pairwise Levenshtein distances over 1,890 SAX strings using `rapidfuzz.process.cdist` (workers=-1). Completed in **0.03 s**.

### Uniqueness

| column | non-null | distinct | duplicates | uniqueness |
|---|---|---|---|---|
| `waveform_data` | 1891 | 1885 | 6 | 99.7% |
| `waveform_sax` | 1890 | 1864 | 26 | 98.6% |

### Distance distribution

- `min=0`, `max=32`, `mean=20.7`, `median=21`
- Distribution is a smooth bell curve centred around 20–21 with a sparse left tail below ~12.
- **No bimodal structure** — there is no natural gap separating "similar" from "dissimilar" tracks.

### Threshold analysis (d ≤ 8 → 355 pairs)

| distance | signal |
|---|---|
| d = 0 | Exact SAX duplicates — same track in two formats (.aif/.mp3), re-rips, or near-identical masters. Clean duplicate signal. |
| d = 1–2 | Near-duplicates: slightly different masters, alternate versions, metadata variants. Still very reliable. |
| d = 3–5 | Mostly still duplicates or alternate versions, some noise begins. |
| d = 6–8 | Noisy — coincidental waveform-shape overlap (e.g. Guru/MC Solaar paired with Judas Priest due to similar loudness envelopes). |

**Conclusion**: plain Levenshtein on SAX strings is an excellent **duplicate detector** (d ≤ 3) but a poor **structural similarity metric** beyond that. All substitutions are treated as equal cost, so `a→e` (quiet→loud) costs the same as `a→b` (quiet→slightly-less-quiet). This collapses musically distinct patterns into the same distance bucket.

### Distance metric experiments summary

All metrics tested — plain Levenshtein (raw), weighted Levenshtein (raw), plain Levenshtein (RLE-compressed), weighted Levenshtein (RLE-compressed), DTW — produce the same unimodal bell curve with no natural gap. No threshold cleanly separates "structurally similar" from "unrelated". This rules out distance-threshold clustering as a standalone approach.

DTW (`dtaidistance`, full matrix in **0.2s**) produces the most semantically coherent low-distance pairs: sustained-energy tracks cluster together, quiet-intro/build tracks cluster together. The signal is real but weak relative to the noise floor given the 5-letter alphabet and 2k library size.

**Conclusion**: waveform SAX is useful for structural similarity as a *pre-filter + reranking signal*, not as a standalone distance metric.

## Arc shape search + nearest-neighbour reranking

The design that emerges from the experiments above:

### Stage 1 — Arc shape filter (RLE regex)

Use the existing structural pattern tags (verse-chorus, drop, quiet-intro, ramp-up, etc.) as hard candidate filters. O(n), already implemented. A query track's RLE pattern becomes the search key — candidates are tracks sharing the same structural archetype. Strictness is a tunable parameter to experiment with:

- **Strict**: exact RLE pattern match
- **Loose**: match by tag set (e.g. "has verse-chorus AND quiet-intro")
- **Looser**: match on one dominant tag only

### Stage 2 — DTW reranking

Within the filtered candidate pool, compute DTW distance against the query's SAX ordinal sequence and sort ascending. At ~200–400 candidates for a common archetype, this is sub-millisecond.

### Query types

**Primary**: click on the waveform in track details — the clicked position maps to a SAX segment, and the full SAX string of that track becomes the query. The track detail panel could display human-readable section labels (Intro, Verse, Chorus, Outro, Bridge) derived from the RLE structural tags already computed, giving the user spatial context before clicking.

**Future**: block-based query form — user sketches an energy arc by placing L/M/H blocks in sequence (see `doc/research/sax_structural_search.md`). The sketch is SAX-encoded and fed into the same two-stage pipeline.

### Final ranking signal

DTW captures envelope shape similarity but is agnostic to timbre — two tracks with identical arcs can sound completely different. Options to experiment with:

- **DTW only** — pure structural match, genre-agnostic
- **CLAP only** — pure timbral/acoustic match (already exists as "sounds similar")
- **DTW + CLAP blended** — user-controllable weight slider: "same structure" ↔ "sounds similar". Lets the user decide what they're looking for.

The blend mode is the most interesting: it would surface tracks that are both structurally and sonically kindred — something no mainstream music app exposes.

### 2. Structural fingerprint (`waveform_fingerprint`)

A human-readable compact encoding of the track's energy arc, stored as a DB column and
computed by the SAX pass alongside `waveform_sax`.

#### Encoding pipeline

```
waveform_sax (32 chars, alphabet a–e)
  → to_lmh:    a/b → L,  c → M,  d/e → H          (energy level)
  → rle:       collapse consecutive identical chars and retain run counts
                 (e.g. LLLMMHH → [('L', 3), ('M', 2), ('H', 2)])
  → tokenise:  greedy left-to-right compound tokens on the runs, summing grouped counts:
                 MHM  →  chorus flanked by mid on both sides
                 MH   →  build/ramp into chorus
                 HM   →  chorus dissolving into mid
                 L/M/H → standalone
  → troll:     troll-count the run counts of each token individually (none = 1, 2, 3, * = 4+)
                 to preserve duration/transition speed (e.g. L*MH* vs LMH2)
```

#### Troll counting (Pratchett)

Counts compress to: (none) = 1, `2` = 2, `3` = 3, `*` = 4 or more.
A reference to Pratchett's trolls, who count "one, two, many."

#### Token legend

| Token | Meaning |
|---|---|
| `L` | quiet section (energy level a or b) |
| `M` | mid-energy section (energy level c) |
| `H` | loud section / chorus peak (energy level d or e) |
| `MH` | build / ramp up into a chorus |
| `HM` | chorus dissolving into mid-energy |
| `MHM` | chorus flanked by mid on both sides |
| `2`, `3`, `*` | ×2, ×3, many (Pratchett counting) |

#### Examples

| `waveform_fingerprint` | Reading |
|---|---|
| `L*MH2L*` | quiet intro (long), fast build→chorus, quiet outro (long) |
| `L*MH*L*` | quiet intro (long), gradual slow build→chorus, quiet outro (long) |
| `LMHMHL` | quiet intro, fast build→chorus→build→chorus, quiet fade |
| `L*H*L*` | quiet intro (long), long choruses, quiet outro (long) |
| `HH*L*` | starts loud, long choruses, quiet outro (orchestral / live) |
| `L*M*H*` | long gradual ramp (ambient / classical) |

#### Display notation

For UI display, compound tokens are formatted: `MH*` is rendered with `×` on the count.
The flat form (e.g. `L*MH*`) is what is stored in the DB.

#### Usefulness for music producers

The fingerprint is immediately meaningful to anyone who structures songs:
`L(MH)2L` = "quiet intro, two build→chorus cycles, quiet outro" is a complete structural
description. Can be used as a search/filter input: `(MH)*L` = "ends with many chorus cycles
then fades out."

#### DB column

`waveform_fingerprint TEXT` — added in migration `25_waveform_fingerprint.sql`.
Computed by the `sax` pass (version bump to 2) alongside `waveform_sax`.

### 3. CLAP fusion pathway

Separately from SAX, the CLAP paper's native fusion pathway uses a 4-window scheme (compressed full-track thumbnail + front + middle + back) processed through attention-based fusion in a single forward pass. The "shrink" thumbnail gives the model a global structural view that our 3-window average lacks. Requires re-exporting the ONNX model with `enable_fusion=True`. See `doc/research/clap_window_selection.md`.

### 3. Timbral evolution timeline

Use SAX segments as anchors for a "Sonic DNA" timeline view — visualise how the track's timbral character evolves across its structure by running CLAP on each SAX segment boundary and plotting the embedding trajectory. See roadmap item 4 (Sonic DNA & DTW).
