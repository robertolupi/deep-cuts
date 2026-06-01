# Tagging System — Ideas & Design Notes

## Motivation

Many of the current filters (key, genre, instruments, mood, vocal/instrumental) are essentially boolean or categorical tags attached to tracks. A generalised tagging system would:

- Unify these into a single queryable concept
- Allow user-defined tags (manual curation, playlists-as-tags, project labels)
- Enable fuzzy/weighted tags from AI analysis (confidence scores rather than hard yes/no)
- Simplify the filter sidebar — instead of N separate filter widgets, one tag-aware search

---

## Tag Categories

### System tags (auto-generated, read-only)

Derived from existing analysis passes. Users cannot edit these but can filter on them.

| Tag | Source | Example values |
|---|---|---|
| `key:*` | Essentia | `key:C`, `key:F#` |
| `scale:*` | Essentia | `scale:major`, `scale:minor` |
| `camelot:*` | Derived | `camelot:8B` |
| `bpm:*` | Essentia / correction pass | `bpm:120` |
| `genre:*` | Essentia classifier | `genre:electronic`, `genre:jazz` |
| `ai-genre:*` | Qwen2-Audio | `ai-genre:ambient` |
| `mood:*` | Essentia mood classifiers | `mood:happy`, `mood:aggressive` |
| `instrument:*` | Qwen2-Audio | `instrument:guitar`, `instrument:piano` |
| `vocal:*` | Essentia / Qwen2 | `vocal:vocals`, `vocal:instrumental` |
| `artist:*` | ID3 tag | `artist:boards of canada` |
| `album:*` | ID3 tag | `album:geogaddi` |
| `year:*` | ID3 tag | `year:2002` |
| `folder:*` | Watched directory | `folder:references` |

### Fuzzy / weighted system tags

Rather than hard thresholds, AI-derived mood and genre tags carry a confidence weight (0.0–1.0). The filter UI can expose a confidence slider: "show tracks where `mood:happy` > 0.7".

This generalises the current "Mood: Happy" Essentia score columns into the tag namespace without losing the continuous nature of the underlying data.

### User tags (manual, read-write)

Free-form tags the user applies manually. Examples: `reference`, `todo-edit`, `favourite`, `loop-worthy`, `needs-key-check`. These are the building blocks of lightweight project organisation without a full playlist system.

- Any string is valid (lowercased, trimmed)
- Multiple tags per track
- Auto-complete from existing tags as the user types
- Can be applied in bulk from the track list (multi-select → tag)

---

## Data Model

### New table: `tags`

```sql
CREATE TABLE tags (
    id      INTEGER PRIMARY KEY,
    name    TEXT NOT NULL UNIQUE  -- e.g. "mood:happy", "reference", "instrument:guitar"
);
```

### New table: `track_tags`

```sql
CREATE TABLE track_tags (
    track_id   INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    tag_id     INTEGER NOT NULL REFERENCES tags(id)   ON DELETE CASCADE,
    weight     REAL,        -- NULL for user tags; 0.0–1.0 for fuzzy system tags
    source     TEXT NOT NULL, -- 'user', 'essentia', 'qwen2', 'musicbrainz', etc.
    PRIMARY KEY (track_id, tag_id)
);

-- Composite Indexes for performant filtering and analytical queries
CREATE INDEX idx_track_tags_tag_id_weight ON track_tags(tag_id, weight);
CREATE INDEX idx_track_tags_track_id_source ON track_tags(track_id, source);
```

System tags are populated by analysis passes. User tags have `source = 'user'` and `weight = NULL`.

---

## Namespaced Tag Sources & Colors

To help users instantly identify tag origins, we structure tag queries and UI chips using visual colon namespaces (`namespace:value`) and specific color schemes:

| Tag Category / Source | Visual Namespace | Background / Accent Color | Description / Examples |
|---|---|---|---|
| **User Custom Tags** | *No namespace* or `custom:` | Teal / Emerald (`#0F766E` / `#047857`) | Purely manual curation: `reference`, `custom:loop-worthy` |
| **Musical Tags** | `key:`, `camelot:`, `bpm:`, `scale:` | Purple / Violet (`#6D28D9` / `#7C3AED`) | Core musical & key attributes computed by Essentia |
| **AI Descriptors** | `mood:`, `genre:`, `instr:`, `vocal:` | Orange / Amber (`#C2410C` / `#D97706`) | AI-generated confidence-weighted parameters (Qwen2/Essentia) |

This system allows high scannability inside the track list and detail panel, as well as a standardized notation inside the tag filter bar.

---

### Existing columns

The current scalar columns (`mood_happy`, `detected_genre`, `key`, etc.) are kept as-is — the tag system is an additional layer on top, not a replacement. Analysis passes write both the raw column and the corresponding tag row. This avoids a large migration and keeps raw data accessible for the map/embeddings.

---

## Filter Sidebar

The tag system would allow the sidebar to evolve in two directions:

1. **Tag search bar** — a single input that accepts tag expressions: `mood:happy instrument:guitar -vocal:vocals`. Supports negation (`-`), wildcards (`mood:*`), and confidence thresholds (`mood:happy>0.7`).

2. **Gradual unification** — existing filter widgets (Key, BPM range, Genre, Vocals) remain for discoverability but are internally backed by the tag query engine rather than bespoke filter logic.

These two directions can coexist: power users use the tag bar, casual users use the widgets.

---

## User-Defined Tag UI

- Tag chips displayed on the selected track in the player bar / detail panel
- Click a chip to filter by that tag
- "+" button to add a new tag (with autocomplete)
- Right-click a chip to remove it
- Bulk tagging: multi-select tracks in the library, right-click → "Add tag…"

---

## Open Questions

1. **Tag namespacing** — should user tags live in a separate namespace (e.g. `user:reference`) to avoid collisions with system tags, or is a flat namespace with a `source` discriminator enough?

2. **Saved tag queries** — should users be able to save a tag expression as a named filter (essentially a smart playlist)? Natural extension but out of scope for the initial design.

3. **Tag-based map colouring** — the map currently colours by genre/camelot/BPM. User tags could be a new colour mode: highlight tracks matching a tag expression. Worth exploring once the tag system exists.

4. **Migration of existing filters** — when the tag system is implemented, analysis passes should backfill tags for already-analysed tracks. This is a one-time migration triggered on first launch after the schema change.

---

## Cross-References

- **AcoustID enrichment** (`acoustid-metadata-enrichment.md`) — MusicBrainz genre tags fetched during enrichment can seed the tag system automatically as `source: 'musicbrainz'`, providing a third tag source alongside Essentia and user-defined tags.
- **Saved searches** (`playlists_and_saved_searches.md`) — saved tag queries are effectively smart playlists; the two systems are composable. A tag expression saved as a search gives tag-based filtering a persistent, named home in the sidebar.
- **Statistics page** (`statistics_page.md`) — the analysis coverage section can auto-apply a `needs-analysis` tag to tracks missing a given pass, turning a coverage gap observation directly into an actionable filter.
