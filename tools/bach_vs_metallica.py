import numpy as np, librosa, torch, torch.nn.functional as F
from transformers import ClapProcessor, ClapModel

MODEL_ID = "laion/clap-htsat-unfused"
CLAP_SR, CLAP_10S_SAMPLES = 48000, 480_000

model = ClapModel.from_pretrained(MODEL_ID).eval()
processor = ClapProcessor.from_pretrained(MODEL_ID)

db_path = '/Users/rlupi/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db'
import sqlite3, sqlite_vec
conn = sqlite3.connect(db_path)
conn.enable_load_extension(True); sqlite_vec.load(conn); conn.enable_load_extension(False)

metallica = [r[0] for r in conn.execute("SELECT path FROM tracks WHERE path LIKE '%etallica%' LIMIT 3").fetchall()]
bach = [r[0] for r in conn.execute("SELECT path FROM tracks WHERE path LIKE '%BWV%' OR path LIKE '%Brandenburg%' LIMIT 3").fetchall()]

def embed(path):
    audio, _ = librosa.load(path, sr=CLAP_SR, mono=True)
    mid = len(audio)//2; half = CLAP_10S_SAMPLES//2
    window = audio[max(0,mid-half):max(0,mid-half)+CLAP_10S_SAMPLES].copy()
    window = np.pad(window, (0, max(0, CLAP_10S_SAMPLES-len(window))))
    inputs = processor(audio=window, sampling_rate=CLAP_SR, return_tensors="pt")
    is_longer = torch.zeros(1, dtype=torch.bool)
    with torch.no_grad():
        out = model.audio_model(input_features=inputs["input_features"], is_longer=is_longer)
        e = F.normalize(model.audio_projection(out.pooler_output), p=2, dim=-1).numpy()[0]
    return e

print("Computing embeddings...")
m_embs = [(p.split('/')[-1][:40], embed(p)) for p in metallica]
b_embs = [(p.split('/')[-1][:40], embed(p)) for p in bach]

print("\n=== Metallica vs Bach ===")
for mn, me in m_embs:
    for bn, be in b_embs:
        d = np.sqrt(np.sum((me-be)**2))
        print(f"  {mn:40s} vs {bn:40s}  dist={d:.4f}")

print("\n=== Metallica vs Metallica ===")
for i, (n1, e1) in enumerate(m_embs):
    for n2, e2 in m_embs[i+1:]:
        d = np.sqrt(np.sum((e1-e2)**2))
        print(f"  {n1:40s} vs {n2:40s}  dist={d:.4f}")

print("\n=== Bach vs Bach ===")
for i, (n1, e1) in enumerate(b_embs):
    for n2, e2 in b_embs[i+1:]:
        d = np.sqrt(np.sum((e1-e2)**2))
        print(f"  {n1:40s} vs {n2:40s}  dist={d:.4f}")
