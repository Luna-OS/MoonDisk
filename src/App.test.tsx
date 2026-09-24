import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { AppInfo, Disk } from "@/types/models";

const mockedInvoke = vi.mocked(invoke);

const sampleDisk: Disk = {
  id: "mock-disk-0",
  displayName: "Datenträger 0",
  vendor: "Lunaris Storage",
  model: "Lunaris NV-1000",
  serial: "MOCK-0001",
  bus: "nvme",
  media: "ssd",
  size: (500n * 1024n * 1024n * 1024n).toString(),
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
        size: (200n * 1024n * 1024n * 1024n).toString(),
        used: null,
        fs: "ntfs",
        kind: "basicData",
        label: "Daten",
        flags: 0,
        driveLetter: "D",
        mountpoints: [],
      },
    },
  ],
};

function mockBackend(disks: Disk[], info: AppInfo = { version: "0.0.0-test" }) {
  mockedInvoke.mockImplementation((cmd) => {
    if (cmd === "app_info") return Promise.resolve(info);
    if (cmd === "disks_list") return Promise.resolve(disks);
    return Promise.reject(new Error(`unexpected invoke: ${String(cmd)}`));
  });
}

describe("App", () => {
  it("renders the MoonDisk heading and slogan", async () => {
    mockBackend([]);
    render(<App />);

    expect(screen.getByRole("heading", { name: "MoonDisk" })).toBeInTheDocument();
    expect(screen.getByText("Deine Laufwerke. Sicher im Mondlicht.")).toBeInTheDocument();

    // Let the mocked backend calls settle inside act() before the test
    // unmounts, same reasoning as in the Phase 2 scaffold smoke test.
    await screen.findByText("Änderungen wirken auf echte Datenträger. Erstelle vorher ein Backup.");
  });

  it("shows the real-disk warning banner once app info loads", async () => {
    mockBackend([]);
    render(<App />);

    expect(
      await screen.findByText(
        "Änderungen wirken auf echte Datenträger. Erstelle vorher ein Backup.",
      ),
    ).toBeInTheDocument();
  });

  it("lists disks returned by the backend", async () => {
    mockBackend([sampleDisk]);
    render(<App />);

    expect(await screen.findByText(/Datenträger 0/)).toBeInTheDocument();
  });

  it("selecting a disk reveals its partitions", async () => {
    mockBackend([sampleDisk]);
    render(<App />);

    const diskButton = await screen.findByText(/Lunaris NV-1000/);
    fireEvent.click(diskButton);

    expect(await screen.findByText(/Partitionen von Datenträger 0/)).toBeInTheDocument();
  });
});
