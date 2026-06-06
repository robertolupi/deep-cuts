#!/usr/bin/env python3
import os
import sys
import json
import torch
import torch.nn.functional as F

sys.path.append(os.path.abspath("doc/collab/sessions/2026-06-06-sax-transformer"))
CLASSES = ["unknown", "intro", "verse", "pre-chorus", "chorus", "bridge", "outro", "end"]
CHAR2IDX = {'a': 1, 'b': 2, 'c': 3, 'd': 4, 'e': 5}

class SAXSequenceTagger(torch.nn.Module):
    def __init__(self, vocab_size, embed_dim=64, hidden_dim=128, num_layers=2, num_classes=8):
        super().__init__()
        self.embedding = torch.nn.Embedding(vocab_size, embed_dim, padding_idx=0)
        self.gru = torch.nn.GRU(embed_dim + 1, hidden_dim, num_layers, 
                          batch_first=True, bidirectional=True)
        self.fc = torch.nn.Sequential(
            torch.nn.Linear(hidden_dim * 2, hidden_dim),
            torch.nn.ReLU(),
            torch.nn.Linear(hidden_dim, num_classes)
        )
        
    def forward(self, sax_ids, waveform):
        x = self.embedding(sax_ids)
        wave_feat = waveform.unsqueeze(-1)
        x = torch.cat([x, wave_feat], dim=-1)
        out, _ = self.gru(x)
        return self.fc(out)

def main():
    json_path = "doc/collab/sessions/2026-06-06-sax-transformer/sample_tracks.json"
    
    vocab_size = len(CHAR2IDX) + 1
    num_classes = len(CLASSES)
    
    # Load model weights
    model = SAXSequenceTagger(vocab_size=vocab_size, num_classes=num_classes)
    # We can load model state or trace it. Let's just run training inline for a second to get weights, or since we just trained, let's load from our checkpoint.
    # Oh! In train_sax_transformer.py, we only exported to ONNX, we didn't save PyTorch state.
    # We can easily load the ONNX model using onnxruntime and run inference!
    # That is even better because it verifies the ONNX model output!
    import onnxruntime as ort
    
    onnx_path = "models/sax_sequence_tagger.onnx"
    print(f"Loading ONNX model from {onnx_path}...")
    session = ort.InferenceSession(onnx_path)
    
    predictions = []
    # Test on the 5 sample tracks from the JSON
    with open(json_path, 'r') as f:
        samples = json.load(f)
        
    for item in samples:
        sax = item["waveform_sax"]
        sax_ids = [CHAR2IDX.get(c, 0) for c in sax]
        
        # Average waveform data to match sax length
        wf_raw = item["waveform_data"]
        chunk_size = len(wf_raw) // len(sax)
        waveform = []
        for i in range(len(sax)):
            chunk = wf_raw[i * chunk_size : (i + 1) * chunk_size]
            waveform.append(sum(chunk) / len(chunk) if chunk else 0.0)
            
        # Run ONNX inference
        # Inputs: sax_ids: [1, seq_len], waveform: [1, seq_len]
        onnx_inputs = {
            'sax_ids': [sax_ids],
            'waveform': [waveform]
        }
        outputs = session.run(['logits'], onnx_inputs)
        logits = torch.tensor(outputs[0][0]) # [seq_len, num_classes]
        probs = F.softmax(logits, dim=-1)
        
        seg_predictions = []
        for step in range(len(sax)):
            step_probs = probs[step].tolist()
            pred_idx = probs[step].argmax().item()
            seg_predictions.append({
                "segment_index": step,
                "sax_char": sax[step],
                "waveform_value": waveform[step],
                "predicted_label": CLASSES[pred_idx],
                "probabilities": {CLASSES[c]: round(step_probs[c], 4) for c in range(num_classes)}
            })
            
        predictions.append({
            "title": item["title"],
            "artist": item["artist"],
            "waveform_sax": sax,
            "predictions": seg_predictions
        })
        
    out_path = "doc/collab/sessions/2026-06-06-sax-transformer/sample_predictions.json"
    with open(out_path, 'w') as f:
        json.dump(predictions, f, indent=2)
    print(f"Wrote predictions JSON successfully to {out_path}.")

if __name__ == "__main__":
    main()
