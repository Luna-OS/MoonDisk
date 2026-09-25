#!/usr/bin/env python3
"""Builds the small ISO fixtures in src-tauri/tests/fixtures/ that the USB
writer's file-copy mode is tested against. They mimic real installer
images: an Arch-like one (Rock Ridge + Joliet, UEFI loader in the file
tree, label-based boot config), a Joliet-only one whose label is too long
for FAT32, a plain ISO9660 one without a UEFI loader, and a Windows-like
one whose files only exist in UDF.

Requires pycdlib (`pip install pycdlib`). Run from the repository root:

    python3 scripts/make-test-isos.py
"""

import io
import os
import struct

import pycdlib

OUT = os.path.join("src-tauri", "tests", "fixtures")


def pattern(size, seed):
    """Every 4-byte word is unique, so a read from a wrong offset can't
    accidentally match."""
    words = (size + 3) // 4
    return b"".join(struct.pack("<I", (seed << 24) + i) for i in range(words))[:size]


def add_file(iso, path, data, rr=True, joliet=True, joliet_path=None):
    """`path` is the real (Rock Ridge / Joliet) path, e.g. /EFI/BOOT/BOOTx64.EFI."""
    parts = path.strip("/").split("/")
    iso_path = "/" + "/".join(iso9660_name(p, i == len(parts) - 1) for i, p in enumerate(parts))
    kwargs = {}
    if rr:
        kwargs["rr_name"] = parts[-1]
    if joliet:
        kwargs["joliet_path"] = joliet_path or path
    iso.add_fp(io.BytesIO(data), len(data), iso_path + ";1", **kwargs)


def add_dir(iso, path, rr=True, joliet=True):
    parts = path.strip("/").split("/")
    iso_path = "/" + "/".join(iso9660_name(p, False) for p in parts)
    kwargs = {}
    if rr:
        kwargs["rr_name"] = parts[-1]
    if joliet:
        kwargs["joliet_path"] = path
    iso.add_directory(iso_path, **kwargs)


def iso9660_name(name, is_file):
    """A level-1 ISO9660 name: upper-case 8.3, only d-characters."""
    allowed = set("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_")
    base, dot, ext = name.upper().lstrip(".").partition(".")
    base = "".join(c if c in allowed else "_" for c in base)[:8] or "_"
    ext = "".join(c if c in allowed else "_" for c in ext.replace(".", "_"))[:3]
    if is_file:
        return f"{base}.{ext}" if ext else base
    return base


def archlike():
    iso = pycdlib.PyCdlib()
    iso.new(vol_ident="ARCH_202409", rock_ridge="1.09", joliet=3)
    for d in ["/EFI", "/EFI/BOOT", "/loader", "/loader/entries", "/arch", "/arch/x86_64",
              "/arch/boot", "/arch/boot/x86_64", "/boot", "/.disk"]:
        add_dir(iso, d)
    add_file(iso, "/EFI/BOOT/BOOTx64.EFI", b"MZ efi loader x64 " + pattern(5000, 1))
    add_file(iso, "/EFI/BOOT/BOOTIA32.EFI", b"MZ efi loader ia32 " + pattern(3000, 2))
    add_file(iso, "/loader/loader.conf", b"timeout 15\ndefault 01-archiso-x86_64-linux.conf\n")
    add_file(
        iso,
        "/loader/entries/01-archiso-x86_64-linux.conf",
        b"title Arch Linux install medium\nlinux /arch/boot/x86_64/vmlinuz-linux\n"
        b"options archisobasedir=arch archisolabel=ARCH_202409\n",
    )
    add_file(iso, "/arch/boot/x86_64/vmlinuz-linux", pattern(70000, 3))
    add_file(iso, "/arch/x86_64/airootfs.sfs", pattern(150000, 4))
    add_file(iso, "/boot/2024-09-01-10-00-00-00.uuid", b"")
    add_file(iso, "/.disk/info", b"Arch-like test image\n")
    long_name = "a-rather-long-file-name-that-does-not-fit-into-eight-dot-three-" + "x" * 60 + ".txt"
    # Joliet names stop at 64 characters; xorriso truncates them the same way.
    add_file(iso, "/" + long_name, b"long name\n", joliet_path="/" + long_name[:60] + ".txt")
    iso.add_symlink("/LATEST", rr_symlink_name="latest", rr_path="arch/x86_64",
                    joliet_path="/latest")
    iso.write(os.path.join(OUT, "archlike.iso"))
    iso.close()


def joliet_only():
    iso = pycdlib.PyCdlib()
    iso.new(vol_ident="FEDORA-LIVE-40", joliet=3)
    for d in ["/EFI", "/EFI/BOOT", "/LiveOS"]:
        add_dir(iso, d, rr=False)
    add_file(iso, "/EFI/BOOT/BOOTX64.EFI", b"MZ shim " + pattern(4000, 5), rr=False)
    add_file(
        iso,
        "/EFI/BOOT/grub.cfg",
        b"search --no-floppy --set=root -l 'FEDORA-LIVE-40'\n"
        b"linux /images/pxeboot/vmlinuz root=live:CDLABEL=FEDORA-LIVE-40 rd.live.image\n",
        rr=False,
    )
    add_file(iso, "/LiveOS/squashfs.img", pattern(90000, 6), rr=False)
    iso.write(os.path.join(OUT, "joliet-only.iso"))
    iso.close()


def plain():
    iso = pycdlib.PyCdlib()
    iso.new(vol_ident="PLAIN")
    iso.add_directory("/ISOLINUX")
    iso.add_fp(io.BytesIO(b"default linux\n"), 14, "/ISOLINUX/ISOLINUX.CFG;1")
    iso.write(os.path.join(OUT, "plain.iso"))
    iso.close()


def windows_like():
    iso = pycdlib.PyCdlib()
    iso.new(vol_ident="CCCOMA_X64FRE_EN-US_DV9", udf="2.60")
    readme = b"This disc contains a \"UDF\" file system and requires an operating system\n"
    iso.add_fp(io.BytesIO(readme), len(readme), "/README.TXT;1")
    iso.add_directory(udf_path="/efi")
    iso.add_directory(udf_path="/efi/boot")
    data = b"MZ bootmgr " + pattern(2000, 7)
    iso.add_fp(io.BytesIO(data), len(data), udf_path="/efi/boot/bootx64.efi")
    iso.write(os.path.join(OUT, "windows-like.iso"))
    iso.close()


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    archlike()
    joliet_only()
    plain()
    windows_like()
    for name in sorted(os.listdir(OUT)):
        print(name, os.path.getsize(os.path.join(OUT, name)))
