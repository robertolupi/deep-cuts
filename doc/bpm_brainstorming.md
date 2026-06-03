# BPM Detection Improvements: Architectural Proposals

This document outlines structural and algorithmic proposals to address weaknesses in the current BPM detection pipeline—specifically, failure modes involving tracks with spoken word intros, beat-less segments, ambient sections, or complex rhythmic shifts.

---

## 1. The Core Issue: Envelope Pollution

The current BPM algorithm ([`compute_bpm_from_mono`](file:///Users/rlupi/src/deep-cuts/src-tauri/src/dsp.rs#L483)) computes the **spectral flux** across the entire 90-second cropped center window of a track, followed by global autocorrelation. 

When a track begins with spoken words, movie samples, or a long ambient soundscape:
* **Onset Noise**: Irregular, non-rhythmic speech onsets pollute the spectral flux envelope.
* **Autocorrelation Smearing**: The lack of periodic peaks in the speech/ambient portions lowers the contrast of the correct lag peak in the global autocorrelation, causing octave errors (halving/doubling) or completely incorrect tempo calculations.

---

# BPM Detection Improvements: Architectural Proposals

This document outlines structural and algorithmic proposals to address weaknesses in the current BPM detection pipeline—specifically, failure modes involving tracks with spoken word intros, beat-less segments, ambient sections, or complex rhythmic shifts.

---

## 1. The Core Issue: Envelope Pollution

The current BPM algorithm ([`compute_bpm_from_mono`](file:///Users/rlupi/src/deep-cuts/src-tauri/src/dsp.rs#L483)) computes the **spectral flux** across the entire 90-second cropped center window of a track, followed by global autocorrelation. 

When a track begins with spoken words, movie samples, or a long ambient soundscape:
* **Onset Noise**: Irregular, non-rhythmic speech onsets pollute the spectral flux envelope.
* **Autocorrelation Smearing**: The lack of periodic peaks in the speech/ambient portions lowers the contrast of the correct lag peak in the global autocorrelation, causing octave errors (halving/doubling) or completely incorrect tempo calculations.

---

## 2. Combined Pipeline: Joint BPM (A+B) and Key Analysis Preprocessing

Instead of running key and BPM analysis as completely separate passes over the raw audio, we propose **folding them together** into a single, cohesive front-end preprocessing pipeline. This optimizes CPU performance (calculating FFTs and spectral metrics once) and ensures that key analysis benefits from the same noise-rejection logic as BPM.

```
                         +------------------------------------+
                         |        Decoded Audio Mono          |
                         +------------------------------------+
                                           |
                                           v
                         +------------------------------------+
                         | Segment into 10s Window blocks     |
                         +------------------------------------+
                                           |
                                           v
                         +------------------------------------+
                         | Compute block-level metrics:       |
                         | - Onset Strength Variance         |
                         | - Spectral Flatness               |
                         +------------------------------------+
                                           |
                                           v
                               [Is block rhythmic & tonal?]
                               /                          \
                            Yes                            No (Discard Block)
                             /                              \
                            v                                v
       +------------------------------------------+    +----------------------------+
       |   Joint Feature Extraction on Block:      |    | Discard speech/ambient segment|
       |   - Run BPM Estimators (ACF, Comb, etc)   |    +----------------------------+
       |   - Extract 12-bin Chromagram vector      |
       +------------------------------------------+
                  /                            \
                 v                              v
     +-----------------------+     +--------------------------+
     | Consensus BPM Voting  |     | Accumulate Block Chromas |
     | (Lag Bucket Histogram)|     | & Run Pearson Match      |
     +-----------------------+     +--------------------------+
                 |                              |
                 v                              v
     +-----------------------+     +--------------------------+
     |    Octave Folding     |     |   Output Root + Scale    |
     |    (Final BPM)        |     |  (Higher Key Confidence) |
     +-----------------------+     +--------------------------+
```

### Integrated Workflow:
1. **Shared Front-End Filter (Proposal A / Key Noise Rejection)**: Slice the track's audio window into sliding 10-second segments. Discard any segments that have low onset strength variance (speech, silence) or high spectral flatness (noise, soundscapes). This ensures we only analyze portions of the track that contain musical rhythm and harmonic tonality.
2. **Joint Feature Extraction (Proposal B / Key Chroma Extract)**: For all surviving rhythmic-harmonic blocks, perform:
   * **BPM Multi-Estimator Run**: Feed the onset envelopes concurrently into ACF on Spectral Flux, Comb Filter Resonators, and Tempograms.
   * **Chroma Extraction**: Accumulate the 12-bin chromagram values from the FFT bins of that block.
3. **BPM consensus & Octave Folding**: Collect the top lag candidates from all detectors across all valid blocks into a global **lag bucket histogram**, applying sub-harmonic folding ($2\times$ and $0.5\times$ votes weighted) to resolve the final BPM.
4. **Harmonic Key Aggregation**: Aggregate only the chroma profiles from the *surviving* tonal blocks. Run HPCP-style harmonic suppression on the aggregated profile, then perform Krumhansl-Schmuckler Pearson correlation to determine the final key, scale, and a much cleaner `key_strength` confidence score.

---

## 3. Proposal C: Improving Existing Correction & Refinement Passes

We already have two post-analysis passes in the codebase:
1. **`bpm_correction`**: Corrects the raw BPM using coarse metadata.
2. **`bpm_refinement`**: Refines the corrected BPM using Discogs subgenres.

Currently, these passes rely on **static binary bounds** defined in [`src-tauri/src/bpm.rs`](file:///Users/rlupi/src/deep-cuts/src-tauri/src/bpm.rs#L15). If a track's estimated BPM is slightly outside the hardcoded range, it is immediately discarded and set to `NULL` (`CorrectResult::Null`).

We propose **improving these existing passes** in two ways:

### A. Dynamic Genre Resolution
Right now, the passes only check standard file tags. We should extend `bpm_correction` and `bpm_refinement` to:
1. First look up the Essentia 400-genres classification already computed and stored in the database.
2. Fall back to the AI-predicted genre (`ai_genre`) parsed from Qwen if standard metadata is missing or inconclusive.

### B. Transitioning from Binary Ranges to Fuzzy Probability Profiles
Instead of hard boundary checks (e.g. `118.0..138.0`), the correction logic should model standard genre tempos as **Gaussian probability envelopes** (bell curves).

```
   Static Binary Bounds (Current)              Fuzzy Probability Profile (Proposed)
    
     Null  |   Corrected   |  Null               Low Prob |   High Confidence   | Low Prob
   --------[===============]--------            ----------(         \         )----------
          118             138                             118       128       138
```

* **How it helps**: If a track is detected at 116 BPM, and its genre is House, a binary check drops it to `NULL`. A fuzzy profile assigns it a confidence score (e.g., $0.65$) rather than $0.0$.
* **Gaussian Math Details**:
  For each genre, define a centroid tempo ($\mu$) and a spread/standard deviation ($\sigma$). The probability score for a given BPM candidate $x$ is:
  $$\text{Score}(x) = e^{-\frac{(x - \mu)^2}{2\sigma^2}}$$
  For example, for House music ($\mu = 126$, $\sigma = 6$):
  * For $x = 126$ (centroid), $\text{Score}(126) = 1.0$.
  * For $x = 116$, $\text{Score}(116) = e^{-100/72} \approx 0.25$.
  * For $x = 232$ (double-tempo error), $\text{Score}(232) \approx 0.0$.
* **Smoothing Decisions**: The decision to halve or double is guided by comparing the probability score of $v$, $v/2$, and $2v$. We select the multiplier that yields the highest score. If all choices yield a probability under a low floor (e.g., $<0.10$), only then do we nullify the BPM.

---

## 4. Verification and Testing Plan

To ensure the new DSP pipelines and corrections are correct, fast, and robust, we will implement the following verification mechanisms:

### A. Testing Proposal A+B (Rhythmic Segmentation & Voting DSP)
1. **Mathematical Unit Tests in `dsp.rs`**:
   * **Segment Filtering**: Write tests with artificial sine sweeps/white noise (low onset variance, high spectral flatness) vs. synthetic drum loops (high onset variance, low spectral flatness) to verify that the front-end correctly filters out non-rhythmic blocks.
   * **Multi-Detector Consensus**: Use synthetic waveforms with known periodicities (e.g., a simple drum impulse train at 120 BPM) and check that the consensus voting engine accurately converges to $120.0$ BPM.
2. **Regression DSP Testing**:
   * Run the updated `compute_bpm_from_mono` on a mock set of audio files (e.g., existing test audio fixtures) to compare execution speed and result consistency before and after the block-slicing refactor.

### B. Testing Proposal C (Fuzzy Probability Profiles & Fallbacks)
1. **Fuzzy Profile Unit Tests in `bpm.rs`**:
   * Add test cases comparing the scoring for standard BPMs, marginal BPMs (e.g. 115 BPM for House, which would have been nullified under old code but should now halve/double or remain valid with low confidence), and extreme outliers.
   * Assert correct octave folding behaviors:
     * e.g., input $240$ BPM for a Hip-Hop track ($\mu=92.5$) must resolve to $120$ BPM because it scores higher on the Gaussian curve than $240$ or $60$.
2. **Database Integration Tests**:
   * Write tests querying mock rows containing Essentia genre fields and Qwen `ai_genre` columns to ensure correct priority fallbacks (`Essentia -> AI Genre -> Coarse Tags`).

### C. Real-World Track Validation
1. **Targeted Failure-Case Testing**:
   * Collect a subset of tracks from the user's library where the current BPM detection pipeline fails (specifically those with spoken introductions, acoustic ambient intros, or complex rhythm envelopes).
   * Record their ground-truth BPMs manually (or from verified external tags).
   * Run both the old and new detection algorithms on these files to verify that:
     * Non-rhythmic intros are ignored.
     * The correct BPM is detected instead of octave-doubled or random speech envelope noise values.
     * The fuzzy post-correction matches the manually expected BPM.

---

## 5. Next Steps: Revised Integration Plan

```markdown
- [ ] Phase 1: Implement the unified A+B and Key analysis pipeline in `src-tauri/src/dsp.rs` (10s block filtering, multi-detector BPM voting, and aggregated segment key extraction).
- [ ] Phase 2: Refactor `src-tauri/src/bpm.rs` to replace static ranges with fuzzy probability profiles.
- [ ] Phase 3: Update the correction passes (`bpm_correction.rs` & `bpm_refinement.rs`) to retrieve the `ai_genre` column and Essentia classifications from the database for resolving the prior profiles.
```

