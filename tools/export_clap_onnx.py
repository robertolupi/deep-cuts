#!/usr/bin/env python3
"""
Export laion/clap-htsat-unfused audio and text encoders to ONNX.
Also exports the CLAP mel filterbank weights as a raw float32 binary.

Run from the repo root with the backend venv active:
  cd /path/to/music-intelligence
  backend/.venv/bin/python tools/export_clap_onnx.py

Output files (all in models/):
  clap_audio_encoder.onnx     — audio encoder, input (1,1,64,1000) -> output (1,512)
  clap_text_encoder.onnx      — text encoder, input_ids+(1,seq) -> output (1,512)
  clap_mel_weights.bin        — 64×513 float32 mel filterbank matrix
  clap-tokenizer.json         — HuggingFace fast tokenizer for text tokenisation in Rust
"""

import sys
import struct
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "backend"))

import numpy as np
import torch
from transformers import ClapModel, ClapProcessor

MODEL_ID = "laion/clap-htsat-unfused"
OUT_DIR = Path(__file__).parent.parent / "models"
OUT_DIR.mkdir(parents=True, exist_ok=True)

# CLAP feature extractor parameters
CLAP_SR = 48000
CLAP_N_FFT = 1024
CLAP_HOP = 480
CLAP_N_MELS = 64
CLAP_N_BINS = CLAP_N_FFT // 2 + 1  # 513
CLAP_F_MIN = 50.0
CLAP_F_MAX = 14000.0
CLAP_MAX_FRAMES = 1000  # 10s * 48000 / 480


# ── Audio encoder wrapper ─────────────────────────────────────────────────────

class ClapAudioEncoderOnnx(torch.nn.Module):
    """Takes a pre-computed log-mel spectrogram, runs the HTSAT audio encoder,
    applies the projection head, and L2-normalises the output."""

    def __init__(self, model: ClapModel):
        super().__init__()
        self.audio_model = model.audio_model
        self.audio_projection = model.audio_projection

    def forward(self, input_features: torch.Tensor) -> torch.Tensor:
        # input_features: (1, 1, 1000, 64) — (batch, 1, time_frames, n_mels)
        # is_longer must be an explicit tensor (not None) so the ONNX tracer
        # captures the correct execution path through the HTSAT attention layers.
        batch = input_features.shape[0]
        is_longer = torch.zeros(batch, dtype=torch.bool, device=input_features.device)
        audio_out = self.audio_model(input_features=input_features, is_longer=is_longer)
        pooled = audio_out.pooler_output          # (1, hidden_dim)
        projected = self.audio_projection(pooled)  # (1, 512)
        return projected / projected.norm(p=2, dim=-1, keepdim=True)


# ── Text encoder wrapper ──────────────────────────────────────────────────────

class ClapTextEncoderOnnx(torch.nn.Module):
    """Tokenised text → L2-normalised 512-d text embedding."""

    def __init__(self, model: ClapModel):
        super().__init__()
        self.text_model = model.text_model
        self.text_projection = model.text_projection

    def forward(self, input_ids: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
        text_out = self.text_model(input_ids=input_ids, attention_mask=attention_mask)
        pooled = text_out.pooler_output            # (1, hidden_dim)
        projected = self.text_projection(pooled)   # (1, 512)
        return projected / projected.norm(p=2, dim=-1, keepdim=True)


# ── Mel filterbank export ─────────────────────────────────────────────────────

def export_mel_filterbank(processor: ClapProcessor) -> None:
    """Exports the exact mel filterbank used by ClapFeatureExtractor as float32 binary."""
    try:
        import librosa
    except ImportError:
        print("  WARNING: librosa not available — install it to export mel weights.")
        print("           Using fallback HTK mel filterbank approximation.")
        _export_mel_filterbank_fallback()
        return

    fe = processor.feature_extractor
    mel_filters = librosa.filters.mel(
        sr=fe.sampling_rate,
        n_fft=fe.fft_window_size,
        n_mels=fe.feature_size,
        fmin=fe.frequency_min,
        fmax=fe.frequency_max,
    )
    # Shape: (n_mels, n_bins) = (64, 513)
    print(f"  Mel filterbank shape: {mel_filters.shape}")
    assert mel_filters.shape == (CLAP_N_MELS, CLAP_N_BINS), \
        f"Unexpected mel filterbank shape: {mel_filters.shape}"

    out_path = OUT_DIR / "clap_mel_weights.bin"
    mel_filters.astype(np.float32).tofile(out_path)
    print(f"  Mel weights saved: {out_path} ({out_path.stat().st_size} bytes)")


def _export_mel_filterbank_fallback() -> None:
    """Pure-numpy HTK mel filterbank (librosa-compatible, norm=None)."""
    def hz_to_mel(hz: float) -> float:
        return 2595.0 * np.log10(1.0 + hz / 700.0)

    def mel_to_hz(mel: float) -> float:
        return 700.0 * (10.0 ** (mel / 2595.0) - 1.0)

    mel_min = hz_to_mel(CLAP_F_MIN)
    mel_max = hz_to_mel(CLAP_F_MAX)
    mel_pts = np.linspace(mel_min, mel_max, CLAP_N_MELS + 2)
    hz_pts = mel_to_hz(mel_pts)
    bin_pts = np.floor((CLAP_N_FFT + 1) * hz_pts / CLAP_SR).astype(int)

    filterbank = np.zeros((CLAP_N_MELS, CLAP_N_BINS), dtype=np.float32)
    for m in range(CLAP_N_MELS):
        for k in range(CLAP_N_BINS):
            if bin_pts[m] <= k < bin_pts[m + 1]:
                filterbank[m, k] = (k - bin_pts[m]) / (bin_pts[m + 1] - bin_pts[m])
            elif bin_pts[m + 1] <= k <= bin_pts[m + 2]:
                filterbank[m, k] = (bin_pts[m + 2] - k) / (bin_pts[m + 2] - bin_pts[m + 1])

    out_path = OUT_DIR / "clap_mel_weights.bin"
    filterbank.tofile(out_path)
    print(f"  Mel weights (fallback HTK) saved: {out_path}")


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    print(f"Loading {MODEL_ID} ...")
    model = ClapModel.from_pretrained(MODEL_ID)
    processor = ClapProcessor.from_pretrained(MODEL_ID)
    model.eval()

    # ── Mel filterbank ────────────────────────────────────────────────────────
    print("Exporting CLAP mel filterbank ...")
    export_mel_filterbank(processor)

    # ── Audio encoder ─────────────────────────────────────────────────────────
    audio_enc = ClapAudioEncoderOnnx(model)
    audio_enc.eval()

    dummy_features = torch.zeros(1, 1, CLAP_MAX_FRAMES, CLAP_N_MELS, dtype=torch.float32)

    audio_onnx_path = OUT_DIR / "clap_audio_encoder.onnx"
    print(f"Exporting CLAP audio encoder to {audio_onnx_path} ...")
    torch.onnx.export(
        audio_enc,
        dummy_features,
        str(audio_onnx_path),
        input_names=["input_features"],
        output_names=["audio_embedding"],
        dynamic_axes={"audio_embedding": {0: "batch"}},
        opset_version=14,
        do_constant_folding=True,
    )
    print(f"  Audio encoder saved: {audio_onnx_path} ({audio_onnx_path.stat().st_size / 1e6:.1f} MB)")

    # ── Text encoder ─────────────────────────────────────────────────────────
    text_enc = ClapTextEncoderOnnx(model)
    text_enc.eval()

    seq_len = 32
    dummy_ids  = torch.zeros(1, seq_len, dtype=torch.long)
    dummy_mask = torch.ones(1, seq_len, dtype=torch.long)

    text_onnx_path = OUT_DIR / "clap_text_encoder.onnx"
    print(f"Exporting CLAP text encoder to {text_onnx_path} ...")
    torch.onnx.export(
        text_enc,
        (dummy_ids, dummy_mask),
        str(text_onnx_path),
        input_names=["input_ids", "attention_mask"],
        output_names=["text_embedding"],
        dynamic_axes={
            "input_ids":      {0: "batch", 1: "seq"},
            "attention_mask": {0: "batch", 1: "seq"},
            "text_embedding": {0: "batch"},
        },
        opset_version=14,
        do_constant_folding=True,
    )
    print(f"  Text encoder saved: {text_onnx_path} ({text_onnx_path.stat().st_size / 1e6:.1f} MB)")

    # ── Tokenizer ─────────────────────────────────────────────────────────────
    tok_dst = OUT_DIR / "clap-tokenizer.json"
    try:
        from transformers.utils import cached_file
        tok_src = Path(cached_file(MODEL_ID, "tokenizer.json"))
        if tok_src.exists():
            import shutil
            shutil.copy(tok_src, tok_dst)
            print(f"  Tokenizer JSON copied: {tok_dst}")
        else:
            raise FileNotFoundError
    except Exception:
        import tempfile, shutil
        with tempfile.TemporaryDirectory() as tmp:
            processor.tokenizer.save_pretrained(tmp)
            src = Path(tmp) / "tokenizer.json"
            if src.exists():
                shutil.copy(src, tok_dst)
                print(f"  Tokenizer JSON saved: {tok_dst}")
            else:
                print("  WARNING: CLAP tokenizer.json not found!")

    print("\nAll CLAP exports complete.")
    print(f"Output directory: {OUT_DIR}")
    print("Add these files to your download_models.py or commit clap_mel_weights.bin.")


if __name__ == "__main__":
    main()
