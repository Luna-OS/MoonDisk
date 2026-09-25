/** Mirrors the alignment rule `operations::validator::validate` enforces
 * server-side (src-tauri/src/operations/validator.rs) for CreatePartition:
 * start and size must be 1 MiB-aligned, and the resulting partition must
 * be at least 1 MiB. A free-space region's own start/size aren't
 * guaranteed to already be aligned — gaps between existing partitions on
 * a real disk often aren't, especially ones created by other tools — so
 * "create a partition over this entire free region" has to shrink the
 * requested range to the largest aligned sub-range that fits, rather than
 * pass the free region's raw bytes straight through and let the backend
 * reject them. */
const ALIGNMENT = 1024n * 1024n; // 1 MiB
const MIN_PARTITION_SIZE = 1024n * 1024n; // 1 MiB

export function alignedFreeRange(
  start: string,
  size: string,
): { start: string; size: string } | null {
  const s = BigInt(start);
  const end = s + BigInt(size);
  const alignedStart = ((s + ALIGNMENT - 1n) / ALIGNMENT) * ALIGNMENT;
  const alignedEnd = (end / ALIGNMENT) * ALIGNMENT;
  if (alignedEnd <= alignedStart || alignedEnd - alignedStart < MIN_PARTITION_SIZE) {
    return null;
  }
  return { start: alignedStart.toString(), size: (alignedEnd - alignedStart).toString() };
}
