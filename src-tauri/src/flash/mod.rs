//! Writing a disk image (e.g. an OS installer ISO) byte for byte onto a
//! whole USB drive — what balenaEtcher or Rufus' "DD mode" do.
//!
//! The copy/verify loop is platform-independent and works on any
//! [`RawTarget`], so it is unit-tested against in-memory buffers. Getting
//! the device ready (unmounting / clearing it) and making the OS re-read
//! the new partition table afterwards are platform-specific and live in
//! the Linux/Windows executor modules.

use crate::models::{BusType, Disk};
use crate::operations::ExecutionError;
use serde::Serialize;
use std::fs::File;
#[cfg(not(target_os = "macos"))]
use std::fs::OpenOptions;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

/// Bytes per read/write; a multiple of every real sector size.
const CHUNK: usize = 4 * 1024 * 1024;
/// Raw device I/O on Windows must be a whole number of sectors. 4 KiB
/// covers both 512-byte and 4Kn drives.
const SECTOR: u64 = 4096;
/// Flush to the device this often, so progress reflects what actually
/// reached the drive rather than the OS write cache.
const SYNC_EVERY: u64 = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum FlashError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("cancelled")]
    Cancelled,
    #[error("verification failed: the drive differs from the image at byte {0}")]
    VerifyMismatch(u64),
    #[error("{0}")]
    Rejected(String),
    #[error("{0}")]
    Platform(#[from] ExecutionError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Preparing,
    Writing,
    Verifying,
    Finishing,
}

/// A whole disk opened for raw access.
pub trait RawTarget: Read + Write + Seek {
    /// Make sure everything written so far has reached the device.
    fn flush_to_device(&mut self) -> io::Result<()>;
}

impl RawTarget for File {
    #[cfg(not(target_os = "macos"))]
    fn flush_to_device(&mut self) -> io::Result<()> {
        self.sync_all()
    }

    /// Writes to macOS' raw `/dev/rdiskN` bypass the buffer cache, and the
    /// `F_FULLFSYNC` behind `sync_all` isn't supported on device nodes.
    #[cfg(target_os = "macos")]
    fn flush_to_device(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Opens the whole disk for raw reading and writing.
#[cfg(not(target_os = "macos"))]
fn open_raw(disk: &Disk) -> io::Result<File> {
    OpenOptions::new().read(true).write(true).open(&disk.id.0)
}

#[cfg(target_os = "macos")]
use platform::open_raw;

fn padded_len(len: u64) -> u64 {
    len.div_ceil(SECTOR) * SECTOR
}

fn gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
}

/// What the UI needs to know about a picked image file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub path: String,
    pub name: String,
    pub size: crate::models::ByteSize,
    /// Whether the image starts with an MBR boot signature (0x55AA at byte
    /// 510). "Hybrid" ISOs made to be written to USB (most Linux distros)
    /// and raw `.img` disk images have one; Windows installer ISOs don't,
    /// and won't boot when written raw.
    pub has_boot_sector: bool,
}

pub fn image_info(path: &Path) -> io::Result<ImageInfo> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut head = [0u8; 512];
    let has_boot_sector = file.read_exact(&mut head).is_ok() && head[510..512] == [0x55, 0xAA];
    Ok(ImageInfo {
        path: path.to_string_lossy().into_owned(),
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        size: crate::models::ByteSize(size),
        has_boot_sector,
    })
}

/// Why `disk` can't take an image of `image_len` bytes, if it can't.
pub fn check_target(disk: &Disk, image_len: u64) -> Result<(), FlashError> {
    let reject = |msg: String| Err(FlashError::Rejected(msg));
    if disk.bus != BusType::Usb {
        return reject("images can only be written to USB drives".into());
    }
    if disk.read_only {
        return reject("the drive is read-only".into());
    }
    if image_len == 0 {
        return reject("the image file is empty".into());
    }
    if padded_len(image_len) > disk.size.0 {
        return reject(format!(
            "the image ({}) is larger than the drive ({})",
            gib(image_len),
            gib(disk.size.0)
        ));
    }
    Ok(())
}

/// Copies `image_len` bytes of `image` onto the start of `target`. The last
/// chunk is zero-padded to a whole sector.
///
/// The first chunk, which holds the image's partition table, is cleared
/// up front and written last. While the rest is copied, the drive has no
/// partition table, so the OS can't pick up the image's partitions
/// half-written and mount them mid-copy. Windows did that, and was left
/// showing stale volumes afterwards. A cancelled write leaves a drive
/// without a partition table, not one that looks bootable but isn't.
pub fn write_image<S: Read, T: RawTarget>(
    image: &mut S,
    image_len: u64,
    target: &mut T,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<(), FlashError> {
    let head_len = image_len.min(CHUNK as u64);
    let mut head = vec![0u8; padded_len(head_len) as usize];
    target.seek(SeekFrom::Start(0))?;
    target.write_all(&head)?;
    image.read_exact(&mut head[..head_len as usize])?;

    let mut buf = vec![0u8; CHUNK];
    let mut offset = head_len;
    let mut unsynced = 0u64;
    while offset < image_len {
        if cancel.load(Ordering::Relaxed) {
            return Err(FlashError::Cancelled);
        }
        let n = (image_len - offset).min(CHUNK as u64) as usize;
        image.read_exact(&mut buf[..n])?;
        let padded = padded_len(n as u64) as usize;
        buf[n..padded].fill(0);
        target.write_all(&buf[..padded])?;
        offset += n as u64;
        unsynced += padded as u64;
        if unsynced >= SYNC_EVERY {
            target.flush_to_device()?;
            unsynced = 0;
        }
        progress(offset - head_len);
    }
    // Everything else must be on the drive before the table appears.
    target.flush_to_device()?;
    if cancel.load(Ordering::Relaxed) {
        return Err(FlashError::Cancelled);
    }

    target.seek(SeekFrom::Start(0))?;
    target.write_all(&head)?;
    target.flush_to_device()?;
    progress(image_len);
    Ok(())
}

/// Reads the image back from `target` and compares it with `image`.
pub fn verify_image<S: Read, T: RawTarget>(
    image: &mut S,
    image_len: u64,
    target: &mut T,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<(), FlashError> {
    let mut expected = vec![0u8; CHUNK];
    let mut actual = vec![0u8; CHUNK];
    let mut done = 0u64;
    target.seek(SeekFrom::Start(0))?;
    while done < image_len {
        if cancel.load(Ordering::Relaxed) {
            return Err(FlashError::Cancelled);
        }
        let n = (image_len - done).min(CHUNK as u64) as usize;
        image.read_exact(&mut expected[..n])?;
        target.read_exact(&mut actual[..padded_len(n as u64) as usize])?;
        if let Some(i) = expected[..n]
            .iter()
            .zip(&actual[..n])
            .position(|(a, b)| a != b)
        {
            return Err(FlashError::VerifyMismatch(done + i as u64));
        }
        done += n as u64;
        progress(done);
    }
    Ok(())
}

/// Writes the image at `image_path` onto `disk`, erasing everything on it.
/// `report` is called with the current phase, bytes done and total bytes.
pub fn run(
    image_path: &Path,
    disk: &Disk,
    verify: bool,
    cancel: &AtomicBool,
    mut report: impl FnMut(Phase, u64, u64),
) -> Result<(), FlashError> {
    let image_len = std::fs::metadata(image_path)?.len();
    check_target(disk, image_len)?;

    report(Phase::Preparing, 0, image_len);
    platform::prepare_raw_write(disk)?;

    let result = (|| {
        let mut target = open_raw(disk)?;
        let mut image = File::open(image_path)?;
        write_image(&mut image, image_len, &mut target, cancel, |done| {
            report(Phase::Writing, done, image_len)
        })?;
        if verify {
            platform::drop_read_cache(disk)?;
            let mut image = File::open(image_path)?;
            verify_image(&mut image, image_len, &mut target, cancel, |done| {
                report(Phase::Verifying, done, image_len)
            })?;
        }
        Ok(())
    })();

    // Always let the OS pick up whatever is on the drive now, even after a
    // failed or cancelled write.
    report(Phase::Finishing, image_len, image_len);
    platform::finish_raw_write(disk);
    result
}

#[cfg(target_os = "linux")]
use crate::platform::linux_executor as platform;
#[cfg(target_os = "macos")]
use crate::platform::macos_executor as platform;
#[cfg(target_os = "windows")]
use crate::platform::windows_executor as platform;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use std::io::Cursor;

    impl RawTarget for Cursor<Vec<u8>> {
        fn flush_to_device(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn pattern(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 31 % 251) as u8).collect()
    }

    fn usb_disk(size: u64) -> Disk {
        Disk {
            id: DiskId::from("/dev/sdz"),
            display_name: "Disk 9".into(),
            vendor: String::new(),
            model: "Stick".into(),
            serial: None,
            bus: BusType::Usb,
            media: MediaType::Unknown,
            size: ByteSize(size),
            logical_sector_size: 512,
            table: PartitionTable::Mbr,
            health: HealthStatus::Unknown,
            read_only: false,
            is_system_disk: false,
            layout: vec![],
        }
    }

    #[test]
    fn writes_and_verifies_an_image_spanning_several_chunks() {
        // Not a multiple of the chunk or sector size, to exercise padding.
        let image = pattern(CHUNK * 2 + 12_345);
        let mut drive = Cursor::new(vec![0xEEu8; CHUNK * 4]);
        let cancel = AtomicBool::new(false);
        let mut seen = Vec::new();

        write_image(
            &mut Cursor::new(&image),
            image.len() as u64,
            &mut drive,
            &cancel,
            |d| seen.push(d),
        )
        .unwrap();

        assert_eq!(&drive.get_ref()[..image.len()], &image[..]);
        let padded_end = padded_len(image.len() as u64) as usize;
        assert!(drive.get_ref()[image.len()..padded_end]
            .iter()
            .all(|&b| b == 0));
        assert_eq!(
            drive.get_ref()[padded_end],
            0xEE,
            "must not write past the padding"
        );
        assert_eq!(seen.last(), Some(&(image.len() as u64)));

        verify_image(
            &mut Cursor::new(&image),
            image.len() as u64,
            &mut drive,
            &cancel,
            |_| {},
        )
        .unwrap();
    }

    #[test]
    fn verify_reports_the_first_differing_byte() {
        let image = pattern(10_000);
        let mut drive = Cursor::new(vec![0u8; 16_384]);
        let cancel = AtomicBool::new(false);
        write_image(
            &mut Cursor::new(&image),
            10_000,
            &mut drive,
            &cancel,
            |_| {},
        )
        .unwrap();
        drive.get_mut()[4_321] ^= 0xFF;

        let err = verify_image(
            &mut Cursor::new(&image),
            10_000,
            &mut drive,
            &cancel,
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(err, FlashError::VerifyMismatch(4_321)), "{err:?}");
    }

    #[test]
    fn stops_when_cancelled() {
        let image = pattern(CHUNK * 3);
        let mut drive = Cursor::new(vec![0xEEu8; CHUNK * 3]);
        let cancel = AtomicBool::new(false);
        let err = write_image(
            &mut Cursor::new(&image),
            image.len() as u64,
            &mut drive,
            &cancel,
            |_| cancel.store(true, Ordering::Relaxed),
        )
        .unwrap_err();
        assert!(matches!(err, FlashError::Cancelled));
        // The partition table area was cleared and never written; only the
        // chunk after it made it before the cancel took effect.
        assert!(drive.get_ref()[..CHUNK].iter().all(|&b| b == 0));
        assert_eq!(&drive.get_ref()[CHUNK..CHUNK * 2], &image[CHUNK..CHUNK * 2]);
        assert!(drive.get_ref()[CHUNK * 2..].iter().all(|&b| b == 0xEE));
    }

    /// Records where each write lands.
    struct Recording {
        inner: Cursor<Vec<u8>>,
        writes: Vec<(u64, Vec<u8>)>,
    }

    impl Read for Recording {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.inner.read(buf)
        }
    }

    impl Write for Recording {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let at = self.inner.position();
            let n = self.inner.write(buf)?;
            self.writes.push((at, buf[..n].to_vec()));
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Seek for Recording {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    impl RawTarget for Recording {
        fn flush_to_device(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn writes_the_partition_table_last() {
        let image = pattern(CHUNK * 2 + 512);
        let mut drive = Recording {
            inner: Cursor::new(vec![0xEEu8; CHUNK * 3]),
            writes: Vec::new(),
        };
        let cancel = AtomicBool::new(false);
        write_image(
            &mut Cursor::new(&image),
            image.len() as u64,
            &mut drive,
            &cancel,
            |_| {},
        )
        .unwrap();

        let (first_at, first) = drive.writes.first().unwrap();
        assert_eq!(*first_at, 0);
        assert!(first.iter().all(|&b| b == 0), "table area cleared first");
        let (last_at, last) = drive.writes.last().unwrap();
        assert_eq!(*last_at, 0);
        assert_eq!(&last[..], &image[..CHUNK]);
        assert_eq!(&drive.inner.get_ref()[..image.len()], &image[..]);
    }

    #[test]
    fn writes_an_image_smaller_than_one_chunk() {
        let image = pattern(3_000);
        let mut drive = Cursor::new(vec![0xEEu8; 8_192]);
        let cancel = AtomicBool::new(false);
        let mut seen = Vec::new();
        write_image(&mut Cursor::new(&image), 3_000, &mut drive, &cancel, |d| {
            seen.push(d)
        })
        .unwrap();
        assert_eq!(&drive.get_ref()[..3_000], &image[..]);
        assert!(drive.get_ref()[3_000..4_096].iter().all(|&b| b == 0));
        assert_eq!(drive.get_ref()[4_096], 0xEE);
        assert_eq!(seen, vec![3_000]);
    }

    #[test]
    fn rejects_non_usb_too_small_and_read_only_targets() {
        let gb = 1024 * 1024 * 1024;
        assert!(check_target(&usb_disk(8 * gb), 4 * gb).is_ok());

        let mut internal = usb_disk(8 * gb);
        internal.bus = BusType::Nvme;
        assert!(check_target(&internal, gb).is_err());

        assert!(check_target(&usb_disk(2 * gb), 4 * gb).is_err());

        let mut locked = usb_disk(8 * gb);
        locked.read_only = true;
        assert!(check_target(&locked, gb).is_err());

        assert!(check_target(&usb_disk(8 * gb), 0).is_err());
    }

    #[test]
    fn detects_a_boot_sector() {
        let dir = std::env::temp_dir().join(format!("moondisk-image-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut hybrid = vec![0u8; 2048];
        hybrid[510] = 0x55;
        hybrid[511] = 0xAA;
        let hybrid_path = dir.join("linux.iso");
        std::fs::write(&hybrid_path, &hybrid).unwrap();

        let plain_path = dir.join("windows.iso");
        std::fs::write(&plain_path, vec![0u8; 2048]).unwrap();

        let info = image_info(&hybrid_path).unwrap();
        assert!(info.has_boot_sector);
        assert_eq!(info.name, "linux.iso");
        assert_eq!(info.size.0, 2048);
        assert!(!image_info(&plain_path).unwrap().has_boot_sector);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
