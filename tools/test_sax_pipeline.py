#!/usr/bin/env python3
import os
import sys
import json
import torch

# Add collab session folder to python path so we can import dataset, models, and export
sys.path.append(os.path.abspath("doc/collab/sessions/2026-06-06-sax-transformer"))

from dataset import SAXTransformerDataset, collate_fn
from models import SAXGRUModel, TinySAXTransformer
from export import export_to_onnx

def main():
    json_path = "doc/collab/sessions/2026-06-06-sax-transformer/sample_tracks.json"
    print(f"Loading dataset from {json_path}...")
    dataset = SAXTransformerDataset(json_path)
    print(f"Dataset loaded. Alphabet: {dataset.alphabet}")
    print(f"Vocabulary mapping: {dataset.char2idx}")
    
    # Vocabulary size (add 1 for pad token)
    vocab_size = len(dataset.char2idx) + 1
    num_classes = 7  # Intro, Verse, Pre-Chorus, Chorus, Bridge, Outro, End
    
    # Load first item
    item = dataset[0]
    print(f"Sample item title: {item['title']}, artist: {item['artist']}")
    print(f"sax_ids shape: {item['sax_ids'].shape}, values: {item['sax_ids']}")
    print(f"waveform_frames shape: {item['waveform_frames'].shape}")
    
    # Create models
    print("\n--- Initializing SAXGRUModel ---")
    gru_model = SAXGRUModel(vocab_size=vocab_size, num_classes=num_classes)
    
    print("\n--- Initializing TinySAXTransformer ---")
    transformer_model = TinySAXTransformer(vocab_size=vocab_size, num_classes=num_classes)
    
    # Run simple dummy forward pass
    dummy_sax = item['sax_ids'].unsqueeze(0)  # batch size = 1
    dummy_wave = item['waveform_frames'].unsqueeze(0)
    dummy_lengths = torch.tensor([item['length']], dtype=torch.long)
    
    print("\nRunning forward pass on GRU model...")
    gru_out = gru_model(dummy_sax, dummy_wave)
    print(f"GRU output shape: {gru_out.shape}, values: {gru_out.detach().numpy()}")
    
    print("\nRunning forward pass on Transformer model...")
    transformer_out = transformer_model(dummy_sax, dummy_wave, dummy_lengths)
    print(f"Transformer output shape: {transformer_out.shape}, values: {transformer_out.detach().numpy()}")
    
    # Export to ONNX
    print("\n--- Exporting GRU Model to ONNX ---")
    os.makedirs("models", exist_ok=True)
    gru_onnx_path = "models/sax_gru.onnx"
    export_to_onnx(gru_model, vocab_size, gru_onnx_path, model_type="gru")
    
    print("\n--- Exporting Transformer Model to ONNX ---")
    transformer_onnx_path = "models/sax_transformer.onnx"
    # Ensure TinySAXTransformer works with the forward format of export.py
    export_to_onnx(transformer_model, vocab_size, transformer_onnx_path, model_type="transformer")
    
    print("\nPipeline test complete and models exported successfully!")

if __name__ == "__main__":
    main()
