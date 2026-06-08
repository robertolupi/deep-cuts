import numpy as np
import onnxruntime as ort
from tokenizers import Tokenizer
from pathlib import Path
from typing import List

class EmbeddingGenerator:
    def __init__(self, root_path: Path):
        self.root_path = Path(root_path).resolve()
        self.model_path = self.root_path / "models" / "all-minilm-l6-v2.onnx"
        self.tokenizer_path = self.root_path / "models" / "all-minilm-l6-v2-tokenizer.json"
        self.session = None
        self.tokenizer = None

    def _load(self):
        if self.session is None:
            if not self.model_path.exists():
                raise FileNotFoundError(f"ONNX model not found at {self.model_path}. Run export scripts first.")
            if not self.tokenizer_path.exists():
                raise FileNotFoundError(f"Tokenizer not found at {self.tokenizer_path}")
            
            # Setup session options to silence standard outputs
            opts = ort.SessionOptions()
            opts.log_severity_level = 3
            
            self.session = ort.InferenceSession(str(self.model_path), sess_options=opts)
            self.tokenizer = Tokenizer.from_file(str(self.tokenizer_path))
            self.tokenizer.enable_truncation(max_length=512)

    def generate_embedding(self, text: str) -> List[float]:
        self._load()
        enc = self.tokenizer.encode(text)
        
        input_ids = np.array([enc.ids], dtype=np.int64)
        attention_mask = np.array([enc.attention_mask], dtype=np.int64)
        token_type_ids = np.array([enc.type_ids], dtype=np.int64)
        
        outputs = self.session.run(
            ["sentence_embedding"],
            {
                "input_ids": input_ids,
                "attention_mask": attention_mask,
                "token_type_ids": token_type_ids
            }
        )
        vec = outputs[0][0].tolist()
        
        # Normalise to unit vector
        norm = sum(x*x for x in vec)**0.5
        if norm > 1e-8:
            vec = [x/norm for x in vec]
        return vec
