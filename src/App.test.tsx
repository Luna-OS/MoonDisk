import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { AppInfo, Disk } from "@/types/models";

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

function mockBackend(disks: Disk[], info: AppInfo = { version: "0.0.0-test", platform: "linux" }) {
  mockedInvoke.mockImplementation((cmd) => {
    if (cmd === "app_info") return Promise.resolve(info);
    if (cmd === "disks_list") return Promise.resolve(disks);
    return Promise.reject(new Error(`unexpected invoke: ${String(cmd)}`));
  });
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
});
