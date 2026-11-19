import { describe, expect, it } from "vitest";
import { columnLevel, decodeLevels, type Waveform } from "./peaks";

/** A waveform whose every bucket says which bucket it is, so a reading names its index. */
function counting(count: number, bucketsPerSecond = 100): Waveform {
  const levels = new Uint8Array(count);
  for (let i = 0; i < count; i += 1) levels[i] = i % 256;
  return { bucketsPerSecond, levels };
}

describe("columnLevel", () => {
  // 100 buckets a second at 25 fps is four buckets a frame, so a column one frame wide
  // covers four of them and the arithmetic is readable by hand.
  const fps = 25;

  it("reads the first bucket at the left edge of an untrimmed clip", () => {
    // Four buckets a frame, one pixel per frame: buckets 0 to 3, the loudest of which
    // is 3.
    expect(columnLevel(counting(400), 0, 0, 1, fps)).toBe(3);
  });

  it("starts where the clip's source does, not at the beginning of the media", () => {
    // A clip starting ten frames in opens on bucket 40.
    expect(columnLevel(counting(400), 10, 0, 1, fps)).toBe(43);
    // And the same column of a clip starting twenty frames in is somewhere else, which
    // is the whole point: trimming a clip slides its waveform rather than rescaling it.
    expect(columnLevel(counting(400), 20, 0, 1, fps)).toBe(83);
  });

  it("walks along the clip one column at a time", () => {
    const wave = counting(400);
    expect(columnLevel(wave, 0, 1, 1, fps)).toBe(7);
    expect(columnLevel(wave, 0, 2, 1, fps)).toBe(11);
  });

  it("takes the loudest bucket a column covers, not their average", () => {
    const levels = new Uint8Array(400);
    // Silence except for one spike, inside the stretch the first column covers.
    levels[2] = 200;
    const wave: Waveform = { bucketsPerSecond: 100, levels };
    expect(columnLevel(wave, 0, 0, 1, fps)).toBe(200);
  });

  it("keeps a transient when zoomed out far enough to hide it in an average", () => {
    const levels = new Uint8Array(4000);
    levels[1500] = 250;
    const wave: Waveform = { bucketsPerSecond: 100, levels };
    // A quarter of a pixel per frame: one column covers four frames, so sixteen
    // buckets. The spike at bucket 1500 falls in the column covering 1488 to 1504,
    // where an average over sixteen buckets would have buried it.
    const column = Math.floor(1500 / 16);
    expect(columnLevel(wave, 0, column, 0.25, fps)).toBe(250);
    expect(columnLevel(wave, 0, column - 1, 0.25, fps)).toBe(0);
  });

  it("covers at least one bucket however far in it is zoomed", () => {
    // Forty pixels per frame: a column is a tenth of a bucket wide, and rounding must
    // not leave it covering none of them.
    const wave = counting(400);
    expect(columnLevel(wave, 0, 0, 40, fps)).toBe(0);
    expect(columnLevel(wave, 0, 400, 40, fps)).toBe(40);
  });

  it("reads silence past the end rather than running off the buckets", () => {
    const wave = counting(8);
    expect(columnLevel(wave, 100, 0, 1, fps)).toBe(0);
  });

  it("handles a frame rate that is not a whole number", () => {
    // 24000/1001 is 23.976: four buckets a frame would be wrong by a thousandth, and
    // the only thing that matters is that it does not land on a different bucket.
    const wave = counting(400, 100);
    expect(columnLevel(wave, 0, 0, 1, 24000 / 1001)).toBe(4);
  });
});

describe("decodeLevels", () => {
  // The expected strings come from Rust's own base64 encoder, run over these bytes, so
  // this test fails if either side of the seam changes how it spells them.
  it("reads back the bytes the shell encoded", () => {
    expect([...decodeLevels("AAf/gA==")]).toEqual([0, 7, 255, 128]);
    expect([...decodeLevels("DcgE")]).toEqual([13, 200, 4]);
  });

  it("reads an empty payload as no buckets rather than failing", () => {
    expect(decodeLevels("").length).toBe(0);
  });
});
