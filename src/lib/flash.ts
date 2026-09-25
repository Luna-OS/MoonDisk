import type { Disk, FlashProgress, ImageInfo } from "@/types/models";
import { bytesValue } from "@/lib/format";

/** The backend writes whole 4 KiB sectors, so the last one is padded. */
const SECTOR = 4096n;

/** Why `disk` can't take `image`, or `null` if it can. */
export function unsuitableReason(disk: Disk, image: ImageInfo | null): string | null {
  if (disk.readOnly) return "Read-only";
  if (image) {
    const padded = ((bytesValue(image.size) + SECTOR - 1n) / SECTOR) * SECTOR;
    if (padded > bytesValue(disk.size)) return "Too small";
  }
  return null;
}

/** Progress over the whole run: writing and verifying each take half when
 * verifying, since both move the same number of bytes. */
export function overallFraction(p: FlashProgress | null, verify: boolean): number {
  if (!p) return 0;
  const total = Number(p.total);
  const f = total > 0 ? Number(p.done) / total : 0;
  switch (p.phase) {
    case "preparing":
      return 0;
    case "writing":
      return verify ? f / 2 : f;
    case "verifying":
      return 0.5 + f / 2;
    case "finishing":
      return 1;
  }
}
