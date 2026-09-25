//! Reads the file tree of an ISO 9660 image, the file system of CD/DVD
//! images. The USB writer's file-copy mode copies this tree onto a FAT32
//! drive, the way Rufus' "ISO mode" does.
//!
//! Names come from Rock Ridge (POSIX names; what Linux ISOs use) when the
//! image has it, else from Joliet (Windows' Unicode names, at most 64
//! characters), else from plain ISO 9660 names. Rock Ridge symbolic links
//! are skipped, since FAT32 can't represent them.

use std::collections::HashSet;
use std::io::{self, Read, Seek, SeekFrom};

const SECTOR: u64 = 2048;
/// Deeper trees than this are treated as broken (or malicious) images.
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IsoDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// A contiguous run of a file's bytes inside the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extent {
    /// Byte offset in the image.
    pub offset: u64,
    pub len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IsoNode {
    Dir {
        name: String,
        date: IsoDate,
        children: Vec<IsoNode>,
    },
    File {
        name: String,
        date: IsoDate,
        size: u64,
        /// Usually one; files over 4 GiB are split into several.
        extents: Vec<Extent>,
    },
}

impl IsoNode {
    pub fn name(&self) -> &str {
        match self {
            IsoNode::Dir { name, .. } | IsoNode::File { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameSource {
    RockRidge,
    Joliet,
    Iso9660,
}

#[derive(Debug)]
pub struct IsoTree {
    /// The primary volume identifier, e.g. `ARCH_202409`.
    pub volume_label: String,
    pub children: Vec<IsoNode>,
    pub names: NameSource,
    pub symlinks_skipped: usize,
    /// The image also has a UDF file system (as Windows installer images
    /// do, whose real files only exist there).
    pub has_udf: bool,
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

fn le_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// A directory record, as stored in a directory's data.
struct Record<'a> {
    extent: u32,
    size: u32,
    flags: u8,
    date: IsoDate,
    name: &'a [u8],
    system_use: &'a [u8],
}

const FLAG_DIR: u8 = 0x02;
const FLAG_ASSOCIATED: u8 = 0x04;
const FLAG_MULTI_EXTENT: u8 = 0x80;

fn parse_record(b: &[u8]) -> Option<Record<'_>> {
    let len = *b.first()? as usize;
    if len < 34 || len > b.len() {
        return None;
    }
    let b = &b[..len];
    let name_len = b[32] as usize;
    let name = b.get(33..33 + name_len)?;
    // A padding byte keeps the system use area at an even offset.
    let su_start = 33 + name_len + (1 - name_len % 2);
    Some(Record {
        extent: le_u32(&b[2..6]),
        size: le_u32(&b[10..14]),
        flags: b[25],
        date: IsoDate {
            year: 1900 + b[18] as u16,
            month: b[19],
            day: b[20],
            hour: b[21],
            minute: b[22],
            second: b[23],
        },
        name,
        system_use: b.get(su_start..).unwrap_or_default(),
    })
}

/// Records never cross a sector boundary; a zero length byte means the
/// rest of the sector is padding.
fn records(data: &[u8]) -> Vec<Record<'_>> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < data.len() {
        if data[pos] == 0 {
            pos = (pos / SECTOR as usize + 1) * SECTOR as usize;
            continue;
        }
        match parse_record(&data[pos..]) {
            Some(r) => {
                pos += data[pos] as usize;
                out.push(r);
            }
            None => break,
        }
    }
    out
}

/// What a record's Rock Ridge entries say about it.
#[derive(Default)]
struct RockRidge {
    name: Vec<u8>,
    has_name: bool,
    symlink: bool,
    /// Relocated directory: listed again at its real place via `CL`.
    relocated: bool,
    child_link: Option<u32>,
}

struct Reader<'r, R> {
    r: &'r mut R,
    names: NameSource,
    /// Bytes to skip at the start of every system use area (SUSP `SP`).
    susp_skip: usize,
    symlinks_skipped: usize,
    nodes: usize,
    visited: HashSet<u32>,
}

impl<R: Read + Seek> Reader<'_, R> {
    fn read_at(&mut self, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.r.seek(SeekFrom::Start(offset))?;
        self.r.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn susp(&mut self, area: &[u8], rr: &mut RockRidge, depth: usize) -> io::Result<()> {
        let mut continuation = None;
        let mut pos = 0;
        while pos + 4 <= area.len() {
            let len = area[pos + 2] as usize;
            if len < 4 || pos + len > area.len() {
                break;
            }
            let data = &area[pos + 4..pos + len];
            match &area[pos..pos + 2] {
                b"NM" if !data.is_empty() => {
                    // Flags 0x02/0x04 mark "." and "..".
                    if data[0] & 0x06 == 0 {
                        rr.name.extend_from_slice(&data[1..]);
                        rr.has_name = true;
                    }
                }
                b"SL" => rr.symlink = true,
                b"RE" => rr.relocated = true,
                b"CL" if data.len() >= 4 => rr.child_link = Some(le_u32(data)),
                b"PX" if data.len() >= 4 => {
                    if le_u32(data) & 0o170000 == 0o120000 {
                        rr.symlink = true;
                    }
                }
                b"CE" if data.len() >= 20 => {
                    continuation =
                        Some((le_u32(&data[0..]), le_u32(&data[8..]), le_u32(&data[16..])));
                }
                b"ST" => break,
                _ => {}
            }
            pos += len;
        }
        if let Some((block, offset, len)) = continuation {
            if depth < 16 && len > 0 && len <= 64 * 1024 {
                let more = self.read_at(block as u64 * SECTOR + offset as u64, len as usize)?;
                self.susp(&more, rr, depth + 1)?;
            }
        }
        Ok(())
    }

    fn decode_name(&self, raw: &[u8]) -> String {
        let name = match self.names {
            NameSource::Joliet => {
                let units: Vec<u16> = raw
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                String::from_utf16_lossy(&units)
            }
            _ => String::from_utf8_lossy(raw).into_owned(),
        };
        // Drop the ";1" version suffix, and the dot of an empty extension.
        let name = name.split(';').next().unwrap_or_default();
        name.strip_suffix('.').unwrap_or(name).to_string()
    }

    /// Size of the directory whose data starts at `extent`, from its "."
    /// record.
    fn dir_size_at(&mut self, extent: u32) -> io::Result<u32> {
        let sector = self.read_at(extent as u64 * SECTOR, SECTOR as usize)?;
        parse_record(&sector)
            .map(|r| r.size)
            .ok_or_else(|| invalid("broken relocated directory"))
    }

    fn dir(&mut self, extent: u32, size: u32, depth: usize) -> io::Result<Vec<IsoNode>> {
        if depth > MAX_DEPTH {
            return Err(invalid("directories nested too deeply"));
        }
        if !self.visited.insert(extent) {
            return Err(invalid("directory loop"));
        }
        let data = self.read_at(extent as u64 * SECTOR, size as usize)?;
        let mut out = Vec::new();
        let mut pending: Vec<Extent> = Vec::new();

        for rec in records(&data) {
            if rec.name == [0] || rec.name == [1] || rec.flags & FLAG_ASSOCIATED != 0 {
                continue;
            }
            self.nodes += 1;
            if self.nodes > MAX_NODES {
                return Err(invalid("too many files"));
            }

            let mut rr = RockRidge::default();
            if self.names == NameSource::RockRidge {
                let area = rec.system_use.get(self.susp_skip..).unwrap_or_default();
                self.susp(area, &mut rr, 0)?;
            }
            if rr.relocated {
                continue;
            }
            if rr.symlink {
                self.symlinks_skipped += 1;
                continue;
            }
            let name = if rr.has_name {
                String::from_utf8_lossy(&rr.name).into_owned()
            } else {
                self.decode_name(rec.name)
            };

            if rec.flags & FLAG_DIR != 0 || rr.child_link.is_some() {
                let (ext, len) = match rr.child_link {
                    Some(block) => (block, self.dir_size_at(block)?),
                    None => (rec.extent, rec.size),
                };
                let children = self.dir(ext, len, depth + 1)?;
                out.push(IsoNode::Dir {
                    name,
                    date: rec.date,
                    children,
                });
            } else {
                pending.push(Extent {
                    offset: rec.extent as u64 * SECTOR,
                    len: rec.size as u64,
                });
                if rec.flags & FLAG_MULTI_EXTENT != 0 {
                    continue;
                }
                let extents = std::mem::take(&mut pending);
                out.push(IsoNode::File {
                    name,
                    date: rec.date,
                    size: extents.iter().map(|e| e.len).sum(),
                    extents,
                });
            }
        }
        Ok(out)
    }
}

/// Reads the whole file tree of the ISO 9660 image in `r`.
pub fn read_tree<R: Read + Seek>(r: &mut R) -> io::Result<IsoTree> {
    let mut primary = None;
    let mut joliet = None;
    let mut has_udf = false;

    // Volume descriptors start at sector 16. UDF's "NSR" descriptors
    // follow the ISO 9660 ones.
    for sector in 16..64u64 {
        let mut vd = vec![0u8; SECTOR as usize];
        r.seek(SeekFrom::Start(sector * SECTOR))?;
        if r.read_exact(&mut vd).is_err() {
            break;
        }
        match &vd[1..6] {
            b"CD001" => match vd[0] {
                1 if primary.is_none() => primary = Some(vd),
                2 if matches!(&vd[88..91], b"%/@" | b"%/C" | b"%/E") => joliet = Some(vd),
                _ => {}
            },
            b"NSR02" | b"NSR03" => has_udf = true,
            b"BEA01" | b"TEA01" | b"BOOT2" | b"CDW02" => {}
            _ => break,
        }
    }
    let primary = primary.ok_or_else(|| invalid("not an ISO 9660 image"))?;
    let volume_label = String::from_utf8_lossy(&primary[40..72]).trim().to_string();

    let root_of = |vd: &[u8]| -> io::Result<(u32, u32, Vec<u8>)> {
        let rec = parse_record(&vd[156..190]).ok_or_else(|| invalid("broken root directory"))?;
        Ok((rec.extent, rec.size, vd[156..190].to_vec()))
    };
    let (p_extent, p_size, _) = root_of(&primary)?;

    let mut reader = Reader {
        r,
        names: NameSource::Iso9660,
        susp_skip: 0,
        symlinks_skipped: 0,
        nodes: 0,
        visited: HashSet::new(),
    };

    // Rock Ridge announces itself with a SUSP "SP" entry in the root's
    // "." record.
    let root_data = reader.read_at(p_extent as u64 * SECTOR, SECTOR as usize)?;
    let rock_ridge = parse_record(&root_data).and_then(|dot| {
        let su = dot.system_use;
        (su.len() >= 7 && &su[0..2] == b"SP" && su[4..6] == [0xBE, 0xEF]).then(|| su[6] as usize)
    });

    let (extent, size) = if let Some(skip) = rock_ridge {
        reader.names = NameSource::RockRidge;
        reader.susp_skip = skip;
        (p_extent, p_size)
    } else if let Some(vd) = &joliet {
        reader.names = NameSource::Joliet;
        let (e, s, _) = root_of(vd)?;
        (e, s)
    } else {
        (p_extent, p_size)
    };

    let children = reader.dir(extent, size, 0)?;
    Ok(IsoTree {
        volume_label,
        children,
        names: reader.names,
        symlinks_skipped: reader.symlinks_skipped,
        has_udf,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read(path).unwrap()
    }

    fn find<'a>(nodes: &'a [IsoNode], path: &str) -> &'a IsoNode {
        let (first, rest) = path.split_once('/').unwrap_or((path, ""));
        let node = nodes
            .iter()
            .find(|n| n.name() == first)
            .unwrap_or_else(|| panic!("{first} not found in {:?}", names(nodes)));
        match (node, rest) {
            (_, "") => node,
            (IsoNode::Dir { children, .. }, rest) => find(children, rest),
            _ => panic!("{first} is not a directory"),
        }
    }

    fn names(nodes: &[IsoNode]) -> Vec<&str> {
        nodes.iter().map(IsoNode::name).collect()
    }

    fn content(image: &[u8], node: &IsoNode) -> Vec<u8> {
        let IsoNode::File { extents, .. } = node else {
            panic!("not a file");
        };
        extents
            .iter()
            .flat_map(|e| image[e.offset as usize..(e.offset + e.len) as usize].to_vec())
            .collect()
    }

    /// What scripts/make-test-isos.py fills files with.
    fn pattern(size: usize, seed: u32) -> Vec<u8> {
        (0..size.div_ceil(4) as u32)
            .flat_map(|i| ((seed << 24) + i).to_le_bytes())
            .take(size)
            .collect()
    }

    #[test]
    fn reads_rock_ridge_names_of_a_linux_style_image() {
        let image = fixture("archlike.iso");
        let tree = read_tree(&mut Cursor::new(&image)).unwrap();
        assert_eq!(tree.names, NameSource::RockRidge);
        assert_eq!(tree.volume_label, "ARCH_202409");
        assert!(!tree.has_udf);

        // Mixed case and names far beyond 8.3 and Joliet's 64 characters.
        let loader = find(&tree.children, "EFI/BOOT/BOOTx64.EFI");
        assert!(content(&image, loader).starts_with(b"MZ efi loader x64 "));
        let long = tree
            .children
            .iter()
            .find(|n| n.name().starts_with("a-rather-long-file-name"))
            .unwrap();
        assert_eq!(long.name().len(), 63 + 60 + 4);

        let rootfs = find(&tree.children, "arch/x86_64/airootfs.sfs");
        assert_eq!(content(&image, rootfs), pattern(150_000, 4));
        let IsoNode::File { size, .. } = find(&tree.children, "boot/2024-09-01-10-00-00-00.uuid")
        else {
            panic!()
        };
        assert_eq!(*size, 0);
        find(&tree.children, ".disk/info");

        // The "latest -> arch/x86_64" symlink can't go onto FAT32.
        assert_eq!(tree.symlinks_skipped, 1);
        assert!(!names(&tree.children).contains(&"latest"));
    }

    #[test]
    fn falls_back_to_joliet_names() {
        let image = fixture("joliet-only.iso");
        let tree = read_tree(&mut Cursor::new(&image)).unwrap();
        assert_eq!(tree.names, NameSource::Joliet);
        assert_eq!(tree.volume_label, "FEDORA-LIVE-40");
        let cfg = find(&tree.children, "EFI/BOOT/grub.cfg");
        assert!(content(&image, cfg).starts_with(b"search --no-floppy"));
        find(&tree.children, "LiveOS/squashfs.img");
    }

    #[test]
    fn reads_plain_iso9660_names_without_the_version_suffix() {
        let image = fixture("plain.iso");
        let tree = read_tree(&mut Cursor::new(&image)).unwrap();
        assert_eq!(tree.names, NameSource::Iso9660);
        let cfg = find(&tree.children, "ISOLINUX/ISOLINUX.CFG");
        assert_eq!(content(&image, cfg), b"default linux\n");
    }

    #[test]
    fn notices_a_windows_style_udf_image() {
        let image = fixture("windows-like.iso");
        let tree = read_tree(&mut Cursor::new(&image)).unwrap();
        assert!(tree.has_udf);
        assert_eq!(names(&tree.children), ["README.TXT"]);
    }

    #[test]
    fn rejects_something_that_is_not_an_iso() {
        let err = read_tree(&mut Cursor::new(vec![0u8; 100_000])).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    /// A minimal image: primary descriptor at sector 16, terminator at 17,
    /// root directory at sector 20 holding `records`.
    fn synthetic_image(records: &[Vec<u8>]) -> Vec<u8> {
        let mut image = vec![0u8; 40 * SECTOR as usize];
        let pvd = 16 * SECTOR as usize;
        image[pvd] = 1;
        image[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        image[pvd + 40..pvd + 44].copy_from_slice(b"TEST");
        let root = record(b"\0", 20, SECTOR as u32, FLAG_DIR);
        image[pvd + 156..pvd + 156 + root.len()].copy_from_slice(&root);
        let term = 17 * SECTOR as usize;
        image[term] = 255;
        image[term + 1..term + 6].copy_from_slice(b"CD001");

        let mut dir = record(b"\0", 20, SECTOR as u32, FLAG_DIR);
        dir.extend(record(b"\x01", 20, SECTOR as u32, FLAG_DIR));
        for r in records {
            dir.extend(r);
        }
        let at = 20 * SECTOR as usize;
        image[at..at + dir.len()].copy_from_slice(&dir);
        image
    }

    fn record(name: &[u8], extent: u32, size: u32, flags: u8) -> Vec<u8> {
        let len = 33 + name.len() + (1 - name.len() % 2);
        let mut r = vec![0u8; len];
        r[0] = len as u8;
        r[2..6].copy_from_slice(&extent.to_le_bytes());
        r[6..10].copy_from_slice(&extent.to_be_bytes());
        r[10..14].copy_from_slice(&size.to_le_bytes());
        r[14..18].copy_from_slice(&size.to_be_bytes());
        r[18] = 124; // 2024
        r[19] = 9;
        r[20] = 1;
        r[25] = flags;
        r[32] = name.len() as u8;
        r[33..33 + name.len()].copy_from_slice(name);
        r
    }

    #[test]
    fn joins_the_extents_of_a_file_over_4_gib() {
        let image = synthetic_image(&[
            record(b"BIG.SFS;1", 30, 4 * SECTOR as u32, FLAG_MULTI_EXTENT),
            record(b"BIG.SFS;1", 25, 1000, 0),
            record(b"SMALL.TXT;1", 35, 10, 0),
        ]);
        let tree = read_tree(&mut Cursor::new(&image)).unwrap();
        assert_eq!(names(&tree.children), ["BIG.SFS", "SMALL.TXT"]);
        let IsoNode::File { size, extents, .. } = &tree.children[0] else {
            panic!()
        };
        assert_eq!(*size, 4 * SECTOR + 1000);
        assert_eq!(
            extents,
            &[
                Extent {
                    offset: 30 * SECTOR,
                    len: 4 * SECTOR
                },
                Extent {
                    offset: 25 * SECTOR,
                    len: 1000
                }
            ]
        );
    }

    #[test]
    fn refuses_a_directory_loop() {
        // A subdirectory pointing back at the root.
        let image = synthetic_image(&[record(b"LOOP", 20, SECTOR as u32, FLAG_DIR)]);
        let err = read_tree(&mut Cursor::new(&image)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
