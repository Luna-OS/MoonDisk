/**
 * Typed wrappers around every Tauri command and event MoonDisk uses. This
 * is the *only* file that talks to `@tauri-apps/api`: no direct network
 * access and no business logic in the frontend. Components import from
 * here, never from `@tauri-apps/api` directly.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AppInfo, Disk, FlashProgress, ImageInfo, OperationRequest } from "@/types/models";

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
  confirmed?: boolean;
}

export interface OperationOutcome {
  applied: boolean;
}

export function executeOperation(args: ExecuteOperationArgs): Promise<OperationOutcome> {
  return invoke("execute_operation", {
    request: args.request,
    confirmed: args.confirmed ?? false,
  });
}

/** Opens the native file picker. `null` if the user closed it. */
export function selectImageFile(): Promise<ImageInfo | null> {
  return invoke("select_image_file");
}

export interface FlashImageArgs {
  imagePath: string;
  diskId: string;
  verify: boolean;
}

/** Erases the drive and writes the image onto it. Only call this after the
 * user confirmed; resolves once the drive is written (and verified). */
export function flashImage(args: FlashImageArgs): Promise<void> {
  return invoke("flash_image", { ...args, confirmed: true });
}

/** Stops a running `flashImage`, which then rejects with "cancelled". */
export function cancelFlash(): Promise<void> {
  return invoke("flash_cancel");
}

/** Subscribes to write progress; resolves to the unsubscribe function. */
export function onFlashProgress(handler: (progress: FlashProgress) => void): Promise<() => void> {
  return listen<FlashProgress>("flash-progress", (event) => handler(event.payload));
}
