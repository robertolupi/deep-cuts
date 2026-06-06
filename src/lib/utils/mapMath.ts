import * as d3 from 'd3';

export interface MappedTrackPoint {
  id: number;
  x: number;
  y: number;
  watched_directory_id: number;
  title: string | null;
  filename: string;
  artist: string | null;
  genre: string | null;
  bpm: number | null;
  key: string | null;
  scale: string | null;
  algorithm?: string | null;
  mood_happy?: number | null;
  mood_sad?: number | null;
  mood_aggressive?: number | null;
  mood_relaxed?: number | null;
  mood_party?: number | null;
  mood_acoustic?: number | null;
  mood_electronic?: number | null;
  structure_cluster_id?: number | null;
}

export const camelotMap: { [key: string]: { code: string; color: string } } = {
  "Abm": { code: "1A", color: "#00E5FF" }, "G#m": { code: "1A", color: "#00E5FF" },
  "Ebm": { code: "2A", color: "#00B0FF" }, "D#m": { code: "2A", color: "#00B0FF" },
  "Bbm": { code: "3A", color: "#2979FF" }, "A#m": { code: "3A", color: "#2979FF" },
  "Fm":  { code: "4A", color: "#651FFF" },
  "Cm":  { code: "5A", color: "#AA00FF" },
  "Gm":  { code: "6A", color: "#D500F9" },
  "Dm":  { code: "7A", color: "#F50057" },
  "Am":  { code: "8A", color: "#FF1744" },
  "Em":  { code: "9A", color: "#FF9100" },
  "Bm":  { code: "10A", color: "#FFEA00" },
  "F#m": { code: "11A", color: "#76FF03" }, "Gbm": { code: "11A", color: "#76FF03" },
  "C#m": { code: "12A", color: "#00E676" }, "Dbm": { code: "12A", color: "#00E676" },
  "B":   { code: "1B", color: "#80DEEA" }, "Cb":  { code: "1B", color: "#80DEEA" },
  "F#":  { code: "2B", color: "#82B1FF" }, "Gb":  { code: "2B", color: "#82B1FF" },
  "C#":  { code: "3B", color: "#8C9EFF" }, "Db":  { code: "3B", color: "#8C9EFF" },
  "Ab":  { code: "4B", color: "#B388FF" }, "G#":  { code: "4B", color: "#B388FF" },
  "Eb":  { code: "5B", color: "#EA80FC" }, "D#":  { code: "5B", color: "#EA80FC" },
  "Bb":  { code: "6B", color: "#FF80AB" }, "A#":  { code: "6B", color: "#FF80AB" },
  "F":   { code: "7B", color: "#FF8A80" },
  "C":   { code: "8B", color: "#FFE082" },
  "G":   { code: "9B", color: "#FFF59D" },
  "D":   { code: "10B", color: "#C6FF00" },
  "A":   { code: "11B", color: "#A7FFEB" },
  "E":   { code: "12B", color: "#A5D6A7" }
};

// tab20-equivalent palette matching Python's matplotlib tab20 for cluster IDs 0–19
// Cluster labels and regexes are now dynamic — load from the `structureClusters` store.
export const STRUCTURE_CLUSTER_COLORS: string[] = [
  '#1f77b4', '#aec7e8', '#ff7f0e', '#ffbb78', '#2ca02c',
  '#98df8a', '#d62728', '#ff9896', '#9467bd', '#c5b0d5',
  '#8c564b', '#c49c94', '#e377c2', '#f7b6d2', '#7f7f7f',
  '#c7c7c7', '#bcbd22', '#dbdb8d', '#17becf', '#9edae5',
];

export function structureClusterColor(clusterId: number | null | undefined): string {
  if (clusterId == null || clusterId < 0) return '#333340';
  return STRUCTURE_CLUSTER_COLORS[clusterId % STRUCTURE_CLUSTER_COLORS.length];
}

export function resolveTrackColor(
  track: MappedTrackPoint,
  colorCoding: 'genre' | 'camelot' | 'bpm' | 'mood' | 'structure',
  dynamicGenreColors: Record<string, string>,
  themeColors: { bpmCool: string; bpmHot: string; dotBorder: string; dotBorderWidth: number; canvasBg: string },
): string {
  if (colorCoding === 'genre') {
    const g = track.genre;
    if (!g || !g.trim()) return dynamicGenreColors["Unknown"];
    const primary = g.split(/[---,;/]/)[0].trim();
    for (const key of Object.keys(dynamicGenreColors)) {
      if (primary.toLowerCase().includes(key.toLowerCase())) {
        return dynamicGenreColors[key];
      }
    }
    return dynamicGenreColors["Other"];
  } else if (colorCoding === 'camelot') {
    const k = track.key || "?";
    const scale = track.scale || "";
    const query = scale.toLowerCase() === "minor" ? `${k}m` : k;
    const match = camelotMap[query];
    return match ? match.color : "#aaaaaa";
  } else if (colorCoding === 'mood') {
    const happy = track.mood_happy ?? 0;
    const sad = track.mood_sad ?? 0;
    const aggressive = track.mood_aggressive ?? 0;
    const relaxed = track.mood_relaxed ?? 0;
    const party = track.mood_party ?? 0;
    const acoustic = track.mood_acoustic ?? 0;
    const electronic = track.mood_electronic ?? 0;

    const moods = [
      { name: 'Happy', val: happy, color: '#ffeb3b' },
      { name: 'Sad', val: sad, color: '#2979ff' },
      { name: 'Aggressive', val: aggressive, color: '#ff1744' },
      { name: 'Relaxed', val: relaxed, color: '#00e676' },
      { name: 'Party', val: party, color: '#d500f9' },
      { name: 'Acoustic', val: acoustic, color: '#ff9100' },
      { name: 'Electronic', val: electronic, color: '#00e5ff' }
    ];

    let maxMood = moods[0];
    for (const m of moods) {
      if (m.val > maxMood.val) {
        maxMood = m;
      }
    }
    if (maxMood.val <= 1e-5) return "#aaaaaa";
    return maxMood.color;
  } else if (colorCoding === 'structure') {
    return structureClusterColor(track.structure_cluster_id);
  } else {
    const bpmVal = track.bpm || 120;
    const pct = Math.max(0, Math.min(1, (bpmVal - 70) / 110));
    return d3.interpolateRgb(themeColors.bpmCool, themeColors.bpmHot)(pct);
  }
}
