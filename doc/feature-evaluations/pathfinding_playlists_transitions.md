# Technical Evaluation: Pathfinding Playlists (Transitions)

## 1. Feature Overview & User Experience
Pathfinding Playlists allow DJs and producers to generate highly cohesive transitional playlists by mapping geometric "journeys" through the UMAP music map:
* **The Interaction**: The user clicks a **Start Song** (e.g. an ambient warm-up track) and an **End Song** (e.g. a peak-time hard rock track) on the 2D map.
* **The Calculation**: The app draws a glowing path connecting the two tracks through intermediate neighboring dots, showing the transition step-by-step.
* **The Playback**: The resulting tracks are loaded into a playlist. Since geometric proximity on the map represents acoustic and semantic similarity, the playlist smoothly morphs in BPM, key, genre, and instruments from start to finish.

## 2. Technical Feasibility & Architecture

### A. Database Changes
* **Database Schema**: No new tables are needed. The coordinates are pre-computed and fetched from the `tracks` table.

### B. Rust Backend Services
* **Tauri Command**: Implement `find_transition_path(start_id: i64, end_id: i64, target_steps: usize)`:
  1. Load all coordinates `(x, y)` and IDs from the `tracks` table.
  2. **Graph Construction**: Construct a $k$-Nearest Neighbor (k-NN) graph where nodes are tracks and edges connect the 5 closest acoustic neighbors.
  3. **Pathfinding Search**: Execute an **$A^*$ or Dijkstra search** on the graph using Euclidean distance as the heuristic to find the shortest, smoothest path from the start node to the end node.
  4. **Path Resampling**: If the shortest path contains more/fewer nodes than the user's `target_steps`, resample the path nodes to yield the desired step count while maintaining smooth spacing.
  5. Return the ordered list of `Track` structs to the frontend.

### C. Svelte Frontend Controls
* **Map Selector Mode**: Add a "Transit Mode" to the map UI. When active, clicking tracks sets the start/end markers.
* **Path Visualizer**: Draw a glowing, pulsating path line (e.g. using canvas `context.lineTo`) connecting the dots.
* **Playlist Creator**: A slide-out panel showing the transitional queue with an "Export to Playlist" button.

## 3. Implementation Roadmap & Sizing
* **Phase 1: Core Backend & Data Models**: 2.0 dev-days (graph-building logic, $A^*$ pathfinding implementation in Rust, and resampling algorithms).
* **Phase 2: Svelte Interface & Visual Layers**: 2.0 dev-days (canvas path rendering, start/end marker UI overlays, and playlist sidebar integrations).
* **Phase 3: Polish, Edge Cases, & Tests**: 1.0 dev-day (handling disconnected clusters/graph islands, smoothing outliers).
* **Total Estimated Dev-Time**: 5.0 dev-days

## 4. Performance & Resource Impact
* **CPU / GPU Overhead**: Very low. Constructing a k-NN graph on 5,000 nodes takes about 10–20ms in Rust; executing an $A^*$ search on it takes less than 2ms.
* **Memory Footprint**: Extremely light ($<5\text{MB}$ RAM for graph representation).
* **Database Size Impact**: Zero. It uses pre-computed coordinates and metadata.

## 5. Technical Uncertainty & Risk Analysis
* **Risk Level**: Medium.
* **Graph Islands (Disconnected Clusters)**: If the library contains highly isolated genre clusters (e.g. an "Island" of electronic tracks far from a "Continent" of acoustic tracks), the $A^*$ search might fail to find any path. The Rust backend must implement a fallback that bridges disconnected clusters by creating "artificial bridges" between the closest boundary nodes.
* **Musical Vibe Consistency**: A purely geometric path can occasionally choose a "bridge" song that has identical UMAP coordinates but represents a jarring key or BPM clash. We should add a penalty weight to edges in the $A^*$ search that clash significantly in BPM or Key.

## 6. Scoring Matrix & Priority
* **Effort Score**: 5 / 10 (5.0 dev-days total)
* **Uncertainty Score**: 4 / 10 (graph connectivity fallbacks and musical alignment heuristics)
* **Performance Impact Score**: 1 / 10 (extremely fast, zero overhead)
* **Wow Factor Score**: 8 / 10 (delivers a highly unique, automated playlist curation experience)
* **Priority Score**: 7 / 10 (blended rating)

### Scoring Rationale
This is a highly innovative DJ/curation tool. It requires writing graph and pathfinding algorithms in Rust and managing zoom-synchronized path lines on the HTML5 canvas. It has high wow factor and low performance overhead, making it a very strong mid-priority feature.
