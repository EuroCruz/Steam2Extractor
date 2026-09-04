use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::aes::{cbc_decrypt, ecb_decrypt_block};
use crate::bail;
use crate::cp1252;
use crate::crc32::crc32;
use crate::error::{Error, Result};
use crate::filter::Filter;
use crate::inflate::raw_inflate;
use crate::sidcli::SidArgs;

const SIM_MAGIC: u32 = 0x3fd0_4c1f;
const PK_LOCAL_MAGIC: u32 = 0x0403_4b50;
const KEY2_CONSTANT: [u8; 16] = [
    0xA8, 0x19, 0x4D, 0x02, 0x19, 0x3C, 0xD0, 0x37, 0x92, 0x93, 0x7D, 0x27, 0x59, 0x0A, 0xEC, 0xBD,
];

struct SimRow {
    file_str_off: u32,
    path_str_off: u32,
    depot: u32,
    data_offset: u64,
    file_size: u64,
    disk_no: u8,
    disk_file_no: u8,
}

struct SimFile {
    string_table: Vec<u8>,
    rows: Vec<SimRow>,
}

fn read_u32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn read_u64(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}

fn cstr_at(table: &[u8], off: usize) -> Result<&[u8]> {
    let bytes = table
        .get(off..)
        .ok_or_else(|| Error::new("sim: string offset out of bounds"))?;
    let end = bytes
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| Error::new("sim: unterminated string"))?;
    Ok(&bytes[..end])
}

impl SimFile {
    fn parse(data: &[u8]) -> Result<SimFile> {
        if data.len() < 16 {
            bail!("sim: file too small");
        }
        let magic = read_u32(data, 0);
        if magic != SIM_MAGIC {
            bail!("sim: bad magic");
        }
        let str_table_size = read_u32(data, 12) as usize;
        let str_table_start = 16;
        let str_table_end = str_table_start + str_table_size;
        if data.len() < str_table_end + 8 {
            bail!("sim: file too small for string table");
        }
        let string_table = data[str_table_start..str_table_end].to_vec();

        let row_count = read_u32(data, str_table_end + 4) as usize;
        let rows_start = str_table_end + 8;
        let mut rows = Vec::with_capacity(row_count);
        for i in 0..row_count {
            let off = rows_start + i * 32;
            if data.len() < off + 32 {
                bail!("sim: table row out of bounds");
            }
            rows.push(SimRow {
                file_str_off: read_u32(data, off),
                path_str_off: read_u32(data, off + 4),
                depot: read_u32(data, off + 8),
                data_offset: read_u64(data, off + 12),
                file_size: read_u64(data, off + 20),
                disk_no: data[off + 28],
                disk_file_no: data[off + 29],
            });
        }

        Ok(SimFile { string_table, rows })
    }

    fn row_path(&self, row: &SimRow) -> Result<PathBuf> {
        let path_str =
            cp1252::decode_string(cstr_at(&self.string_table, row.path_str_off as usize)?);
        let file_str =
            cp1252::decode_string(cstr_at(&self.string_table, row.file_str_off as usize)?);
        let mut path = PathBuf::from(row.depot.to_string());
        let path_str = cp1252::sanitize_path(&path_str);
        if !path_str.is_empty() {
            path.push(path_str);
        }
        path.push(cp1252::sanitize_path(&file_str));
        Ok(path)
    }
}

fn find_sid_files(sim_path: &Path) -> Result<Vec<PathBuf>> {
    let dir = sim_path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = sim_path
        .file_stem()
        .ok_or_else(|| Error::new("sid: invalid .sim filename"))?
        .to_string_lossy()
        .into_owned();

    let mut files = Vec::new();
    let alt_first = dir.join(format!("{stem}.sid"));
    if alt_first.exists() {
        files.push(alt_first);
        let mut idx = 2;
        loop {
            let candidate = dir.join(format!("{stem}{idx}.sid"));
            if !candidate.exists() {
                break;
            }
            files.push(candidate);
            idx += 1;
        }
    } else {
        let mut idx = 0;
        loop {
            let candidate = dir.join(format!("{stem}_{idx}.sid"));
            if !candidate.exists() {
                break;
            }
            files.push(candidate);
            idx += 1;
        }
    }

    if files.is_empty() {
        bail!("sid: no .sid files found next to {}", sim_path.display());
    }
    Ok(files)
}

struct SidStream {
    disks: Vec<Vec<PathBuf>>,
    file: Option<File>,
    file_len: u64,
    disk_no: u8,
    sid_idx: usize,
}

impl SidStream {
    fn new(disks: Vec<Vec<PathBuf>>) -> SidStream {
        SidStream {
            disks,
            file: None,
            file_len: 0,
            disk_no: 0,
            sid_idx: 0,
        }
    }

    fn open(&mut self, disk_no: u8, sid_idx: usize) -> Result<()> {
        let disk = self
            .disks
            .get(disk_no.wrapping_sub(1) as usize)
            .ok_or_else(|| Error::new(format!("sid: missing disk {disk_no}")))?;
        let path = disk.get(sid_idx).ok_or_else(|| {
            Error::new(format!(
                "sid: missing .sid index {sid_idx} on disk {disk_no}"
            ))
        })?;
        let file = File::open(path)?;
        self.file_len = file.metadata()?.len();
        self.file = Some(file);
        self.disk_no = disk_no;
        self.sid_idx = sid_idx;
        Ok(())
    }

    fn ensure_open(&mut self, disk_no: u8, sid_idx: usize) -> Result<()> {
        if self.file.is_some() && self.disk_no == disk_no && self.sid_idx == sid_idx {
            return Ok(());
        }
        self.open(disk_no, sid_idx)
    }

    fn advance(&mut self) -> Result<()> {
        let disk_len = self.disks[self.disk_no.wrapping_sub(1) as usize].len();
        if self.sid_idx + 1 < disk_len {
            self.open(self.disk_no, self.sid_idx + 1)
        } else if (self.disk_no as usize) < self.disks.len() {
            self.open(self.disk_no + 1, 0)
        } else {
            bail!("sid: ran out of .sid files while reading a block")
        }
    }

    fn seek(&mut self, pos: u64) -> Result<()> {
        self.file.as_mut().unwrap().seek(SeekFrom::Start(pos))?;
        Ok(())
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        self.file.as_mut().unwrap().read_exact(buf)?;
        Ok(())
    }
}

struct BlockHeader {
    size: u32,
    pad_size: u8,
    flags: u8,
}

fn read_u24_le(b: &[u8]) -> u32 {
    b[0] as u32 | (b[1] as u32) << 8 | (b[2] as u32) << 16
}

fn read_block_header(stream: &mut SidStream) -> Result<BlockHeader> {
    let mut buf = [0u8; 8];
    stream.read_exact(&mut buf)?;
    let size = read_u24_le(&buf[0..3]);
    let pad_size = buf[3];
    let flags = buf[7];
    if flags > 3 {
        bail!("sid: unexpected block flags {}", flags);
    }
    if pad_size as u32 > size {
        bail!("sid: pad size larger than block size");
    }
    Ok(BlockHeader {
        size,
        pad_size,
        flags,
    })
}

struct PkLocalHeader {
    crc32: u32,
    header_len: usize,
}

fn parse_pk_header(data: &[u8]) -> Result<PkLocalHeader> {
    if data.len() < 30 {
        bail!("sid: block too small for PK local file header");
    }
    let signature = read_u32(data, 0);
    if signature != PK_LOCAL_MAGIC {
        bail!("sid: expected PK local file header");
    }
    let crc32 = read_u32(data, 14);
    let filename_size = u16::from_le_bytes(data[26..28].try_into().unwrap()) as usize;
    let extra_size = u16::from_le_bytes(data[28..30].try_into().unwrap()) as usize;
    let header_len = 30 + filename_size + extra_size;
    if data.len() < header_len {
        bail!("sid: PK header fields exceed block size");
    }
    Ok(PkLocalHeader { crc32, header_len })
}

fn extract_entry(stream: &mut SidStream, row: &SimRow, key: &[u8; 16]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(row.file_size as usize);
    let mut next_offset = row.data_offset;
    stream.ensure_open(row.disk_no, row.disk_file_no as usize)?;

    while (out.len() as u64) < row.file_size {
        if next_offset >= stream.file_len {
            stream.advance()?;
            next_offset = 0;
        }
        stream.seek(next_offset)?;
        let hdr = read_block_header(stream)?;
        next_offset += 8 + hdr.size as u64;

        let encrypted = hdr.flags & 1 != 0;
        let compressed = hdr.flags & 2 != 0;

        let mut payload_len = hdr.size as usize;
        let mut iv = None;
        let mut key32 = [0u8; 32];
        if encrypted {
            key32[..16].copy_from_slice(key);
            key32[16..].copy_from_slice(&KEY2_CONSTANT);

            let mut iv_enc = [0u8; 16];
            stream.read_exact(&mut iv_enc)?;
            iv = Some(ecb_decrypt_block(&key32, &iv_enc));
            payload_len -= 16;
        }

        let mut buf = vec![0u8; payload_len];
        stream.read_exact(&mut buf)?;
        if let Some(iv) = iv {
            cbc_decrypt(&key32, &iv, &mut buf);
        }

        let real_len = (hdr.size as usize)
            .checked_sub(hdr.pad_size as usize)
            .ok_or_else(|| Error::new("sid: pad size exceeds block size"))?;
        if real_len > payload_len {
            bail!("sid: pad size too small for encrypted block");
        }
        let real = &buf[..real_len];

        if compressed {
            let pk = parse_pk_header(real)?;
            let deflated = &real[pk.header_len..];
            let remaining = (row.file_size - out.len() as u64) as usize;
            let inflated = raw_inflate(deflated, remaining.min(1 << 20))?;
            if crc32(0, &inflated) != pk.crc32 {
                bail!("sid: CRC32 mismatch while decompressing block");
            }
            out.extend_from_slice(&inflated);
        } else {
            out.extend_from_slice(real);
        }
    }

    if out.len() as u64 != row.file_size {
        bail!("sid: output size doesn't match advertised file size");
    }

    Ok(out)
}

pub fn run(args: &SidArgs) -> Result<()> {
    let sim_paths: Vec<PathBuf> = args.sim_files.iter().map(PathBuf::from).collect();
    let sim_data = fs::read(&sim_paths[0])?;
    let sim = SimFile::parse(&sim_data)?;

    let mut disks = Vec::with_capacity(sim_paths.len());
    for sim_path in &sim_paths {
        disks.push(find_sid_files(sim_path)?);
    }
    let mut stream = SidStream::new(disks);

    let filter = Filter::new(&args.filter)?;

    let base = PathBuf::from(args.out.clone().unwrap_or_else(|| "extracted".to_string()));
    let mut warned_depots = std::collections::HashSet::new();

    println!("{} entries in manifest", sim.rows.len());

    for row in &sim.rows {
        let rel_path = sim.row_path(row)?;
        let rel_str = rel_path.to_string_lossy().into_owned();

        if !filter.matches(&rel_str) {
            continue;
        }

        let key = args.keys.resolve(row.depot).unwrap_or_else(|| {
            if warned_depots.insert(row.depot) {
                eprintln!(
                    "no known key for depot {}; some depots use an all-zero key, trying that",
                    row.depot
                );
            }
            [0u8; 16]
        });

        let final_path = base.join(&rel_path);
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }

        print!("{rel_str}: ");
        let data = extract_entry(&mut stream, row, &key)?;
        let mut out_file = File::create(&final_path)?;
        out_file.write_all(&data)?;
        println!("OK");
    }

    Ok(())
}
