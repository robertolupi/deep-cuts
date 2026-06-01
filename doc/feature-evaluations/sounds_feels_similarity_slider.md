# Technical Evaluation: "Sounds vs. Feels" Similarity Slider

## 1. Feature Overview & User Experience
The "Sounds vs. Feels" Similarity Slider provides an interactive recommendation sidebar in the player panel, letting DJs and producers dynamically adjust what kind of similarity they want to prioritize when looking for matching songs:
* **"Sounds Like" (Acoustic)**: Matches tracks that share acoustic properties like tempo, groove, density, spectral weight, and vocal presence (CLAP embeddings).
* **"Feels Like" (Semantic)**: Matches tracks that share conceptual, narrative, instrumental, or mood qualities (MiniLM description embeddings).
* **The Interaction**: The sidebar renders a slider. Sliding it toward "Sounds Like" highlights songs with identical acoustic textures. Sliding it toward "Feels Like" highlights songs that tell similar stories, use identical instruments, or share emotional moods, regardless of genre or tempo.

## 2. Technical Feasibility & Architecture

### A. Database Changes
* **Database Schema**: No new tables are needed. It uses the existing `audio_embeddings` (512-d) and `description_embeddings` (384-d) `vec0` virtual tables.
* **Queries**: The Rust backend will execute two parallel nearest-neighbor vector similarity lookups to retrieve the top 100 closest matches in each space:
  ```sql
  -- Query 1: Acoustic Nearest Neighbors
  SELECT track_id, distance FROM audio_embeddings WHERE embedding MATCH ? ORDER BY distance LIMIT 100;
  
  -- Query 2: Semantic Nearest Neighbors
  SELECT track_id, distance FROM description_embeddings WHERE embedding MATCH ? ORDER BY distance LIMIT 100;
  ```

### B. Rust Backend Services
* **Tauri Command**: Implement `get_similar_tracks_blended(track_id: i64, clap_weight: f64, limit: usize)` which:
  1. Fetches the CLAP vector and the description vector for the target `track_id` from the database.
  2. Runs both SQL queries to fetch the top 100 neighbors in each embedding space.
  3. **Z-Score Normalization & Percentile Distance Blend**: Standardizes raw cosine distances using Z-score normalization and mapping to percentile ranks within each embedding domain. This resolves manifold scale skewing between the high-dimensional CLAP acoustic space and MiniLM semantic space, ensuring both dimensions contribute equally.
  4. **Linear Blending**: Computes a blended similarity score based on the normalized percentile ranks:
     $$S_{\text{blend}} = w_{\text{clap}} \cdot S_{\text{clap\_percentile}} + (1.0 - w_{\text{clap}}) \cdot S_{\text{desc\_percentile}}$$
  5. Sorts the results, selects the top `limit` tracks, and returns them along with their matching percentages.

### C. Svelte Frontend Controls
* **Sidebar Component**: Add an expandable sidebar to the audio player or detail panel.
* **Interactive Slider**: A custom glassmorphic slider styled with a Cyberpunk glow (left colored cyan, right colored magenta). It includes explicit **Vibe Anchors** marked along the slider track: **"Groove/Texture"** (100% CLAP), **"Balanced Blend"** (50% CLAP / 50% MiniLM), and **"Narrative Vibe"** (100% MiniLM) to guide user interaction.
* **Rune Binding**: Bind the slider value to a reactive `$state` rune. Use a debounced effect (`$effect`) to invoke the blended similarity endpoint as the slider moves, preventing UI lag.
* **Dynamic List-Morphing**: The list of recommended tracks integrates Svelte 5's `animate:flip` directive to animate item re-ordering dynamically as the slider shifts weights, providing a highly fluid, morphing list UX.

## 3. Implementation Roadmap & Sizing
* **Phase 1: Core Backend & Data Models**: 1.5 dev-days (Rust command, parallel SQL queries, distance standardization, and linear blending mathematics).
* **Phase 2: Svelte Interface & Visual Layers**: 1.5 dev-days (Svelte sidebar, custom debounced slider, and animated result list).
* **Phase 3: Polish, Edge Cases, & Tests**: 0.5 dev-days (handling tracks without description embeddings, empty libraries, unit tests).
* **Total Estimated Dev-Time**: 3.5 dev-days

## 4. Performance & Resource Impact
* **CPU / GPU Overhead**: Extremely low. Running distance calculations on $2 \times 100$ items takes less than 1ms in Rust and doesn't require any neural network inference (pre-computed embeddings are read from disk).
* **Memory Footprint**: Negligible. Storing a few arrays of floats in memory uses virtually zero RAM.
* **Database Size Impact**: Zero. It executes standard read-only SELECT queries on pre-existing indexes.

## 5. Technical Uncertainty & Risk Analysis
* **Risk Level**: Low. The distance combining logic is simple, standard math.
* **Potential Gaps**: Tracks without descriptions (e.g. tracks where the Qwen pass hasn't run yet) won't have a semantic vector. The Rust backend must gracefully fall back to $100\%$ CLAP acoustic similarity when the description vector is missing.

## 6. Scoring Matrix & Priority
* **Effort Score**: 3 / 10 (3.5 dev-days total)
* **Uncertainty Score**: 2 / 10 (minor logic needed to merge datasets and handle missing values)
* **Performance Impact Score**: 1 / 10 (completely pre-computed reading, blazing fast)
* **Wow Factor Score**: 9 / 10 (highly satisfying interaction that gives direct, visual feedback on audio clustering)
* **Priority Score**: 9 / 10 (blended rating)

### Scoring Rationale
This feature has a massive visual impact and demonstrates the combined power of acoustic (CLAP) and semantic (Qwen) embeddings. It has very low dev effort and runs entirely on pre-calculated data, making it a high-priority addition.
