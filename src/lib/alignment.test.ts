import { describe, expect, it } from "vitest";
import { alignedFreeRange } from "./alignment";

const MIB = 1024n * 1024n;

describe("alignedFreeRange", () => {
  it("passes an already-aligned region through unchanged", () => {
    expect(alignedFreeRange((10n * MIB).toString(), (50n * MIB).toString())).toEqual({
      start: (10n * MIB).toString(),
      size: (50n * MIB).toString(),
    });
  });

  it("rounds an unaligned start up and an unaligned end down", () => {
    // A gap from byte 10 MiB + 7 to 60 MiB - 3 (not multiples of 1 MiB, as
    // real disks often have between pre-existing partitions) shrinks to
    // the 11 MiB..59 MiB aligned sub-range.
    const start = 10n * MIB + 7n;
    const end = 60n * MIB - 3n;
    const result = alignedFreeRange(start.toString(), (end - start).toString());
    expect(result).toEqual({
      start: (11n * MIB).toString(),
      size: (48n * MIB).toString(),
    });
  });

  it("returns null when nothing 1 MiB or larger remains after aligning", () => {
    // A region entirely inside one MiB boundary.
    const start = 10n * MIB + 100n;
    const size = 500n;
    expect(alignedFreeRange(start.toString(), size.toString())).toBeNull();
  });

  it("returns null for a region just under 1 MiB even when the start is aligned", () => {
    expect(alignedFreeRange((10n * MIB).toString(), (MIB - 1n).toString())).toBeNull();
  });

  it("accepts a region of exactly 1 MiB", () => {
    expect(alignedFreeRange((10n * MIB).toString(), MIB.toString())).toEqual({
      start: (10n * MIB).toString(),
      size: MIB.toString(),
    });
  });
});
