export function waveformBarsFromJson(
  waveformData: string | null,
  stride = 3,
): number[] {
  if (!waveformData) return [];

  try {
    const parsed: unknown = JSON.parse(waveformData);
    if (!Array.isArray(parsed)) return [];

    return parsed
      .filter((_, index) => index % stride === 0)
      .filter((value): value is number => typeof value === "number" && Number.isFinite(value));
  } catch {
    return [];
  }
}
