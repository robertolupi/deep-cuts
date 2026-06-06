import json
import torch
from torch.utils.data import Dataset
from torch.nn.utils.rnn import pad_sequence

class SAXTransformerDataset(Dataset):
    """
    Expects JSON format:
    [
      {
        "title": str,
        "artist": str,
        "sax_string": "abccba...",  # symbolic SAX fingerprint
        "waveform": [float, ...],   # raw waveform at 16kHz, optional
        "genius_tags": [...]        # optional alignment data
      },
      ...
    ]
    """
    def __init__(self, json_path, alphabet=None, max_sax_len=512):
        with open(json_path, 'r') as f:
            self.data = json.load(f)
        
        # Build alphabet from data if not provided
        if alphabet is None:
            chars = set()
            for item in self.data:
                # Use waveform_sax as the source string
                chars.update(item['waveform_sax'])
            self.alphabet = sorted(list(chars))
        else:
            self.alphabet = alphabet
        
        self.char2idx = {c: i+1 for i, c in enumerate(self.alphabet)}  # 0 = pad
        self.idx2char = {i: c for c, i in self.char2idx.items()}
        self.max_len = max_sax_len

    def __len__(self):
        return len(self.data)

    def __getitem__(self, idx):
        item = self.data[idx]
        sax = item['waveform_sax'][:self.max_len]
        # Convert to indices
        sax_ids = torch.tensor([self.char2idx.get(c, 0) for c in sax], dtype=torch.long)
        
        # Optional waveform - downsample to 40ms frames to match SAX
        # The database waveform_data is already a pre-computed envelope (typically 128 values).
        # We downsample it to match the SAX string length (typically 32) using PAA (averaging chunks).
        waveform_raw = torch.tensor(item.get('waveform_data', []), dtype=torch.float32)
        if len(waveform_raw) >= len(sax_ids) and len(sax_ids) > 0:
            chunk_size = len(waveform_raw) // len(sax_ids)
            waveform = torch.zeros(len(sax_ids), dtype=torch.float32)
            for i in range(len(sax_ids)):
                waveform[i] = waveform_raw[i * chunk_size : (i + 1) * chunk_size].mean()
        else:
            waveform = torch.zeros(len(sax_ids), dtype=torch.float32)
        
        return {
            'sax_ids': sax_ids,
            'waveform_frames': waveform,
            'length': len(sax_ids),
            'title': item['title'],
            'artist': item['artist']
        }

def collate_fn(batch):
    sax_ids = [b['sax_ids'] for b in batch]
    waveforms = [b['waveform_frames'] for b in batch]
    lengths = torch.tensor([b['length'] for b in batch])
    
    sax_padded = pad_sequence(sax_ids, batch_first=True, padding_value=0)
    wave_padded = pad_sequence(waveforms, batch_first=True, padding_value=0.0)
    
    return {
        'sax_ids': sax_padded,
        'waveform': wave_padded,
        'lengths': lengths,
        'titles': [b['title'] for b in batch]
    }
