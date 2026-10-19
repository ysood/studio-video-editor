import { describe, expect, it } from "vitest";
import { microsToFrames, timeAgo, timebase, timecode } from "./format";

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

describe("timeAgo", () => {
  const now = Date.UTC(2026, 9, 19, 12, 0, 0);
  const ago = (ms: number) => timeAgo(now - ms, now);
  const minute = 60_000;
  const hour = 60 * minute;
  const day = 24 * hour;

  it("says just now for anything too recent to name", () => {
    expect(ago(5_000)).toBe("just now");
  });

  it("counts in the largest unit that still fits", () => {
    expect(ago(90 * minute)).toBe("1 hour ago");
    expect(ago(2 * day)).toBe("2 days ago");
    expect(ago(8 * day)).toBe("1 week ago");
    expect(ago(60 * day)).toBe("2 months ago");
  });

  it("rounds down, so nothing is ever reported as older than it is", () => {
    expect(ago(13 * day)).toBe("1 week ago");
  });

  it("does not go backwards for a clock that is slightly ahead", () => {
    expect(timeAgo(now + 5_000, now)).toBe("just now");
  });
});
