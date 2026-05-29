#!/usr/bin/env python3
"""
Export laion/clap-htsat-unfused audio and text encoders to ONNX.
Also exports the CLAP mel filterbank weights as a raw float32 binary
and the tokenizer JSON for the Rust `tokenizers` crate.

Run from the repo root with the tools venv active:
  source tools/.venv/bin/activate
  python tools/export_clap_onnx.py

Output files (all written to models/):
  clap_audio_encoder.onnx      — audio encoder: input (1,1,1000,64) -> output (1,512)
  clap_audio_encoder.onnx.data — external weight data (split for git-friendliness)
  clap_text_encoder.onnx       — text encoder: (input_ids, attention_mask) -> (1,512)
  clap_text_encoder.onnx.data  — external weight data
  clap_mel_weights.bin         — 64×513 float32 mel filterbank matrix
  clap-tokenizer.json          — HuggingFace fast tokenizer for Rust

Both encoders include the projection head and L2 normalisation baked in,
so the Rust inference code receives unit-norm 512-d embeddings directly.
"""

import shutil
import struct
import tempfile
import warnings
from pathlib import Path

# Suppress torch dynamo cosmetic warning about shared dynamic dimension names
warnings.filterwarnings("ignore", message=".*axis name.*will not be used.*shares the same shape constraints.*")

import numpy as np
import torch
from transformers import ClapModel, ClapProcessor

MODEL_ID = "laion/clap-htsat-unfused"
OUT_DIR = Path(__file__).parent.parent / "models"
OUT_DIR.mkdir(parents=True, exist_ok=True)

# CLAP feature extractor parameters (must match embeddings.rs)
CLAP_SR = 48_000
CLAP_N_FFT = 1_024
CLAP_HOP = 480
CLAP_N_MELS = 64
CLAP_N_BINS = CLAP_N_FFT // 2 + 1   # 513
CLAP_F_MIN = 50.0
CLAP_F_MAX = 14_000.0
CLAP_MAX_FRAMES = 1_000              # 10 s × 48 kHz / 480


# ── Encoder wrappers ──────────────────────────────────────────────────────────

class ClapAudioEncoderOnnx(torch.nn.Module):
    """Pre-computed log-mel spectrogram → L2-normalised 512-d audio embedding."""

    def __init__(self, model: ClapModel):
        super().__init__()
        self.audio_model = model.audio_model
        self.audio_projection = model.audio_projection

    def forward(self, input_features: torch.Tensor) -> torch.Tensor:
        # input_features: (1, 1, CLAP_MAX_FRAMES, CLAP_N_MELS)
        out = self.audio_model(input_features=input_features, is_longer=None)
        projected = self.audio_projection(out.pooler_output)   # (1, 512)
        return projected / projected.norm(p=2, dim=-1, keepdim=True)


class ClapTextEncoderOnnx(torch.nn.Module):
    """Tokenised text → L2-normalised 512-d text embedding."""

    def __init__(self, model: ClapModel):
        super().__init__()
        self.text_model = model.text_model
        self.text_projection = model.text_projection

    def forward(self, input_ids: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
        out = self.text_model(input_ids=input_ids, attention_mask=attention_mask)
        projected = self.text_projection(out.pooler_output)    # (1, 512)
        return projected / projected.norm(p=2, dim=-1, keepdim=True)


# ── Mel filterbank ────────────────────────────────────────────────────────────

def export_mel_filterbank(processor: ClapProcessor) -> None:
    """Exports the exact mel filterbank used by ClapFeatureExtractor as a flat
    float32 binary (64 × 513 = 32 832 values, row-major)."""
    out_path = OUT_DIR / "clap_mel_weights.bin"

    try:
        import librosa
        fe = processor.feature_extractor
        mel = librosa.filters.mel(
            sr=fe.sampling_rate,
            n_fft=fe.fft_window_size,
            n_mels=fe.feature_size,
            fmin=fe.frequency_min,
            fmax=fe.frequency_max,
        )
    except ImportError:
        print("  librosa not available — using HTK fallback approximation.")
        mel = _htk_mel_filterbank()

    assert mel.shape == (CLAP_N_MELS, CLAP_N_BINS), \
        f"Unexpected mel filterbank shape: {mel.shape}"
    mel.astype(np.float32).tofile(out_path)
    print(f"  Mel filterbank saved: {out_path} ({out_path.stat().st_size:,} bytes)")


def _htk_mel_filterbank() -> np.ndarray:
    """Pure-numpy HTK mel filterbank (librosa-compatible, norm=None)."""
    def hz_to_mel(hz):
        return 2595.0 * np.log10(1.0 + hz / 700.0)
    def mel_to_hz(mel):
        return 700.0 * (10.0 ** (mel / 2595.0) - 1.0)

    mel_pts = np.linspace(hz_to_mel(CLAP_F_MIN), hz_to_mel(CLAP_F_MAX), CLAP_N_MELS + 2)
    hz_pts  = mel_to_hz(mel_pts)
    bins    = np.floor((CLAP_N_FFT + 1) * hz_pts / CLAP_SR).astype(int)

    fb = np.zeros((CLAP_N_MELS, CLAP_N_BINS), dtype=np.float32)
    for m in range(CLAP_N_MELS):
        for k in range(CLAP_N_BINS):
            if bins[m] <= k < bins[m + 1]:
                fb[m, k] = (k - bins[m]) / (bins[m + 1] - bins[m])
            elif bins[m + 1] <= k <= bins[m + 2]:
                fb[m, k] = (bins[m + 2] - k) / (bins[m + 2] - bins[m + 1])
    return fb


# ── Main ──────────────────────────────────────────────────────────────────────

def main() -> None:
    print(f"Loading {MODEL_ID} …")
    model = ClapModel.from_pretrained(MODEL_ID)
    processor = ClapProcessor.from_pretrained(MODEL_ID)
    model.eval()

    # Mel filterbank
    print("Exporting mel filterbank …")
    export_mel_filterbank(processor)

    # Audio encoder
    audio_enc = ClapAudioEncoderOnnx(model)
    audio_enc.eval()
    dummy = torch.zeros(1, 1, CLAP_MAX_FRAMES, CLAP_N_MELS)
    audio_path = OUT_DIR / "clap_audio_encoder.onnx"
    print(f"Exporting audio encoder → {audio_path} …")
    torch.onnx.export(
        audio_enc, dummy, str(audio_path),
        input_names=["input_features"],
        output_names=["audio_embedding"],
        opset_version=18,
        do_constant_folding=True,
    )
    print(f"  Saved ({audio_path.stat().st_size / 1e6:.1f} MB header)")

    # Text encoder
    text_enc = ClapTextEncoderOnnx(model)
    text_enc.eval()
    seq = 32
    dummy_ids  = torch.zeros(1, seq, dtype=torch.long)
    dummy_mask = torch.ones(1, seq, dtype=torch.long)
    text_path = OUT_DIR / "clap_text_encoder.onnx"
    print(f"Exporting text encoder → {text_path} …")
    seq_dim = torch.export.Dim("seq", min=1, max=77)
    torch.onnx.export(
        text_enc, (dummy_ids, dummy_mask), str(text_path),
        input_names=["input_ids", "attention_mask"],
        output_names=["text_embedding"],
        dynamic_shapes={
            "input_ids":      {1: seq_dim},
            "attention_mask": {1: seq_dim},
        },
        opset_version=18,
        do_constant_folding=True,
    )
    print(f"  Saved ({text_path.stat().st_size / 1e6:.1f} MB header)")

    # Tokenizer JSON
    tok_dst = OUT_DIR / "clap-tokenizer.json"
    try:
        from transformers.utils import cached_file
        src = Path(cached_file(MODEL_ID, "tokenizer.json"))
        if src.exists():
            shutil.copy(src, tok_dst)
            print(f"  Tokenizer copied: {tok_dst}")
            return
    except Exception:
        pass
    with tempfile.TemporaryDirectory() as tmp:
        processor.tokenizer.save_pretrained(tmp)
        src = Path(tmp) / "tokenizer.json"
        if src.exists():
            shutil.copy(src, tok_dst)
            print(f"  Tokenizer saved: {tok_dst}")
        else:
            print("  WARNING: tokenizer.json not found.")

    print("\nCLAP export complete.")


if __name__ == "__main__":
    main()
