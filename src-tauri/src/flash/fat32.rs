//! Builds a complete FAT32 file system in one sequential pass.
//!
//! All files and their sizes are known up front (they come from an ISO
//! image), so every directory and file gets a contiguous cluster range
//! before anything is written, and the file system is then streamed from
//! its first byte to its last in large, aligned writes — what raw device
//! access on Windows and macOS requires. Formatting this way also isn't
//! limited to 32 GB, unlike Windows' own FAT32 formatter; Rufus formats
//! large FAT32 drives itself for the same reason.

use std::collections::HashSet;
use std::io::{self, Write};

/// Where FAT32 stops counting as FAT16 and where cluster numbers run out.
const MIN_CLUSTERS: u64 = 65_525;
const MAX_CLUSTERS: u64 = 0x0FFF_FFF5;
const RESERVED_MIN: u64 = 32;
/// The data region starts on a 1 MiB boundary, like every partition:
/// flash memory is fastest with aligned clusters, and aligned writes are
/// what raw device I/O needs.
const ALIGN: u64 = 1024 * 1024;
const WRITE_CHUNK: usize = 4 * 1024 * 1024;
const EOC: u32 = 0x0FFF_FFFF;
const ATTR_VOLUME_ID: u8 = 0x08;
const ATTR_DIRECTORY: u8 = 0x10;
const ATTR_ARCHIVE: u8 = 0x20;
const ATTR_LFN: u8 = 0x0F;
/// FAT32 can't store a file of 4 GiB or more.
pub const MAX_FILE_SIZE: u64 = u32::MAX as u64;

/// A FAT date and time (2-second resolution, from 1980 on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FatDateTime {
    pub date: u16,
    pub time: u16,
}

impl FatDateTime {
    pub fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
        if !(1980..=2107).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day)
        {
            return FatDateTime::default();
        }
        FatDateTime {
            date: ((year - 1980) << 9) | ((month as u16) << 5) | day as u16,
            time: ((hour.min(23) as u16) << 11)
                | ((minute.min(59) as u16) << 5)
                | (second.min(59) as u16 / 2),
        }
    }
}

impl Default for FatDateTime {
    /// 1980-01-01 00:00.
    fn default() -> Self {
        FatDateTime {
            date: (1 << 5) | 1,
            time: 0,
        }
    }
}

/// What goes onto the volume. File contents are provided by the caller
/// while writing, identified by `id`.
#[derive(Debug, Clone)]
pub enum FatNode {
    Dir {
        name: String,
        date: FatDateTime,
        children: Vec<FatNode>,
    },
    File {
        name: String,
        date: FatDateTime,
        size: u64,
        id: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub bytes_per_sector: u32,
    pub sectors_per_cluster: u32,
    pub reserved_sectors: u32,
    pub fat_sectors: u32,
    pub total_sectors: u32,
    /// Number of data clusters (numbered from 2).
    pub clusters: u32,
}

impl Geometry {
    /// Picks cluster size and region sizes for a partition of
    /// `partition_bytes`. Cluster sizes follow Microsoft's defaults.
    pub fn plan(partition_bytes: u64, bytes_per_sector: u32) -> Result<Geometry, String> {
        let bps = bytes_per_sector as u64;
        if bps != 512 && bps != 4096 {
            return Err(format!("unsupported sector size {bps}"));
        }
        let total = (partition_bytes / bps).min(u32::MAX as u64);
        let gib = 1024 * 1024 * 1024;
        let mut cluster: u64 = match partition_bytes {
            b if b <= 8 * gib => 4096,
            b if b <= 16 * gib => 8192,
            b if b <= 32 * gib => 16384,
            _ => 32768,
        };
        cluster = cluster.max(bps);
        loop {
            let spc = cluster / bps;
            // Over-estimates the clusters (ignores the FATs), so the FATs
            // always have room for every cluster.
            let fat_sectors = ((total.saturating_sub(RESERVED_MIN) / spc + 2) * 4).div_ceil(bps);
            let data_start = (RESERVED_MIN + 2 * fat_sectors).next_multiple_of(ALIGN / bps);
            let clusters = total.saturating_sub(data_start) / spc;
            if clusters > MAX_CLUSTERS && cluster < 65536 {
                cluster *= 2;
                continue;
            }
            if clusters < MIN_CLUSTERS {
                if cluster > bps {
                    cluster /= 2;
                    continue;
                }
                return Err("the drive is too small for FAT32".into());
            }
            return Ok(Geometry {
                bytes_per_sector,
                sectors_per_cluster: spc as u32,
                reserved_sectors: (data_start - 2 * fat_sectors) as u32,
                fat_sectors: fat_sectors as u32,
                total_sectors: total as u32,
                clusters: clusters as u32,
            });
        }
    }

    pub fn cluster_bytes(&self) -> u64 {
        self.sectors_per_cluster as u64 * self.bytes_per_sector as u64
    }

    /// Byte offset of the data region from the start of the partition.
    pub fn data_offset(&self) -> u64 {
        (self.reserved_sectors as u64 + 2 * self.fat_sectors as u64) * self.bytes_per_sector as u64
    }

    fn cluster_offset(&self, cluster: u32) -> u64 {
        self.data_offset() + (cluster as u64 - 2) * self.cluster_bytes()
    }
}

/// Where a file's contents ended up, for verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub id: usize,
    /// Bytes from the start of the partition.
    pub offset: u64,
    pub size: u64,
}

struct Entry {
    short: [u8; 11],
    long: Option<Vec<u16>>,
    date: FatDateTime,
    kind: EntryKind,
}

enum EntryKind {
    Dir(usize),
    File { id: usize, size: u64, first: u32 },
}

struct PlannedDir {
    first: u32,
    clusters: u32,
    parent_first: u32,
    is_root: bool,
    entries: Vec<Entry>,
}

enum Slot {
    Dir(usize),
    File { id: usize, size: u64 },
}

/// The complete layout of a volume, ready to be written.
pub struct Plan {
    pub geometry: Geometry,
    label: [u8; 11],
    dirs: Vec<PlannedDir>,
    slots: Vec<Slot>,
    /// Last cluster of every allocated chain, ascending.
    chain_ends: Vec<u32>,
    used_clusters: u32,
    pub placements: Vec<Placement>,
}

impl Plan {
    pub fn new(geometry: Geometry, label: &str, root: &[FatNode]) -> Result<Plan, String> {
        let mut plan = Plan {
            geometry,
            label: volume_label(label),
            dirs: Vec::new(),
            slots: Vec::new(),
            chain_ends: Vec::new(),
            used_clusters: 0,
            placements: Vec::new(),
        };
        let root_dir = plan.add_dir(root, true)?;
        let mut next = 2u32;
        plan.allocate_dir(root_dir, &mut next)?;
        plan.used_clusters = next - 2;
        Ok(plan)
    }

    /// Creates the directory and, recursively, its subdirectories; no
    /// clusters yet.
    fn add_dir(&mut self, children: &[FatNode], is_root: bool) -> Result<usize, String> {
        let index = self.dirs.len();
        self.dirs.push(PlannedDir {
            first: 0,
            clusters: 0,
            parent_first: 0,
            is_root,
            entries: Vec::new(),
        });
        let long_names = unique_long_names(children.iter().map(|c| match c {
            FatNode::Dir { name, .. } | FatNode::File { name, .. } => name.as_str(),
        }));
        let mut taken = HashSet::new();
        let mut entries = Vec::new();
        for (child, long) in children.iter().zip(long_names) {
            let (short, needs_long) = short_name(&long, &mut taken);
            let long = needs_long.then(|| long.encode_utf16().collect::<Vec<u16>>());
            let entry = match child {
                FatNode::Dir { date, children, .. } => Entry {
                    short,
                    long,
                    date: *date,
                    kind: EntryKind::Dir(self.add_dir(children, false)?),
                },
                FatNode::File { date, size, id, .. } => {
                    if *size > MAX_FILE_SIZE {
                        return Err(format!(
                            "{} is larger than FAT32's 4 GB file limit",
                            child_name(child)
                        ));
                    }
                    Entry {
                        short,
                        long,
                        date: *date,
                        kind: EntryKind::File {
                            id: *id,
                            size: *size,
                            first: 0,
                        },
                    }
                }
            };
            entries.push(entry);
        }
        let dir = &mut self.dirs[index];
        dir.entries = entries;
        let count = dir
            .entries
            .iter()
            .map(|e| 1 + e.long.as_ref().map_or(0, |l| l.len().div_ceil(13)))
            .sum::<usize>()
            + if is_root { 1 } else { 2 };
        dir.clusters = ((count as u64 * 32).div_ceil(self.geometry.cluster_bytes())).max(1) as u32;
        Ok(index)
    }

    fn take(&mut self, next: &mut u32, clusters: u32) -> Result<u32, String> {
        let first = *next;
        let end = first as u64 + clusters as u64;
        if end > 2 + self.geometry.clusters as u64 {
            return Err("the files don't fit onto the drive".into());
        }
        *next = end as u32;
        if clusters > 0 {
            self.chain_ends.push(end as u32 - 1);
        }
        Ok(first)
    }

    /// Allocates the directory's own clusters, then its files, then its
    /// subdirectories, depth-first — the order they're written in.
    fn allocate_dir(&mut self, index: usize, next: &mut u32) -> Result<(), String> {
        let clusters = self.dirs[index].clusters;
        let first = self.take(next, clusters)?;
        self.dirs[index].first = first;
        self.slots.push(Slot::Dir(index));

        let cluster_bytes = self.geometry.cluster_bytes();
        for e in 0..self.dirs[index].entries.len() {
            if let EntryKind::File { id, size, .. } = self.dirs[index].entries[e].kind {
                let clusters = size.div_ceil(cluster_bytes) as u32;
                let file_first = if clusters > 0 {
                    self.take(next, clusters)?
                } else {
                    0
                };
                if let EntryKind::File { first, .. } = &mut self.dirs[index].entries[e].kind {
                    *first = file_first;
                }
                if clusters > 0 {
                    self.placements.push(Placement {
                        id,
                        offset: self.geometry.cluster_offset(file_first),
                        size,
                    });
                }
                self.slots.push(Slot::File { id, size });
            }
        }
        for e in 0..self.dirs[index].entries.len() {
            if let EntryKind::Dir(sub) = self.dirs[index].entries[e].kind {
                self.dirs[sub].parent_first = if self.dirs[index].is_root { 0 } else { first };
                self.allocate_dir(sub, next)?;
            }
        }
        Ok(())
    }

    /// Bytes of file data (without directories and padding).
    pub fn content_bytes(&self) -> u64 {
        self.placements.iter().map(|p| p.size).sum()
    }

    /// Writes the whole volume to `out`, which must be positioned at the
    /// start of the partition. `hidden_sectors` is the partition's first
    /// sector on the disk. `copy` must write exactly `size` bytes of file
    /// `id`.
    pub fn write<W: Write>(
        &self,
        out: &mut W,
        hidden_sectors: u32,
        volume_id: u32,
        mut copy: impl FnMut(usize, u64, &mut dyn Write) -> io::Result<()>,
    ) -> io::Result<()> {
        let g = &self.geometry;
        let bps = g.bytes_per_sector as usize;
        let mut w = SeqWriter::new(out);

        // Reserved region: boot sector, FSInfo, their backups at 6 and 7.
        let boot = self.boot_sector(hidden_sectors, volume_id);
        let fsinfo = self.fsinfo_sector();
        for sector in 0..g.reserved_sectors as usize {
            match sector {
                0 | 6 => w.write_all(&pad(&boot, bps))?,
                1 | 7 => w.write_all(&pad(&fsinfo, bps))?,
                _ => w.write_zeros(bps as u64)?,
            }
        }

        // Both FATs.
        for _ in 0..2 {
            self.write_fat(&mut w)?;
        }

        // Data region, in allocation order.
        let cluster_bytes = g.cluster_bytes();
        for slot in &self.slots {
            match slot {
                Slot::Dir(index) => {
                    let bytes = self.dir_bytes(*index);
                    w.write_all(&bytes)?;
                    let padded = self.dirs[*index].clusters as u64 * cluster_bytes;
                    w.write_zeros(padded - bytes.len() as u64)?;
                }
                Slot::File { id, size } => {
                    let before = w.written;
                    copy(*id, *size, &mut w)?;
                    let copied = w.written - before;
                    if copied != *size {
                        return Err(io::Error::other(format!(
                            "file {id}: expected {size} bytes, got {copied}"
                        )));
                    }
                    w.write_zeros(size.next_multiple_of(cluster_bytes) - size)?;
                }
            }
        }
        w.finish()
    }

    fn write_fat(&self, w: &mut SeqWriter<impl Write>) -> io::Result<()> {
        let entries = self.geometry.fat_sectors as u64 * self.geometry.bytes_per_sector as u64 / 4;
        let used_end = 2 + self.used_clusters as u64;
        let mut ends = self.chain_ends.iter().peekable();
        let mut buf = Vec::with_capacity(WRITE_CHUNK);
        for cluster in 0..entries {
            let value: u32 = match cluster {
                0 => 0x0FFF_FFF8, // media descriptor F8
                1 => EOC,
                c if c < used_end => {
                    while ends.peek().is_some_and(|&&e| (e as u64) < c) {
                        ends.next();
                    }
                    if ends.peek().is_some_and(|&&e| e as u64 == c) {
                        EOC
                    } else {
                        c as u32 + 1
                    }
                }
                _ => 0,
            };
            buf.extend_from_slice(&value.to_le_bytes());
            if buf.len() >= WRITE_CHUNK {
                w.write_all(&buf)?;
                buf.clear();
            }
        }
        w.write_all(&buf)
    }

    fn dir_bytes(&self, index: usize) -> Vec<u8> {
        let dir = &self.dirs[index];
        let mut out = Vec::new();
        if dir.is_root {
            out.extend(short_entry(
                &self.label,
                ATTR_VOLUME_ID,
                0,
                0,
                FatDateTime::default(),
            ));
        } else {
            let date = FatDateTime::default();
            out.extend(short_entry(
                b".          ",
                ATTR_DIRECTORY,
                dir.first,
                0,
                date,
            ));
            out.extend(short_entry(
                b"..         ",
                ATTR_DIRECTORY,
                dir.parent_first,
                0,
                date,
            ));
        }
        for e in &dir.entries {
            if let Some(long) = &e.long {
                for lfn in lfn_entries(long, lfn_checksum(&e.short)) {
                    out.extend(lfn);
                }
            }
            let entry = match e.kind {
                EntryKind::Dir(sub) => {
                    short_entry(&e.short, ATTR_DIRECTORY, self.dirs[sub].first, 0, e.date)
                }
                EntryKind::File { size, first, .. } => {
                    short_entry(&e.short, ATTR_ARCHIVE, first, size as u32, e.date)
                }
            };
            out.extend(entry);
        }
        out
    }

    fn boot_sector(&self, hidden_sectors: u32, volume_id: u32) -> [u8; 512] {
        let g = &self.geometry;
        let mut b = [0u8; 512];
        b[0..3].copy_from_slice(&[0xEB, 0x58, 0x90]);
        b[3..11].copy_from_slice(b"MSWIN4.1");
        b[11..13].copy_from_slice(&(g.bytes_per_sector as u16).to_le_bytes());
        b[13] = g.sectors_per_cluster as u8;
        b[14..16].copy_from_slice(&(g.reserved_sectors as u16).to_le_bytes());
        b[16] = 2; // FATs
        b[21] = 0xF8; // fixed disk
        b[24..26].copy_from_slice(&63u16.to_le_bytes());
        b[26..28].copy_from_slice(&255u16.to_le_bytes());
        b[28..32].copy_from_slice(&hidden_sectors.to_le_bytes());
        b[32..36].copy_from_slice(&g.total_sectors.to_le_bytes());
        b[36..40].copy_from_slice(&g.fat_sectors.to_le_bytes());
        b[44..48].copy_from_slice(&2u32.to_le_bytes()); // root cluster
        b[48..50].copy_from_slice(&1u16.to_le_bytes()); // FSInfo sector
        b[50..52].copy_from_slice(&6u16.to_le_bytes()); // backup boot sector
        b[64] = 0x80; // drive number
        b[66] = 0x29; // extended boot signature
        b[67..71].copy_from_slice(&volume_id.to_le_bytes());
        b[71..82].copy_from_slice(&self.label);
        b[82..90].copy_from_slice(b"FAT32   ");
        b[510] = 0x55;
        b[511] = 0xAA;
        b
    }

    fn fsinfo_sector(&self) -> [u8; 512] {
        let mut b = [0u8; 512];
        b[0..4].copy_from_slice(&0x4161_5252u32.to_le_bytes());
        b[484..488].copy_from_slice(&0x6141_7272u32.to_le_bytes());
        let free = self.geometry.clusters - self.used_clusters;
        b[488..492].copy_from_slice(&free.to_le_bytes());
        let next_free = if free > 0 {
            2 + self.used_clusters
        } else {
            u32::MAX
        };
        b[492..496].copy_from_slice(&next_free.to_le_bytes());
        b[508..512].copy_from_slice(&0xAA55_0000u32.to_le_bytes());
        b
    }
}

fn child_name(node: &FatNode) -> &str {
    match node {
        FatNode::Dir { name, .. } | FatNode::File { name, .. } => name,
    }
}

fn pad(sector: &[u8; 512], bps: usize) -> Vec<u8> {
    let mut v = sector.to_vec();
    v.resize(bps, 0);
    v
}

/// Buffers everything into large sequential writes.
struct SeqWriter<'a, W: Write> {
    out: &'a mut W,
    buf: Vec<u8>,
    written: u64,
}

impl<'a, W: Write> SeqWriter<'a, W> {
    fn new(out: &'a mut W) -> Self {
        SeqWriter {
            out,
            buf: Vec::with_capacity(WRITE_CHUNK),
            written: 0,
        }
    }

    fn write_zeros(&mut self, mut n: u64) -> io::Result<()> {
        static ZEROS: [u8; 64 * 1024] = [0; 64 * 1024];
        while n > 0 {
            let k = n.min(ZEROS.len() as u64) as usize;
            self.write_all(&ZEROS[..k])?;
            n -= k as u64;
        }
        Ok(())
    }

    fn finish(mut self) -> io::Result<()> {
        self.out.write_all(&self.buf)?;
        self.buf.clear();
        self.out.flush()
    }
}

impl<W: Write> Write for SeqWriter<'_, W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let room = WRITE_CHUNK - self.buf.len();
        let n = data.len().min(room);
        self.buf.extend_from_slice(&data[..n]);
        self.written += n as u64;
        if self.buf.len() == WRITE_CHUNK {
            self.out.write_all(&self.buf)?;
            self.buf.clear();
        }
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// An 11-byte volume label: upper case, only characters FAT allows.
pub fn volume_label(label: &str) -> [u8; 11] {
    let mut out = [b' '; 11];
    let cleaned: Vec<u8> = label
        .trim()
        .chars()
        .map(|c| {
            let c = c.to_ascii_uppercase();
            if c == ' ' || is_short_char(c) {
                c as u8
            } else {
                b'_'
            }
        })
        .take(11)
        .collect();
    if cleaned.iter().all(|&b| b == b' ') {
        out[..8].copy_from_slice(b"MOONDISK");
    } else {
        out[..cleaned.len()].copy_from_slice(&cleaned);
    }
    out
}

/// The label as text, without the space padding.
pub fn volume_label_text(label: &str) -> String {
    String::from_utf8_lossy(&volume_label(label))
        .trim_end()
        .to_string()
}

fn is_short_char(c: char) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit() || "!#$%&'()-@^_`{}~".contains(c)
}

/// Makes every name valid and unique (ignoring case) for a long FAT name.
fn unique_long_names<'a>(names: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut seen = HashSet::new();
    names
        .map(|name| {
            let mut clean: String = name
                .chars()
                .map(|c| {
                    if c < ' ' || "\"*/:<>?\\|".contains(c) {
                        '_'
                    } else {
                        c
                    }
                })
                .collect();
            clean = clean.trim_end_matches(['.', ' ']).to_string();
            if clean.is_empty() {
                clean = "_".into();
            }
            if clean.encode_utf16().count() > 255 {
                clean =
                    String::from_utf16_lossy(&clean.encode_utf16().take(255).collect::<Vec<_>>());
            }
            let mut candidate = clean.clone();
            let (stem, ext) = match clean.rfind('.') {
                Some(i) if i > 0 => (&clean[..i], &clean[i..]),
                _ => (clean.as_str(), ""),
            };
            let mut n = 2;
            while !seen.insert(candidate.to_lowercase()) {
                candidate = format!("{stem} ({n}){ext}");
                n += 1;
            }
            candidate
        })
        .collect()
}

/// The 8.3 name for `name`, unique among `taken`, and whether a long name
/// entry is needed to keep the real name (lower case, too long, …).
fn short_name(name: &str, taken: &mut HashSet<[u8; 11]>) -> ([u8; 11], bool) {
    let upper = name.to_ascii_uppercase();
    let (base, ext) = match upper.rfind('.') {
        Some(i) => (&upper[..i], &upper[i + 1..]),
        None => (upper.as_str(), ""),
    };
    let fits = !base.is_empty()
        && base.len() <= 8
        && ext.len() <= 3
        && base.chars().chain(ext.chars()).all(is_short_char);
    if fits {
        let short = pack_short(base, ext);
        if taken.insert(short) {
            return (short, upper != name);
        }
    }

    // Lossy: keep what's allowed, then add a "~N" tail.
    let clean = |s: &str| -> String {
        s.chars()
            .filter(|&c| c != ' ' && c != '.')
            .map(|c| if is_short_char(c) { c } else { '_' })
            .collect()
    };
    let trimmed = upper.trim_start_matches('.');
    let (base, ext) = match trimmed.rfind('.') {
        Some(i) => (clean(&trimmed[..i]), clean(&trimmed[i + 1..])),
        None => (clean(trimmed), String::new()),
    };
    let base = if base.is_empty() {
        "_".to_string()
    } else {
        base
    };
    let ext: String = ext.chars().take(3).collect();
    for n in 1u32.. {
        let tail = format!("~{n}");
        let keep = 8 - tail.len();
        let short = pack_short(&format!("{}{tail}", &base[..base.len().min(keep)]), &ext);
        if taken.insert(short) {
            return (short, true);
        }
    }
    unreachable!()
}

fn pack_short(base: &str, ext: &str) -> [u8; 11] {
    let mut out = [b' '; 11];
    for (i, b) in base.bytes().take(8).enumerate() {
        out[i] = b;
    }
    for (i, b) in ext.bytes().take(3).enumerate() {
        out[8 + i] = b;
    }
    out
}

fn lfn_checksum(short: &[u8; 11]) -> u8 {
    short
        .iter()
        .fold(0u8, |sum, &b| sum.rotate_right(1).wrapping_add(b))
}

/// Long name entries in on-disk order (the last part first).
fn lfn_entries(name: &[u16], checksum: u8) -> Vec<[u8; 32]> {
    let parts = name.len().div_ceil(13);
    let mut units = name.to_vec();
    if units.len() % 13 != 0 {
        units.push(0);
    }
    units.resize(parts * 13, 0xFFFF);
    (0..parts)
        .rev()
        .map(|i| {
            let chunk = &units[i * 13..i * 13 + 13];
            let mut e = [0u8; 32];
            e[0] = (i + 1) as u8 | if i + 1 == parts { 0x40 } else { 0 };
            e[11] = ATTR_LFN;
            e[13] = checksum;
            let slots = (1..11)
                .step_by(2)
                .chain((14..26).step_by(2))
                .chain((28..32).step_by(2));
            for (unit, pos) in chunk.iter().zip(slots) {
                e[pos..pos + 2].copy_from_slice(&unit.to_le_bytes());
            }
            e
        })
        .collect()
}

fn short_entry(name: &[u8; 11], attr: u8, cluster: u32, size: u32, dt: FatDateTime) -> [u8; 32] {
    let mut e = [0u8; 32];
    e[0..11].copy_from_slice(name);
    e[11] = attr;
    e[14..16].copy_from_slice(&dt.time.to_le_bytes());
    e[16..18].copy_from_slice(&dt.date.to_le_bytes());
    e[18..20].copy_from_slice(&dt.date.to_le_bytes());
    e[20..22].copy_from_slice(&((cluster >> 16) as u16).to_le_bytes());
    e[22..24].copy_from_slice(&dt.time.to_le_bytes());
    e[24..26].copy_from_slice(&dt.date.to_le_bytes());
    e[26..28].copy_from_slice(&(cluster as u16).to_le_bytes());
    e[28..32].copy_from_slice(&size.to_le_bytes());
    e
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * MIB;

    #[test]
    fn plans_large_drives_with_aligned_32k_clusters() {
        let g = Geometry::plan(114 * GIB, 512).unwrap();
        assert_eq!(g.cluster_bytes(), 32 * 1024);
        assert_eq!(g.data_offset() % MIB, 0);
        assert!(g.clusters as u64 >= MIN_CLUSTERS);
        // The FATs have an entry for every cluster.
        assert!(g.fat_sectors as u64 * 512 / 4 >= g.clusters as u64 + 2);
    }

    #[test]
    fn shrinks_clusters_to_stay_fat32_on_small_drives() {
        let g = Geometry::plan(100 * MIB, 512).unwrap();
        assert!(g.clusters as u64 >= MIN_CLUSTERS);
        assert_eq!(g.data_offset() % MIB, 0);
        assert!(Geometry::plan(16 * MIB, 512).is_err());
    }

    #[test]
    fn generates_unique_short_names() {
        let mut taken = HashSet::new();
        assert_eq!(
            short_name("BOOTX64.EFI", &mut taken),
            (*b"BOOTX64 EFI", false)
        );
        assert_eq!(
            short_name("BOOTx64.EFI", &mut taken),
            (*b"BOOTX6~1EFI", true)
        );
        assert_eq!(
            short_name("vmlinuz-linux", &mut taken),
            (*b"VMLINU~1   ", true)
        );
        assert_eq!(short_name(".disk", &mut taken), (*b"DISK~1     ", true));
        assert_eq!(
            short_name("readme.txt", &mut taken),
            (*b"README  TXT", true)
        );
        assert_eq!(
            short_name("a b+c.tar.gz", &mut taken),
            (*b"AB_CTA~1GZ ", true)
        );
    }

    #[test]
    fn keeps_long_names_unique_ignoring_case() {
        let names = unique_long_names(["README", "readme", "a:b", "x.txt."].into_iter());
        assert_eq!(names, ["README", "readme (2)", "a_b", "x.txt"]);
    }

    fn content(id: usize, size: u64) -> Vec<u8> {
        (0..size.div_ceil(4) as u32)
            .flat_map(|i| ((id as u32) << 24 | i).to_le_bytes())
            .take(size as usize)
            .collect()
    }

    fn file(name: &str, id: usize, size: u64) -> FatNode {
        FatNode::File {
            name: name.into(),
            date: FatDateTime::new(2024, 9, 1, 10, 30, 0),
            size,
            id,
        }
    }

    fn dir(name: &str, children: Vec<FatNode>) -> FatNode {
        FatNode::Dir {
            name: name.into(),
            date: FatDateTime::new(2024, 9, 1, 10, 30, 0),
            children,
        }
    }

    fn sample_tree() -> Vec<FatNode> {
        let many: Vec<FatNode> = (0..120)
            .map(|i| {
                file(
                    &format!("package-number-{i:03}-with-a-long-name.pkg.tar.zst"),
                    100 + i,
                    700,
                )
            })
            .collect();
        vec![
            dir(
                "EFI",
                vec![dir(
                    "BOOT",
                    vec![file("BOOTx64.EFI", 1, 5000), file("BOOTIA32.EFI", 2, 3000)],
                )],
            ),
            dir(
                "loader",
                vec![dir("entries", vec![file("01-archiso.conf", 3, 120)])],
            ),
            dir(
                "arch",
                vec![dir("x86_64", vec![file("airootfs.sfs", 4, 3 * MIB + 17)])],
            ),
            dir("pkgs", many),
            file("README", 5, 10),
            file("readme", 6, 11),
            file("empty.uuid", 7, 0),
        ]
    }

    /// Writes the sample tree into an in-memory partition.
    fn build(partition: u64) -> (Plan, Vec<u8>) {
        let g = Geometry::plan(partition, 512).unwrap();
        let plan = Plan::new(g, "ARCH_202409", &sample_tree()).unwrap();
        let mut out = Cursor::new(Vec::new());
        plan.write(&mut out, 2048, 0x1234_5678, |id, size, w| {
            w.write_all(&content(id, size))
        })
        .unwrap();
        let mut bytes = out.into_inner();
        bytes.resize(partition as usize, 0);
        (plan, bytes)
    }

    fn read_all(fs: &fatfs::FileSystem<Cursor<Vec<u8>>>, path: &str) -> Vec<u8> {
        let mut f = fs.root_dir().open_file(path).unwrap();
        let mut v = Vec::new();
        f.read_to_end(&mut v).unwrap();
        v
    }

    #[test]
    fn writes_a_volume_another_fat_implementation_can_read() {
        let (plan, bytes) = build(100 * MIB);
        let fs = fatfs::FileSystem::new(Cursor::new(bytes), fatfs::FsOptions::new()).unwrap();
        assert_eq!(fs.fat_type(), fatfs::FatType::Fat32);
        assert_eq!(fs.volume_label(), "ARCH_202409");

        assert_eq!(read_all(&fs, "EFI/BOOT/BOOTx64.EFI"), content(1, 5000));
        assert_eq!(
            read_all(&fs, "loader/entries/01-archiso.conf"),
            content(3, 120)
        );
        assert_eq!(
            read_all(&fs, "arch/x86_64/airootfs.sfs"),
            content(4, 3 * MIB + 17)
        );
        assert_eq!(read_all(&fs, "README"), content(5, 10));
        assert_eq!(read_all(&fs, "readme (2)"), content(6, 11));
        assert!(read_all(&fs, "empty.uuid").is_empty());

        // A directory spanning several clusters, all names intact.
        let names: Vec<String> = fs
            .root_dir()
            .open_dir("pkgs")
            .unwrap()
            .iter()
            .map(|e| e.unwrap().file_name())
            .filter(|n| n != "." && n != "..")
            .collect();
        assert_eq!(names.len(), 120);
        assert_eq!(names[42], "package-number-042-with-a-long-name.pkg.tar.zst");
        assert_eq!(
            read_all(&fs, "pkgs/package-number-119-with-a-long-name.pkg.tar.zst"),
            content(219, 700)
        );

        // Firmware without long name support finds the loader by its 8.3
        // name, which is exactly what UEFI looks for.
        let boot = fs.root_dir().open_dir("EFI/BOOT").unwrap();
        let loader = boot
            .iter()
            .map(|e| e.unwrap())
            .find(|e| e.file_name() == "BOOTx64.EFI")
            .unwrap();
        assert_eq!(loader.short_file_name(), "BOOTX64.EFI");

        let stats = fs.stats().unwrap();
        assert_eq!(
            stats.free_clusters(),
            plan.geometry.clusters - plan.used_clusters
        );
    }

    #[test]
    fn places_files_where_the_plan_says() {
        let (plan, bytes) = build(100 * MIB);
        for p in &plan.placements {
            let at = p.offset as usize;
            assert_eq!(&bytes[at..at + p.size as usize], &content(p.id, p.size)[..]);
        }
    }

    #[test]
    fn refuses_files_of_4_gib_or_more() {
        let g = Geometry::plan(100 * MIB, 512).unwrap();
        let err = Plan::new(g, "X", &[file("huge.sfs", 1, 4 * GIB)])
            .err()
            .unwrap();
        assert!(err.contains("4 GB"), "{err}");
    }

    #[test]
    fn refuses_files_that_do_not_fit() {
        let g = Geometry::plan(100 * MIB, 512).unwrap();
        let err = Plan::new(g, "X", &[file("big.sfs", 1, 200 * MIB)])
            .err()
            .unwrap();
        assert!(err.contains("fit"), "{err}");
    }
}
