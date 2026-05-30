import { describe, it, expect } from "vitest";
import { formatDuration, formatSize } from "$lib/utils/format";

describe("formatDuration", () => {
  it("formats whole minutes with zero seconds", () => {
    expect(formatDuration(60)).toBe("1:00");
  });

  it("pads seconds with a leading zero when < 10", () => {
    expect(formatDuration(65)).toBe("1:05");
  });

  it("handles zero", () => {
    expect(formatDuration(0)).toBe("0:00");
  });

  it("handles sub-minute durations", () => {
    expect(formatDuration(9)).toBe("0:09");
    expect(formatDuration(59)).toBe("0:59");
  });

  it("handles long tracks (> 1 hour)", () => {
    expect(formatDuration(3661)).toBe("61:01");
  });

  it("truncates fractional seconds (floor, not round)", () => {
    // 90.9 seconds → 1:30, not 1:31
    expect(formatDuration(90.9)).toBe("1:30");
  });
});

describe("formatSize", () => {
  it("formats bytes as MB with one decimal place", () => {
    expect(formatSize(1_048_576)).toBe("1.0 MB");
  });

  it("formats a typical FLAC file size", () => {
    expect(formatSize(50_000_000)).toBe("47.7 MB");
  });

  it("formats a small file", () => {
    expect(formatSize(524_288)).toBe("0.5 MB");
  });

  it("formats zero bytes", () => {
    expect(formatSize(0)).toBe("0.0 MB");
  });
});
