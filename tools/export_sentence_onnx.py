#!/usr/bin/env python3
"""
Export sentence-transformers/all-MiniLM-L6-v2 to ONNX.
Mean-pooling and L2 normalisation are baked into the graph so the Rust
inference code receives a unit-norm 384-d embedding directly.

Run from the repo root with the tools venv active:
  source tools/.venv/bin/activate
  python tools/export_sentence_onnx.py

Output files (written to models/):
  all-minilm-l6-v2.onnx       — model: (input_ids, attention_mask, token_type_ids) -> (1,384)
  all-minilm-l6-v2.onnx.data  — external weight data
  all-minilm-l6-v2-tokenizer.json — HuggingFace fast tokenizer for Rust
"""

import shutil
import tempfile
from pathlib import Path

import torch
import torch.nn.functional as F
from transformers import AutoModel, AutoTokenizer

MODEL_ID = "sentence-transformers/all-MiniLM-L6-v2"
OUT_DIR = Path(__file__).parent.parent / "models"
OUT_DIR.mkdir(parents=True, exist_ok=True)


class MiniLMWithPooling(torch.nn.Module):
    """Wraps the base transformer with mean-pooling + L2 normalisation."""

    def __init__(self, model):
        super().__init__()
        self.model = model

    def forward(
        self,
        input_ids: torch.Tensor,
        attention_mask: torch.Tensor,
        token_type_ids: torch.Tensor,
    ) -> torch.Tensor:
        out = self.model(
            input_ids=input_ids,
            attention_mask=attention_mask,
            token_type_ids=token_type_ids,
        )
        token_embs  = out.last_hidden_state
        mask_exp    = attention_mask.unsqueeze(-1).expand(token_embs.size()).float()
        pooled      = torch.sum(token_embs * mask_exp, 1) / mask_exp.sum(1).clamp(min=1e-9)
        return F.normalize(pooled, p=2, dim=1)


def main() -> None:
    print(f"Loading {MODEL_ID} …")
    tokenizer = AutoTokenizer.from_pretrained(MODEL_ID)
    model = AutoModel.from_pretrained(MODEL_ID)
    model.eval()

    wrapper = MiniLMWithPooling(model)
    wrapper.eval()

    seq = 32
    dummy_ids  = torch.zeros(1, seq, dtype=torch.long)
    dummy_mask = torch.ones(1, seq, dtype=torch.long)
    dummy_type = torch.zeros(1, seq, dtype=torch.long)

    onnx_path = OUT_DIR / "all-minilm-l6-v2.onnx"
    print(f"Exporting ONNX model → {onnx_path} …")
    torch.onnx.export(
        wrapper,
        (dummy_ids, dummy_mask, dummy_type),
        str(onnx_path),
        input_names=["input_ids", "attention_mask", "token_type_ids"],
        output_names=["sentence_embedding"],
        dynamic_axes={
            "input_ids":          {0: "batch", 1: "seq"},
            "attention_mask":     {0: "batch", 1: "seq"},
            "token_type_ids":     {0: "batch", 1: "seq"},
            "sentence_embedding": {0: "batch"},
        },
        opset_version=14,
        do_constant_folding=True,
    )
    print(f"  Saved ({onnx_path.stat().st_size / 1e6:.1f} MB header)")

    # Tokenizer JSON
    tok_dst = OUT_DIR / "all-minilm-l6-v2-tokenizer.json"
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
        tokenizer.save_pretrained(tmp)
        src = Path(tmp) / "tokenizer.json"
        if src.exists():
            shutil.copy(src, tok_dst)
            print(f"  Tokenizer saved: {tok_dst}")
        else:
            print("  WARNING: tokenizer.json not found.")

    print("\nMiniLM export complete.")


if __name__ == "__main__":
    main()
