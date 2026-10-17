import { describe, expect, it } from "vitest";
import { microsToFrames, timebase, timecode } from "./format";

describe("timecode", () => {
  it("matches the Rust implementation for integer and NTSC rates", () => {
    expect(timecode(0, { num: 24, den: 1 })).toBe("00:00:00:00");
    expect(timecode(24 * 3661 + 5, { num: 24, den: 1 })).toBe("01:01:01:05");
    expect(timebase({ num: 30000, den: 1001 })).toBe(30);
  });

  it("floors media length like MediaAsset::frames_at", () => {
    expect(microsToFrames(3_600_000_000, { num: 24000, den: 1001 })).toBe(86_313);
  });
});
