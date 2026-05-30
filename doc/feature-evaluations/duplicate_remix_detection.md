# Technical Evaluation: Duplicate & Remix Detection

## 1. Feature Overview & User Experience

Deep Cuts can detect three tiers of track relationships using only the data already computed during the analysis pipeline — no external fingerprinting library required.

* **Exact / Encoding Duplicates**: The same audio master stored twice, possibly at different bitrates or in different formats (e.g. a 320kbps MP3 and a FLAC of the same song). Identical musically; one copy is redundant storage.
* **Near-Duplicates**: The same song with minor audio differences — a slightly different mix, a remaster, a single edit vs. an album version with a different intro length. Acoustically near-identical but technically distinct files.
* **Remixes & Versions**: The same underlying musical material rearranged, extended, slowed down, or reinterpreted — DJ remixes, radio edits, instrumentals, acapellas. Musically related but genuinely different tracks.

**The User Flow:**
* A **Duplicates** section appears in the Library Settings page and the filter sidebar. It lists detected duplicate groups with their confidence tier (Exact / Near / Remix).
* In the track table, duplicate tracks show a small badge icon. Hovering reveals "2 copies — same master" or "Remix of: Track X".
* In the Music Map, exact duplicates **collapse to a single dot** with a badge showing the copy count. Remixes stay as separate dots but are **linked by a faint edge** that appears when a track is selected, surfacing the remix cluster.
* A **"Clean up duplicates"** action in Library Settings lets the user review duplicate groups and choose which copies to keep, reveal in Finder, or simply acknowledge.

---

## 2. Technical Feasibility & Architecture

### A. Detection Tiers & Signals

Detection is a multi-signal scoring system. Each tier requires a different combination of signals to fire:

**Tier 1 — Exact / Encoding Duplicate**

| Signal | Threshold |
|---|---|
| CLAP cosine similarity | ≥ 0.97 |
| Duration difference | ≤ 2 seconds |
| BPM match | within 2% (or both NULL) |
| Key + scale match | same (or both NULL) |

All four signals must pass. These are file-level duplicates — the same master re-encoded or copied.

**Tier 2 — Near-Duplicate**

| Signal | Threshold |
|---|---|
| CLAP cosine similarity | ≥ 0.90 |
| Duration difference | ≤ 15 seconds |
| BPM match | within 5% |
| Key match | same |

At least CLAP similarity + one of the remaining signals must pass. Covers remasters, alternate mixes, single vs. album versions.

**Tier 3 — Remix / Version**

| Signal | Threshold |
|---|---|
| CLAP cosine similarity | ≥ 0.75 |
| Title keyword heuristic | title contains "remix", "edit", "mix", "version", "instrumental", "acapella", "radio edit", "extended", "vip", "dub" (case-insensitive) |
| Artist overlap | at least one artist token in common |

CLAP similarity + at least one of the metadata heuristics must pass. BPM is intentionally not required — remixes are frequently pitched or stretched.

---

### B. Database Changes

**New table: `track_relationships`**

```sql
CREATE TABLE track_relationships (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id_a      INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    track_id_b      INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    relationship    TEXT NOT NULL CHECK(relationship IN ('duplicate', 'near_duplicate', 'remix')),
    confidence      REAL NOT NULL,   -- CLAP cosine similarity, 0.0–1.0
    confirmed       INTEGER DEFAULT 0, -- 0 = auto-detected, 1 = user confirmed, -1 = user dismissed
    UNIQUE(track_id_a, track_id_b)
);

CREATE INDEX idx_track_relationships_a ON track_relationships(track_id_a);
CREATE INDEX idx_track_relationships_b ON track_relationships(track_id_b);
```

**Additions to `tracks` table** (via migration):
```sql
ALTER TABLE tracks ADD COLUMN duplicate_group_id INTEGER DEFAULT NULL;
-- NULL = no duplicates; non-NULL = all tracks sharing this ID form a duplicate group
-- Populated only for Tier 1 (exact) duplicates; used to collapse map dots.
```

Migration file: `14_track_relationships.sql`.

---

### C. Rust Backend Services

**Detection command: `detect_duplicates(conn_state)`**

Run on demand (user clicks "Scan for duplicates" in settings) or automatically after a library scan completes. The algorithm:

1. **Load all CLAP embeddings** into memory as a matrix of L2-normalised 512-d vectors.
2. **Batch cosine similarity scan**: For each track, query the `audio_embeddings` vec0 virtual table for its top-K nearest neighbours (K=20) using the existing `MATCH` syntax. This leverages the already-built ANN index and runs in O(N · log N) time.
3. **Apply tier rules**: For each (track_a, track_b) pair returned above the Tier 3 threshold (0.75), cross-check duration, BPM, key, and title heuristics to assign the correct tier.
4. **Assign `duplicate_group_id`**: For Tier 1 pairs, run a union-find to group all connected exact duplicates and assign a shared group ID. Within each group, the track with the highest bitrate (or largest file size) is designated the "primary".
5. **Persist** all detected relationships to `track_relationships`, skipping pairs where `confirmed = -1` (user-dismissed).

```rust
#[tauri::command]
pub async fn detect_duplicates(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    app: tauri::AppHandle,
) -> Result<DuplicateScanResult, String>
```

Returns a `DuplicateScanResult` struct summarising counts per tier.

**Query command: `get_track_relationships(track_id, conn_state)`**

Returns all relationships for a given track, used by the frontend to render the badge and the map edge overlay.

**Dismiss / confirm commands:**
```rust
pub fn confirm_relationship(id: i64, conn_state: ...) -> Result<(), String>
pub fn dismiss_relationship(id: i64, conn_state: ...) -> Result<(), String>
```

Setting `confirmed = -1` excludes a pair from future scans.

---

### D. Svelte Frontend Controls

* **Library Settings page**: A "Duplicates" card showing the scan button, last-scan timestamp, and a grouped list of detected relationships with tier badges and confidence percentages. Each group has "Keep best quality / Reveal all in Finder / Dismiss" actions.
* **TrackList badge**: A small icon on duplicate/remix rows (e.g. a stack-of-pages icon for duplicates, a rotate icon for remixes). Clicking it opens a popover showing the related tracks.
* **Music Map**: 
  - Tier 1 exact duplicates collapse to one dot. A badge on the dot shows the copy count.
  - Tier 3 remix edges: when a dot is selected, faint lines connect it to its remix relations. A "Show remixes" toggle in the map toolbar enables/disables all edges globally.
* **Filter sidebar**: A "Duplicates" section with toggles: "Hide exact duplicates", "Show only duplicates", "Show remix clusters".

---

## 3. Implementation Roadmap & Sizing

* **Phase 1: Core Backend & Data Models** — 1.5 dev-days
  - `14_track_relationships.sql` migration
  - `detect_duplicates` command with batch cosine scan and tier classification
  - Union-find for `duplicate_group_id` assignment
  - `get_track_relationships`, `confirm_relationship`, `dismiss_relationship` commands

* **Phase 2: Svelte Interface** — 1.5 dev-days
  - Duplicates card in Library Settings
  - TrackList row badges and popover
  - Map dot collapsing for exact duplicates
  - Map remix edge overlay

* **Phase 3: Polish & Edge Cases** — 0.5 dev-days
  - Handle tracks where embeddings are not yet computed (skip gracefully)
  - Re-scan behaviour after new tracks are added to the library
  - Performance test on libraries of 10,000+ tracks

* **Total Estimated Dev-Time**: 3.5 dev-days

---

## 4. Performance & Resource Impact

* **CPU Overhead**: Low. The detection scan uses the existing sqlite-vec ANN index, so finding top-20 neighbours for all N tracks is O(N · log N). On a 5,000-track library, a full scan takes an estimated 2–5 seconds in Rust.
* **Memory Footprint**: Low. The vec0 index is already in the database; no in-memory matrix is needed.
* **Database Size Impact**: Minimal. The `track_relationships` table adds at most N×K rows (e.g. 5,000 × 20 = 100,000 rows × ~40 bytes = ~4 MB). In practice far fewer rows survive the tier thresholds.
* **Re-analysis required**: No. Detection runs entirely on existing CLAP embeddings and track metadata.

---

## 5. Technical Uncertainty & Risk Analysis

* **Risk Level**: Low.
* **CLAP similarity false positives for remixes**: Two tracks in the same genre with a very similar production style (e.g. two minimal techno tracks by the same producer) might score above 0.75 CLAP similarity without being genuine remixes. The title heuristic + artist overlap cross-check significantly reduces false positives. User dismissal (`confirmed = -1`) handles any remaining noise.
* **Large-library scan time**: On libraries above 50,000 tracks, the O(N · log N) ANN scan might take 30–60 seconds. This can be mitigated by running the scan as a background job emitting progress events, and by only re-scanning tracks added since the last scan rather than the full library every time.
* **Threshold tuning**: The CLAP similarity thresholds (0.97 / 0.90 / 0.75) are derived from the pairwise distance distribution observed in the current library (median pairwise L2 ≈ 1.1, nearest-neighbour median ≈ 0.3). They may need adjustment for libraries with a narrower genre range (e.g. an all-jazz or all-EDM collection where overall similarity is higher).
