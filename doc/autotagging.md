# Brainstorming: AI Auto-Tagging & Suggestion Engine

This document outlines the technical design and query flow to leverage local Qwen2-Audio models for generating creative, non-repetitive metadata tags and normalising them within the Deep Cuts SQLite database.

---

## 1. Objectives

1. **Enrich Metadata**: Go beyond basic classifier categories (e.g., standard Essentia genres) to extract rich, vibe-based descriptors (e.g., `#tension-building-beats`, `#ominous-soundscapes`, `#sonorous-textures`).
2. **Minimize Repetition**: Ensure that suggested tags are not redundant repetitions of fields already extracted (genre, mood, instruments).
3. **Normalize Inconsistencies**: Auto-clean, lowercase, strip junk punctuation, and merge semantic duplicates (e.g., merging "ambient electronic", "ambient-electronic", and "ambient").

---

## 2. Model Prompting & Pipeline Integration

The current Qwen analysis pass in [qwen.rs](file:///Users/rlupi/src/deep-cuts/src-tauri/src/analysis/qwen.rs#L271-L277) executes a multi-step conversation loop. Rather than asking a single general tagging question (which can result in generic or repetitive answers), we will split the query into focused questions targeting separate aspects:
- **Vocals**: Identifying singer characteristics (e.g. `male vocal`, `female vocal`, `instrumental`, `duet`) and language (e.g. `english`, `spanish`, `instrumental`).
- **Vibe/Atmosphere**: Creative descriptors of the emotional style and sonic atmosphere.
- **Context/Era**: Suitable listening situations (e.g. `club`, `workout`, `sleep`) and estimated decade/release era.

### Proposed Step Configuration
```rust
let steps: Vec<(&str, Option<&str>)> = vec![
    ("genre", None),
    ("mood", Some("What is the mood and emotional feel of this track in a few words? Respond strictly in English in this format:\nMOOD: mood and emotional feel")),
    ("instruments", Some("What are the main instruments playing in this track, comma-separated? Respond strictly in English in this format:\nINSTRUMENTS: main instruments")),
    ("description", Some("Provide two to three sentences of plain prose describing the track. Respond strictly in English in this format:\nDESCRIPTION: description")),
    // Focused Aspect Tagging Steps (Mapped to namespaces on ingestion):
    ("tags_vibe", Some("Suggest 3 creative tags capturing the atmosphere, vibe, or style of this song, without repeating any genres, moods, instruments, or descriptions already discussed. Respond strictly in English in this format:\nVIBE_TAGS: tag1, tag2, tag3")),
    ("tags_vocals", Some("Identify the singer voice type (e.g., male, female, instrumental, ensemble, choir) and lyrics language, without repeating any categories already discussed. Respond strictly in this format:\nVOCAL_TAGS: voice_type, language")),
    ("tags_context", Some("Suggest 2 tags indicating suitable listening contexts (e.g. study, club, sleep, workout) and 1 tag indicating the estimated release decade/era, without repeating any categories already discussed. Respond strictly in this format:\nCONTEXT_TAGS: context1, context2, era_decade")),
];
```

### Combined Tagging Pass (Deterministic, Non-LLM)
To ensure all tracks get descriptive metadata tags instantly even without running the heavy local Qwen LLM, we will introduce a **Combined Tagging Pass** at the end of the core pipeline.
- **Dependencies**: Depends on `audio_analysis`, `bpm_correction`, `essentia`, and `bpm_refinement` (does **not** depend on `qwen`).
- **Execution Speed**: Runs in `<1ms` per track as a set of compiled Rust rules.
- **Source Marker**: Tags created by this pass are labeled with the source `'combined'` in the `track_tags` database table.

#### Rule-Based Auto-Tagging Categories:
1. **Tempo Categories (BPM)**:
   - `< 90 BPM` $\rightarrow$ `bpm:downtempo`
   - `90 – 125 BPM` $\rightarrow$ `bpm:midtempo`
   - `> 125 BPM` $\rightarrow$ `bpm:uptempo`
2. **Harmonic Key Type**:
   - `scale == "minor"` $\rightarrow$ `key:minor`
   - `scale == "major"` $\rightarrow$ `key:major`
3. **Mastering Dynamics (Loudness)**:
   - `loudness_lufs > -7.0` AND `loudness_range < 4.0` $\rightarrow$ `mastering:brickwalled` (highly limited/compressed)
   - `loudness_range > 8.0` $\rightarrow$ `mastering:dynamic` (broad acoustic/classical range)
4. **Length Profile (Duration)**:
   - `< 120 seconds` $\rightarrow$ `len:short`
   - `> 420 seconds` $\rightarrow$ `len:extended`
5. **Vocal Presence (Essentia)**:
   - `detected_vocal == "voice"` AND `detected_vocal_confidence >= 0.80` $\rightarrow$ `vocals:present`
   - `detected_vocal == "instrumental"` AND `detected_vocal_confidence >= 0.80` $\rightarrow$ `vocals:instrumental`
6. **Emotive Profile (Essentia Mood Probabilities $\ge 0.75$)**:
   - `mood_sad >= 0.75` $\rightarrow$ `mood:sad`
   - `mood_aggressive >= 0.75` $\rightarrow$ `mood:aggressive`
   - `mood_relaxed >= 0.75` $\rightarrow$ `mood:relaxed`
   - `mood_party >= 0.75` $\rightarrow$ `mood:party`
   - `mood_acoustic >= 0.75` $\rightarrow$ `mood:acoustic`
   - `mood_electronic >= 0.75` $\rightarrow$ `mood:electronic`

---

## 3. Database Schema

Rather than storing tags as a comma-separated text column in the `tracks` table (which is slow to index and query), we normalize them into a standard relational schema, while keeping raw lists for diagnostic analysis:

```sql
-- Represents unique, normalized tags in the system
CREATE TABLE tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,          -- E.g. 'tension-building beats'
    normalized_name TEXT NOT NULL UNIQUE -- E.g. 'tension_building_beats' (slugified)
);

-- Many-to-many relationship mapping tags to tracks
CREATE TABLE track_tags (
    track_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    source TEXT NOT NULL,                -- 'qwen', 'essentia', 'combined', or 'user'
    PRIMARY KEY (track_id, tag_id),
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE INDEX idx_track_tags_tag ON track_tags(tag_id);

-- Diagnostic logs: preserves the original raw suggestion outputs and the outcome
-- of cleanup questions for future comparison and analysis.
CREATE TABLE tag_diagnostic_logs (
    track_id INTEGER PRIMARY KEY,
    raw_suggestions TEXT,                -- Exactly what Qwen suggested in prompt Step 5
    cleanup_outcome TEXT,                -- Normalized tags mapped after merging/cleanup checks
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

-- Synonym decision cache: stores historical evaluation decisions for tag pairs.
-- Because querying local LLMs is a high-latency CPU/GPU bound operation,
-- caching these decisions prevents duplicate LLM round-trips for common terms.
CREATE TABLE tag_synonym_cache (
    tag_a TEXT NOT NULL,                 -- Alphabetically smaller tag name (to enforce unique pairs)
    tag_b TEXT NOT NULL,                 -- Alphabetically larger tag name
    is_synonym INTEGER NOT NULL,          -- 1 if YES, 0 if NO
    evaluated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tag_a, tag_b)
);
```

---

## 4. Tag Normalization & Merging Rules

To prevent tag pollution (e.g., having separate tags for `"Ambient electronic"`, `"ambient-electronic"`, and `"ambient electronic."`), the system normalizes incoming tokens and merges synonyms:

### Normalization Pipeline
1. **Lowercase**: Convert all tags to lowercase.
2. **Sanitize Characters**:
   - Strip leading/trailing punctuation (`.`, `,`, `-`, `_`, `#`, ` `).
   - Convert hyphens and slashes to spaces or unified separators: `ambient-electronic` $\rightarrow$ `ambient electronic`.
3. **Deduplicate Multi-words**:
   - Trim interior multiple spaces to a single space.
4. **Stemming & Lemmatization (Heuristic)**:
   - Plurals to singular: `beats` $\rightarrow$ `beat`, `textures` $\rightarrow$ `texture` (when doing token matching).

### Merging Synonyms & LLM Verification
To avoid restricting the vocabulary too much while still cleaning up multiple spellings, the merging rules use a hybrid algorithm:
- **Phase A (Local Database Search)**: Search the `tags` table for spelling overlaps or high string similarity (e.g., Levenshtein distance $\le 2$).
- **Phase B (Synonym Cache Check)**: Enforce alphabetical ordering `(tag_a = min(new_tag, old_tag), tag_b = max(new_tag, old_tag))` and query the `tag_synonym_cache` table.
  - If a cached record exists $\rightarrow$ Immediately use the cached `is_synonym` result (YES/NO) and bypass the LLM.
- **Phase C (Synonym Query Validation)**: If the pair is not cached, run a validation query to Qwen:
  - *Prompt*: `"Are 'Ambient electronic' and 'ambient electronica' synonyms describing the same musical vibe? Respond with YES or NO."`
  - *Cache Writeback*: Immediately store the result in the `tag_synonym_cache` table to speed up all future scans.
- **Phase D (Preserve Diagnostics & Map)**: 
  - If YES $\rightarrow$ Map the new track to the existing tag ID to deduplicate.
  - If NO $\rightarrow$ Create a new tag entry to maintain detailed tag vocabulary.
  - Save the raw and final outputs in `tag_diagnostic_logs`.

### Concurrency Optimization: Memory-Based Union-Find Consolidation
During heavy multi-threaded library imports, scanning threads execute rapid parallel tag inserts. To avoid database write lock collisions (`SQLITE_BUSY` errors) caused by constant updates to the `tags` and `track_tags` tables:
- **Disjoint-Set Data Structure (Union-Find)**: Maintain a shared, thread-safe in-memory Disjoint-Set (Union-Find) registry representing active tags during the import session.
- **Workflow**:
  - Scanning threads push newly suggested tags into the in-memory Union-Find structure where spelling and synonym grouping is resolved in memory in $O(\alpha(N))$ time.
  - At the end of the scanning session (or periodically via a batch worker), the consolidated tags and relationships are flushed to the SQLite database in a single atomic write transaction, maximizing throughput.

### Pass Re-run Tag Clean-up Strategy
To prevent stale tag accumulation when a track's audio file is updated or a specific analysis pass is reset and re-executed:
- **Clean-up Hook**: At the start of any analysis pass (e.g., Qwen, Essentia, or Combined Tagging), the runner executes a targeted deletion utilizing the tags' source column:
  ```sql
  DELETE FROM track_tags WHERE track_id = ?1 AND source = ?2;
  ```
- **Benefit**: This cleanly wipes out previously generated tags from that specific source (e.g., `'combined'` tags) before compiling the new ones, keeping user-created tags (`source = 'user'`) fully intact.


### Offline WordNet Integration for Whitelist Expansion
While WordNet has limitations for real-time synonym matching due to multi-word compound expressions (like `"tension-building beats"`), we can use it **offline** during development to expand the hardcoded `COMBINED_WHITELIST` in [qwen.rs](file:///Users/rlupi/src/deep-cuts/src-tauri/src/analysis/qwen.rs#L690-L706):
- **Mechanism**: Run an offline generator script using WordNet data to fetch all hyponyms of the `"musical instrument"` (e.g. `synthesizer`, `contrabass`, `harpsichord`) and `"music genre"` synsets.
- **Benefits**:
  - Automatically builds a highly comprehensive compile-time whitelist of hundreds of instruments/genres.
  - Zero runtime dependencies or bundle size penalties, as the expanded lists are compiled as static Rust array constants.
  - Significantly improves the accuracy of `clean_qwen_tags` token filtering.
- **Recommendation**: Maintain a static, generated list built via WordNet for instrument/genre filtering, while using the local `tag_synonym_cache` + Qwen verification pipeline for semantic matching of dynamic tags.

## 5. UI Integration

* **Track details pane**: Display tags as clickable, rounded border chips using the cyberpunk/secondary palette `var(--sg-secondary, #fe00fe)` or cyan accents. Clicking a tag immediately filters the track library by appending `#namespace:tag` to the main search bar.
* **Filter sidebar**: Add a "Tags" autocomplete field or tag cloud containing popular tags for quick selection.
* **Inline Autocomplete**: When typing in the main "Keyword Search" input, typing a `#` triggers an autocomplete dropdown popover showing matching tag names (e.g. typing `#mo` shows `#mood:sad`, `#mood:happy`) sorted by frequency in the database.

---

## 6. Tag Search & Query Integration (Phased Implementation)

To lower implementation complexity and ensure rapid development, tag searching will be introduced in two distinct phases:

### Phase 1: Dedicated Sidebar Tag Filter (Simple AND Logic)
Instead of building a full query parser immediately, we will add a dedicated **"Tags"** selection panel to the Filter Sidebar:
- **UI Input**: An autocomplete multi-select dropdown. Typing filters the available tags in the database; clicking a suggestion adds it as a visual chip.
- **Logic**: Multiple selected tags are combined using logical `AND`. A track must contain all selected tags to match.
- **Click-to-Filter**: Clicking a tag chip in the Track Details Pane simply adds that tag to the sidebar's active tags collection.
- **Implementation Cost**: Very low (reuses existing multi-select state patterns like selected directories/keys).

### Phase 2: Unified Boolean Search (Future/Advanced)
Once the tagging database and Phase 1 filters are stable, we can optionaly upgrade the main **Keyword Search** bar to parse boolean logic expressions:
- **Syntax**: Supporting `#mood:sad AND Vangelis` or `(#mood:sad OR #key:major) NOT #bpm:uptempo`.
- **Parsing**: Lexes terms and builds a small Svelte-side Abstract Syntax Tree (AST) to evaluate track tag vectors and string metadata in sub-milliseconds.


