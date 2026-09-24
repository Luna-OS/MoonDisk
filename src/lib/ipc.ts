/**
 * Typed wrappers around every Tauri command MoonDisk exposes. This is the
 * *only* file that calls `@tauri-apps/api/core`'s `invoke` — see
 * docs/architecture.md §7 ("Keine direkten Netzwerkzugriffe" /
 * "keine Geschäftslogik im Frontend"). Components import from here, never
 * from `@tauri-apps/api` directly.
 */
import { invoke } from "@tauri-apps/api/core";
import type { AppInfo, Disk, OperationRequest } from "@/types/models";

export function getAppInfo(): Promise<AppInfo> {
  return invoke("app_info");
}

export function listDisks(): Promise<Disk[]> {
  return invoke("disks_list");
}

export function getDisk(diskId: string): Promise<Disk> {
  return invoke("disk_get", { diskId });
}

export interface ExecuteOperationArgs {
  request: OperationRequest;
  confirmationPhrase?: string;
  language: string;
}

export interface OperationOutcome {
  applied: boolean;
}

export function executeOperation(args: ExecuteOperationArgs): Promise<OperationOutcome> {
  return invoke("execute_operation", {
    request: args.request,
    confirmationPhrase: args.confirmationPhrase ?? null,
    language: args.language,
  });
}
