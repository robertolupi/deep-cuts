export interface WatchedDirectory {
  id: number;
  name: string;
  path: string;
}

export interface Track {
  id: number;
  watched_directory_id: number;
  path: string;
  filename: string;
  size_bytes: number;
  last_modified: number;
  duration_seconds: number;
  sample_rate: number | null;
  bitrate: number | null;
  channels: number | null;
  bit_depth: number | null;
  title: string | null;
  artist: string | null;
  album: string | null;
  genre: string | null;
  year: number | null;
  track_number: number | null;
  track_total: number | null;
  disc_number: number | null;
  disc_total: number | null;
  album_artist: string | null;
  composer: string | null;
  comment: string | null;
  bpm: number | null;
  lyrics: string | null;

  // Analysis results
  waveform_data: string | null;
  key: string | null;
  scale: string | null;
  key_strength: number | null;
  loudness_lufs: number | null;
  loudness_range: number | null;
  silence_regions: string | null;
  has_long_silence: number;

  // Essentia classifier results
  detected_genre: string | null;
  detected_vocal: string | null;
  detected_vocal_confidence: number | null;
  mood_happy: number | null;
  mood_sad: number | null;
  mood_aggressive: number | null;
  mood_relaxed: number | null;
  mood_party: number | null;
  mood_acoustic: number | null;
  mood_electronic: number | null;

  // Qwen / LLM analysis results
  is_music: number | null;
  ai_genre: string | null;
  ai_mood: string | null;
  ai_instruments: string | null;
  description: string | null;
}
