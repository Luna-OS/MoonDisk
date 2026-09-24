import { describe, expect, it } from "vitest";
import { formatBytes } from "./format";

describe("formatBytes", () => {
  it("formats zero", () => {
    expect(formatBytes("0")).toBe("0 B");
  });

  it("formats exact GiB values", () => {
    expect(formatBytes(String(2n * 1024n * 1024n * 1024n))).toBe("2,0 GiB");
  });

  it("handles sizes beyond Number.MAX_SAFE_INTEGER", () => {
    // 16 TiB, well past 2^53-1 — this is exactly the case ByteSize is a
    // string over the wire for (see types/models.ts).
    const sixteenTiB = 16n * 1024n * 1024n * 1024n * 1024n;
    expect(formatBytes(sixteenTiB.toString())).toBe("16,0 TiB");
  });
});
