# Technical Evaluation: Local NLP Semantic Search

## 1. Feature Overview & User Experience
Local NLP Semantic Search allows users to query their entire music library using natural language descriptions of the music's vibe, instrumentation, or atmospheric qualities, rather than relying on exact filename matching or tag filtering.
* **The User Flow**: Inside the standard Search input, the user can toggle between `Standard Text` and `Semantic (AI)` modes. 
* **Interaction**: The user types queries like *"heartfelt singer-songwriter ballads with acoustic guitar"* or *"heavy industrial synth drops with distorted beats"*. 
* **The Experience**: The list reactively filters and ranks tracks by concept similarity in real-time, showing a small similarity percentage match badge (e.g. `94% match`) next to each result, providing a premium, fluid search feel.

## 2. Technical Feasibility & Architecture

### A. Database Changes
* **Database Schema**: No new tables are needed. The SQLite schema migration [11_description_embeddings.sql](file:///Users/rlupi/src/deep-cuts/src-tauri/migrations/11_description_embeddings.sql) already sets up a 384-dimensional `vec0` virtual table named `description_embeddings`.
* **Queries**: Similarity queries will run against the virtual table using `MATCH` syntax:
  ```sql
  SELECT track_id, distance 
  FROM description_embeddings 
  WHERE embedding MATCH ? 
  ORDER BY distance 
  LIMIT ?;
  ```

### B. Rust Backend Services
* **Model Inference**: We already have `run_sentence_embed(text, app)` fully implemented inside [embeddings.rs](file:///Users/rlupi/src/deep-cuts/src-tauri/src/embeddings.rs) which leverages the local sentence transformer session (`all-MiniLM-L6-v2.onnx`) to generate L2-normalized 384-d float vectors.
* **Tauri Command**: Implement a new IPC command `search_similar_tracks_semantic(query: String, limit: usize)` inside `commands/map.rs` or `commands/analysis.rs` that:
  1. Executes `run_sentence_embed` on the user's query.
  2. Converts the float array into standard bytes.
  3. Queries `description_embeddings` to retrieve the closest matching `track_id` rows.
  4. Returns the associated `Track` structs loaded from the `tracks` table.

### C. Svelte Frontend Controls
* **Search Input Modifier**: Update the standard Search text field in `TrackList.svelte` to include a sleek toggle button (a subtle AI spark icon).
* **Rune State**: Bind search state reactively using `$state` runes. If the AI mode is active, query the Tauri semantic search IPC endpoint instead of running local client-side string regex filters.
* **Visuals**: Display a subtle glowing matching percentage next to search results.

## 3. Implementation Roadmap & Sizing
* **Phase 1: Core Backend & Data Models**: 1.0 dev-day (implementing `search_similar_tracks_semantic` and basic SQL binding).
* **Phase 2: Svelte Interface & Visual Layers**: 1.0 dev-day (adding toggle, AI input styling, and match indicators).
* **Phase 3: Polish, Edge Cases, & Tests**: 0.5 dev-days (empty queries, missing model alerts, error banners).
* **Total Estimated Dev-Time**: 2.5 dev-days

## 4. Performance & Resource Impact
* **CPU / GPU Overhead**: Negligible. Executing MiniLM ONNX inference on a single text sentence takes less than 30–50ms on M-series chips and consumes a tiny slice of CPU.
* **Memory Footprint**: Extremely light. The MiniLM ONNX session and tokenizer take less than 100MB of RAM, which is kept lazy-loaded in the Rust session pool.
* **Database Size Impact**: Zero. The `description_embeddings` virtual table is already bootstrapped; search operations are read-only.

## 5. Technical Uncertainty & Risk Analysis
* **Risk Level**: Extremely Low. The model sessions are already compiled and tested, the virtual SQLite table is bootstrapped, and the embedding generator is active.
* **Potential Gaps**: None. This is a highly straightforward, standard application of our existing machine learning foundation.

## 6. Scoring Matrix & Priority
* **Effort Score**: 2 / 10 (2.5 dev-days total)
* **Uncertainty Score**: 1 / 10 (completely standard math and fully functional codebase endpoints)
* **Performance Impact Score**: 1 / 10 (instant execution, negligible RAM/CPU)
* **Wow Factor Score**: 9 / 10 (a local, offline AI search that works instantly is highly impressive)
* **Priority Score**: 9.5 / 10 (blended rating)

### Scoring Rationale
As a low-risk, extremely high-yield implementation using already-completed backend code, this stands as our highest priority feature.
