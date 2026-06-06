# Design: Batch Analysis Passes in Rust

**Author:** Claude  
**Date:** 2026-06-06  
**Status:** Ready for implementation  

---

## Problem Statement

The existing `AnalysisPass` trait is one-track-at-a-time:

```
load_jobs() → [Job, Job, Job, …]
    for each Job:
        lock conn → SELECT waveform_data → unlock
        compute
        lock conn → UPDATE tracks → UPDATE track_passes → unlock
```

This shape fits passes that are slow per-track (CLAP, Qwen, Essentia) — the per-track overhead is negligible compared to the inference cost. It breaks down for two different reasons in two passes:

### Reason A: I/O-bound passes (sax)

The `sax` pass reads `waveform_data` (a large JSON float array) from SQLite, runs a fast in-memory computation (PAA + z-normalise + quantise), and writes a 32-character string back. The computation is microseconds; the I/O dominates.

With 1800 tracks the current loop does **~5400 individual SQLite operations** (1 SELECT + 2 UPDATEs per track), each acquiring the connection lock separately. This freezes the UI for minutes.

The fix: read all pending tracks in one SELECT, compute all in memory, write all in one transaction. That's **3 operations total** regardless of library size.

### Reason B: Algorithmically global passes (structure_cluster)

The structure clustering pass cannot process one track at a time by definition — DBSCAN requires the full pairwise distance matrix. The existing trait cannot express this at all.

### Solution: `BatchAnalysisPass` trait

A single new trait covers both cases. The pass owns its entire read-compute-write loop; the orchestrator just calls `execute()` once.

---

## 1. The `BatchAnalysisPass` Trait

```rust
pub trait BatchAnalysisPass: Send + Sync {
    fn name(&self) -> &'static str;
    fn priority(&self) -> i32;
    fn version(&self) -> u32;
    fn dependencies(&self) -> &'static [&'static str];
    fn owned_tables(&self) -> &'static [&'static str];  // cleared on reset

    /// Return true if there is work to do.
    fn needs_run(&self, conn: &Connection) -> Result<bool, String>;

    /// Read, compute, write — all in one call.
    /// Returns a human-readable summary for logging ("1782 tracks, 14 clusters").
    fn execute(&self, app: &AppHandle, conn: &Connection) -> Result<String, String>;
}
```

### Orchestrator helper

```rust
pub fn run_batch_pass<P: BatchAnalysisPass>(
    app: &AppHandle,
    conn_arc: &Arc<Mutex<Connection>>,
    pass: P,
    run_id: &str,
) -> Result<(), String> {
    let conn = conn_arc.lock()…;
    if !pass.needs_run(&conn)? {
        log::info!("[{}] nothing to do, skipping", pass.name());
        return Ok(());
    }
    log::info!("[{}] starting", pass.name());
    let summary = pass.execute(app, &conn)?;
    log::info!("[{}] done — {}", pass.name(), summary);
    // emit progress event to frontend
    Ok(())
}
```

### `track_passes` integration for batch passes

Batch passes still participate in the per-track pass tracking table — this is how reset, dependency checking, and version management work. The difference is they update all rows in a single transaction rather than one at a time.

**`needs_run`:** query `SELECT COUNT(*) FROM track_passes WHERE pass_name = ? AND status = 'pending'`. Returns true if > 0.

**Inside `execute`:** at the end of the transaction, run `UPDATE track_passes SET status = 'done', finished_at = … WHERE pass_name = ? AND track_id IN (…)` for all processed tracks in one statement.

This preserves compatibility with the existing reset infrastructure (`reset_pass_for_track`, `PASS_REGISTRY`) without any changes.

---

## 2. Pass A: Sax (converted from `AnalysisPass` to `BatchAnalysisPass`)

### Current behaviour (broken for large libraries)

```
for each pending track:           ← N iterations
    SELECT waveform_data          ← 1 read per track
    compute waveform_to_sax()     ← fast, microseconds
    UPDATE tracks SET waveform_sax
    UPDATE track_passes SET done  ← 2 writes per track
```

Total: `3N` DB operations, `2N` lock acquisitions.

### New behaviour

```
SELECT id, waveform_data FROM tracks
  JOIN track_passes ON … WHERE status = 'pending' AND pass_name = 'sax'
  ← 1 read, all tracks

for each row (in memory, no DB):
    compute waveform_to_sax()     ← pure CPU

BEGIN TRANSACTION
  for each result:
    UPDATE tracks SET waveform_sax = ?
    (batch via executemany equivalent)
  UPDATE track_passes SET status = 'done' WHERE track_id IN (…)
COMMIT
  ← 1 transaction, O(1) lock acquisitions
```

Total: **2 DB operations** (1 SELECT + 1 transaction), regardless of library size.

### `SaxPass` as `BatchAnalysisPass`

```rust
pub struct SaxPass;

impl BatchAnalysisPass for SaxPass {
    fn name(&self)         -> &'static str           { "sax" }
    fn priority(&self)     -> i32                    { 12 }
    fn version(&self)      -> u32                    { pass_version::SAX }
    fn dependencies(&self) -> &'static [&'static str] { &["audio_analysis"] }
    fn owned_tables(&self) -> &'static [&'static str] { &[] }

    fn needs_run(&self, conn: &Connection) -> Result<bool, String> {
        // SELECT COUNT(*) FROM track_passes WHERE pass_name='sax' AND status='pending'
    }

    fn execute(&self, _app: &AppHandle, conn: &Connection) -> Result<String, String> {
        // 1. SELECT all pending tracks with their waveform_data
        // 2. Compute waveform_to_sax() for each (skip + log on error)
        // 3. conn.execute("BEGIN")?
        // 4. For each result: UPDATE tracks SET waveform_sax = ?
        // 5. UPDATE track_passes SET status='done' WHERE pass_name='sax' AND track_id IN (…)
        // 6. conn.execute("COMMIT")?
        // 7. Sidecar save (can also be batched)
        // 8. Return "1782 processed, 3 skipped (flat/short)"
    }
}
```

The `waveform_to_sax()` function itself is unchanged — it is already a pure function with no DB dependency.

### Progress events

The per-track loop currently emits progress events on each track, driving the UI progress bar. With a batch pass this is replaced by a single event before and after (or a few intermediate events for very large libraries). This is acceptable — the sax pass is fast enough in batch mode that the UI will update effectively instantaneously.

### Sidecar saves

`sidecar::save()` currently writes per-track. It can stay that way — call it in a loop after the main transaction, or batch it into the same transaction. Either is fine; sidecar writes are tiny.

---

## 3. Pass B: Structure Cluster (new `BatchAnalysisPass`)

### New DB migration 30 — `structure_clusters` table

```sql
CREATE TABLE structure_clusters (
    id          INTEGER PRIMARY KEY,  -- matches structure_cluster_id in tracks
    label       TEXT NOT NULL,        -- human-readable: "I·VPC×2·O"
    regex       TEXT NOT NULL,        -- filter-compatible: "^I+(V+P+C+){2,}O+$"
    track_count INTEGER NOT NULL DEFAULT 0
);
```

`tracks.structure_cluster_id` already exists (migration 28). It now semantically references `structure_clusters.id`.

### Algorithm

Implemented entirely in `src-tauri/src/analysis/structure_cluster.rs` in four self-contained layers:

**Layer A — string utilities**

```rust
/// Collapse adjacent identical characters: "IIVVPCCCCO" → "IVPCO"
pub fn skeleton(s: &str) -> String { … }

/// O(mn) dynamic-programming edit distance
pub fn levenshtein(a: &str, b: &str) -> usize { … }
```

**Layer B — clustering**

DBSCAN on the pairwise skeleton Levenshtein distance matrix.

Why DBSCAN rather than HDBSCAN + UMAP:
- UMAP has no production Rust implementation
- Skeleton strings are 4–12 chars — the distance space is already low-dimensional
- DBSCAN gives noise points and variable cluster count like HDBSCAN
- ~50 lines of Rust, deterministic, zero external crates

```rust
/// eps      – max skeleton Levenshtein distance to be neighbours (start: 2)
/// min_pts  – minimum neighbourhood to form a cluster (start: 20)
/// Returns per-point label: -1 = noise, 0..k = cluster id
pub fn dbscan(dist: &[Vec<usize>], eps: usize, min_pts: usize) -> Vec<i32> { … }
```

Starting hyperparameters: `eps = 2`, `min_pts = 20`. Defined as `const` in the pass — tune if cluster count is outside a useful range (8–20) after a full library run.

**Layer C — naming**

```rust
/// Mode skeleton across a set of alignment strings
pub fn dominant_skeleton(alignments: &[&str]) -> String { … }

/// "IVPCVPCVPCO" → label="I·VPC×3·O", regex="^I+(V+P+C+){3,}O+$"
///
/// Algorithm:
///   1. Peel single-occurrence prefix (typically 'I' or 'E')
///   2. Peel single-occurrence suffix (typically 'O' or 'V')
///   3. Find shortest repeating unit in the middle:
///        try lengths 1..middle.len()/2, accept first that tiles exactly
///   4. Format label (omit ×1); format regex (Letter+ per char, (Block){n,} for repeats)
///      anchored with ^ and $
pub fn name_skeleton(sk: &str) -> (String, String) { … }
```

The produced regex is directly usable as `filters.structureFilter` — same JS regex engine, same `sax_alignment` target field.

**Layer D — the pass**

```rust
impl BatchAnalysisPass for StructureClusterPass {
    fn name(&self)         -> &'static str           { "structure_cluster" }
    fn priority(&self)     -> i32                    { 14 }  // after sax_alignment (13)
    fn dependencies(&self) -> &'static [&'static str] { &["sax_alignment"] }
    fn owned_tables(&self) -> &'static [&'static str] { &["structure_clusters"] }

    fn needs_run(&self, conn: &Connection) -> Result<bool, String> {
        // true if structure_clusters is empty OR
        // any track has sax_alignment NOT NULL AND structure_cluster_id IS NULL
    }

    fn execute(&self, _app: &AppHandle, conn: &Connection) -> Result<String, String> {
        // 1. SELECT id, sax_alignment FROM tracks WHERE sax_alignment IS NOT NULL
        // 2. skeleton() each alignment
        // 3. pairwise skeleton Levenshtein → dist: Vec<Vec<usize>>
        // 4. dbscan(dist, eps=2, min_pts=20) → per-track cluster labels
        // 5. Group track ids by cluster label
        // 6. For each cluster:
        //      dominant_skeleton → name_skeleton
        //      INSERT OR REPLACE INTO structure_clusters (id, label, regex, track_count)
        // 7. BEGIN TRANSACTION
        //      UPDATE tracks SET structure_cluster_id = ? for each track (NULL for noise)
        // 8. COMMIT
        // 9. Return "14 clusters, 35 noise points, 1780 tracks classified"
    }
}
```

---

## 4. Comparison: What Each Pass Gets From Batching

| | sax | structure_cluster |
|---|---|---|
| **Motivation** | I/O overhead per track | Algorithm requires all tracks |
| **Read pattern** | All pending in one SELECT | All classified in one SELECT |
| **Compute** | Independent per track (parallelisable) | Global (distance matrix + DBSCAN) |
| **Write pattern** | One transaction for all UPDATEs | One transaction for all UPDATEs |
| **Writes to** | `tracks.waveform_sax` | `tracks.structure_cluster_id`, `structure_clusters` |
| **`needs_run`** | Pending track_passes rows | Empty clusters table or unclassified tracks |

---

## 5. Pipeline Wiring (`analysis/mod.rs`)

```rust
// Phase 1b: SAX (was AnalysisPass, now BatchAnalysisPass)
run_batch_pass(&app, &conn_arc, sax::SaxPass, &run_id)?;

// Phase 1c: SAX alignment (stays AnalysisPass — fast and per-track is fine)
run_pass_pipeline(&app, &conn_arc, sax_alignment::SaxAlignmentPass, &run_id)?;

// Phase 1d: Structure clustering (new BatchAnalysisPass)
run_batch_pass(&app, &conn_arc, structure_cluster::StructureClusterPass, &run_id)?;
```

---

## 6. IPC Command: `get_structure_clusters`

```rust
#[derive(serde::Serialize)]
pub struct StructureClusterInfo {
    pub id:          i64,
    pub label:       String,
    pub regex:       String,
    pub track_count: i64,
}

#[tauri::command]
pub fn get_structure_clusters(
    conn: tauri::State<ConnState>,
) -> Result<Vec<StructureClusterInfo>, String> {
    // SELECT id, label, regex, track_count
    // FROM structure_clusters ORDER BY track_count DESC
}
```

---

## 7. Frontend Changes

### Remove hardcoded constants from `mapMath.ts`

`STRUCTURE_CLUSTER_LABELS` and `STRUCTURE_CLUSTER_REGEX` are deleted. `STRUCTURE_CLUSTER_COLORS` (tab20 palette) stays — it is independent of cluster semantics.

### New store: `src/lib/stores/structureClusters.svelte.ts`

```ts
interface StructureCluster { id: number; label: string; regex: string; track_count: number; }

function createStructureClustersStore() {
  let clusters = $state<StructureCluster[]>([]);
  let loaded = $state(false);

  async function load() {
    if (loaded) return;
    clusters = await invoke<StructureCluster[]>('get_structure_clusters');
    loaded = true;
  }

  const byId = $derived(Object.fromEntries(clusters.map(c => [c.id, c])));
  return { get clusters() { return clusters; }, byId, load };
}

export const structureClusters = createStructureClustersStore();
```

Load lazily when the Structure color toggle is first activated (or eagerly at boot — it's a cheap read).

### `MusicMap.svelte` and `TrackDetailPane.svelte`

Replace `STRUCTURE_CLUSTER_LABELS[id]` → `structureClusters.byId[id]?.label ?? 'Cluster ${id}'`  
Replace `STRUCTURE_CLUSTER_REGEX[id]` → `structureClusters.byId[id]?.regex`

---

## 8. File Inventory

| File | Change |
|------|--------|
| `src-tauri/migrations/30_structure_clusters.sql` | New — `structure_clusters` table |
| `src-tauri/src/analysis/mod.rs` | Add `BatchAnalysisPass` trait + `run_batch_pass()` + wiring |
| `src-tauri/src/analysis/sax.rs` | Convert from `AnalysisPass` to `BatchAnalysisPass` |
| `src-tauri/src/analysis/structure_cluster.rs` | New — skeleton, levenshtein, dbscan, naming, pass |
| `src-tauri/src/scanner/sidecar.rs` | Add `STRUCTURE_CLUSTER` version constant |
| `src-tauri/src/commands/structure.rs` | New — `get_structure_clusters` IPC command |
| `src-tauri/src/lib.rs` | Register new command |
| `src/lib/stores/structureClusters.svelte.ts` | New — reactive cluster store |
| `src/lib/utils/mapMath.ts` | Remove hardcoded label/regex constants |
| `src/lib/components/MusicMap.svelte` | Read label/regex from store |
| `src/lib/components/TrackDetailPane.svelte` | Read label/regex from store |

---

## 9. Implementation Order

1. **Migration** — `30_structure_clusters.sql`, register in `database.rs`
2. **Trait** — `BatchAnalysisPass` + `run_batch_pass()` in `mod.rs` (no implementation yet, just the skeleton)
3. **Sax conversion** — rewrite `sax.rs` as a `BatchAnalysisPass`; verify with `cargo test` and a manual run that waveform_sax is still populated correctly
4. **Structure cluster pure functions** — `skeleton()`, `levenshtein()`, `dbscan()`, `name_skeleton()` with full unit tests, no DB
5. **Structure cluster pass** — wire pure functions into `BatchAnalysisPass::execute()`; add to pipeline
6. **IPC** — `get_structure_clusters` command
7. **Frontend** — store, remove hardcoded constants, update components

### Unit test cases for the pure functions

- `skeleton`: empty, single char, all-same, alternating, realistic alignment (`IIVVPCCCCO`)
- `levenshtein`: empty strings, identical, single insert/delete/replace, known pairs
- `dbscan`: 3 clean clusters with no noise, noise-only input, all-same-cluster, known 2-cluster case
- `name_skeleton`: no repeating block (`IVPCO`), repeating block ×2 and ×3, single-char block (`IVVVCCO`), no suffix, no prefix, no prefix and no suffix

---

## 10. Decisions on Open Questions

**Cluster count stability.** ✓ Accepted. `eps` and `min_pts` stay as `const` values in the pass. After the first full Rust run, verify the cluster count sits in the 8–20 range and adjust if needed.

**Re-clustering on library growth.** Keep simple for now — re-cluster the full library whenever any track lacks a cluster ID. A smarter trigger (only re-cluster if new tracks don't fit neatly into existing clusters) is possible but premature until we know how fast the Rust implementation actually runs on the full library.

**Noise tracks and non-music outliers.** Run the structure cluster pass *after* the `essentia` pass (priority 50) rather than after `sax_alignment` (13). Change the `execute` query to:

```sql
SELECT id, sax_alignment FROM tracks
WHERE sax_alignment IS NOT NULL
  AND is_music = 1        -- essentia classifier result
```

This filters out non-music tracks before clustering, which should eliminate most structural outliers (speech, ambient, sound effects) that would otherwise inflate the noise bucket. Tracks with `is_music = 0` or `is_music IS NULL` keep `structure_cluster_id = NULL` and show as "Unclassified" in the map — correct and intentional.

Update the pass priority accordingly:

| Pass | Priority |
|------|----------|
| `sax_alignment` | 13 |
| `essentia` | 50 |
| `structure_cluster` | 55 — after essentia, before `bpm_refinement` (55 → bump bpm_refinement to 60) |

And update `dependencies`:
```rust
fn dependencies(&self) -> &'static [&'static str] { &["sax_alignment", "essentia"] }
```

**`sax_alignment` stays per-track.** The `sax_alignment` pass (Viterbi decoder, priority 13) is fast enough per-track that converting it to a batch pass is not necessary. It stays as an `AnalysisPass`.

---

## 11. Post-Implementation: Update `skills/add-analysis-pass/SKILL.md`

Once `BatchAnalysisPass` is implemented and both the sax and structure_cluster passes are working, the skill must be updated to reflect the new option. An agent reading the skill should immediately know which trait to reach for.

### What to add to the skill

**Update the introduction** to mention that two traits exist and when to choose each:

> The pipeline has two pass shapes:
> - **`AnalysisPass`** — one track at a time. Use for any pass where per-track compute dominates (inference, DSP, API calls). The orchestrator handles the job loop, progress events, pause/resume, and metrics automatically.
> - **`BatchAnalysisPass`** — all tracks at once. Use when either: (a) the algorithm requires global data (e.g. clustering, nearest-neighbour indexing), or (b) the pass is I/O-bound and per-track SQLite round-trips are the bottleneck. You own the read-compute-write loop; the orchestrator calls `execute()` once.

**Add a new section: "Adding a Batch Analysis Pass"** mirroring the existing step-by-step, covering:

1. Implement `BatchAnalysisPass` — `needs_run()` and `execute()`. The `execute()` body follows the pattern: one SELECT for all pending data → compute entirely in memory → one transaction for all writes → return summary string.
2. Add a `SPEC` constant (same `PassSpec` struct, `owned_columns: &[]` since batch passes write via explicit SQL, not null-on-reset).
3. Register in `PASS_REGISTRY` — same as `AnalysisPass`, gives automatic backfill, version invalidation, reset endpoints.
4. Call `run_batch_pass(&app, &conn_arc, YourPass, &run_id_spawn)` in the pipeline runner instead of `run_pass_pipeline`.

**Update the "Common mistakes" table** with batch-specific entries:

| Mistake | Symptom | Fix |
|---------|---------|-----|
| Using `AnalysisPass` for a clustering or embedding-index pass | Must process all tracks but trait forces one-at-a-time | Switch to `BatchAnalysisPass` |
| Using `AnalysisPass` for a fast per-track pass that reads large blobs | App freezes during analysis; thousands of SQLite round-trips | Convert to `BatchAnalysisPass`: one SELECT + one transaction |
| Doing per-track DB writes inside `BatchAnalysisPass::execute()` | Defeats the purpose; still causes lock contention | Collect all results in memory, write in a single `BEGIN`/`COMMIT` block |
| Emitting per-track progress events from a batch pass | UI receives thousands of events at once | Emit one event before and one after (or a few coarse progress checkpoints for very large libraries) |

**Update the pass priority table** to include the two new batch passes:

| Pass | Priority | Trait | File |
|------|----------|-------|------|
| `sax` | 12 | `BatchAnalysisPass` | `analysis/sax.rs` |
| `sax_alignment` | 13 | `AnalysisPass` | `analysis/sax_alignment.rs` |
| `structure_cluster` | 14 | `BatchAnalysisPass` | `analysis/structure_cluster.rs` |

**Update the "Pause / Resume support" section** to note that `BatchAnalysisPass` passes do not get automatic pause/resume (the compute loop is internal). For long-running batch passes, check the pause atomics at natural breakpoints (e.g. after computing the distance matrix, before writing back).

### What does NOT need to change in the skill

- The DB migration step — identical for both traits
- The `PassSpec` / `PASS_REGISTRY` step — identical
- The `pass_version` constant step — identical
- The metrics instrumentation note — batch passes log a single summary metric rather than per-track metrics; note this but the infrastructure is the same
