#!/usr/bin/env python3
"""
Export the Essentia MelBands filterbank matrix used by the spectrogram pass
(Discogs-Effnet classifier, 96 mel bands at 16 kHz).

This script requires the `essentia` package, which is NOT available on PyPI
for Apple Silicon. Install it via conda or a Linux x86-64 environment:
  conda install -c mtgupf essentia

Run from the repo root with the tools venv active:
  source tools/.venv/bin/activate
  python tools/export_essentia_mel_weights.py

Output (compiled into the Rust binary via include_bytes!):
  src-tauri/src/mel_weights.bin  — 96×257 float32 row-major matrix

NOTE: This is only needed for the Essentia classifier pass (future).
The CLAP pass uses clap_mel_weights.bin exported by export_clap_onnx.py.
"""

from pathlib import Path
import numpy as np

try:
    import essentia.standard as es
except ImportError:
    raise SystemExit(
        "essentia is not installed. Install the tensorflow variant which works on Apple Silicon:\n"
        "  pip install -e \".[essentia]\"\n"
        "from the tools/ directory, or directly:\n"
        "  pip install essentia-tensorflow"
    )

N_BANDS    = 96
FFT_SIZE   = 512
N_BINS     = FFT_SIZE // 2 + 1   # 257
SAMPLE_RATE = 16_000

mel = es.MelBands(
    numberBands=N_BANDS,
    sampleRate=SAMPLE_RATE,
    lowFrequencyBound=0.0,
    highFrequencyBound=8_000.0,
    type="magnitude",
    warpingFormula="slaneyMel",
    weighting="linear",
    normalize="unit_tri",
)

# Extract the filter matrix column-by-column via unit impulse spectra
matrix = np.zeros((N_BANDS, N_BINS), dtype=np.float32)
for k in range(N_BINS):
    impulse = np.zeros(N_BINS, dtype=np.float32)
    impulse[k] = 1.0
    matrix[:, k] = mel(impulse)

print(f"Extracted matrix shape: {matrix.shape}  (expected {N_BANDS} × {N_BINS})")

dest = Path(__file__).parent.parent / "src-tauri" / "src" / "mel_weights.bin"
dest.parent.mkdir(parents=True, exist_ok=True)
matrix.tofile(dest)
print(f"Wrote {dest.stat().st_size:,} bytes → {dest}")
