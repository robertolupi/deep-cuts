# Brainstorming: AI Auto-Tagging & Suggestion Engine

This document outlines the technical design and query flow to leverage local Qwen2-Audio models for generating creative, non-repetitive metadata tags and normalising them within the Deep Cuts SQLite database.

---

## 1. Objectives

1. **Enrich Metadata**: Go beyond basic classifier categories (e.g., standard Essentia genres) to extract rich, vibe-based descriptors (e.g., `#tension-building-beats`, `#ominous-soundscapes`, `#sonorous-textures`).
2. **Minimize Repetition**: Ensure that suggested tags are not redundant repetitions of fields already extracted (genre, mood, instruments).
3. **Normalize Inconsistencies**: Auto-clean, lowercase, strip junk punctuation, and merge semantic duplicates (e.g., merging "ambient electronic", "ambient-electronic", and "ambient").

---

## 2. Model Prompting & Pipeline Integration

The current Qwen analysis pass in [qwen.rs](file:///Users/rlupi/src/deep-cuts/src-tauri/src/analysis/qwen.rs#L271-L277) executes a multi-step conversation loop. We can add a 5th step specifically for creative tags.

### Proposed Step Configuration
```rust
let steps: Vec<(&str, Option<&str>)> = vec![
    ("genre", None),
    ("mood", Some("What is the mood and emotional feel of this track in a few words? Respond strictly in English in this format:\nMOOD: mood and emotional feel")),
    ("instruments", Some("What are the main instruments playing in this track, comma-separated? Respond strictly in English in this format:\nINSTRUMENTS: main instruments")),
    ("description", Some("Provide two to three sentences of plain prose describing the track. Respond strictly in English in this format:\nDESCRIPTION: description")),
    // New Step
    ("tags", Some("Suggest 5 other descriptive tags (as a comma-separated list) that capture the vibe, atmosphere, or style of this song, ensuring they are not repetitions of what we have already discussed. Respond strictly in English in this format:\nTAGS: tag1, tag2, tag3, tag4, tag5")),
];
```

### Combined Tagging Pass (Pipeline Integration)
In addition to the Qwen prompt pass, we will introduce a **Combined Tagging Pass** at the end of the analysis pipeline. This pass depends on all prior passes (`essentia`, `qwen`, `bpm_refinement`, `audio_analysis`). It inspects the combined output of the entire pipeline and synthesizes tags (e.g., auto-tagging a track as `#uptempo` or `#minor_key` or combining Essentia's `mood_sad` and Qwen's `description` to create unified tags).

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

### Offline WordNet Integration for Whitelist Expansion
While WordNet has limitations for real-time synonym matching due to multi-word compound expressions (like `"tension-building beats"`), we can use it **offline** during development to expand the hardcoded `COMBINED_WHITELIST` in [qwen.rs](file:///Users/rlupi/src/deep-cuts/src-tauri/src/analysis/qwen.rs#L690-L706):
- **Mechanism**: Run an offline generator script using WordNet data to fetch all hyponyms of the `"musical instrument"` (e.g. `synthesizer`, `contrabass`, `harpsichord`) and `"music genre"` synsets.
- **Benefits**:
  - Automatically builds a highly comprehensive compile-time whitelist of hundreds of instruments/genres.
  - Zero runtime dependencies or bundle size penalties, as the expanded lists are compiled as static Rust array constants.
  - Significantly improves the accuracy of `clean_qwen_tags` token filtering.
- **Recommendation**: Maintain a static, generated list built via WordNet for instrument/genre filtering, while using the local `tag_synonym_cache` + Qwen verification pipeline for semantic matching of dynamic tags.

---

## 5. UI Integration

* **Track details pane**: Display tags as clickable, rounded border chips using the cyberpunk/secondary palette `var(--sg-secondary, #fe00fe)` or cyan accents. Clicking a tag immediately filters the track library by that tag.
* **Filter sidebar**: Add a "Tags" autocomplete field or tag cloud containing popular tags for quick selection.
