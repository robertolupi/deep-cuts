import { describe, expect, it } from "vitest";
import { waveformBarsFromJson } from "./waveform";

describe("waveformBarsFromJson", () => {
  it("returns downsampled numeric bars", () => {
    expect(waveformBarsFromJson("[1,2,3,4,5,6,7]", 3)).toEqual([1, 4, 7]);
  });

  it("returns an empty array for malformed JSON", () => {
    expect(waveformBarsFromJson("not json")).toEqual([]);
  });

  it("returns an empty array for non-array JSON", () => {
    expect(waveformBarsFromJson('{"peak":1}')).toEqual([]);
  });

  it("filters non-finite and non-number values", () => {
    expect(waveformBarsFromJson('[1,"bad",2,null,3]', 1)).toEqual([1, 2, 3]);
  });
});
