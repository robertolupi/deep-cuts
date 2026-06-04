# Design Proposal: Local-First Metrics & Telemetry Monitoring

To diagnose performance bottlenecks (e.g. analysis pass duration, llama-server latency) and debug issues with early adopters without violating privacy, Deep Cuts needs a robust, local-first monitoring system.

---

## 1. Objectives

1. **Performance Profiling**: Measure execution latency for all pipeline passes (Audio Analysis, Essentia, Qwen, CLAP) to isolate slow segments (audio decoding vs. inference).
2. **Diagnostics for Early Adopters**: Allow users to easily share debug telemetry (e.g., anonymized execution metrics) without sharing private music databases.
3. **No Database Bloat**: Keep metric records isolated from the main library database (`deep_cuts.db`) to ensure database size remains compact.

---

## 2. Storage Strategy: Dedicated Metrics Database

Instead of saving telemetry inside `deep_cuts.db`, we will write metrics to a separate database located in the application's logs directory:
* **Path**: `~/Library/Logs/com.rlupi.deep-cuts/telemetry.db` (on macOS)
* **Rationale**: If the main database gets corrupted, the metrics DB is isolated. Users can zip and share `telemetry.db` together with log files for troubleshooting.

### Proposed Schema (`telemetry.db`)
```sql
-- Tracks duration and success status of pipeline jobs
CREATE TABLE pipeline_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER NOT NULL,
    pass_name TEXT NOT NULL,          -- 'audio_analysis', 'qwen', 'essentia', etc.
    status TEXT NOT NULL,             -- 'success', 'failed'
    duration_ms INTEGER NOT NULL,      -- Total execution time
    audio_duration_sec REAL,          -- Length of track (to calculate processing-to-audio ratio)
    error_message TEXT,               -- Error details if status = 'failed'
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Stores internal system states (like llama-server load times and port crashes)
CREATE TABLE system_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,         -- 'llama_boot', 'llama_crash', 'db_locked'
    details TEXT,
    duration_ms INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Periodically captures resource usage statistics
CREATE TABLE resource_snapshots (
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    cpu_percent REAL,
    memory_bytes INTEGER,
    active_threads INTEGER
);
```

---

## 3. Local-First Prometheus Alternative

In cloud architectures, Prometheus pulls metrics via HTTP endpoints. For a desktop Tauri app, we will use a **Push/Collect** model inside the Rust runtime:

### In-Memory Metrics Registry
* Maintain a thread-safe static registry (e.g., using `lazy_static` or `once_cell` + `parking_lot::Mutex` in Rust).
* Passes record durations in memory:
  ```rust
  metrics::histogram!("analysis.pass.duration", duration, "pass" => job.name());
  ```

### Batch Writer Thread
* A background thread periodically flushes accumulated metrics from the in-memory registry to the local `telemetry.db` (e.g., every 10 seconds or at the end of an import run) using a single transaction to prevent file-system lock contention.

---

## 4. Diagnostics Export Flow

To assist developers in debugging issues, we will add a utility under "Library Settings" in the UI:
1. **Export Telemetry Button**: Zips the `telemetry.db` and the standard log files (`app.log`) into a single file: `deep_cuts_diagnostics_[date].zip`.
2. **Review Dialog**: Displays the exact JSON content of the metrics to the user first to guarantee that no private filenames or track metadata are leaked before export.
