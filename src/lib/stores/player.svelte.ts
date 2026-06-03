/**
 * Player store — owns all WaveSurfer / playback state.
 *
 * DOM containers are registered by PlayerBar.svelte via register() using
 * a $effect, so WaveSurfer can find them once the DOM nodes exist.
 */
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { tick } from "svelte";
import WaveSurfer from "wavesurfer.js";
import Spectrogram from "wavesurfer.js/dist/plugins/spectrogram.esm.js";
import type { Track } from "$lib/types";
import { formatDuration, formatSize } from "$lib/utils/format";
import { theme } from "./theme.svelte";
import { filters } from "./filters.svelte";

// Re-export so consumers don't need a separate import
export { formatDuration, formatSize };

class PlayerStore {
  // ── Reactive state ──────────────────────────────────────────────────────────
  selectedTrack = $state<Track | null>(null);
  isPlaying     = $state(false);
  currentTime   = $state(0);
  duration      = $state(0);

  // Internal — not exposed to templates
  #wavesurfer          = $state<WaveSurfer | null>(null);
  #waveformContainer   = $state<HTMLDivElement | null>(null);
  #spectrogramContainer = $state<HTMLDivElement | null>(null);

  // ── Container registration (called by PlayerBar.svelte via $effect) ─────────
  register(waveform: HTMLDivElement, spectrogram: HTMLDivElement | null) {
    this.#waveformContainer   = waveform;
    this.#spectrogramContainer = spectrogram;
  }

  unregister() {
    this.#waveformContainer   = null;
    this.#spectrogramContainer = null;
  }

  // ── Playback ─────────────────────────────────────────────────────────────────

  async playTrack(track: Track) {
    const resolvedTheme = theme.resolvedTheme;
    const filteredTracks = filters.filteredTracks;
    this.selectedTrack = track;
    this.isPlaying     = false;
    this.currentTime   = 0;
    this.duration      = 0;

    // Trigger lazy AcoustID / MusicBrainz metadata enrichment (fire and forget)
    invoke("enrich_track_metadata", { trackId: track.id, force: false }).catch((e) => {
      console.error("[acoustid] Failed to trigger lazy enrichment:", e);
    });

    // Tear down previous instance
    if (this.#wavesurfer) {
      this.#wavesurfer.destroy();
      this.#wavesurfer = null;
    }

    const assetUrl = convertFileSrc(track.path);

    // Wait for Svelte DOM tick so PlayerBar's $effect can register containers
    await tick();

    if (!this.#waveformContainer) {
      console.error("[PlayerStore] waveformContainer not registered — PlayerBar must call player.register() via $effect.");
      return;
    }

    // Build WaveSurfer
    this.#wavesurfer = WaveSurfer.create({
      container:   this.#waveformContainer,
      waveColor:   resolvedTheme === "light" ? "rgba(28, 25, 23, 0.35)" : "rgba(255, 255, 255, 0.08)",
      cursorColor: resolvedTheme === "light" ? "var(--sg-primary)"      : "var(--sg-primary)",
      cursorWidth: 2,
      barWidth:    3,
      barGap:      2.2,
      barRadius:   2,
      height:      75,
      normalize:   true,
      plugins: this.#spectrogramContainer ? [
        Spectrogram.create({
          container:   this.#spectrogramContainer,
          labels:      true,
          fftSamples:  512,
          height:      75,
          labelsColor: resolvedTheme === "light" ? "#57534e" : "var(--sg-primary)",
        }),
      ] : [],
    });

    // Theme-aware progress gradient
    const ctx = document.createElement("canvas").getContext("2d");
    if (ctx) {
      const gradient = ctx.createLinearGradient(0, 0, 800, 0);
      if (resolvedTheme === "accessible") {
        this.#wavesurfer.setOptions({ progressColor: "var(--sg-primary)" });
      } else if (resolvedTheme === "light") {
        gradient.addColorStop(0,   "#0d7377");
        gradient.addColorStop(0.5, "#7c2d6b");
        gradient.addColorStop(1,   "#0a5f63");
        this.#wavesurfer.setOptions({ progressColor: gradient });
      } else {
        gradient.addColorStop(0,   "#00f0ff"); // Cyber Cyan
        gradient.addColorStop(0.5, "#fe00fe"); // Studio Pink
        gradient.addColorStop(1,   "#00dbe9");
        this.#wavesurfer.setOptions({ progressColor: gradient });
      }
    }

    this.#wavesurfer.load(assetUrl);

    // Event hooks
    this.#wavesurfer.on("play",  () => { this.isPlaying = true; });
    this.#wavesurfer.on("pause", () => { this.isPlaying = false; });
    this.#wavesurfer.on("timeupdate", (time) => { this.currentTime = time; });
    this.#wavesurfer.on("ready", () => {
      if (this.#wavesurfer) {
        this.duration = this.#wavesurfer.getDuration();
        this.#wavesurfer.play();
      }
    });
    this.#wavesurfer.on("finish", () => {
      this.isPlaying   = false;
      this.currentTime = 0;
      this.#advance(+1);
    });
  }

  togglePlayback() {
    this.#wavesurfer?.playPause();
  }

  resetPlayer() {
    if (this.#wavesurfer) {
      this.#wavesurfer.destroy();
      this.#wavesurfer = null;
    }
    this.selectedTrack = null;
    this.isPlaying     = false;
    this.currentTime   = 0;
    this.duration      = 0;
  }

  handlePrevTrack() {
    const ft = filters.filteredTracks;
    if (!this.selectedTrack || ft.length === 0) return;
    const idx = ft.findIndex(t => t.id === this.selectedTrack!.id);
    const prev = idx > 0 ? ft[idx - 1] : ft[ft.length - 1];
    this.playTrack(prev);
  }

  handleNextTrack() {
    this.#advance(+1);
  }

  // ── Private ──────────────────────────────────────────────────────────────────
  #advance(dir: 1 | -1) {
    const ft = filters.filteredTracks;
    if (!this.selectedTrack || ft.length === 0) return;
    const idx  = ft.findIndex(t => t.id === this.selectedTrack!.id);
    const next = idx !== -1 && idx + dir < ft.length && idx + dir >= 0
      ? ft[idx + dir]
      : ft[dir === 1 ? 0 : ft.length - 1];
    this.playTrack(next);
  }

  // ── Peaks export (for peaks-only WaveSurfer instances, e.g. ChatPanel) ───────
  exportPeaks(): number[][] | null {
    return this.#wavesurfer?.exportPeaks() ?? null;
  }

  // ── Reveal in Finder ─────────────────────────────────────────────────────────
  async revealInFinder(path: string) {
    try {
      await invoke("reveal_in_finder", { path });
    } catch (e) {
      console.error("[PlayerStore] reveal_in_finder failed:", e);
    }
  }
}

export const player = new PlayerStore();
