import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { AppInfo, Disk, ImageInfo } from "@/types/models";

const mockedInvoke = vi.mocked(invoke);

const GIB = 1024n * 1024n * 1024n;

const sampleDisk: Disk = {
  id: "mock-disk-0",
  displayName: "Disk 0",
  vendor: "Lunaris Storage",
  model: "Lunaris NV-1000",
  serial: "MOCK-0001",
  bus: "nvme",
  media: "ssd",
  size: (500n * GIB).toString(),
  logicalSectorSize: 512,
  table: "gpt",
  health: "ok",
  readOnly: false,
  isSystemDisk: false,
  layout: [
    {
      kind: "partition",
      value: {
        id: "mock-disk-0#1",
        number: 1,
        start: "1048576",
        size: (200n * GIB).toString(),
        used: null,
        fs: "ntfs",
        kind: "basicData",
        label: "Data",
        flags: 0,
        driveLetter: "D",
        mountpoints: [],
      },
    },
    {
      kind: "unallocated",
      value: { start: (1048576n + 200n * GIB).toString(), size: (100n * GIB).toString() },
    },
  ],
};

const usbDisk: Disk = {
  ...sampleDisk,
  id: "mock-usb-1",
  displayName: "Disk 1",
  vendor: "Lunaris",
  model: "Lunaris Stick",
  serial: null,
  bus: "usb",
  size: (8n * GIB).toString(),
  table: "mbr",
  layout: [],
};

function image(overrides: Partial<ImageInfo> = {}): ImageInfo {
  return {
    path: "/home/me/linux.iso",
    name: "linux.iso",
    size: (2n * GIB).toString(),
    hasBootSector: true,
    copyMode: { supported: true, reason: null, label: "ARCH_202409" },
    ...overrides,
  };
}

const WINDOWS_ISO = image({
  name: "Win11.iso",
  hasBootSector: false,
  copyMode: {
    supported: false,
    reason: "Windows installer images can't be copied file by file yet",
    label: null,
  },
});

function mockBackend(
  disks: Disk[],
  info: AppInfo = { version: "0.0.0-test", platform: "linux" },
  extra: Record<string, () => Promise<unknown>> = {},
) {
  mockedInvoke.mockImplementation((cmd) => {
    if (cmd === "app_info") return Promise.resolve(info);
    if (cmd === "disks_list") return Promise.resolve(disks);
    if (cmd in extra) return extra[cmd]();
    return Promise.reject(new Error(`unexpected invoke: ${String(cmd)}`));
  });
}

async function openUsbWriter() {
  render(<App />);
  await screen.findByText(WARNING);
  fireEvent.click(screen.getByRole("tab", { name: "USB writer" }));
}

const WARNING = "Changes are applied to real disks. Make a backup first.";

describe("App", () => {
  it("renders the MoonDisk heading and slogan", async () => {
    mockBackend([]);
    render(<App />);

    expect(screen.getByRole("heading", { name: "MoonDisk" })).toBeInTheDocument();
    expect(screen.getByText("Your disks, safely under the moon.")).toBeInTheDocument();

    // Let the mocked backend calls settle inside act() before the test
    // unmounts.
    await screen.findByText(WARNING);
  });

  it("shows the real-disk warning banner once app info loads", async () => {
    mockBackend([]);
    render(<App />);

    expect(await screen.findByText(WARNING)).toBeInTheDocument();
  });

  it("lists disks returned by the backend", async () => {
    mockBackend([sampleDisk]);
    render(<App />);

    expect(await screen.findByText("Disk 0")).toBeInTheDocument();
    expect(screen.getByText("Lunaris NV-1000")).toBeInTheDocument();
  });

  it("selecting a disk reveals its partitions", async () => {
    mockBackend([sampleDisk]);
    render(<App />);

    fireEvent.click(await screen.findByText("Lunaris NV-1000"));

    expect(await screen.findByRole("heading", { name: "Disk 0" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /D: Data/ })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /Data/ })).toBeInTheDocument();
  });

  it("offers a drive letter when creating a partition on Windows", async () => {
    mockBackend([sampleDisk], { version: "0.0.0-test", platform: "windows" });
    render(<App />);

    fireEvent.click(await screen.findByText("Lunaris NV-1000"));
    fireEvent.click(await screen.findByRole("button", { name: /Unallocated/ }));

    expect(screen.getByRole("button", { name: "Create partition" })).toBeEnabled();
    expect(screen.getByLabelText("Drive letter")).toBeInTheDocument();
  });

  it("does not offer drive letters on Linux", async () => {
    mockBackend([sampleDisk]);
    render(<App />);

    fireEvent.click(await screen.findByText("Lunaris NV-1000"));
    fireEvent.click(await screen.findByRole("button", { name: /Unallocated/ }));

    expect(screen.getByRole("button", { name: "Create partition" })).toBeEnabled();
    expect(screen.queryByLabelText("Drive letter")).not.toBeInTheDocument();
  });

  it("offers the file systems macOS can create", async () => {
    mockBackend([sampleDisk], { version: "0.0.0-test", platform: "macos" });
    render(<App />);

    fireEvent.click(await screen.findByText("Lunaris NV-1000"));
    fireEvent.click(await screen.findByRole("button", { name: /Unallocated/ }));

    const fs = screen.getByRole("combobox", { name: "File system" });
    expect(within(fs).getByRole("option", { name: "APFS" })).toBeInTheDocument();
    expect(within(fs).getByRole("option", { name: "Mac OS Extended" })).toBeInTheDocument();
    expect(within(fs).queryByRole("option", { name: "NTFS" })).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Drive letter")).not.toBeInTheDocument();
  });

  it("USB writer only offers USB drives", async () => {
    mockBackend([sampleDisk, usbDisk]);
    await openUsbWriter();

    const drives = screen.getByRole("radiogroup", { name: "USB drive" });
    expect(within(drives).getByRole("radio", { name: /Disk 1/ })).toBeEnabled();
    expect(within(drives).queryByRole("radio", { name: /Disk 0/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Write image" })).toBeDisabled();
  });

  it("warns when an image has no boot sector", async () => {
    mockBackend([usbDisk], undefined, {
      select_image_file: () => Promise.resolve(WINDOWS_ISO),
    });
    await openUsbWriter();

    fireEvent.click(screen.getByRole("button", { name: "Choose image" }));

    expect(await screen.findByText("Win11.iso")).toBeInTheDocument();
    expect(screen.getByText(/has no boot sector/)).toBeInTheDocument();
  });

  it("does not offer drives smaller than the image", async () => {
    mockBackend([usbDisk], undefined, {
      select_image_file: () => Promise.resolve(image({ size: (16n * GIB).toString() })),
    });
    await openUsbWriter();

    fireEvent.click(screen.getByRole("button", { name: "Choose image" }));

    expect(await screen.findByText("Too small")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /Disk 1/ })).toBeDisabled();
  });

  it("writes an image to the chosen drive after confirmation", async () => {
    mockBackend([usbDisk], undefined, {
      select_image_file: () => Promise.resolve(image()),
      flash_image: () => Promise.resolve(null),
    });
    await openUsbWriter();

    fireEvent.click(screen.getByRole("button", { name: "Choose image" }));
    fireEvent.click(await screen.findByRole("radio", { name: /Disk 1/ }));
    fireEvent.click(screen.getByRole("button", { name: "Write image" }));
    fireEvent.click(screen.getByRole("button", { name: "Erase and write" }));

    expect(await screen.findByText("All done")).toBeInTheDocument();
    expect(mockedInvoke).toHaveBeenCalledWith("flash_image", {
      imagePath: "/home/me/linux.iso",
      diskId: "mock-usb-1",
      mode: "copy",
      verify: true,
      confirmed: true,
    });
  });

  it("restores a USB drive to one empty partition", async () => {
    mockBackend([sampleDisk, usbDisk], undefined, {
      execute_operation: () => Promise.resolve({ applied: true }),
    });
    await openUsbWriter();

    fireEvent.click(screen.getByRole("button", { name: "Restore" }));
    expect(screen.getByRole("heading", { name: "Restore a USB drive" })).toBeInTheDocument();
    // Only USB drives, and nothing happens before one is chosen.
    expect(screen.queryByRole("radio", { name: /Disk 0/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Restore drive" })).toBeDisabled();

    fireEvent.click(screen.getByRole("radio", { name: /Disk 1/ }));
    fireEvent.click(screen.getByRole("button", { name: "Restore drive" }));
    fireEvent.click(screen.getByRole("button", { name: "Erase and restore" }));

    expect(await screen.findByText("Drive restored")).toBeInTheDocument();
    expect(mockedInvoke).toHaveBeenCalledWith("execute_operation", {
      request: { type: "eraseDisk", disk: "mock-usb-1", filesystem: "exFat", label: "USB" },
      confirmed: true,
    });
  });

  it("copies files like Rufus by default and says when it can't", async () => {
    let pick = image();
    mockBackend([usbDisk], undefined, {
      select_image_file: () => Promise.resolve(pick),
    });
    await openUsbWriter();

    fireEvent.click(screen.getByRole("button", { name: "Choose image" }));
    const copy = await screen.findByRole("radio", { name: /Copy files/ });
    await waitFor(() => expect(copy).toBeChecked());
    expect(screen.getByRole("radio", { name: /Raw image/ })).not.toBeChecked();

    pick = WINDOWS_ISO;
    fireEvent.click(screen.getByRole("button", { name: "Choose another" }));
    await waitFor(() => expect(screen.getByRole("radio", { name: /Raw image/ })).toBeChecked());
    expect(screen.getByRole("radio", { name: /Copy files/ })).toBeDisabled();
    expect(screen.getByText(/Windows installer images can't be copied/)).toBeInTheDocument();
  });
});
