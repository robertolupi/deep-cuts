import { describe, it, expect, vi, beforeEach } from "vitest";
import { player } from "$lib/stores/player.svelte";
import { MOCK_TRACKS, createTrack } from "../../test/fixtures";

// WaveSurfer and Tauri are mocked globally in src/test/setup.ts

describe("PlayerStore — initial state", () => {
  it("starts with no selected track", () => {
    expect(player.selectedTrack).toBeNull();
  });

  it("starts not playing", () => {
    expect(player.isPlaying).toBe(false);
  });

  it("starts at time zero", () => {
    expect(player.currentTime).toBe(0);
    expect(player.duration).toBe(0);
  });
});

describe("PlayerStore — container registration", () => {
  it("registers and unregisters DOM containers", () => {
    const waveform = document.createElement("div");
    const spectrogram = document.createElement("div");

    // Should not throw
    expect(() => player.register(waveform, spectrogram)).not.toThrow();
    expect(() => player.unregister()).not.toThrow();
  });
});

describe("PlayerStore — resetPlayer", () => {
  beforeEach(() => {
    // Give the store a track so we can test resetting
    player.selectedTrack = createTrack({ id: 99, title: "Reset Me" });
    player.isPlaying = true;
    player.currentTime = 45;
    player.duration = 210;
  });

  it("clears selected track", () => {
    player.resetPlayer();
    expect(player.selectedTrack).toBeNull();
  });

  it("resets playback state to zero", () => {
    player.resetPlayer();
    expect(player.isPlaying).toBe(false);
    expect(player.currentTime).toBe(0);
    expect(player.duration).toBe(0);
  });
});

describe("PlayerStore — handlePrevTrack", () => {
  beforeEach(() => {
    player.resetPlayer();
  });

  it("does nothing when no track is selected", () => {
    player.selectedTrack = null;
    // Should not throw
    expect(() => player.handlePrevTrack(MOCK_TRACKS, "dark")).not.toThrow();
  });

  it("does nothing when the track list is empty", () => {
    player.selectedTrack = MOCK_TRACKS[1];
    expect(() => player.handlePrevTrack([], "dark")).not.toThrow();
  });

  it("navigates to the previous track", async () => {
    player.selectedTrack = MOCK_TRACKS[2]; // id=3 Glitch Step
    const waveform = document.createElement("div");
    const spectrogram = document.createElement("div");
    player.register(waveform, spectrogram);

    await player.handlePrevTrack(MOCK_TRACKS, "dark");

    expect(player.selectedTrack?.id).toBe(MOCK_TRACKS[1].id);
  });

  it("wraps around to the last track when on the first", async () => {
    player.selectedTrack = MOCK_TRACKS[0]; // first track
    const waveform = document.createElement("div");
    const spectrogram = document.createElement("div");
    player.register(waveform, spectrogram);

    await player.handlePrevTrack(MOCK_TRACKS, "dark");

    expect(player.selectedTrack?.id).toBe(MOCK_TRACKS[MOCK_TRACKS.length - 1].id);
  });
});

describe("PlayerStore — handleNextTrack", () => {
  beforeEach(() => {
    player.resetPlayer();
  });

  it("does nothing when no track is selected", () => {
    player.selectedTrack = null;
    expect(() => player.handleNextTrack(MOCK_TRACKS, "dark")).not.toThrow();
  });

  it("navigates to the next track", async () => {
    player.selectedTrack = MOCK_TRACKS[0]; // first
    const waveform = document.createElement("div");
    const spectrogram = document.createElement("div");
    player.register(waveform, spectrogram);

    await player.handleNextTrack(MOCK_TRACKS, "dark");

    expect(player.selectedTrack?.id).toBe(MOCK_TRACKS[1].id);
  });

  it("wraps around to the first track when on the last", async () => {
    player.selectedTrack = MOCK_TRACKS[MOCK_TRACKS.length - 1]; // last track
    const waveform = document.createElement("div");
    const spectrogram = document.createElement("div");
    player.register(waveform, spectrogram);

    await player.handleNextTrack(MOCK_TRACKS, "dark");

    expect(player.selectedTrack?.id).toBe(MOCK_TRACKS[0].id);
  });
});

describe("PlayerStore — revealInFinder", () => {
  it("calls invoke with reveal_in_finder and the track path", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValueOnce(undefined);

    await player.revealInFinder("/music/test.flac");

    expect(invoke).toHaveBeenCalledWith("reveal_in_finder", { path: "/music/test.flac" });
  });

  it("does not throw if invoke rejects", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockRejectedValueOnce(new Error("not found"));

    await expect(player.revealInFinder("/missing.flac")).resolves.toBeUndefined();
  });
});
