/**
 * Shared test fixtures — deterministic, minimal Track and WatchedDirectory objects.
 * Import these in every test file instead of constructing inline objects.
 */
import type { Track, WatchedDirectory } from "$lib/types";

export function createTrack(overrides: Partial<Track> = {}): Track {
  return {
    id: 1,
    watched_directory_id: 1,
    path: "/music/test-track.flac",
    filename: "test-track.flac",
    size_bytes: 50_000_000,
    last_modified: 1_700_000_000,
    duration_seconds: 210,
    sample_rate: 44100,
    bitrate: 1411,
    channels: 2,
    bit_depth: 16,
    title: "Test Track",
    artist: "Test Artist",
    album: "Test Album",
    genre: "Electronic",
    year: 2023,
    track_number: 1,
    track_total: 10,
    disc_number: 1,
    disc_total: 1,
    album_artist: "Test Artist",
    composer: null,
    comment: null,
    bpm: 128,
    lyrics: null,
    waveform_data: null,
    waveform_sax: null,

    sax_alignment: null,
    sax_alignment_segments: null,
    key: "C",
    scale: "major",
    key_strength: 0.82,
    loudness_lufs: -14.2,
    loudness_range: 6.1,
    silence_regions: null,
    has_long_silence: 0,
    detected_genre: "electronic",
    detected_vocal: "instrumental",
    detected_vocal_confidence: 0.91,
    mood_happy: 0.6,
    mood_sad: 0.1,
    mood_aggressive: 0.2,
    mood_relaxed: 0.5,
    mood_party: 0.7,
    mood_acoustic: 0.05,
    mood_electronic: 0.9,
    is_music: 1,
    ai_genre: null,
    ai_mood: null,
    ai_instruments: null,
    description: null,
    is_stale: 0,
    ...overrides,
  };
}

export function createDir(overrides: Partial<WatchedDirectory> = {}): WatchedDirectory {
  return {
    id: 1,
    name: "My Music",
    path: "/music",
    ...overrides,
  };
}

/** A small set of tracks covering common edge cases used across filter tests */
export const MOCK_TRACKS: Track[] = [
  createTrack({ id: 1, title: "Cyan Dreams",  artist: "Artist A", genre: "Electronic", bpm: 128, key: "C",  scale: "major" }),
  createTrack({ id: 2, title: "Pink Noise",   artist: "Artist B", genre: "Ambient",    bpm: 72,  key: "A",  scale: "minor" }),
  createTrack({ id: 3, title: "Glitch Step",  artist: "Artist A", genre: "Electronic", bpm: 145, key: "F",  scale: "major" }),
  createTrack({ id: 4, title: "Jazz Smoke",   artist: "Artist C", genre: "Jazz",       bpm: 95,  key: "Bb", scale: "minor" }),
  createTrack({ id: 5, title: "No BPM Track", artist: "Artist D", genre: "Classical",  bpm: null, key: null, scale: null }),
];
