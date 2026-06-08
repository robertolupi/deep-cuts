import { invoke, listen } from "$lib/ipc";
import type { WatchedDirectory, Track } from "$lib/types";
import { player } from "./player.svelte";

/**
 * @concept LibraryStore
 * Client-side library store managing directory scans, tracks, tags, and IPC events.
 */
class LibraryStore {
  // Reactive Svelte 5 state runes
  directories = $state<WatchedDirectory[]>([]);
  tracks = $state<Track[]>([]);
  trackCount = $state(0);
  trackTagMap = $state<Map<number, string[]>>(new Map());
  allTags = $state<string[]>([]);
  staleCount = $derived(this.tracks.filter(t => t.is_stale === 1).length);

  isScanning = $state(false);
  scanProgress = $state(0);
  scanCurrentFile = $state("Idle");
  scanProcessedCount = $state(0);
  scanTotalCount = $state(0);

  tauriConnected = $state(false);
  analysisRunning = $state(false);
  analysisManuallyPaused = $state(false);
  analysisAutoPaused = $state(false);
  analysisPaused = $derived(this.analysisManuallyPaused || this.analysisAutoPaused);

  private initialized = false;
  private unlisteners: Array<() => void> = [];

  // Initialize and load initial database states
  async init() {
    if (this.initialized) return;
    this.initialized = true;

    try {
      await this.fetchDirectories();
      await this.fetchTrackCount();
      await this.fetchTracks();
      this.tauriConnected = true;

      invoke("is_analysis_running").then(v => { this.analysisRunning = v; }).catch(() => {});
      invoke("get_analysis_paused_status")
        .then(v => {
          this.analysisManuallyPaused = v.manually_paused;
          this.analysisAutoPaused = v.auto_paused;
        }).catch(() => {});

      // Sync paused changes from backend
      this.unlisteners.push(await listen<{ manually_paused: boolean; auto_paused: boolean }>("analysis-paused-changed", (event) => {
        this.analysisManuallyPaused = event.payload.manually_paused;
        this.analysisAutoPaused = event.payload.auto_paused;
      }));

      // Reload tracks after each analysis phase completes so extracted data is visible
      this.unlisteners.push(await listen<any>("analysis-phase-complete", () => {
        this.fetchTracks();
      }));

      let tagsRefreshTimer: ReturnType<typeof setTimeout> | null = null;
      this.unlisteners.push(await listen<any>("analysis-progress", () => {
        this.analysisRunning = true;
        // Debounce tag refresh so the filter stays current without flooding the DB.
        if (tagsRefreshTimer) clearTimeout(tagsRefreshTimer);
        tagsRefreshTimer = setTimeout(() => {
          this.fetchTags();
          tagsRefreshTimer = null;
        }, 2000);
      }));

      this.unlisteners.push(await listen<any>("analysis-complete", () => {
        this.analysisRunning = false;
        this.analysisManuallyPaused = false;
        this.analysisAutoPaused = false;
        this.fetchTracks();
      }));

      this.unlisteners.push(await listen<any>("analysis-error", () => {
        this.analysisRunning = false;
        this.analysisManuallyPaused = false;
        this.analysisAutoPaused = false;
      }));

      // Listen for AcoustID dynamic enrichment events to refresh the library and details view
      this.unlisteners.push(await listen<number>("track-enriched", async (event) => {
        const enrichedId = event.payload;
        try {
          const freshTrack = await invoke("get_track", { trackId: enrichedId });
          if (freshTrack) {
            const idx = this.tracks.findIndex(t => t.id === enrichedId);
            if (idx !== -1) {
              this.tracks[idx] = freshTrack;
            }
            // TODO: known cross-store coupling — player.selectedTrack is mutated here to keep the
            // details view in sync after enrichment. Decouple by having the player store react to
            // library track changes rather than being written to directly.
            if (player.selectedTrack && player.selectedTrack.id === enrichedId) {
              player.selectedTrack = freshTrack;
            }
          }
        } catch (e) {
          console.error("Failed to fetch enriched track:", e);
        }
      }));

      // Listen for progress updates emitted by the background scanner
      this.unlisteners.push(await listen<any>("scan:progress", (event) => {
        const payload = event.payload;
        this.isScanning = payload.is_scanning;
        this.scanProgress = payload.progress;
        this.scanCurrentFile = payload.current_file;
        this.scanProcessedCount = payload.processed_count;
        this.scanTotalCount = payload.total_count;

        // Automatically reload caches on successful scan completion
        if (!payload.is_scanning && payload.progress === 100) {
          this.fetchTrackCount();
          this.fetchTracks();
        }
      }));
    } catch (e) {
      this.initialized = false;
      console.warn("Tauri context offline or library database loading.");
    }
  }

  dispose() {
    for (const unlisten of this.unlisteners) {
      unlisten();
    }
    this.unlisteners = [];
    this.initialized = false;
  }

  async fetchDirectories() {
    this.directories = await invoke("get_watched_directories");
  }

  async fetchTrackCount() {
    this.trackCount = await invoke("get_track_count");
  }

  async fetchTracks() {
    this.tracks = await invoke("get_tracks");
    this.fetchTags();
  }

  async fetchTags() {
    const [rawMap, allTags] = await Promise.all([
      invoke("get_all_track_tags"),
      invoke("get_all_tags"),
    ]);
    this.trackTagMap = new Map(Object.entries(rawMap).map(([k, v]) => [Number(k), v]));
    this.allTags = allTags;
  }

  async addDirectory(name: string, path: string) {
    await invoke("add_watched_directory", { name, path });
    await this.fetchDirectories();
  }

  async removeDirectory(id: number) {
    await invoke("remove_watched_directory", { id });
    await this.fetchDirectories();
    await this.fetchTrackCount();
    await this.fetchTracks();
  }

  async triggerScan() {
    if (this.isScanning) return;
    this.isScanning = true;
    this.scanProgress = 0;
    this.scanCurrentFile = "Starting library scan...";
    await invoke("scan_all_libraries");
  }

  async exportSidecars(): Promise<number> {
    return await invoke("export_sidecars");
  }
}

export const library = new LibraryStore();
