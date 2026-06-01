# Technical Evaluation: Pathfinding Playlists (Transitions)

## 1. Feature Overview & User Experience
Pathfinding Playlists allow DJs and producers to generate highly cohesive transitional playlists by mapping geometric "journeys" through the UMAP music map:
* **The Interaction**: The user clicks a **Start Song** (e.g. an ambient warm-up track) and an **End Song** (e.g. a peak-time hard rock track) on the 2D map.
* **Interactive Map Waypoints**: Users can drag the glowing transition path line directly on the 2D map to create custom routing path nodes, forcing the algorithm to pathfind through specific acoustic waypoints (e.g., dragging the line through a jazz-funk region to bridge ambient electronica and garage rock).
* **The Calculation**: The app draws a glowing path connecting the start, waypoints, and end tracks through intermediate neighboring dots, showing the transition step-by-step.
* **Cross-Genre Bridge Recommendations**: When the path generator detects a major boundary jump between distinct genre clusters (where UMAP distance exceeds a set threshold), it flags the jump and inserts bridge suggestions, recommending specific transition techniques (e.g., *Echo-out*, *Tempo-ramping*, or *Power Intro*).
* **The Playback**: The resulting tracks are loaded into a playlist. Since geometric proximity on the map represents acoustic and semantic similarity, the playlist smoothly morphs in BPM, key, genre, and instruments from start to finish.

## 2. Technical Feasibility & Architecture

### A. Database Changes
* **Database Schema**: No new tables are needed. The coordinates are pre-computed and fetched from the `tracks` table.

### B. Rust Backend Services
* **Tauri Command**: Implement `find_transition_path(start_id: i64, end_id: i64, waypoints: Vec<i64>, target_steps: usize)`:
  1. Load all coordinates `(x, y)` and IDs from the `tracks` table.
  2. **Graph Construction**: Construct a $k$-Nearest Neighbor (k-NN) graph where nodes are tracks and edges connect the 5 closest acoustic neighbors.
  3. **Multi-Objective $A^*$ Cost Function**: Execute an $A^*$ search on each leg of the journey (start ➔ waypoint A ➔ waypoint B ➔ end). The cost function $C(u, v)$ for moving from node $u$ to $v$ is calculated as:
     $$C(u, v) = w_1 \cdot D_{\text{UMAP}}(u, v) + w_2 \cdot D_{\text{Camelot}}(u, v) + w_3 \cdot D_{\text{BPM}}(u, v)$$
     Where:
     - $D_{\text{UMAP}}(u, v)$ is the Euclidean distance in UMAP space.
     - $D_{\text{Camelot}}(u, v)$ is the step distance on the Camelot wheel (e.g. $1$ step = adjacent/same; $>2$ steps penalizes highly).
     - $D_{\text{BPM}}(u, v) = \frac{|\text{BPM}_u - \text{BPM}_v|}{\max(\text{BPM}_u, \text{BPM}_v)}$ is the normalized tempo delta.
     - $w_1, w_2, w_3$ are user-configurable weighting coefficients.
  4. **Path Resampling & Bridge Flagging**: If a segment contains a high UMAP distance gap, mark it for a transition helper and return bridge suggestions. Resample the path nodes to yield the desired step count while maintaining smooth spacing.
  5. Return the ordered list of `Track` structs with transition markers to the frontend.

### C. Svelte Frontend Controls
* **Map Selector Mode**: Add a "Transit Mode" to the map UI. When active, clicking tracks sets the start/end markers.
* **Interactive Waypoint Handles**: Draw draggable circle handles on the path canvas line. When a user drags a point near an unselected track, it snaps as an intermediate waypoint, triggering an asynchronous path recalculation.
* **Transition Bridge Cards**: Display popovers in the playlist sidebar highlighting suggested transition types (e.g., "Tempo-ramping suggested here: 95 BPM ➔ 124 BPM" or "Echo-out suggested: incompatible harmonic keys").

## 3. Implementation Roadmap & Sizing
* **Phase 1: Core Backend & Data Models**: 3.0 dev-days (graph-building logic, multi-objective $A^*$ pathfinding implementation in Rust, waypoint routing, and resampling algorithms).
* **Phase 2: Svelte Interface & Visual Layers**: 3.0 dev-days (canvas path rendering, interactive waypoint dragging mechanics, start/end marker UI overlays, and playlist sidebar integrations).
* **Phase 3: Polish, Edge Cases, & Tests**: 1.0 dev-day (handling disconnected clusters/graph islands, smoothing outliers).
* **Total Estimated Dev-Time**: 7.0 dev-days

## 4. Performance & Resource Impact
* **CPU / GPU Overhead**: Very low. Constructing a k-NN graph on 5,000 nodes takes about 10–20ms in Rust; executing a multi-objective $A^*$ search on it takes less than 5ms. Recalculating pathing on-the-fly during waypoint dragging is fast and fluid.
* **Memory Footprint**: Extremely light ($<5\text{MB}$ RAM for graph representation).
* **Database Size Impact**: Zero. It uses pre-computed coordinates and metadata.

## 5. Technical Uncertainty & Risk Analysis
* **Risk Level**: Medium.
* **Graph Islands (Disconnected Clusters)**: If the library contains highly isolated genre clusters (e.g. an "Island" of electronic tracks far from a "Continent" of acoustic tracks), the $A^*$ search might fail to find any path. The Rust backend must implement a fallback that bridges disconnected clusters by creating "artificial bridges" between the closest boundary nodes.
* **UI Responsiveness during Dragging**: Recalculating the A* path dynamically on every mousemove event can cause stuttering. We mitigate this by throttling/debouncing the path recalculation during waypoint drags, or running the path recalculation inside a Web Worker / lightweight Rust thread.

## 6. Scoring Matrix & Priority
* **Effort Score**: 6.5 / 10 (7.0 dev-days total due to interactive dragging and multi-objective routing math)
* **Uncertainty Score**: 4 / 10 (graph connectivity fallbacks and mathematical weight tuning)
* **Performance Impact Score**: 1 / 10 (extremely fast, zero overhead)
* **Wow Factor Score**: 9.5 / 10 (delivers a spectacular, interactive playlist curation experience unlike any traditional DJ software)
* **Priority Score**: 8.0 / 10 (blended rating)

### Scoring Rationale
This is a highly innovative DJ/curation tool. The addition of interactive dragging waypoints and custom transition advice gives the app a top-tier visual flow. While it increases front-end complexity, the math is robust, and the resulting user experience delivers a high "wow factor" that justifies a strong priority.
