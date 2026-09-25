import { describe, expect, it } from "vitest";
import type { Disk, FlashProgress } from "@/types/models";
import { overallFraction, unsuitableReason } from "./flash";

const MIB = 1024 * 1024;

const drive = { size: String(64 * MIB), readOnly: false } as Disk;

function progress(phase: FlashProgress["phase"], done: number): FlashProgress {
  return { phase, done: String(done), total: "100", bytesPerSecond: 1 };
}

describe("unsuitableReason", () => {
  it("accepts an image that fits", () => {
    expect(unsuitableReason(drive, { size: String(64 * MIB) } as never)).toBeNull();
  });

  it("counts the padding of the last sector", () => {
    expect(unsuitableReason(drive, { size: String(64 * MIB - 1) } as never)).toBeNull();
    expect(unsuitableReason(drive, { size: String(64 * MIB + 1) } as never)).toBe("Too small");
  });

  it("rejects read-only drives", () => {
    expect(unsuitableReason({ ...drive, readOnly: true }, null)).toBe("Read-only");
  });
});

describe("overallFraction", () => {
  it("splits writing and verifying in halves", () => {
    expect(overallFraction(progress("writing", 50), true)).toBe(0.25);
    expect(overallFraction(progress("verifying", 50), true)).toBe(0.75);
  });

  it("uses the whole range for writing without verification", () => {
    expect(overallFraction(progress("writing", 50), false)).toBe(0.5);
    expect(overallFraction(progress("finishing", 0), false)).toBe(1);
    expect(overallFraction(null, false)).toBe(0);
  });
});
