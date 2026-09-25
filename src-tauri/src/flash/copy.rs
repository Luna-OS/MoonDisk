//! File-copy mode, what Rufus calls "ISO mode": instead of copying the
//! image byte for byte, the drive gets an MBR with one FAT32 partition
//! holding the image's files. Windows and macOS can open the drive
//! afterwards, and it boots on UEFI PCs through the image's own EFI boot
//! loader (`EFI/BOOT/BOOTX64.EFI`). Old BIOS-only PCs need raw mode.
//!
//! Linux live systems find their files either by searching for them or by
//! the volume label. FAT32 labels hold only 11 upper-case characters, so
//! when the image's label doesn't fit, the boot configuration files are
//! patched to the shortened label — Rufus does the same.

use super::fat32::{self, FatDateTime, FatNode, Geometry, Plan};
use super::iso9660::{self, Extent, IsoDate, IsoNode, IsoTree};
use super::{check_target, open_raw, platform, FlashError, Phase, RawTarget};
use crate::models::Disk;
use serde::Serialize;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

const MIB: u64 = 1024 * 1024;
const CHUNK: usize = 4 * 1024 * 1024;
/// The partition starts 1 MiB in, like every modern partitioning tool.
const PARTITION_START: u64 = MIB;
/// Config files larger than this aren't searched for the label.
const MAX_PATCHED_FILE: u64 = MIB;

/// Whether an image can be written in file-copy mode, for the UI.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CopyModeInfo {
    pub supported: bool,
    /// Why not, if not.
    pub reason: Option<String>,
    /// The FAT32 volume label the drive would get.
    pub label: Option<String>,
}

/// Where a file's contents come from.
#[derive(Debug)]
enum Source {
    Image(Vec<Extent>),
    /// A boot config file with the volume label patched.
    Patched(Vec<u8>),
}

/// An image's files, ready to be laid out on FAT32.
#[derive(Debug)]
pub struct Prepared {
    tree: Vec<FatNode>,
    sources: Vec<Source>,
    label: String,
    patched: Vec<String>,
}

fn rejected(msg: impl Into<String>) -> FlashError {
    FlashError::Rejected(msg.into())
}

fn fat_date(d: IsoDate) -> FatDateTime {
    FatDateTime::new(d.year, d.month, d.day, d.hour, d.minute, d.second)
}

/// Whether the tree contains a UEFI boot loader, `EFI/BOOT/BOOT*.EFI`.
fn has_uefi_loader(children: &[IsoNode]) -> bool {
    let find = |nodes: &'_ [IsoNode], name: &str| -> Option<Vec<IsoNode>> {
        nodes.iter().find_map(|n| match n {
            IsoNode::Dir {
                name: n, children, ..
            } if n.eq_ignore_ascii_case(name) => Some(children.clone()),
            _ => None,
        })
    };
    let Some(efi) = find(children, "efi") else {
        return false;
    };
    let Some(boot) = find(&efi, "boot") else {
        return false;
    };
    boot.iter().any(|n| {
        let lower = n.name().to_ascii_lowercase();
        matches!(n, IsoNode::File { .. }) && lower.starts_with("boot") && lower.ends_with(".efi")
    })
}

fn largest_file(nodes: &[IsoNode]) -> Option<(&str, u64)> {
    nodes
        .iter()
        .filter_map(|n| match n {
            IsoNode::File { name, size, .. } => Some((name.as_str(), *size)),
            IsoNode::Dir { children, .. } => largest_file(children),
        })
        .max_by_key(|(_, size)| *size)
}

fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

/// Checks that a file copy of `tree` can boot and fit on FAT32.
fn check_tree(tree: &IsoTree) -> Result<(), FlashError> {
    if !has_uefi_loader(&tree.children) {
        return Err(if tree.has_udf {
            rejected(
                "Windows installer images can't be copied file by file yet — \
                 use Microsoft's Media Creation Tool for them",
            )
        } else {
            rejected(
                "the image has no UEFI boot loader among its files, so a copy wouldn't \
                 boot — write it as a raw image instead",
            )
        });
    }
    if let Some((name, size)) = largest_file(&tree.children) {
        if size > fat32::MAX_FILE_SIZE {
            return Err(rejected(format!(
                "{name} is {}, more than the 4 GB a FAT32 file can hold — write the image \
                 as a raw image instead",
                gb(size)
            )));
        }
    }
    Ok(())
}

pub fn analyze(path: &Path) -> CopyModeInfo {
    let result = File::open(path)
        .map_err(FlashError::from)
        .and_then(|mut f| iso9660::read_tree(&mut f).map_err(|_| rejected("")))
        .and_then(|tree| {
            check_tree(&tree)?;
            Ok(fat32::volume_label_text(&tree.volume_label))
        });
    match result {
        Ok(label) => CopyModeInfo {
            supported: true,
            reason: None,
            label: Some(label),
        },
        Err(FlashError::Rejected(reason)) if !reason.is_empty() => CopyModeInfo {
            supported: false,
            reason: Some(reason),
            label: None,
        },
        Err(_) => CopyModeInfo {
            supported: false,
            reason: Some("only ISO images can be copied file by file".into()),
            label: None,
        },
    }
}

fn is_boot_config(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".cfg") || lower.ends_with(".conf")
}

/// Reads the image's tree and turns it into what the FAT32 builder takes.
pub fn prepare<R: Read + Seek>(image: &mut R) -> Result<Prepared, FlashError> {
    let tree = iso9660::read_tree(image)?;
    check_tree(&tree)?;
    let label = fat32::volume_label_text(&tree.volume_label);
    let old_label = tree.volume_label.trim().to_string();
    let patch = !old_label.is_empty() && old_label != label;

    let mut prepared = Prepared {
        tree: Vec::new(),
        sources: Vec::new(),
        label,
        patched: Vec::new(),
    };
    prepared.tree = convert(
        &tree.children,
        "",
        image,
        patch.then_some(&old_label),
        &mut prepared,
    )?;
    Ok(prepared)
}

fn convert<R: Read + Seek>(
    nodes: &[IsoNode],
    dir: &str,
    image: &mut R,
    old_label: Option<&String>,
    out: &mut Prepared,
) -> Result<Vec<FatNode>, FlashError> {
    let mut result = Vec::new();
    for node in nodes {
        match node {
            IsoNode::Dir {
                name,
                date,
                children,
            } => {
                let path = format!("{dir}/{name}");
                result.push(FatNode::Dir {
                    name: name.clone(),
                    date: fat_date(*date),
                    children: convert(children, &path, image, old_label, out)?,
                });
            }
            IsoNode::File {
                name,
                date,
                size,
                extents,
            } => {
                let mut source = Source::Image(extents.clone());
                if let Some(old) = old_label {
                    if is_boot_config(name) && *size <= MAX_PATCHED_FILE {
                        let mut text = Vec::new();
                        SourceReader::new(image, &source).read_to_end(&mut text)?;
                        if let Some(patched) = replace_label(&text, old, &out.label) {
                            out.patched.push(format!("{dir}/{name}"));
                            source = Source::Patched(patched);
                        }
                    }
                }
                let size = match &source {
                    Source::Patched(bytes) => bytes.len() as u64,
                    Source::Image(_) => *size,
                };
                result.push(FatNode::File {
                    name: name.clone(),
                    date: fat_date(*date),
                    size,
                    id: out.sources.len(),
                });
                out.sources.push(source);
            }
        }
    }
    Ok(result)
}

/// Replaces `old` (also in its `\x20`-escaped form, which GRUB configs
/// use for spaces) with `new`. `None` if `old` doesn't occur.
fn replace_label(text: &[u8], old: &str, new: &str) -> Option<Vec<u8>> {
    let mut out = text.to_vec();
    let mut changed = false;
    for (from, to) in [
        (old.replace(' ', "\\x20"), new.replace(' ', "\\x20")),
        (old.to_string(), new.to_string()),
    ] {
        if from.is_empty() {
            continue;
        }
        let (from, to) = (from.as_bytes(), to.as_bytes());
        let mut next = Vec::with_capacity(out.len());
        let mut i = 0;
        while i < out.len() {
            if out[i..].starts_with(from) {
                next.extend_from_slice(to);
                i += from.len();
                changed = true;
            } else {
                next.push(out[i]);
                i += 1;
            }
        }
        out = next;
    }
    changed.then_some(out)
}

/// Reads a file's contents from wherever they come from.
struct SourceReader<'a, R> {
    image: &'a mut R,
    source: &'a Source,
    extent: usize,
    pos: u64,
}

impl<'a, R: Read + Seek> SourceReader<'a, R> {
    fn new(image: &'a mut R, source: &'a Source) -> Self {
        SourceReader {
            image,
            source,
            extent: 0,
            pos: 0,
        }
    }
}

impl<R: Read + Seek> Read for SourceReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.source {
            Source::Patched(bytes) => {
                let rest = &bytes[self.pos as usize..];
                let n = rest.len().min(buf.len());
                buf[..n].copy_from_slice(&rest[..n]);
                self.pos += n as u64;
                Ok(n)
            }
            Source::Image(extents) => loop {
                let Some(e) = extents.get(self.extent) else {
                    return Ok(0);
                };
                if self.pos >= e.len {
                    self.extent += 1;
                    self.pos = 0;
                    continue;
                }
                let n = ((e.len - self.pos) as usize).min(buf.len());
                self.image.seek(SeekFrom::Start(e.offset + self.pos))?;
                self.image.read_exact(&mut buf[..n])?;
                self.pos += n as u64;
                return Ok(n);
            },
        }
    }
}

/// The MBR: one active FAT32 (LBA) partition, type 0x0C.
fn mbr(sector: usize, start_lba: u32, sectors: u32, disk_signature: u32) -> Vec<u8> {
    let mut m = vec![0u8; sector];
    m[440..444].copy_from_slice(&disk_signature.to_le_bytes());
    let e = 446;
    m[e] = 0x80;
    m[e + 1..e + 4].copy_from_slice(&[0xFE, 0xFF, 0xFF]); // CHS unused: LBA only
    m[e + 4] = 0x0C;
    m[e + 5..e + 8].copy_from_slice(&[0xFE, 0xFF, 0xFF]);
    m[e + 8..e + 12].copy_from_slice(&start_lba.to_le_bytes());
    m[e + 12..e + 16].copy_from_slice(&sectors.to_le_bytes());
    m[510] = 0x55;
    m[511] = 0xAA;
    m
}

fn random_u32() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    (nanos ^ (nanos >> 32)) as u32 | 1
}

/// Lays the prepared files out on a drive of `disk_size` bytes.
pub fn plan(prepared: &Prepared, disk_size: u64, sector: u32) -> Result<Plan, FlashError> {
    // MBR sector numbers are 32 bits.
    let end = (disk_size / MIB * MIB).min(u32::MAX as u64 * sector as u64 / MIB * MIB);
    let partition = end.saturating_sub(PARTITION_START);
    let geometry = Geometry::plan(partition, sector).map_err(rejected)?;
    Plan::new(geometry, &prepared.label, &prepared.tree).map_err(rejected)
}

fn zero_range<T: Write + Seek>(target: &mut T, start: u64, len: u64) -> io::Result<()> {
    target.seek(SeekFrom::Start(start))?;
    let zeros = vec![0u8; len as usize];
    target.write_all(&zeros)
}

/// Writes the partition table and file system onto `target`, then
/// optionally reads every file back. `drop_read_cache` runs before that.
#[allow(clippy::too_many_arguments)]
pub fn write_to<R: Read + Seek, T: RawTarget>(
    image: &mut R,
    prepared: &Prepared,
    plan: &Plan,
    target: &mut T,
    disk_size: u64,
    verify: bool,
    mut drop_read_cache: impl FnMut() -> Result<(), FlashError>,
    cancel: &AtomicBool,
    report: &mut impl FnMut(Phase, u64, u64),
) -> Result<(), FlashError> {
    let sector = plan.geometry.bytes_per_sector as u64;
    let total = plan.content_bytes();

    // Clear the old partition table and anything a previous image left
    // at either end (an ISO9660 header, a backup GPT). The new table is
    // written last, so the OS only sees the drive once it's complete.
    zero_range(target, 0, PARTITION_START)?;
    let tail = (disk_size / sector * sector)
        .saturating_sub(MIB)
        .max(PARTITION_START);
    zero_range(target, tail, disk_size / sector * sector - tail)?;

    target.seek(SeekFrom::Start(PARTITION_START))?;
    let start_lba = (PARTITION_START / sector) as u32;
    let mut done = 0u64;
    let mut buf = vec![0u8; CHUNK];
    let written = plan.write(target, start_lba, random_u32(), |id, _size, out| {
        let mut reader = SourceReader::new(image, &prepared.sources[id]);
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
            }
            let n = reader.read(&mut buf)?;
            if n == 0 {
                return Ok(());
            }
            out.write_all(&buf[..n])?;
            done += n as u64;
            report(Phase::Writing, done, total);
        }
    });
    if cancel.load(Ordering::Relaxed) {
        return Err(FlashError::Cancelled);
    }
    written?;
    target.flush_to_device()?;

    target.seek(SeekFrom::Start(0))?;
    target.write_all(&mbr(
        sector as usize,
        start_lba,
        plan.geometry.total_sectors,
        random_u32(),
    ))?;
    target.flush_to_device()?;
    report(Phase::Writing, total, total);

    if verify {
        drop_read_cache()?;
        verify_files(image, prepared, plan, target, cancel, |d| {
            report(Phase::Verifying, d, total)
        })?;
    }
    Ok(())
}

fn verify_files<R: Read + Seek, T: RawTarget>(
    image: &mut R,
    prepared: &Prepared,
    plan: &Plan,
    target: &mut T,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<(), FlashError> {
    let mut expected = vec![0u8; CHUNK];
    let mut actual = vec![0u8; CHUNK];
    let mut done = 0u64;
    let sector = plan.geometry.bytes_per_sector as usize;
    for p in &plan.placements {
        let mut reader = SourceReader::new(image, &prepared.sources[p.id]);
        let start = PARTITION_START + p.offset;
        target.seek(SeekFrom::Start(start))?;
        let mut pos = 0u64;
        while pos < p.size {
            if cancel.load(Ordering::Relaxed) {
                return Err(FlashError::Cancelled);
            }
            let n = (p.size - pos).min(CHUNK as u64) as usize;
            reader.read_exact(&mut expected[..n])?;
            // Raw devices only read whole sectors; the file's last cluster
            // is padded, so rounding up stays inside it.
            let padded = n.next_multiple_of(sector);
            target.read_exact(&mut actual[..padded])?;
            if let Some(i) = expected[..n]
                .iter()
                .zip(&actual[..n])
                .position(|(a, b)| a != b)
            {
                return Err(FlashError::VerifyMismatch(start + pos + i as u64));
            }
            pos += n as u64;
            done += n as u64;
            progress(done);
        }
    }
    Ok(())
}

/// Copies the files of the ISO at `image_path` onto `disk`.
pub fn run(
    image_path: &Path,
    disk: &Disk,
    verify: bool,
    cancel: &AtomicBool,
    mut report: impl FnMut(Phase, u64, u64),
) -> Result<(), FlashError> {
    let mut image = File::open(image_path)?;
    let prepared = prepare(&mut image)?;
    let sector = if disk.logical_sector_size == 4096 {
        4096
    } else {
        512
    };
    let plan = plan(&prepared, disk.size.0, sector)?;
    check_target(disk, plan.content_bytes().max(1))?;
    let total = plan.content_bytes();

    report(Phase::Preparing, 0, total);
    platform::prepare_raw_write(disk)?;
    let result = open_raw(disk)
        .map_err(FlashError::from)
        .and_then(|mut target| {
            write_to(
                &mut image,
                &prepared,
                &plan,
                &mut target,
                disk.size.0,
                verify,
                || platform::drop_read_cache(disk).map_err(FlashError::from),
                cancel,
                &mut report,
            )
        });
    report(Phase::Finishing, total, total);
    platform::finish_raw_write(disk);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read(path).unwrap()
    }

    fn fixture_path(name: &str) -> std::path::PathBuf {
        format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR")).into()
    }

    #[test]
    fn supports_linux_images_with_a_uefi_loader() {
        let info = analyze(&fixture_path("archlike.iso"));
        assert_eq!(
            info,
            CopyModeInfo {
                supported: true,
                reason: None,
                label: Some("ARCH_202409".into())
            }
        );
    }

    #[test]
    fn explains_why_other_images_are_not_supported() {
        let windows = analyze(&fixture_path("windows-like.iso"));
        assert!(!windows.supported);
        assert!(windows.reason.unwrap().contains("Windows"));

        let no_loader = analyze(&fixture_path("plain.iso"));
        assert!(no_loader.reason.unwrap().contains("UEFI"));

        let not_iso =
            std::env::temp_dir().join(format!("moondisk-not-iso-{}.img", std::process::id()));
        std::fs::write(&not_iso, vec![0u8; 100_000]).unwrap();
        let raw = analyze(&not_iso);
        std::fs::remove_file(&not_iso).unwrap();
        assert!(raw.reason.unwrap().contains("only ISO images"));
    }

    #[test]
    fn replaces_the_label_in_plain_and_escaped_form() {
        let text = b"search -l 'My Distro 40'\nroot=live:CDLABEL=My\\x20Distro\\x2040 quiet\n";
        let patched = replace_label(text, "My Distro 40", "MY DISTRO 4").unwrap();
        assert_eq!(
            patched,
            b"search -l 'MY DISTRO 4'\nroot=live:CDLABEL=MY\\x20DISTRO\\x204 quiet\n"
        );
        assert!(replace_label(b"nothing here", "My Distro 40", "X").is_none());
    }

    struct Written {
        drive: Vec<u8>,
        prepared: Prepared,
    }

    fn write_fixture(name: &str, disk_size: u64) -> Written {
        let image = fixture(name);
        let mut reader = Cursor::new(image);
        let prepared = prepare(&mut reader).unwrap();
        let plan = plan(&prepared, disk_size, 512).unwrap();
        let mut drive = Cursor::new(vec![0xEEu8; disk_size as usize]);
        let cancel = AtomicBool::new(false);
        write_to(
            &mut reader,
            &prepared,
            &plan,
            &mut drive,
            disk_size,
            true,
            || Ok(()),
            &cancel,
            &mut |_, _, _| {},
        )
        .unwrap();
        Written {
            drive: drive.into_inner(),
            prepared,
        }
    }

    fn partition(drive: &[u8]) -> fatfs::FileSystem<Cursor<Vec<u8>>> {
        let start = u32::from_le_bytes(drive[454..458].try_into().unwrap()) as usize * 512;
        let sectors = u32::from_le_bytes(drive[458..462].try_into().unwrap()) as usize;
        let bytes = drive[start..start + sectors * 512].to_vec();
        fatfs::FileSystem::new(Cursor::new(bytes), fatfs::FsOptions::new()).unwrap()
    }

    fn read(fs: &fatfs::FileSystem<Cursor<Vec<u8>>>, path: &str) -> Vec<u8> {
        let mut v = Vec::new();
        fs.root_dir()
            .open_file(path)
            .unwrap()
            .read_to_end(&mut v)
            .unwrap();
        v
    }

    fn iso_file(image: &[u8], path: &str) -> Vec<u8> {
        let tree = iso9660::read_tree(&mut Cursor::new(image)).unwrap();
        let mut nodes = &tree.children;
        let parts: Vec<&str> = path.split('/').collect();
        for (i, part) in parts.iter().enumerate() {
            let node = nodes.iter().find(|n| n.name() == *part).unwrap();
            match node {
                IsoNode::Dir { children, .. } => nodes = children,
                IsoNode::File { extents, .. } => {
                    assert_eq!(i, parts.len() - 1);
                    return extents
                        .iter()
                        .flat_map(|e| {
                            image[e.offset as usize..(e.offset + e.len) as usize].to_vec()
                        })
                        .collect();
                }
            }
        }
        panic!("{path} is a directory")
    }

    const DISK: u64 = 100 * MIB;

    #[test]
    fn copies_an_arch_style_image_onto_a_bootable_fat32_partition() {
        let w = write_fixture("archlike.iso", DISK);
        let image = fixture("archlike.iso");

        // MBR: one active FAT32 LBA partition at 1 MiB.
        assert_eq!(&w.drive[510..512], &[0x55, 0xAA]);
        assert_eq!(w.drive[446], 0x80);
        assert_eq!(w.drive[450], 0x0C);
        assert_eq!(
            u32::from_le_bytes(w.drive[454..458].try_into().unwrap()),
            2048
        );
        // Both ends cleared of what was there before.
        assert!(w.drive[512..MIB as usize].iter().all(|&b| b == 0));
        assert!(w.drive[(DISK - MIB) as usize..].iter().all(|&b| b == 0));

        let fs = partition(&w.drive);
        assert_eq!(fs.volume_label(), "ARCH_202409");
        for path in [
            "EFI/BOOT/BOOTx64.EFI",
            "EFI/BOOT/BOOTIA32.EFI",
            "loader/entries/01-archiso-x86_64-linux.conf",
            "arch/x86_64/airootfs.sfs",
            "arch/boot/x86_64/vmlinuz-linux",
            ".disk/info",
        ] {
            assert_eq!(read(&fs, path), iso_file(&image, path), "{path}");
        }
        assert!(read(&fs, "boot/2024-09-01-10-00-00-00.uuid").is_empty());
        // The label fits, so nothing needed patching.
        assert!(w.prepared.patched.is_empty());
    }

    #[test]
    fn patches_boot_configs_when_the_label_is_too_long() {
        let w = write_fixture("joliet-only.iso", DISK);
        let fs = partition(&w.drive);
        assert_eq!(fs.volume_label(), "FEDORA-LIVE");
        assert_eq!(w.prepared.patched, ["/EFI/BOOT/grub.cfg"]);
        let cfg = String::from_utf8(read(&fs, "EFI/BOOT/grub.cfg")).unwrap();
        assert!(cfg.contains("-l 'FEDORA-LIVE'"), "{cfg}");
        assert!(cfg.contains("CDLABEL=FEDORA-LIVE rd.live.image"), "{cfg}");
        assert!(!cfg.contains("FEDORA-LIVE-40"), "{cfg}");
    }

    /// A stick that returns one wrong byte when read back.
    struct Faulty {
        inner: Cursor<Vec<u8>>,
        bad: u64,
    }

    impl Read for Faulty {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let at = self.inner.position();
            let n = self.inner.read(buf)?;
            if (at..at + n as u64).contains(&self.bad) {
                buf[(self.bad - at) as usize] ^= 0xFF;
            }
            Ok(n)
        }
    }

    impl Write for Faulty {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.inner.write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Seek for Faulty {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    impl RawTarget for Faulty {
        fn flush_to_device(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn verification_catches_a_wrong_byte() {
        let mut reader = Cursor::new(fixture("archlike.iso"));
        let prepared = prepare(&mut reader).unwrap();
        let plan = plan(&prepared, DISK, 512).unwrap();
        let bad = PARTITION_START + plan.placements[2].offset + 7;
        let mut drive = Faulty {
            inner: Cursor::new(vec![0u8; DISK as usize]),
            bad,
        };
        let cancel = AtomicBool::new(false);
        let result = write_to(
            &mut reader,
            &prepared,
            &plan,
            &mut drive,
            DISK,
            true,
            || Ok(()),
            &cancel,
            &mut |_, _, _| {},
        );
        assert!(
            matches!(result, Err(FlashError::VerifyMismatch(at)) if at == bad),
            "{result:?}"
        );
    }

    #[test]
    fn a_cancelled_copy_leaves_no_partition_table() {
        let image = fixture("archlike.iso");
        let mut reader = Cursor::new(image);
        let prepared = prepare(&mut reader).unwrap();
        let plan = plan(&prepared, DISK, 512).unwrap();
        let mut drive = Cursor::new(vec![0xEEu8; DISK as usize]);
        let cancel = AtomicBool::new(false);
        let result = write_to(
            &mut reader,
            &prepared,
            &plan,
            &mut drive,
            DISK,
            false,
            || Ok(()),
            &cancel,
            &mut |_, done, _| {
                if done > 0 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
        );
        assert!(matches!(result, Err(FlashError::Cancelled)), "{result:?}");
        assert!(drive.get_ref()[..512].iter().all(|&b| b == 0));
    }

    #[test]
    fn refuses_a_drive_that_is_too_small() {
        let mut reader = Cursor::new(fixture("archlike.iso"));
        let prepared = prepare(&mut reader).unwrap();
        assert!(matches!(
            plan(&prepared, 20 * MIB, 512),
            Err(FlashError::Rejected(_))
        ));
    }
}
