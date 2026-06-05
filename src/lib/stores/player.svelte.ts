/**
 * Player store — owns all WaveSurfer / playback state.
 *
 * DOM containers are registered by PlayerBar.svelte via register() using
 * a $effect, so WaveSurfer can find them once the DOM nodes exist.
 */
import { invoke } from "$lib/ipc";
import { convertFileSrc } from "@tauri-apps/api/core";
import { tick } from "svelte";
import WaveSurfer from "wavesurfer.js";
import Spectrogram from "wavesurfer.js/dist/plugins/spectrogram.esm.js";
import RegionsPlugin from "wavesurfer.js/dist/plugins/regions.esm.js";
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
  showLoudestMarker = $state(
    typeof localStorage !== "undefined"
      ? localStorage.getItem("deep-cuts-show-loudest-marker") === "true"
      : false
  );

  // Internal — not exposed to templates
  #wavesurfer          = $state<WaveSurfer | null>(null);
  #waveformContainer   = $state<HTMLDivElement | null>(null);
  #spectrogramContainer = $state<HTMLDivElement | null>(null);
  #regionsPlugin       = $state<any>(null);

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
      this.#regionsPlugin = null;
    }

    const assetUrl = convertFileSrc(track.path);

    // Wait for Svelte DOM tick so PlayerBar's $effect can register containers
    await tick();

    if (!this.#waveformContainer) {
      console.error("[PlayerStore] waveformContainer not registered — PlayerBar must call player.register() via $effect.");
      return;
    }

    this.#regionsPlugin = RegionsPlugin.create();

    const plugins: any[] = [this.#regionsPlugin];
    if (this.#spectrogramContainer) {
      plugins.push(
        Spectrogram.create({
          container:   this.#spectrogramContainer,
          labels:      true,
          fftSamples:  512,
          height:      75,
          labelsColor: resolvedTheme === "light" ? "#57534e" : "var(--sg-primary)",
        })
      );
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
      plugins,
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
        this.updateMarkers();
        this.#wavesurfer.play();
      }
    });
    this.#wavesurfer.on("finish", () => {
      this.isPlaying   = false;
      this.currentTime = 0;
      this.#advance(+1);
    });
  }

  setShowLoudestMarker(val: boolean) {
    this.showLoudestMarker = val;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("deep-cuts-show-loudest-marker", String(val));
    }
    this.updateMarkers();
  }

  updateMarkers() {
    if (!this.#wavesurfer || !this.#regionsPlugin) return;
    this.#regionsPlugin.clearRegions();
    if (!this.showLoudestMarker || !this.selectedTrack) return;

    const waveformData = this.selectedTrack.waveform_data;
    if (!waveformData) return;

    const pct = selectBestEnergyWindowPct(waveformData);
    const center = this.duration * pct;

    const createMarkerLabel = (text: string) => {
      const el = document.createElement("div");
      el.innerText = text;
      el.style.fontSize = "8px";
      el.style.fontFamily = "JetBrains Mono, monospace";
      el.style.padding = "1px 4px";
      el.style.background = "rgba(0, 0, 0, 0.75)";
      el.style.color = "#fff";
      el.style.borderRadius = "3px";
      el.style.position = "absolute";
      el.style.top = "2px";
      el.style.left = "4px";
      el.style.whiteSpace = "nowrap";
      el.style.pointerEvents = "none";
      el.style.zIndex = "10";
      return el;
    };

    const getSlidingWindow = (c: number, windowSize: number) => {
      if (this.duration <= windowSize) {
        return { start: 0, end: this.duration };
      }
      const half = windowSize / 2;
      let start = Math.max(0, c - half);
      let end = start + windowSize;
      if (end > this.duration) {
        end = this.duration;
        start = end - windowSize;
      }
      return { start, end };
    };

    // Qwen Window (30s)
    const qwenWin = getSlidingWindow(center, 30);
    this.#regionsPlugin.addRegion({
      start: qwenWin.start,
      end: qwenWin.end,
      color: theme.resolvedTheme === "light" ? "rgba(124, 45, 107, 0.06)" : "rgba(254, 0, 254, 0.06)",
      drag: false,
      resize: false,
      content: createMarkerLabel("Qwen (30s)"),
    });

    // Essentia Window (60s)
    const essentiaWin = getSlidingWindow(center, 60);
    this.#regionsPlugin.addRegion({
      start: essentiaWin.start,
      end: essentiaWin.end,
      color: theme.resolvedTheme === "light" ? "rgba(13, 115, 119, 0.03)" : "rgba(0, 240, 255, 0.03)",
      drag: false,
      resize: false,
      content: createMarkerLabel("Essentia (60s)"),
    });

    // CLAP Windows (3 windows, 10s duration each)
    const clapPcts = selectClapWindowPcts(waveformData, this.duration);
    clapPcts.forEach((clapPct, idx) => {
      const c = clapPct * this.duration;
      const clapWin = getSlidingWindow(c, 10);
      this.#regionsPlugin.addRegion({
        start: clapWin.start,
        end: clapWin.end,
        color: theme.resolvedTheme === "light" ? "rgba(240, 160, 48, 0.05)" : "rgba(240, 160, 48, 0.08)",
        drag: false,
        resize: false,
        content: createMarkerLabel(`CLAP Win ${idx + 1}`),
      });
    });

    // Loudest Point Marker Line (start == end)
    this.#regionsPlugin.addRegion({
      start: center,
      end: center,
      color: theme.resolvedTheme === "light" ? "rgba(13, 115, 119, 0.8)" : "rgba(0, 240, 255, 0.8)",
      drag: false,
      resize: false,
      content: createMarkerLabel("Loudest Point"),
    });
  }

  togglePlayback() {
    this.#wavesurfer?.playPause();
  }

  resetPlayer() {
    if (this.#wavesurfer) {
      this.#wavesurfer.destroy();
      this.#wavesurfer = null;
      this.#regionsPlugin = null;
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

function selectBestEnergyWindowPct(waveformData: string | null): number {
  if (!waveformData) return 0.5;
  try {
    const waveform = JSON.parse(waveformData) as number[];
    if (!Array.isArray(waveform) || waveform.length === 0) return 0.5;
    let maxVal = -Infinity;
    let maxIdx = 0;
    for (let i = 0; i < waveform.length; i++) {
      const v = waveform[i];
      if (isFinite(v) && v > maxVal) {
        maxVal = v;
        maxIdx = i;
      }
    }
    return (maxIdx + 0.5) / waveform.length;
  } catch (e) {
    return 0.5;
  }
}

function selectClapWindowPcts(waveformData: string | null, durationSeconds: number): number[] {
  const defaults = [0.25, 0.50, 0.75];
  if (!waveformData) return defaults;
  try {
    const waveform = JSON.parse(waveformData) as number[];
    if (!Array.isArray(waveform) || waveform.length === 0) return defaults;
    if (waveform.every(v => !isFinite(v) || v <= 0)) return defaults;

    const binCount = waveform.length;
    const minSepBins = durationSeconds > 0
      ? Math.max(1, Math.ceil((10.0 / durationSeconds) * binCount))
      : Math.max(1, Math.floor(binCount / 12));

    let ranked: { idx: number; val: number }[] = waveform
      .map((val, idx) => ({ idx, val }))
      .filter(item => isFinite(item.val));

    // Sort descending by value, then ascending by index
    ranked.sort((a, b) => b.val - a.val || a.idx - b.idx);

    const selected: number[] = [];
    for (const item of ranked) {
      const farEnough = selected.every(picked => Math.abs(item.idx - picked) >= minSepBins);
      if (farEnough) {
        selected.push(item.idx);
      }
      if (selected.length === 3) break;
    }

    for (const item of ranked) {
      if (selected.length === 3) break;
      if (!selected.includes(item.idx)) {
        selected.push(item.idx);
      }
    }

    if (selected.length < 3) return defaults;

    selected.sort((a, b) => a - b);
    return [
      (selected[0] + 0.5) / binCount,
      (selected[1] + 0.5) / binCount,
      (selected[2] + 0.5) / binCount,
    ];
  } catch (e) {
    return defaults;
  }
}

