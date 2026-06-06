import torch
import torch.nn as nn

class SAXGRUModel(nn.Module):
    """Baseline: Embedding + BiGRU for SAX sequence classification"""
    def __init__(self, vocab_size, embed_dim=64, hidden_dim=128, num_layers=2, num_classes=10, dropout=0.2):
        super().__init__()
        self.embedding = nn.Embedding(vocab_size, embed_dim, padding_idx=0)
        self.gru = nn.GRU(embed_dim + 1, hidden_dim, num_layers, 
                          batch_first=True, bidirectional=True, dropout=dropout if num_layers>1 else 0)
        self.fc = nn.Sequential(
            nn.Linear(hidden_dim * 2, hidden_dim),
            nn.ReLU(),
            nn.Dropout(dropout),
            nn.Linear(hidden_dim, num_classes)
        )
    
    def forward(self, sax_ids, waveform, lengths=None):
        # sax_ids: [B, L], waveform: [B, L]
        x = self.embedding(sax_ids)  # [B, L, E]
        # Concat waveform as extra feature
        wave_feat = waveform.unsqueeze(-1)  # [B, L, 1]
        x = torch.cat([x, wave_feat], dim=-1)
        
        # Pack for efficiency if lengths provided
        if lengths is not None:
            x = nn.utils.rnn.pack_padded_sequence(x, lengths.cpu(), batch_first=True, enforce_sorted=False)
        
        out, _ = self.gru(x)
        
        if lengths is not None:
            out, _ = nn.utils.rnn.pad_packed_sequence(out, batch_first=True)
        
        # Use last valid output
        if lengths is not None:
            idx = (lengths - 1).unsqueeze(1).unsqueeze(2).expand(-1, 1, out.size(2))
            last_out = out.gather(1, idx).squeeze(1)
        else:
            last_out = out[:, -1, :]
        
        return self.fc(last_out)


class TinySAXTransformer(nn.Module):
    """Tiny Transformer for SAX: better for long-range patterns"""
    def __init__(self, vocab_size, d_model=128, nhead=4, num_layers=4, num_classes=10, max_len=512, dropout=0.1):
        super().__init__()
        self.embedding = nn.Embedding(vocab_size, d_model, padding_idx=0)
        self.wave_proj = nn.Linear(1, d_model)
        self.pos_encoder = nn.Parameter(torch.randn(1, max_len, d_model) * 0.02)
        
        encoder_layer = nn.TransformerEncoderLayer(
            d_model=d_model, nhead=nhead, dim_feedforward=d_model*4,
            dropout=dropout, batch_first=True, activation='gelu'
        )
        self.transformer = nn.TransformerEncoder(encoder_layer, num_layers=num_layers)
        self.fc = nn.Sequential(
            nn.LayerNorm(d_model),
            nn.Linear(d_model, num_classes)
        )
        self.d_model = d_model
    
    def forward(self, sax_ids, waveform, lengths=None):
        B, L = sax_ids.shape
        x = self.embedding(sax_ids) * (self.d_model ** 0.5)
        wave_emb = self.wave_proj(waveform.unsqueeze(-1))
        x = x + wave_emb + self.pos_encoder[:, :L, :]
        
        # Create padding mask
        if lengths is not None:
            mask = torch.arange(L, device=sax_ids.device).expand(B, L) >= lengths.unsqueeze(1)
        else:
            mask = sax_ids == 0
        
        x = self.transformer(x, src_key_padding_mask=mask)
        
        # Mean pooling over valid tokens
        if lengths is not None:
            mask_float = (~mask).float().unsqueeze(-1)
            x = (x * mask_float).sum(dim=1) / mask_float.sum(dim=1).clamp(min=1)
        else:
            x = x.mean(dim=1)
        
        return self.fc(x)
