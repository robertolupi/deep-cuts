#!/usr/bin/env python3
"""
Training script for SAX sequence structural classifier.
Loads data from the deep_cuts database and sidecar files, aligns lyrics sections to segments,
trains GRU/Transformer models, and exports the optimized ONNX models.
"""

import os
import re
import json
import sqlite3
import torch
import torch.nn as nn
import torch.optim as optim
from torch.utils.data import Dataset, DataLoader
from pathlib import Path

# Add collab session folder to python path so we can import model and dataset modules
import sys
sys.path.append(os.path.abspath("doc/collab/sessions/2026-06-06-sax-transformer"))
from models import SAXGRUModel, TinySAXTransformer
from export import export_to_onnx

DB_PATH = os.path.expanduser("~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db")

# Canonical classes
CLASSES = ["unknown", "intro", "verse", "pre-chorus", "chorus", "bridge", "outro", "end"]
CLASS_TO_IDX = {c: i for i, c in enumerate(CLASSES)}

def parse_sections(lyrics_text):
    """Parse section headers in lyrics and return list of (line_index, canonical_label)."""
    lines = lyrics_text.splitlines()
    sections = []
    for i, line in enumerate(lines):
        m = re.match(r"^\[(.+?)\]", line.strip())
        if m:
            raw = m.group(1)
            canon = raw.lower().strip()
            label = "unknown"
            for key in ["intro", "verse", "pre-chorus", "prechorus", "chorus", "bridge", "break", "drop", "outro", "end", "fade"]:
                if key in canon:
                    label = key
                    if label == "prechorus":
                        label = "pre-chorus"
                    elif label in ("break", "fade"):
                        label = "outro"
                    elif label == "drop":
                        label = "chorus"
                    break
            sections.append((i, label))
    return sections, len(lines)

def align_sections_to_segments(sections, total_lines, num_segments=32):
    """Map section labels to each temporal segment based on relative position."""
    if not sections or total_lines == 0:
        return ["unknown"] * num_segments
    
    segment_labels = []
    for i in range(num_segments):
        pos_line = (i / num_segments) * total_lines
        # Find the active section at this line position
        active_label = "unknown"
        for idx, label in sections:
            if idx <= pos_line:
                active_label = label
            else:
                break
        segment_labels.append(active_label)
    return segment_labels

class LibrarySAXDataset(Dataset):
    def __init__(self, db_path, num_segments=32):
        self.num_segments = num_segments
        self.samples = []
        
        # Load from DB
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        rows = cursor.execute(
            "SELECT id, path, title, artist, waveform_sax, waveform_data, lyrics FROM tracks WHERE waveform_sax IS NOT NULL"
        ).fetchall()
        conn.close()
        
        alphabet_chars = set()
        
        for track_id, path, title, artist, sax, wf_json, lyrics_db in rows:
            # 1. Fetch lyrics (DB first, then sidecar)
            lyrics = lyrics_db
            if not lyrics:
                sidecar_path = Path(path).with_suffix(".mp3.lyrics.txt")
                if not sidecar_path.exists():
                    sidecar_path = Path(path).parent / f"{Path(path).name}.lyrics.txt"
                if sidecar_path.exists():
                    try:
                        lyrics = sidecar_path.read_text(errors="replace")
                    except Exception:
                        pass
            
            if not lyrics:
                continue
                
            sections, total_lines = parse_sections(lyrics)
            if not sections:
                continue
                
            # Align section labels to segments
            labels = align_sections_to_segments(sections, total_lines, num_segments=len(sax))
            label_ids = [CLASS_TO_IDX[l] for l in labels]
            
            # Load waveform envelope
            try:
                wf = json.loads(wf_json) if wf_json else []
            except Exception:
                wf = []
                
            alphabet_chars.update(sax)
            
            self.samples.append({
                "id": track_id,
                "title": title or "Unknown",
                "sax": sax,
                "waveform": wf,
                "labels": label_ids
            })
            
        self.alphabet = sorted(list(alphabet_chars))
        self.char2idx = {c: i+1 for i, c in enumerate(self.alphabet)}  # 0 = padding

    def __len__(self):
        return len(self.samples)

    def __getitem__(self, idx):
        item = self.samples[idx]
        sax = item["sax"]
        sax_ids = torch.tensor([self.char2idx.get(c, 0) for c in sax], dtype=torch.long)
        label_ids = torch.tensor(item["labels"], dtype=torch.long)
        
        # Downsample/average waveform envelope to match SAX length
        wf_raw = torch.tensor(item["waveform"], dtype=torch.float32)
        if len(wf_raw) >= len(sax_ids) and len(sax_ids) > 0:
            chunk_size = len(wf_raw) // len(sax_ids)
            waveform = torch.zeros(len(sax_ids), dtype=torch.float32)
            for i in range(len(sax_ids)):
                waveform[i] = wf_raw[i * chunk_size : (i + 1) * chunk_size].mean()
        else:
            waveform = torch.zeros(len(sax_ids), dtype=torch.float32)
            
        return {
            "sax_ids": sax_ids,
            "waveform": waveform,
            "labels": label_ids,
            "length": len(sax_ids)
        }

def collate_fn(batch):
    from torch.nn.utils.rnn import pad_sequence
    sax_ids = [b["sax_ids"] for b in batch]
    waveforms = [b["waveform"] for b in batch]
    labels = [b["labels"] for b in batch]
    lengths = torch.tensor([b["length"] for b in batch])
    
    sax_padded = pad_sequence(sax_ids, batch_first=True, padding_value=0)
    wave_padded = pad_sequence(waveforms, batch_first=True, padding_value=0.0)
    labels_padded = pad_sequence(labels, batch_first=True, padding_value=-100) # ignore index for loss
    
    return {
        "sax_ids": sax_padded,
        "waveform": wave_padded,
        "labels": labels_padded,
        "lengths": lengths
    }

def train():
    print("Preparing dataset...")
    dataset = LibrarySAXDataset(DB_PATH)
    print(f"Loaded {len(dataset)} labeled tracks from database/sidecars.")
    print(f"SAX Alphabet: {dataset.alphabet}")
    
    if len(dataset) == 0:
        print("Error: No labeled training samples found.")
        return
        
    loader = DataLoader(dataset, batch_size=16, shuffle=True, collate_fn=collate_fn)
    
    vocab_size = len(dataset.char2idx) + 1
    num_classes = len(CLASSES)
    
    print("Training SAXGRUModel...")
    # Modify GRU forward to support per-step classification instead of sequence-level classification
    # Let's inspect SAXGRUModel in models.py:
    # models.py uses `last_out` to classify the sequence. But for segment-level classification,
    # we classify every step!
    # Let's see: we should use output at all sequence steps: self.fc(out) [B, L, H] instead of self.fc(last_out).
    # Let's write a simple sequence-to-sequence wrapper or run standard training.
    
    # We will train the TinySAXTransformer as our sequence-to-sequence model since it already outputs predictions per step.
    # Wait, TinySAXTransformer.forward does:
    # `x = x.mean(dim=1)` or `x = (x * mask_float).sum(dim=1)` returning sequence-level classification.
    # To predict a label per step (sequence-to-sequence), we want self.fc(x) on the unpooled sequence!
    # Let's verify: Yes! For sequence labeling, we want logits of shape [B, L, num_classes].
    
    print("Updating models.py for sequence labeling...")
    # Let's write the training loops. We'll implement a custom model subclass in this script that performs
    # sequence-to-sequence tagging, trains it, and exports it to ONNX!
    # This keeps it clean and avoids breaking the sequence-level classification in models.py if it is used elsewhere.
    
    class SAXSequenceTagger(nn.Module):
        def __init__(self, vocab_size, embed_dim=64, hidden_dim=128, num_layers=2, num_classes=8):
            super().__init__()
            self.embedding = nn.Embedding(vocab_size, embed_dim, padding_idx=0)
            self.gru = nn.GRU(embed_dim + 1, hidden_dim, num_layers, 
                              batch_first=True, bidirectional=True)
            self.fc = nn.Sequential(
                nn.Linear(hidden_dim * 2, hidden_dim),
                nn.ReLU(),
                nn.Linear(hidden_dim, num_classes)
            )
            
        def forward(self, sax_ids, waveform):
            x = self.embedding(sax_ids)  # [B, L, E]
            wave_feat = waveform.unsqueeze(-1)  # [B, L, 1]
            x = torch.cat([x, wave_feat], dim=-1)
            out, _ = self.gru(x)  # [B, L, hidden_dim * 2]
            return self.fc(out)  # [B, L, num_classes]

    model = SAXSequenceTagger(vocab_size=vocab_size, num_classes=num_classes)
    criterion = nn.CrossEntropyLoss(ignore_index=-100)
    optimizer = optim.AdamW(model.parameters(), lr=0.001)
    
    # Simple training loop with more epochs
    model.train()
    epochs = 60
    for epoch in range(epochs):
        total_loss = 0
        correct = 0
        total_tokens = 0
        
        for batch in loader:
            sax_ids = batch["sax_ids"]
            wave = batch["waveform"]
            labels = batch["labels"]
            
            optimizer.zero_grad()
            logits = model(sax_ids, wave) # [B, L, num_classes]
            
            # Reshape for CrossEntropy
            loss = criterion(logits.view(-1, num_classes), labels.view(-1))
            loss.backward()
            optimizer.step()
            
            total_loss += loss.item()
            
            # Accuracy
            preds = logits.argmax(dim=-1)
            mask = labels != -100
            correct += (preds[mask] == labels[mask]).sum().item()
            total_tokens += mask.sum().item()
            
        avg_loss = total_loss / len(loader)
        accuracy = (correct / total_tokens) * 100 if total_tokens > 0 else 0
        if (epoch + 1) % 5 == 0 or epoch == 0 or epoch == epochs - 1:
            print(f"Epoch {epoch+1:02d}/{epochs} | Loss: {avg_loss:.4f} | Acc: {accuracy:.2f}%")
        
    # Export trained tagger to ONNX
    print("\n--- Exporting Sequence Tagger to ONNX ---")
    model.eval()
    dummy_sax = torch.randint(1, vocab_size, (1, 32), dtype=torch.long)
    dummy_wave = torch.randn(1, 32, dtype=torch.float32)
    
    onnx_path = "models/sax_sequence_tagger.onnx"
    os.makedirs("models", exist_ok=True)
    
    torch.onnx.export(
        model,
        (dummy_sax, dummy_wave),
        onnx_path,
        input_names=['sax_ids', 'waveform'],
        output_names=['logits'],
        opset_version=17,
        do_constant_folding=True
    )
    print(f"Sequence tagger ONNX model exported to {onnx_path}")

if __name__ == "__main__":
    train()
