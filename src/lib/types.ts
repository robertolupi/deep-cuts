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
}
