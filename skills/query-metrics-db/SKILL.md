---
name: query-metrics-db
description: How to locate and query the deep-cuts telemetry/metrics SQLite database
---

# Querying the Metrics (Telemetry) Database

The app maintains a second SQLite database — separate from the main library DB — that records pipeline performance metrics and system lifecycle events.

---

## Database location

```
~/Library/Logs/com.rlupi.deep-cuts/telemetry.db
```

Store it in a shell variable to avoid retyping:

```bash
TDB="$HOME/Library/Logs/com.rlupi.deep-cuts/telemetry.db"
```

---

## Schema

### `pipeline_metrics`

One row per analysis pass execution (success or failure).

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Auto-increment PK |
| `run_id` | TEXT | Shared across all passes in a pipeline run (Unix ms timestamp as string) |
| `track_id` | INTEGER | Anonymised track ID (no filenames stored) |
| `pass_name` | TEXT | e.g. `audio_analysis`, `clap`, `essentia`, `qwen`, `description_embed` |
| `status` | TEXT | `success` or `failed` |
| `duration_ms` | INTEGER | Wall-clock time for just this pass on this track |
| `started_at` | INTEGER | Unix timestamp in milliseconds |
| `ended_at` | INTEGER | Unix timestamp in milliseconds |
| `audio_duration_sec` | REAL | Track length in seconds (NULL if unavailable) |
| `error_message` | TEXT | Error string on failure (NULL on success) |

### `system_events`

Pipeline lifecycle events.

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Auto-increment PK |
| `event_type` | TEXT | `pipeline_start` or `pipeline_end` |
| `details` | TEXT | e.g. `run_id=1780559866025` or `run_id=... (nothing to do)` |
| `duration_ms` | INTEGER | Total pipeline wall-clock time (only on `pipeline_end`) |
| `created_at` | TIMESTAMP | SQLite wall-clock (CURRENT_TIMESTAMP, UTC) |

---

## Useful queries

### Recent pipeline runs

```bash
sqlite3 "$TDB" ".headers on" ".mode column" \
  "SELECT * FROM system_events ORDER BY id DESC LIMIT 20;"
```

### Pass latency summary (all time)

```bash
sqlite3 "$TDB" ".headers on" ".mode column" "
SELECT pass_name, status,
       COUNT(*) as cnt,
       ROUND(AVG(duration_ms)/1000.0, 2) as avg_s,
       ROUND(MIN(duration_ms)/1000.0, 2) as min_s,
       ROUND(MAX(duration_ms)/1000.0, 2) as max_s
FROM pipeline_metrics
GROUP BY pass_name, status
ORDER BY pass_name, status;"
```

### Metrics for a specific run

```bash
RUN_ID="1780560383909"
sqlite3 "$TDB" ".headers on" ".mode column" "
SELECT pass_name, track_id, status, duration_ms, audio_duration_sec
FROM pipeline_metrics
WHERE run_id = '$RUN_ID'
ORDER BY started_at;"
```

### Recent failures with error messages

```bash
sqlite3 "$TDB" ".headers on" ".mode column" "
SELECT pass_name, track_id, error_message, datetime(started_at/1000, 'unixepoch') as started
FROM pipeline_metrics
WHERE status = 'failed'
ORDER BY id DESC LIMIT 20;"
```

### Real-time speed ratio (audio processed per second of wall clock)

```bash
sqlite3 "$TDB" ".headers on" ".mode column" "
SELECT pass_name,
       ROUND(AVG(audio_duration_sec / (duration_ms / 1000.0)), 1) as avg_realtime_ratio
FROM pipeline_metrics
WHERE status = 'success' AND audio_duration_sec IS NOT NULL AND duration_ms > 0
GROUP BY pass_name;"
```

---

## Safety notes

- **Do not write to the DB while the app is running.** External writes can corrupt in-flight WAL transactions.
- The metrics DB grows unboundedly — clear it via the **Privacy & Raw JSON** tab in the Telemetry Inspector (Library Settings → Inspect Telemetry & Traces → Privacy & Raw JSON → Clear Telemetry), or with:

```bash
sqlite3 "$TDB" "DELETE FROM pipeline_metrics; DELETE FROM system_events;"
```
