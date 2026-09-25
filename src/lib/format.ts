/** Formats a `ByteSize` (a decimal string, see types/models.ts) as a
 * human-readable IEC size (GiB/MiB/...). Uses BigInt throughout since the
 * value can exceed Number.MAX_SAFE_INTEGER for large disks. */
export function formatBytes(value: string): string {
  const bytes = BigInt(value);
  if (bytes === 0n) return "0 B";

  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  let unitIndex = 0;
  let display = Number(bytes);
  let remaining = bytes;
  while (remaining >= 1024n && unitIndex < units.length - 1) {
    remaining /= 1024n;
    unitIndex += 1;
    display = Number(bytes) / 1024 ** unitIndex;
  }
  const precision = unitIndex === 0 ? 0 : 1;
  return `${display.toLocaleString("en-US", {
    minimumFractionDigits: precision,
    maximumFractionDigits: precision,
  })} ${units[unitIndex]}`;
}

export function bytesValue(value: string): bigint {
  return BigInt(value);
}
