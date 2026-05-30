/**
 * Player store — owns all WaveSurfer / playback state.
 * Extracted from +page.svelte as part of Phase 1.1 store refactor.
 *
 * DOM containers (waveformContainer, spectrogramContainer) are registered
 * by AudioPlayer.svelte via register() on mount, so WaveSurfer can find
 * them regardless of which component renders the containers.
 *
 * NOTE: playTrack() still receives resolvedTheme as an argument until the
 * theme store (Phase 1.3) exists and can be imported directly.
 */
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { tick } from "svelte";
import WaveSurfer from "wavesurfer.js";
import Spectrogram from "wavesurfer.js/dist/plugins/spectrogram.esm.js";
import type { Track } from "$lib/types";
import { formatDuration, formatSize } from "$lib/utils/format";

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

  // ── Container registration (called by AudioPlayer.svelte on mount) ──────────
  register(waveform: HTMLDivElement, spectrogram: HTMLDivElement) {
    this.#waveformContainer   = waveform;
    this.#spectrogramContainer = spectrogram;
  }

  unregister() {
    this.#waveformContainer   = null;
    this.#spectrogramContainer = null;
  }

  // ── Playback ─────────────────────────────────────────────────────────────────

  /**
   * Load and play a track. Destroys any existing WaveSurfer instance first.
   * @param track   Track to play.
   * @param resolvedTheme  Current resolved theme ('dark' | 'light' | 'accessible').
   *                       Temporary until theme store (Phase 1.3) is available.
   */
  async playTrack(track: Track, resolvedTheme: string, filteredTracks: Track[]) {
    this.selectedTrack = track;
    this.isPlaying     = false;
    this.currentTime   = 0;
    this.duration      = 0;

    // Tear down previous instance
    if (this.#wavesurfer) {
      this.#wavesurfer.destroy();
      this.#wavesurfer = null;
    }

    const assetUrl = convertFileSrc(track.path);

    // Wait for Svelte DOM tick so AudioPlayer can bind its containers
    await tick();

    if (!this.#waveformContainer) {
      console.error("[PlayerStore] waveformContainer not registered — call player.register() in AudioPlayer onMount.");
      return;
    }

    // Build WaveSurfer
    this.#wavesurfer = WaveSurfer.create({
      container:   this.#waveformContainer,
      waveColor:   resolvedTheme === "light" ? "rgba(28, 25, 23, 0.10)" : "rgba(255, 255, 255, 0.08)",
      cursorColor: resolvedTheme === "light" ? "var(--sg-primary)"      : "var(--sg-primary)",
      cursorWidth: 2,
      barWidth:    3,
      barGap:      2.2,
      barRadius:   2,
      height:      75,
      normalize:   true,
      plugins: [
        Spectrogram.create({
          container:   this.#spectrogramContainer!,
          labels:      true,
          fftSamples:  512,
          height:      75,
          labelsColor: resolvedTheme === "light" ? "#57534e" : "var(--sg-primary)",
        }),
      ],
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
      this.#advance(filteredTracks, +1);
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

  /**
   * Prev/next track navigation.
   * filteredTracks comes from the caller (Phase 1.2 will remove this arg
   * once the filter store can be imported directly).
   */
  handlePrevTrack(filteredTracks: Track[], resolvedTheme: string) {
    if (!this.selectedTrack || filteredTracks.length === 0) return;
    const idx = filteredTracks.findIndex(t => t.id === this.selectedTrack!.id);
    const prev = idx > 0 ? filteredTracks[idx - 1] : filteredTracks[filteredTracks.length - 1];
    this.playTrack(prev, resolvedTheme, filteredTracks);
  }

  handleNextTrack(filteredTracks: Track[], resolvedTheme: string) {
    this.#advance(filteredTracks, +1, resolvedTheme);
  }

  // ── Private ──────────────────────────────────────────────────────────────────
  #advance(filteredTracks: Track[], dir: 1 | -1, resolvedTheme = "dark") {
    if (!this.selectedTrack || filteredTracks.length === 0) return;
    const idx  = filteredTracks.findIndex(t => t.id === this.selectedTrack!.id);
    const next = idx !== -1 && idx + dir < filteredTracks.length && idx + dir >= 0
      ? filteredTracks[idx + dir]
      : filteredTracks[dir === 1 ? 0 : filteredTracks.length - 1];
    this.playTrack(next, resolvedTheme, filteredTracks);
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
