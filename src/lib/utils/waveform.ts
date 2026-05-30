const cache = new Map<string, number[]>();

export function waveformBarsFromJson(
  waveformData: string | null,
  stride = 3,
): number[] {
  if (!waveformData) return [];

  const key = stride === 3 ? waveformData : `${stride}\0${waveformData}`;
  const cached = cache.get(key);
  if (cached) return cached;

  try {
    const parsed: unknown = JSON.parse(waveformData);
    if (!Array.isArray(parsed)) return [];

    const result = parsed
      .filter((_, index) => index % stride === 0)
      .filter((value): value is number => typeof value === "number" && Number.isFinite(value));
    cache.set(key, result);
    return result;
  } catch {
    return [];
  }
}
