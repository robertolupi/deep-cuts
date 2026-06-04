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

## 4. Diagnostics Export & Interactive Inspection Modal

To reassure privacy-minded users and provide transparency, the diagnostic flow includes an interactive **Telemetry Inspector Dialog** in the settings panel:

### 1. Transparency & Inspection
* Before exporting the diagnostics bundle, clicking **"Inspect Telemetry Database"** opens a modal displaying the exact raw data stored in `telemetry.db` in a scrollable, syntax-highlighted JSON viewer.
* **Privacy Assurance**: The viewer demonstrates that the database stores only numeric durations, status codes, and anonymized metrics — **no file paths, music metadata (titles, artist names), or private content keys** are leaked.

### 2. Predefined Metric Views
Instead of exposing a generic SQL query interface, the modal provides simple, tabbed preset views of the diagnostic data:
- **Average Latency**: Displays a clean table of average execution durations grouped by pass name.
- **Failures & Errors**: Shows a list of recent failed passes along with their specific error messages.
- **Raw Payload**: Shows the exact raw JSON object that will be packaged into the diagnostic bundle.

### 3. Verification & Export
Once the user reviews the exact metrics and confirms that the log files are clean, they can click **"Approve & Export Diagnostics Zip"** to download the bundle.

