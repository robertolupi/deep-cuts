import torch
import json

def export_to_onnx(model, vocab_size, output_path="sax_model.onnx", model_type="gru"):
    """Export SAX model to ONNX with dynamic axes"""
    model.eval()
    
    # Dummy inputs matching your JSON format
    batch_size = 1
    seq_len = 128
    dummy_sax = torch.randint(1, vocab_size, (batch_size, seq_len), dtype=torch.long)
    dummy_wave = torch.randn(batch_size, seq_len, dtype=torch.float32)
    dummy_lengths = torch.tensor([seq_len], dtype=torch.long)
    
    # For ONNX, simplify to avoid packing
    if model_type == "gru":
        # Use version without packing for export
        def forward_export(sax_ids, waveform):
            return model(sax_ids, waveform, lengths=None)
        torch.onnx.export(
            forward_export,
            (dummy_sax, dummy_wave),
            output_path,
            input_names=['sax_ids', 'waveform'],
            output_names=['logits'],
            dynamic_axes={
                'sax_ids': {0: 'batch', 1: 'sequence'},
                'waveform': {0: 'batch', 1: 'sequence'},
                'logits': {0: 'batch'}
            },
            opset_version=17,
            dynamo=True
        )
    else:
        torch.onnx.export(
            model,
            (dummy_sax, dummy_wave, dummy_lengths),
            output_path,
            input_names=['sax_ids', 'waveform', 'lengths'],
            output_names=['logits'],
            dynamic_axes={
                'sax_ids': {0: 'batch', 1: 'sequence'},
                'waveform': {0: 'batch', 1: 'sequence'},
                'lengths': {0: 'batch'},
                'logits': {0: 'batch'}
            },
            opset_version=17,
            dynamo=True
        )
    
    print(f"Model exported to {output_path}")
    # Verify
    import onnxruntime as ort
    sess = ort.InferenceSession(output_path)
    print("ONNX inputs:", [i.name for i in sess.get_inputs()])

# Example usage:
# with open('2026-06-06-sax-transformer.json') as f:
#     data = json.load(f)
# dataset = SAXTransformerDataset('2026-06-06-sax-transformer.json')
# vocab_size = len(dataset.char2idx) + 1
# model = TinySAXTransformer(vocab_size=vocab_size, num_classes=5)  # 5 tracks
# export_to_onnx(model, vocab_size, "sax_transformer.onnx", model_type="transformer")
