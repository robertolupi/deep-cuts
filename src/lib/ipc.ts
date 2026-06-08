import { invoke as tauriInvoke, convertFileSrc as tauriConvertFileSrc } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getVersion as tauriGetVersion } from "@tauri-apps/api/app";
import { openUrl as tauriOpenUrl } from "@tauri-apps/plugin-opener";
import {
  MOCK_DIRECTORIES,
  MOCK_TRACKS,
  MOCK_TAGS,
  MOCK_ALL_TAGS,
  MOCK_PLAYLISTS,
  MOCK_SAVED_SEARCHES,
} from "$lib/mock-data";
import type {
  Track,
  WatchedDirectory,
  Playlist,
  PlaylistTrack,
  SavedSearch,
} from "$lib/types";

export type { UnlistenFn } from "@tauri-apps/api/event";

// ── Domain types returned by commands but not in types.ts ─────────────────────

export interface SemanticSearchResult {
  id: number;
  title: string | null;
  filename: string;
  artist: string | null;
  genre: string | null;
  bpm: number | null;
  key: string | null;
  scale: string | null;
  score: number;
}

export interface PassError {
  path: string;
  log: string | null;
  duration_ms: number | null;
  last_run_at: string | null;
}

export interface PassStats {
  pass_name: string;
  pending: number;
  in_progress: number;
  done: number;
  failed: number;
  skipped: number;
  total: number;
  avg_duration_ms: number | null;
  concurrency: number;
  errors: PassError[];
}

export interface ModelExistence {
  qwen_exists: boolean;
  sentence_exists: boolean;
  clap_exists: boolean;
  essentia_exists: boolean;
  sax_exists: boolean;
  all_exist: boolean;
  missing_files: string[];
  [key: string]: boolean | string[];
}

export interface ChatSession {
  id: number;
  track_id: number;
  title: string;
  window_start_secs: number | null;
  window_duration_secs: number | null;
  created_at: number;
  updated_at: number;
}

export interface ChatMessage {
  id: number;
  session_id: number;
  role: string;
  content: string;
  created_at: number;
}

export interface ChatSearchResult {
  session_id: number;
  track_id: number;
  track_title: string;
  session_title: string;
  excerpt: string;
}

export interface ResumableFile {
  filename: string;
  offset: number;
}

export interface ModelFile {
  filename: string;
  url: string;
  sha256: string;
  size_bytes: number;
}

export interface ModelGroup {
  label: string;
  files: ModelFile[];
}

export interface ModelManifest {
  manifest_version: number;
  min_app_version: string;
  update_notice: string | null;
  models: Record<string, ModelGroup>;
}

export interface AppManifestResponse {
  manifest: ModelManifest;
  update_available: boolean;
}

export interface DownloadProgressEvent {
  model: string;
  file: string;
  bytes_done: number;
  bytes_total: number;
}

export interface M3UTrackInfo {
  path: string;
  title: string | null;
  artist: string | null;
  duration_seconds: number | null;
}

export interface MappedTrackPoint {
  id: number;
  x: number;
  y: number;
  watched_directory_id: number;
  title: string | null;
  filename: string;
  artist: string | null;
  genre: string | null;
  bpm: number | null;
  key: string | null;
  scale: string | null;
  algorithm?: string | null;
  mood_happy?: number | null;
  mood_sad?: number | null;
  mood_aggressive?: number | null;
  mood_relaxed?: number | null;
  mood_party?: number | null;
  mood_acoustic?: number | null;
  mood_electronic?: number | null;
  structure_cluster_id?: number | null;
}

export interface AudioSimilarityResult {
  id: number;
  title: string | null;
  filename: string;
  artist: string | null;
  genre: string | null;
  bpm: number | null;
  key: string | null;
  scale: string | null;
  score: number;
}

export interface DuplicatePair {
  id_a: number;
  id_b: number;
  title_a: string | null;
  title_b: string | null;
  artist_a: string | null;
  artist_b: string | null;
  filename_a: string;
  filename_b: string;
  path_a: string;
  path_b: string;
  distance: number;
}

export interface LabelCount {
  label: string;
  count: number;
}

export interface TrackSetStats {
  track_count: number;
  total_duration_seconds: number;
  avg_bpm: number | null;
  bpm_stddev: number | null;
  most_common_key: string | null;
  key_variety: number;
  pct_vocals: number;
  pct_analysed: number;
  avg_loudness_lufs: number | null;
  avg_mood_happy: number | null;
  avg_mood_sad: number | null;
  avg_mood_aggressive: number | null;
  avg_mood_relaxed: number | null;
  avg_mood_party: number | null;
  avg_mood_acoustic: number | null;
  avg_mood_electronic: number | null;
  major_count: number;
  minor_count: number;
  vocal_count: number;
  instrumental_count: number;
  unknown_vocal_count: number;
  coverage_essentia: number;
  coverage_mood: number;
  coverage_qwen: number;
  coverage_qwen_description: number;
  coverage_qwen_instruments: number;
  coverage_qwen_mood: number;
  coverage_qwen_genre: number;
  coverage_clap: number;
  coverage_umap: number;
  coverage_acoustid: number;
  bpm_values: number[];
  duration_values: number[];
  loudness_values: number[];
  key_distribution: LabelCount[];
  genre_distribution: LabelCount[];
  instrument_distribution: LabelCount[];
}

export interface LatencyStat {
  pass_name: string;
  avg_duration_ms: number;
  min_duration_ms: number;
  max_duration_ms: number;
  count: number;
}

export interface PipelineMetricRow {
  id: number;
  run_id: string;
  track_id: number;
  pass_name: string;
  status: string;
  duration_ms: number;
  started_at: number;
  ended_at: number;
  audio_duration_sec: number | null;
  error_message: string | null;
}

export interface MetricsSummary {
  latencies: LatencyStat[];
  recent_failures: PipelineMetricRow[];
}

export interface AggregatedPassSpan {
  run_id: string;
  pass_name: string;
  started_at: number;
  ended_at: number;
  total: number;
  succeeded: number;
  failed: number;
}

export interface StructureClusterInfo {
  id: number;
  label: string;
  regex: string;
  track_count: number;
}

export interface DebugTrackRawResult {
  track: any;
  passes: any[];
  coords: any;
  tags: any[];
  suppressions: string[];
  chat_sessions: any[];
}

// ── CommandMap ────────────────────────────────────────────────────────────────
// Maps every IPC command name to its (args, result) types.
// Add an entry here for every new command added to generate_handler![] in lib.rs.

/**
 * @concept CommandMap
 * @concept IPC
 * @skill add-ipc-command
 * Type definition mapping IPC command names to their argument and return value signatures.
 */
export type CommandMap = {
  // config
  get_theme: { args: Record<string, never>; result: string };
  save_theme: { args: { theme: string }; result: void };
  get_model_path_setting: { args: Record<string, never>; result: string | null };
  save_model_path_setting: { args: { path: string | null }; result: void };
  get_acoustid_setting: { args: Record<string, never>; result: string };
  save_acoustid_setting: { args: { value: string }; result: void };
  get_sidecar_setting: { args: Record<string, never>; result: boolean };
  save_sidecar_setting: { args: { enabled: boolean }; result: void };

  // library — directories
  select_directory: { args: Record<string, never>; result: string | null };
  get_watched_directories: { args: Record<string, never>; result: WatchedDirectory[] };
  add_watched_directory: { args: { name: string; path: string }; result: void };
  remove_watched_directory: { args: { id: number }; result: void };

  // library — tracks
  get_track_count: { args: Record<string, never>; result: number };
  get_tracks: { args: Record<string, never>; result: Track[] };
  get_track: { args: { trackId: number }; result: Track | null };

  // library — tags
  get_tags_for_tracks: {
    args: { trackIds: number[] };
    result: Record<number, string[]>;
  };
  get_tags_with_meta_for_tracks: {
    args: { trackIds: number[] };
    result: Record<
      number,
      Array<{ name: string; source: string; score: number | null; discard: boolean }>
    >;
  };
  get_all_track_tags: { args: Record<string, never>; result: Record<number, string[]> };
  get_all_tags: { args: Record<string, never>; result: string[] };
  add_user_tag: { args: { trackPath: string; tagName: string }; result: void };
  remove_user_tag: { args: { trackPath: string; tagName: string }; result: void };
  suppress_tag: { args: { trackPath: string; tagName: string }; result: void };
  unsuppress_tag: { args: { trackPath: string; tagName: string }; result: void };
  get_suppressed_tags: { args: { trackPath: string }; result: string[] };

  // library — file system helpers
  reveal_in_finder: { args: { path: string }; result: void };
  open_log_dir: { args: Record<string, never>; result: void };
  open_data_dir: { args: Record<string, never>; result: void };

  // library — sidecars
  get_cover_art: { args: { path: string }; result: string | null };
  save_sidecar: { args: { trackId: number }; result: void };
  export_sidecars: { args: Record<string, never>; result: number };

  // library — semantic / hybrid search
  search_semantic_tracks: {
    args: { query: string; limit?: number };
    result: SemanticSearchResult[];
  };
  search_clap_tracks: {
    args: { query: string; limit?: number };
    result: SemanticSearchResult[];
  };
  search_hybrid_vibe: {
    args: { query: string; clapWeight: number; limit?: number };
    result: SemanticSearchResult[];
  };

  // scanner
  scan_all_libraries: { args: Record<string, never>; result: string };

  // analysis
  run_analysis_pipeline: { args: Record<string, never>; result: void };
  is_analysis_running: { args: Record<string, never>; result: boolean };
  get_pass_stats: { args: Record<string, never>; result: PassStats[] };
  recover_stuck_passes: { args: Record<string, never>; result: number };
  reset_pass: { args: { passName: string }; result: void };
  reset_pass_for_track: { args: { passName: string; trackId: number }; result: void };
  reset_all_passes: { args: Record<string, never>; result: void };
  check_models_exist: { args: Record<string, never>; result: ModelExistence };
  set_analysis_manually_paused: { args: { paused: boolean }; result: void };
  set_analysis_auto_paused: { args: { paused: boolean }; result: void };
  get_analysis_paused_status: {
    args: Record<string, never>;
    result: { manually_paused: boolean; auto_paused: boolean };
  };

  // map / projection
  get_projection_coordinates: { args: { musicOnly: boolean }; result: MappedTrackPoint[] };
  search_similar_tracks_audio: {
    args: { trackId: number; directoryId?: number | null; clapWeight?: number | null };
    result: AudioSimilarityResult[];
  };
  recompute_projection: {
    args: {
      musicOnly: boolean;
      clapWeight: number | null;
      algorithm: string;
      nNeighbors: number;
      minDist: number;
      perplexity: number;
      projectionMode: string | null;
    };
    result: number;
  };
  find_duplicate_pairs: { args: { threshold: number }; result: DuplicatePair[] };

  // manifest / updates
  fetch_app_manifest: { args: Record<string, never>; result: AppManifestResponse };
  get_update_settings: { args: Record<string, never>; result: boolean };
  set_update_settings: { args: { enabled: boolean }; result: void };

  // downloads
  check_pending_resume: { args: Record<string, never>; result: ResumableFile[] };
  cancel_model_download: { args: Record<string, never>; result: void };
  download_models: {
    args: {
      models: string[];
      customUrlBase?: string | null;
      customManifest?: string | null;
    };
    result: void;
  };
  get_download_status: { args: Record<string, never>; result: DownloadProgressEvent | null };

  // chat / Qwen
  ask_qwen: {
    args: {
      trackId: number;
      question: string;
      windowStartSecs?: number | null;
      windowDurationSecs?: number | null;
      history: [string, string][];
    };
    result: string;
  };
  create_chat_session: {
    args: { trackId: number; windowStartSecs?: number | null; windowDurationSecs?: number | null };
    result: ChatSession;
  };
  list_chat_sessions: { args: { trackId: number }; result: ChatSession[] };
  rename_chat_session: { args: { sessionId: number; title: string }; result: void };
  delete_chat_session: { args: { sessionId: number }; result: void };
  get_chat_messages: { args: { sessionId: number }; result: ChatMessage[] };
  save_chat_message: {
    args: { sessionId: number; role: string; content: string };
    result: ChatMessage;
  };
  search_chats: { args: { query: string }; result: ChatSearchResult[] };

  // playlists
  get_playlists: { args: Record<string, never>; result: Playlist[] };
  create_playlist: { args: { name: string }; result: number };
  delete_playlist: { args: { id: number }; result: void };
  rename_playlist: { args: { id: number; newName: string }; result: void };
  get_playlist_tracks: { args: { playlistId: number }; result: PlaylistTrack[] };
  add_tracks_to_playlist: { args: { playlistId: number; trackIds: number[] }; result: void };
  remove_track_from_playlist: { args: { playlistId: number; position: number }; result: void };
  reorder_playlist_track: {
    args: { playlistId: number; fromPos: number; toPos: number };
    result: void;
  };
  get_playlists_for_track: { args: { trackId: number }; result: Playlist[] };
  remove_track_from_playlist_by_id: {
    args: { playlistId: number; trackId: number };
    result: void;
  };
  export_m3u_playlist: { args: { tracks: M3UTrackInfo[] }; result: boolean };

  // saved searches
  get_saved_searches: { args: Record<string, never>; result: SavedSearch[] };
  create_saved_search: { args: { name: string; queryJson: string }; result: number };
  delete_saved_search: { args: { id: number }; result: void };
  update_saved_search: { args: { id: number; queryJson: string }; result: void };

  // statistics / metrics / structure
  get_track_stats: { args: { trackIds: number[] | null }; result: TrackSetStats };
  get_metrics_summary: { args: Record<string, never>; result: MetricsSummary };
  get_pipeline_run_traces: { args: Record<string, never>; result: AggregatedPassSpan[] };
  get_structure_clusters: { args: Record<string, never>; result: StructureClusterInfo[] };

  // debug-only commands (compiled with #[cfg(debug_assertions)] in Rust)
  enrich_track_metadata: { args: { trackId: number; force?: boolean }; result: void };
  enrich_all_pending_acoustid: { args: Record<string, never>; result: number };
  debug_track_raw: { args: { trackId: number }; result: DebugTrackRawResult };
};

export const LOCAL_DEBUG = typeof window !== "undefined" &&
  new URLSearchParams(window.location.search).has("local_debug");

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const MOCK_RESPONSES: Record<string, (args?: any) => unknown> = {
  get_watched_directories: () => MOCK_DIRECTORIES,
  get_track_count: () => MOCK_TRACKS.length,
  get_tracks: () => MOCK_TRACKS,
  get_track: ({ trackId }: { trackId: number }) =>
    MOCK_TRACKS.find((t) => t.id === trackId) ?? null,
  get_all_track_tags: () => MOCK_TAGS,
  get_all_tags: () => MOCK_ALL_TAGS,
  get_playlists: () => MOCK_PLAYLISTS,
  get_saved_searches: () => MOCK_SAVED_SEARCHES,
  get_playlist_tracks: () => [],
  get_playlists_for_track: () => [],
  is_analysis_running: () => false,
  get_analysis_paused_status: () => ({ manually_paused: false, auto_paused: false }),
  get_theme: () => "system",
};

// Overload 1: known command — args and result are fully typed via CommandMap.
// Overload 2: unknown/legacy string — caller supplies explicit T (backward-compatible escape hatch).
export function invoke<K extends keyof CommandMap>(
  cmd: K,
  args?: CommandMap[K]["args"]
): Promise<CommandMap[K]["result"]>;
export function invoke<T = unknown>(
  cmd: string,
  args?: Record<string, unknown>
): Promise<T>;
export function invoke(
  cmd: string,
  args?: Record<string, unknown>
): Promise<unknown> {
  if (LOCAL_DEBUG) {
    const handler = MOCK_RESPONSES[cmd];
    if (handler) return Promise.resolve(handler(args));
    // Unknown commands silently resolve to undefined in mock mode
    console.warn(`[local_debug] unhandled invoke: ${cmd}`);
    return Promise.resolve(undefined);
  }
  return tauriInvoke(cmd, args);
}

export function listen<T>(
  event: string,
  handler: (event: { payload: T }) => void
): Promise<UnlistenFn> {
  if (LOCAL_DEBUG) {
    // No real events in mock mode — return a no-op unlisten
    return Promise.resolve(() => {});
  }
  return tauriListen<T>(event, handler);
}

export function convertFileSrc(filePath: string, protocol?: string): string {
  if (LOCAL_DEBUG) return filePath;
  return tauriConvertFileSrc(filePath, protocol);
}

export async function getVersion(): Promise<string> {
  if (LOCAL_DEBUG) return "0.0.0-dev";
  return tauriGetVersion();
}

export async function openUrl(url: string): Promise<void> {
  if (LOCAL_DEBUG) return;
  return tauriOpenUrl(url);
}
