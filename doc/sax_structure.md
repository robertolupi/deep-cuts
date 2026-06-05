# SAX-Based Track Structure Analysis

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

## Future directions

### 1. Structural similarity search in the app

Store the SAX string (or its RLE form) as a new column `waveform_sax TEXT` on the `tracks` table. Expose two search modes:

- **Structural similarity**: find tracks whose SAX string has low MINDIST or Hamming distance to a query track. Cheap — O(n × string_length) over 2k tracks.
- **Pattern search**: let power users type a structural regex (`L+H+L+H+`, `^L+.*H+.*L+$`) to filter the library by song architecture.

This is complementary to CLAP similarity and would be a unique feature — no mainstream music app exposes structural shape as a search axis.

### 2. CLAP fusion pathway

Separately from SAX, the CLAP paper's native fusion pathway uses a 4-window scheme (compressed full-track thumbnail + front + middle + back) processed through attention-based fusion in a single forward pass. The "shrink" thumbnail gives the model a global structural view that our 3-window average lacks. Requires re-exporting the ONNX model with `enable_fusion=True`. See `doc/clap_window_selection.md`.

### 3. Timbral evolution timeline

Use SAX segments as anchors for a "Sonic DNA" timeline view — visualise how the track's timbral character evolves across its structure by running CLAP on each SAX segment boundary and plotting the embedding trajectory. See roadmap item 4 (Sonic DNA & DTW).
