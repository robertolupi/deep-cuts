# Design Proposal: Qwen & CLAP Synergy for Audio Tagging & Verification

This document proposes combining the generative strengths of Qwen (audio-to-text LLM) with the discriminative/contrastive alignment capability of CLAP (Contrastive Language-Audio Pretraining) to produce cleaner, verified, and hallucination-free metadata for our music library.

---

## 1. Context & Motivation

Currently, we run two separate analysis passes on each audio track:
1. **CLAP Pass ([clap.rs](file:///Users/rlupi/src/deep-cuts/src-tauri/src/analysis/clap.rs)):** Computes a 512-dimensional audio embedding using `clap_audio_encoder.onnx` and saves it to the `audio_embeddings` database table.
2. **Qwen Pass ([qwen.rs](file:///Users/rlupi/src/deep-cuts/src-tauri/src/analysis/qwen.rs)):** Queries a local `llama-server` using Qwen-Audio to generate a natural language description and extract structured keywords (genre, mood, instruments).

### The Problem
Generative models like Qwen are highly expressive and excellent at synthesising context, but they suffer from **hallucinations**. For example:
- Generating instruments that are not present (e.g., claiming a track has "violin" or "flute" when it is pure synthesizer).
- Misidentifying the emotional mood or genre under format constraints.

### The Opportunity
CLAP was trained to map audio and text to the **same 512-dimensional space**, optimizing for cosine similarity between matching audio and text pairs.
* **Qwen is Generative:** It can compose novel description text.
* **CLAP is Discriminative:** It is very good at scoring how well a given text aligns with an audio clip.

By checking Qwen's output text against the CLAP text encoder, we can quantitatively verify, rank, and clean up the generated metadata.

---

## 2. Model Training & Theoretical Basis

CLAP (`laion/clap-htsat-unfused`) is trained on paired audio and text data using a **contrastive InfoNCE loss**. The training objective pulls matching audio-text pairs close together in the shared embedding space, while pushing unmatched pairs apart.

Since both the audio embedding ($E_{\text{audio}}$) and the text embedding ($E_{\text{text}}$) are L2-normalized:
$$\text{Cosine Similarity} = \text{Dot Product} = \sum_{i=1}^{512} E_{\text{audio}}[i] \times E_{\text{text}}[i]$$

* **Perfect match:** Similarity approaches $1.0$ (practically, strong matches score between $0.20$ and $0.45$).
* **Irrelevant/Mismatched:** Similarity approaches $0.0$ or becomes negative.

We can run the Qwen text through the CLAP text encoder ([run_clap_text_embed](file:///Users/rlupi/src/deep-cuts/src-tauri/src/embeddings.rs#L661)) and calculate the dot product with the already saved audio embedding to evaluate Qwen's description.

---

## 3. Proposed Integration Strategies

### Strategy A: Full Description Validation & Selective Resampling (ON-DEMAND RETRY)
We can calculate a "confidence score" for the generated description. If Qwen hallucinates or completely misses the track's context, the CLAP similarity will be low.

> [!IMPORTANT]
> **Performance Optimization:** Because Qwen inference is extremely slow (~5-10x slower than other passes), we **must not** run multiple generative passes by default. Instead, we use an **early-exit / on-demand resampling** pipeline:

1. **First-Pass Generation:** Run Qwen once (standard generation) to get the initial description.
2. **CLAP Verification:** Compute CLAP similarity between the track's audio embedding and the generated description:
   ```rust
   let text_embedding = crate::embeddings::run_clap_text_embed(&description, Some(&app_handle))?;
   ```
3. **Threshold Check:** 
   * If similarity $\ge \text{Threshold}$ (e.g., $0.16$): **Accept description immediately** and proceed. No extra performance overhead.
   * If similarity $< \text{Threshold}$: **Trigger Resampling**. Only for this small percentage of tracks do we re-run Qwen with different parameters (e.g., lower temperature, more conservative prompt, or direct reference to key acoustic characteristics) to correct the hallucination.

---

### Strategy B: Granular Keyword & Attribute Filtering (Soft-Label Tuning)
Qwen often produces mostly accurate descriptions but gets specific lists (like `INSTRUMENTS` or `MOOD`) slightly wrong. We can use CLAP as a zero-shot classifier to weed out false positives.

1. **Extraction:** Qwen returns `INSTRUMENTS: piano, violin, synthesizer, drums`.
2. **Template Expansion:** For each instrument, we wrap it in a natural language template (CLAP embeddings are sensitive to sentence structure; full sentences usually score better than raw keywords):
   - `"A recording featuring piano."`
   - `"A recording featuring violin."`
   - `"A recording featuring synthesizer."`
   - `"A recording featuring drums."`
3. **CLAP Verification:** Compute CLAP text embeddings for each template and calculate similarity against the track's audio embedding.
4. **Filtering:** Apply a threshold (e.g., $0.18$):
   - `piano` (Score: $0.28$) $\rightarrow$ **Keep**
   - `violin` (Score: $0.08$) $\rightarrow$ **Discard**
   - `synthesizer` (Score: $0.34$) $\rightarrow$ **Keep**
   - `drums` (Score: $0.30$) $\rightarrow$ **Keep**

---

### Strategy C: Multi-Candidate Selection (DISCARDED)
* **Status:** **DISCARDED**
* **Rationale:** Prompting Qwen to generate 3 separate completions and selecting the best one via CLAP would take $3\times$ longer. Since Qwen is already the most expensive pass in the pipeline, this is not viable for desktop-grade local execution.

---

## 4. Threshold Calibration Procedure (Using Ground Truth)

Choosing the correct similarity thresholds (for both description validation and keyword filtering) is critical to avoid infinite loops or discarding valid labels. We can use the user's **180-song ground truth dataset** (with Suno-style prompts) to tune these thresholds empirically.

### The Methodology

To determine the optimal threshold, we must characterize two distributions:
1. **Positive Pairs ($S_{\text{pos}}$):** Cosine similarities of matching audio and ground-truth text.
2. **Negative Pairs ($S_{\text{neg}}$):** Cosine similarities of mismatched audio and text (e.g., audio $A_i$ matched with text $T_j$ from a different song).

```
   Negative Distribution (Mismatched)        Positive Distribution (Matched)
             ┌───┐                                     ┌───┐
             │   │                                     │   │
           ┌─┘   └─┐                                 ┌─┘   └─┐
           │       │                                 │       │
      ─────┴───────┴───────┬─────────────────────────┴───────┴─────►
                          0.15 (Optimal Threshold)              Cosine Similarity
```

### Steps to Run the Calibration:
1. Calculate the 512-d CLAP audio embedding $A_i$ for each track $i \in [1, 180]$.
2. Calculate the 512-d CLAP text embedding $T_i$ for its corresponding Suno-style ground truth prompt.
3. Compute the **Positive Similarities**:
   $$S_{\text{pos}} = \{ A_i \cdot T_i \mid i \in [1, 180] \}$$
4. Compute the **Negative Similarities** (by shuffling the matches):
   $$S_{\text{neg}} = \{ A_i \cdot T_j \mid i \neq j \text{ for a sample of random pairs} \}$$
5. Plot the distributions (or compute statistics):
   * **Recall-focused selection:** If we want to capture 95% of valid descriptions without triggering resampling, we set the threshold at the **5th percentile** of the positive distribution (e.g., if only 5% of true matches fall below $0.15$, then $0.15$ is our threshold).
   * **ROC Curve / Precision-Recall:** Run a sweep of thresholds from $0.0$ to $0.5$ and find the threshold that maximizes the F1-score or separates matching/mismatched distributions with the least overlap.

### Calibrating Keywords (Soft-Labels)
We can use the detailed Suno-style prompts as **soft-labels** for keywords:
1. Extract list of instruments from the Suno prompts (e.g. if the prompt says "acoustic guitar, light piano", the ground truth instruments are `guitar` and `piano`).
2. Run the CLAP template verification for all candidate instruments.
3. Use the presence/absence of instruments in the prompt as labels to calculate the Precision/Recall of CLAP's instrument detection at different thresholds.
4. Set the keyword threshold (e.g., $0.18$) at the point that maximizes the balanced F1-score.

---

## 5. Script Outline for Calibration

A diagnostic script (similar to [check_embeddings.py](file:///Users/rlupi/src/deep-cuts/tools/check_embeddings.py)) can be written to perform this sweep:

```python
# tools/calibrate_thresholds.py
import numpy as np

def calibrate(audio_embeddings, text_embeddings):
    # positive pairs (diagonal)
    pos_similarities = np.sum(audio_embeddings * text_embeddings, axis=1)
    
    # negative pairs (all off-diagonal pairs)
    neg_similarities = []
    for i in range(len(audio_embeddings)):
        for j in range(len(text_embeddings)):
            if i != j:
                neg_similarities.append(np.dot(audio_embeddings[i], text_embeddings[j]))
    
    neg_similarities = np.array(neg_similarities)
    
    print(f"Positive Matches: Mean={pos_similarities.mean():.4f}, Std={pos_similarities.std():.4f}")
    print(f"Negative Matches: Mean={neg_similarities.mean():.4f}, Std={neg_similarities.std():.4f}")
    
    # Sweep threshold to find point of minimum classification error
    best_threshold = 0.0
    best_accuracy = 0.0
    for t in np.linspace(0.0, 0.4, 41):
        tp = np.sum(pos_similarities >= t)
        fp = np.sum(neg_similarities >= t)
        fn = np.sum(pos_similarities < t)
        tn = np.sum(neg_similarities < t)
        accuracy = (tp + tn) / (tp + fp + fn + tn)
        if accuracy > best_accuracy:
            best_accuracy = accuracy
            best_threshold = t
            
    print(f"Optimal Threshold: {best_threshold:.2f} (Accuracy={best_accuracy * 100:.2f}%)")
```
