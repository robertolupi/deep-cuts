import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { WatchedDirectory, Track } from "$lib/types";
import { player } from "./player.svelte";

class LibraryStore {
  // Reactive Svelte 5 state runes
  directories = $state<WatchedDirectory[]>([]);
  tracks = $state<Track[]>([]);
  trackCount = $state(0);
  staleCount = $derived(this.tracks.filter(t => t.is_stale === 1).length);
  
  isScanning = $state(false);
  scanProgress = $state(0);
  scanCurrentFile = $state("Idle");
  scanProcessedCount = $state(0);
  scanTotalCount = $state(0);
  
  tauriConnected = $state(false);

  // Initialize and load initial database states
  async init() {
    try {
      await this.fetchDirectories();
      await this.fetchTrackCount();
      await this.fetchTracks();
      this.tauriConnected = true;

      // Reload tracks after each analysis phase completes so extracted data is visible
      await listen<any>("analysis-phase-complete", () => {
        this.fetchTracks();
      });

      await listen<any>("analysis-complete", () => {
        this.fetchTracks();
      });

      // Listen for AcoustID dynamic enrichment events to refresh the library and details view
      await listen<any>("track-enriched", async (event) => {
        const enrichedId = event.payload;
        try {
          const freshTrack = await invoke<Track | null>("get_track", { trackId: enrichedId });
          if (freshTrack) {
            const idx = this.tracks.findIndex(t => t.id === enrichedId);
            if (idx !== -1) {
              this.tracks[idx] = freshTrack;
            }
            if (player.selectedTrack && player.selectedTrack.id === enrichedId) {
              player.selectedTrack = freshTrack;
            }
          }
        } catch (e) {
          console.error("Failed to fetch enriched track:", e);
        }
      });

      // Listen for progress updates emitted by the background scanner
      await listen<any>("scan:progress", (event) => {
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
      });
    } catch (e) {
      console.warn("Tauri context offline or library database loading.");
    }
  }

  async fetchDirectories() {
    this.directories = await invoke<WatchedDirectory[]>("get_watched_directories");
  }

  async fetchTrackCount() {
    this.trackCount = await invoke<number>("get_track_count");
  }

  async fetchTracks() {
    this.tracks = await invoke<Track[]>("get_tracks");
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
    return await invoke<number>("export_sidecars");
  }
}

export const library = new LibraryStore();
