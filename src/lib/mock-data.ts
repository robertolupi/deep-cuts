import type { Track, WatchedDirectory, Playlist, SavedSearch } from "$lib/types";

// ── Mock waveform generator ───────────────────────────────────────────────────
// Takes a 32-point energy shape (0–1) and returns waveform_data (128-bin JSON)
// and waveform_sax (32-char a–e string) that match each other.
function makeMockWaveform(shape: number[]): { waveform_data: string; waveform_sax: string } {
  // Expand 32 segments → 128 bins with a simple within-segment taper
  const bins: number[] = [];
  for (let i = 0; i < 32; i++) {
    const v = shape[i];
    const next = shape[Math.min(31, i + 1)];
    for (let j = 0; j < 4; j++) {
      const t = j / 4;
      bins.push(Math.max(0.01, v * (1 - t) + next * t));
    }
  }

  // Z-normalise the 32 PAA means and quantise to SAX letters a–e
  const mean = shape.reduce((a, b) => a + b, 0) / 32;
  const std  = Math.sqrt(shape.reduce((a, b) => a + (b - mean) ** 2, 0) / 32);
  const BREAKS = [-0.841, -0.253, 0.253, 0.841];
  const sax = shape.map(v => {
    if (std < 1e-6) return 'c';
    const z = (v - mean) / std;
    if (z < BREAKS[0]) return 'a';
    if (z < BREAKS[1]) return 'b';
    if (z < BREAKS[2]) return 'c';
    if (z < BREAKS[3]) return 'd';
    return 'e';
  }).join('');

  return { waveform_data: JSON.stringify(bins), waveform_sax: sax };
}

// Pre-computed mock waveforms — each with a distinct structural arc
const WF_KONG          = makeMockWaveform([0.10,0.10,0.15,0.20,0.30,0.40,0.55,0.70,0.65,0.50,0.35,0.30,0.50,0.65,0.70,0.60,0.45,0.35,0.55,0.70,0.65,0.50,0.35,0.30,0.25,0.20,0.20,0.15,0.10,0.10,0.08,0.05]);
const WF_ANGEL_ECHOES  = makeMockWaveform([0.10,0.12,0.15,0.18,0.22,0.28,0.35,0.42,0.50,0.55,0.60,0.65,0.70,0.72,0.74,0.75,0.76,0.75,0.74,0.73,0.72,0.71,0.70,0.68,0.65,0.60,0.55,0.50,0.45,0.40,0.35,0.28]);
const WF_LESALPX       = makeMockWaveform([0.30,0.35,0.45,0.55,0.60,0.65,0.70,0.72,0.70,0.65,0.60,0.70,0.75,0.80,0.75,0.70,0.65,0.70,0.75,0.80,0.75,0.70,0.65,0.60,0.55,0.50,0.45,0.40,0.35,0.30,0.25,0.20]);
const WF_SUN           = makeMockWaveform([0.15,0.20,0.30,0.60,0.65,0.70,0.65,0.60,0.25,0.30,0.35,0.65,0.70,0.75,0.70,0.65,0.30,0.35,0.40,0.70,0.75,0.80,0.75,0.70,0.30,0.35,0.65,0.70,0.75,0.70,0.40,0.20]);
const WF_OPEN_EYE      = makeMockWaveform([0.05,0.06,0.07,0.08,0.10,0.12,0.15,0.18,0.22,0.27,0.32,0.38,0.45,0.52,0.58,0.64,0.70,0.74,0.78,0.82,0.85,0.87,0.88,0.89,0.90,0.90,0.91,0.90,0.89,0.88,0.87,0.85]);

export const MOCK_DIRECTORIES: WatchedDirectory[] = [
  { id: 1, name: "Music", path: "/Users/demo/Music" },
];

export const MOCK_TRACKS: Track[] = [
  {
    id: 1, watched_directory_id: 1,
    path: "/Users/demo/Music/Bonobo - Kong.flac",
    filename: "Bonobo - Kong.flac",
    size_bytes: 42_000_000, last_modified: 1700000000,
    duration_seconds: 312, sample_rate: 44100, bitrate: 1411, channels: 2, bit_depth: 16,
    title: "Kong", artist: "Bonobo", album: "The North Borders", genre: "Electronic",
    year: 2013, track_number: 3, track_total: 12, disc_number: 1, disc_total: 1,
    album_artist: "Bonobo", composer: null, comment: null, bpm: 93, lyrics: null,
    ...WF_KONG,
    sax_alignment: "IIVVVVCCBBOOO",
    sax_alignment_segments: "intro,intro,verse,verse,verse,verse,verse,chorus,chorus,chorus,bridge,bridge,outro,outro,outro,outro",
    sax_alignment_boundaries: "[39,78,136.5,195,234,265]",
    key: "Am", scale: "minor", key_strength: 0.82,
    loudness_lufs: -14.2, loudness_range: 7.1, silence_regions: null, has_long_silence: 0,
    detected_genre: "electronic", detected_vocal: "instrumental", detected_vocal_confidence: 0.91,
    mood_happy: 0.4, mood_sad: 0.2, mood_aggressive: 0.15, mood_relaxed: 0.7,
    mood_party: 0.3, mood_acoustic: 0.1, mood_electronic: 0.9,
    is_music: 1, ai_genre: "Downtempo / Trip-Hop", ai_mood: "Melancholic, Atmospheric",
    ai_instruments: "Piano, Bass, Drums, Strings", description: "Lush downtempo track with cinematic strings and a driving bassline.",
    structure_cluster_id: null,
    is_stale: 0,
  },
  {
    id: 2, watched_directory_id: 1,
    path: "/Users/demo/Music/Four Tet - Angel Echoes.flac",
    filename: "Four Tet - Angel Echoes.flac",
    size_bytes: 38_000_000, last_modified: 1700001000,
    duration_seconds: 427, sample_rate: 44100, bitrate: 1411, channels: 2, bit_depth: 16,
    title: "Angel Echoes", artist: "Four Tet", album: "There Is Love In You", genre: "Electronic",
    year: 2010, track_number: 1, track_total: 9, disc_number: 1, disc_total: 1,
    album_artist: "Four Tet", composer: null, comment: null, bpm: 128, lyrics: null,
    ...WF_ANGEL_ECHOES, sax_alignment: null, sax_alignment_segments: null, sax_alignment_boundaries: null, key: "C", scale: "major", key_strength: 0.75,
    loudness_lufs: -12.8, loudness_range: 5.4, silence_regions: null, has_long_silence: 0,
    detected_genre: "electronic", detected_vocal: "vocal", detected_vocal_confidence: 0.78,
    mood_happy: 0.65, mood_sad: 0.1, mood_aggressive: 0.05, mood_relaxed: 0.8,
    mood_party: 0.5, mood_acoustic: 0.05, mood_electronic: 0.95,
    is_music: 1, ai_genre: "House / Electronica", ai_mood: "Euphoric, Dreamy",
    ai_instruments: "Synth pads, Sampled vocals, Drums", description: "Hypnotic house track built on looped vocal samples and shimmering pads.",
    structure_cluster_id: null,
    is_stale: 0,
  },
  {
    id: 3, watched_directory_id: 1,
    path: "/Users/demo/Music/Floating Points - LesAlpx.flac",
    filename: "Floating Points - LesAlpx.flac",
    size_bytes: 55_000_000, last_modified: 1700002000,
    duration_seconds: 518, sample_rate: 48000, bitrate: 1411, channels: 2, bit_depth: 24,
    title: "LesAlpx", artist: "Floating Points", album: "Elaenia", genre: "Electronic",
    year: 2015, track_number: 2, track_total: 8, disc_number: 1, disc_total: 1,
    album_artist: "Floating Points", composer: "Sam Shepherd", comment: null, bpm: 140, lyrics: null,
    ...WF_LESALPX, sax_alignment: null, sax_alignment_segments: null, sax_alignment_boundaries: null, key: "F#", scale: "minor", key_strength: 0.69,
    loudness_lufs: -11.5, loudness_range: 9.2, silence_regions: null, has_long_silence: 0,
    detected_genre: "electronic", detected_vocal: "instrumental", detected_vocal_confidence: 0.95,
    mood_happy: 0.3, mood_sad: 0.35, mood_aggressive: 0.4, mood_relaxed: 0.25,
    mood_party: 0.2, mood_acoustic: 0.3, mood_electronic: 0.85,
    is_music: 1, ai_genre: "Jazz-influenced Electronic", ai_mood: "Intense, Complex",
    ai_instruments: "Synth, Live drums, Bass, Piano", description: "Intricate layered electronic piece blending jazz harmony with dense rhythmic patterns.",
    structure_cluster_id: null,
    is_stale: 0,
  },
  {
    id: 4, watched_directory_id: 1,
    path: "/Users/demo/Music/Caribou - Sun.mp3",
    filename: "Caribou - Sun.mp3",
    size_bytes: 9_800_000, last_modified: 1700003000,
    duration_seconds: 245, sample_rate: 44100, bitrate: 320, channels: 2, bit_depth: null,
    title: "Sun", artist: "Caribou", album: "Swim", genre: "Psychedelic",
    year: 2010, track_number: 5, track_total: 9, disc_number: 1, disc_total: 1,
    album_artist: "Caribou", composer: "Dan Snaith", comment: null, bpm: 107, lyrics: null,
    ...WF_SUN, sax_alignment: null, sax_alignment_segments: null, sax_alignment_boundaries: null, key: "D", scale: "major", key_strength: 0.71,
    loudness_lufs: -10.9, loudness_range: 4.8, silence_regions: null, has_long_silence: 0,
    detected_genre: "electronic", detected_vocal: "vocal", detected_vocal_confidence: 0.84,
    mood_happy: 0.75, mood_sad: 0.05, mood_aggressive: 0.1, mood_relaxed: 0.6,
    mood_party: 0.7, mood_acoustic: 0.2, mood_electronic: 0.75,
    is_music: 1, ai_genre: "Psychedelic Pop / Electronic", ai_mood: "Euphoric, Summery",
    ai_instruments: "Drums, Bass, Guitar, Synth, Vocals", description: "Bright psychedelic pop with propulsive drums and layered vocals.",
    structure_cluster_id: null,
    is_stale: 0,
  },
  {
    id: 5, watched_directory_id: 1,
    path: "/Users/demo/Music/Jon Hopkins - Open Eye Signal.flac",
    filename: "Jon Hopkins - Open Eye Signal.flac",
    size_bytes: 61_000_000, last_modified: 1700004000,
    duration_seconds: 614, sample_rate: 44100, bitrate: 1411, channels: 2, bit_depth: 16,
    title: "Open Eye Signal", artist: "Jon Hopkins", album: "Immunity", genre: "Electronic",
    year: 2013, track_number: 3, track_total: 8, disc_number: 1, disc_total: 1,
    album_artist: "Jon Hopkins", composer: null, comment: null, bpm: 138, lyrics: null,
    ...WF_OPEN_EYE, sax_alignment: null, sax_alignment_segments: null, sax_alignment_boundaries: null, key: "Bb", scale: "minor", key_strength: 0.88,
    loudness_lufs: -9.4, loudness_range: 6.7, silence_regions: null, has_long_silence: 0,
    detected_genre: "electronic", detected_vocal: "instrumental", detected_vocal_confidence: 0.97,
    mood_happy: 0.2, mood_sad: 0.4, mood_aggressive: 0.7, mood_relaxed: 0.1,
    mood_party: 0.6, mood_acoustic: 0.0, mood_electronic: 0.99,
    is_music: 1, ai_genre: "Techno / Ambient Techno", ai_mood: "Driving, Hypnotic",
    ai_instruments: "Synthesizers, Drum machine, Sub bass", description: "Relentless techno build over ten minutes with intricate hi-hat patterns and heavy sub bass.",
    structure_cluster_id: null,
    is_stale: 0,
  },
];

export const MOCK_TAGS: Record<number, string[]> = {
  1: ["chill", "study"],
  2: ["dance", "late-night"],
  3: ["focus", "complex"],
  4: ["summer", "dance"],
  5: ["peak-time", "dark"],
};

export const MOCK_ALL_TAGS: string[] = ["chill", "complex", "dance", "dark", "focus", "late-night", "peak-time", "study", "summer"];

export const MOCK_PLAYLISTS: Playlist[] = [
  { id: 1, name: "Late Night Set", created_at: 1700010000, updated_at: 1700010000 },
  { id: 2, name: "Study Focus", created_at: 1700020000, updated_at: 1700020000 },
];

export const MOCK_SAVED_SEARCHES: SavedSearch[] = [];
