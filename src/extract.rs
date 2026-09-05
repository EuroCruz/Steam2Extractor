use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use crate::bail;
use crate::blob::{Blob, decompress_blob, rb32, value_u32, value_u64};
use crate::chunk::handle_chunk;
use crate::cli::Args;
use crate::cp1252;
use crate::error::{Error, Result};
use crate::filter::Filter;
use crate::manifest::{CompressionType, Manifest};
use crate::reader::ByteReader;
use crate::source::Source;

const CHECKSUM_TABLE_MAGIC: u32 = 0x34457234;

#[derive(Default, Clone)]
struct FileIdMapping {
    filemode: u8,
    offset: u64,
}

#[derive(Clone, Copy)]
struct ChecksumEntry {
    compressed_size: u32,
}

#[derive(Default, Clone)]
struct FileIdInfo {
    info: FileIdMapping,
    checksums: Vec<ChecksumEntry>,
    part: i64,
}

fn parse_leading_int(s: &str, pos: &mut usize) -> Result<i64> {
    let bytes = s.as_bytes();
    let start = *pos;
    let neg = *pos < bytes.len() && bytes[*pos] == b'-';
    if neg {
        *pos += 1;
    }
    let digits_start = *pos;
    while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
        *pos += 1;
    }
    if *pos == digits_start {
        bail!("could not parse integer in '{}'", s);
    }
    s[start..*pos]
        .parse()
        .map_err(|_| Error::new(format!("could not parse integer in '{}'", s)))
}

fn parse_two_ints_prefix(s: &str) -> Result<(i64, i64)> {
    let mut pos = 0;
    let a = parse_leading_int(s, &mut pos)?;
    if s.as_bytes().get(pos) != Some(&b'_') {
        bail!("expected '_' while parsing '{}'", s);
    }
    pos += 1;
    let b = parse_leading_int(s, &mut pos)?;
    Ok((a, b))
}

fn parse_out_checksum_info(
    blob_source: &Source,
    filename: &str,
) -> Result<BTreeMap<u32, FileIdInfo>> {
    let (_depot, archive_part) = parse_two_ints_prefix(filename)?;

    let path = blob_source.resolve(filename)?;
    let data = fs::read(&path)?;
    let blob = Blob::parse(&data)?;
    let csum = blob
        .get(&rb32(4))
        .ok_or_else(|| Error::new("parse_out_checksum_info: missing checksum table key"))?;

    let mut r = ByteReader::new(csum);
    let magic = r.read_u32()?;
    let version = r.read_u32()?;
    let num_fileblocks = r.read_u32()?;
    let num_items = r.read_u32()?;
    let offset1 = r.read_u32()?;
    let offset2 = r.read_u32()?;
    let blocksize = r.read_u32()?;
    let largest_num_blocks = r.read_u32()?;

    if magic != CHECKSUM_TABLE_MAGIC {
        bail!("parse_out_checksum_info: bad file magic");
    }
    if blocksize != 0x8000 {
        bail!("parse_out_checksum_info: blocksize != 0x8000");
    }
    if version != 0 && version != 1 {
        bail!("parse_out_checksum_info: bad version");
    }
    if offset1 != 0x20 {
        bail!("parse_out_checksum_info: bad fileidtable offset");
    }
    if offset2 != 0x20 + 0x10 * num_fileblocks {
        bail!("parse_out_checksum_info: bad mapping table offset");
    }

    struct TableEntry {
        fileid_start: u32,
        filecount: u32,
        offset: u32,
    }
    if num_fileblocks as usize > csum.len() / 16 {
        bail!("parse_out_checksum_info: implausible fileblock count");
    }
    let mut table = Vec::with_capacity(num_fileblocks as usize);
    for _ in 0..num_fileblocks {
        let fileid_start = r.read_u32()?;
        let filecount = r.read_u32()?;
        let offset = r.read_u32()?;
        let _dummy4 = r.read_u32()?;
        table.push(TableEntry {
            fileid_start,
            filecount,
            offset,
        });
    }

    let mut filecount_actual = 0u32;
    let mut max_blocks_actual = 0u32;
    let mut fileids = BTreeMap::new();

    for entry in &table {
        if r.pos as u32 != entry.offset {
            bail!("parse_out_checksum_info: reader position != offset");
        }
        filecount_actual += entry.filecount;
        for fileid in entry.fileid_start..entry.fileid_start + entry.filecount {
            let (filesize_or_offset_pair, num_blocks_raw) = if version == 0 {
                let _filesize = r.read_u32()? as u64;
                let offset = r.read_u32()? as u64;
                let raw = r.read_u32()?;
                (offset, raw)
            } else {
                let _filesize = r.read_u64()?;
                let offset = r.read_u64()?;
                let raw = r.read_u32()?;
                (offset, raw)
            };
            let filemode = (num_blocks_raw >> 24) as u8;
            let numblocks = num_blocks_raw & 0x00ff_ffff;
            if !(filemode == 1 || filemode == 2 || filemode == 3) {
                bail!("parse_out_checksum_info: filemode out of range");
            }
            max_blocks_actual = max_blocks_actual.max(numblocks);

            if numblocks as usize > csum.len() / 8 {
                bail!("parse_out_checksum_info: implausible block count");
            }
            let mut checksums = Vec::with_capacity(numblocks as usize);
            for _ in 0..numblocks {
                let compressed_size = r.read_u32()?;
                let _checksum = r.read_u32()?;
                checksums.push(ChecksumEntry { compressed_size });
            }

            fileids.insert(
                fileid,
                FileIdInfo {
                    info: FileIdMapping {
                        filemode,
                        offset: filesize_or_offset_pair,
                    },
                    checksums,
                    part: archive_part,
                },
            );
        }
    }

    if r.read_u32()? != CHECKSUM_TABLE_MAGIC {
        bail!("parse_out_checksum_info: bad footer magic");
    }
    if max_blocks_actual != largest_num_blocks {
        bail!("parse_out_checksum_info: maximum file blockcount != header count");
    }
    if filecount_actual != num_items {
        bail!(
            "parse_out_checksum_info: actual count of files is different than one reported in the header"
        );
    }

    Ok(fileids)
}

struct WantedFiles {
    dats: BTreeMap<i64, String>,
    blobs: BTreeMap<i64, String>,
}

fn find_wanted_files_naive(
    blob_source: &Source,
    dat_source: &Source,
    depot: u32,
    version: u32,
) -> Result<WantedFiles> {
    let prefix = format!("{}_", depot);
    let mut wanted_blobs = BTreeMap::new();
    for filename in blob_source.list(depot, &prefix, ".blob")? {
        let (_depot, blobver) = parse_two_ints_prefix(&filename)?;
        if wanted_blobs.contains_key(&blobver) {
            bail!(
                "find_wanted_files[naive]: more than one blob found, please pass --blobcrc to specify exactly which blob you want"
            );
        }
        if blobver <= version as i64 {
            wanted_blobs.insert(blobver, filename);
        }
    }

    let mut wanted_dats = BTreeMap::new();
    for filename in dat_source.list(depot, &prefix, ".dat")? {
        let (_depot, blobver) = parse_two_ints_prefix(&filename)?;
        if wanted_dats.contains_key(&blobver) {
            bail!(
                "find_wanted_files[naive]: more than one dat found, please pass --blobcrc to specify exactly which blob you want"
            );
        }
        if blobver <= version as i64 {
            wanted_dats.insert(blobver, filename);
        }
    }

    for i in 0..=version as i64 {
        if !wanted_dats.contains_key(&i) || !wanted_blobs.contains_key(&i) {
            bail!("find_wanted_files[naive]: missing a blob or a dat file!");
        }
    }

    Ok(WantedFiles {
        dats: wanted_dats,
        blobs: wanted_blobs,
    })
}

fn find_wanted_files_smart(
    blob_source: &Source,
    dat_source: &Source,
    depot: u32,
    version: u32,
    crc_top: &str,
) -> Result<WantedFiles> {
    let prefix = format!("{}_", depot);
    let considered_blobs = blob_source.list(depot, &prefix, ".blob")?;
    let considered_dats = dat_source.list(depot, &prefix, ".dat")?;

    let looking_for_top = format!("{}_{}_{}_", depot, version, crc_top);
    let top_blob = considered_blobs
        .iter()
        .find(|f| f.starts_with(&looking_for_top))
        .cloned()
        .ok_or_else(|| Error::new("no blob found with this crc value"))?;

    let mut wanted_dats = BTreeMap::new();
    let mut wanted_blobs = BTreeMap::new();
    wanted_blobs.insert(version as i64, top_blob.clone());

    let mut current_blob = top_blob;
    let mut current_version = version as i64;
    loop {
        let path = blob_source.resolve(&current_blob)?;
        let data = fs::read(&path)?;
        let blob = Blob::parse(&data)?;

        let format_code = value_u32(
            blob.get(&rb32(0))
                .ok_or_else(|| Error::new("smart: missing format code key"))?,
        )?;
        let dat_size = if format_code == 3 {
            value_u32(
                blob.get(&rb32(13))
                    .ok_or_else(|| Error::new("smart: missing dat size key"))?,
            )? as u64
        } else if format_code == 4 {
            value_u64(
                blob.get(&rb32(13))
                    .ok_or_else(|| Error::new("smart: missing dat size key"))?,
            )?
        } else {
            bail!("find_wanted_files[smart]: unknown blob format code");
        };

        let looking_for_dat = format!("{}_{}_", depot, current_version);
        let mut our_dat = None;
        for candidate in &considered_dats {
            if candidate.starts_with(&looking_for_dat) && dat_source.size(candidate)? == dat_size {
                our_dat = Some(candidate.clone());
                break;
            }
        }
        let our_dat = our_dat.ok_or_else(|| {
            Error::new("find_wanted_files[smart]: no corresponding dat file for blob")
        })?;
        wanted_dats.insert(current_version, our_dat);

        if current_version == 0 {
            break;
        }

        let prev_crc = value_u32(
            blob.get(&rb32(12))
                .ok_or_else(|| Error::new("smart: missing prev crc key"))?,
        )?;
        let looking_for = format!("{}_{}_{:08x}_", depot, current_version - 1, prev_crc);
        let parent = considered_blobs
            .iter()
            .find(|f| f.starts_with(&looking_for))
            .cloned()
            .ok_or_else(|| {
                Error::new("find_wanted_files[smart]: couldn't find a child blob (missing/valve fuckup/corrupted parent blob)")
            })?;
        wanted_blobs.insert(current_version - 1, parent.clone());

        current_blob = parent;
        current_version -= 1;
    }

    Ok(WantedFiles {
        dats: wanted_dats,
        blobs: wanted_blobs,
    })
}

pub fn run(args: &Args) -> Result<()> {
    let filter = Filter::new(&args.filter)?;

    let key = args.keys.resolve(args.depot).unwrap_or_else(|| {
        eprintln!(
            "no known key for depot {}; some depots use an all-zero key, trying that",
            args.depot
        );
        [0u8; 16]
    });

    let blob_source = Source::blobs(PathBuf::from(&args.blob_dir), args.offline);
    let dat_source = Source::dats(PathBuf::from(&args.dat_dir), args.offline);

    let wanted = match &args.blobcrc {
        Some(crc) => {
            find_wanted_files_smart(&blob_source, &dat_source, args.depot, args.version, crc)?
        }
        None => find_wanted_files_naive(&blob_source, &dat_source, args.depot, args.version)?,
    };

    let mut fileids: BTreeMap<u32, FileIdInfo> = BTreeMap::new();
    for filename in wanted.blobs.values() {
        let parsed = parse_out_checksum_info(&blob_source, filename)?;
        fileids.extend(parsed);
    }
    println!("fileid table created");

    let mut dat_files: BTreeMap<i64, fs::File> = BTreeMap::new();
    for (&index, filename) in &wanted.dats {
        let path = dat_source.resolve(filename)?;
        dat_files.insert(index, fs::File::open(path)?);
    }
    println!("dat files opened");

    let last_blob_name = wanted.blobs.values().last().unwrap();
    let last_blob_path = blob_source.resolve(last_blob_name)?;
    let last_blob_data = fs::read(&last_blob_path)?;
    let last_blob = Blob::parse(&last_blob_data)?;
    let manifest_container = last_blob
        .get(&rb32(3))
        .ok_or_else(|| Error::new("extract: missing manifest key in top blob"))?;
    let manifest_blob_data = decompress_blob(manifest_container)?;
    let manifest_blob = Blob::parse(&manifest_blob_data)?;
    let manifest_data = manifest_blob
        .get(&rb32(0))
        .ok_or_else(|| Error::new("extract: missing manifest data key"))?;
    let manifest = Manifest::parse(manifest_data)?;

    println!(
        "manifest loaded {} {}",
        manifest.header.app_id, manifest.header.ver_id
    );

    let base = match &args.out {
        Some(out) => PathBuf::from(out),
        None => PathBuf::from(format!(
            "{}_{}",
            manifest.header.app_id, manifest.header.ver_id
        )),
    };

    for entry in &manifest.nodes {
        let raw_rel_path = manifest
            .id_to_path
            .get(&entry.file_id)
            .cloned()
            .unwrap_or_default();
        let rel_path = cp1252::sanitize_path(&raw_rel_path);

        if !filter.matches(&rel_path) {
            continue;
        }
        if entry.flags == 0 {
            continue;
        }

        let final_path = base.join(&rel_path);
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out_file = fs::File::create(&final_path)?;

        let info = fileids.get(&entry.file_id).cloned().unwrap_or_default();
        let mut current_offset = info.info.offset;
        let cmptype = if info.checksums.is_empty() {
            None
        } else {
            Some(CompressionType::from_u8(info.info.filemode)?)
        };

        for block in &info.checksums {
            if block.compressed_size == 0 {
                continue;
            }
            if block.compressed_size > 0x10000 {
                bail!("extract: implausible compressed block size");
            }
            let dat_file = dat_files
                .get_mut(&info.part)
                .ok_or_else(|| Error::new("extract: missing dat file part for entry"))?;
            dat_file.seek(SeekFrom::Start(current_offset))?;
            let mut buf = vec![0u8; block.compressed_size as usize];
            dat_file.read_exact(&mut buf)?;
            let decoded = handle_chunk(&buf, cmptype.unwrap(), &key)?;
            out_file.write_all(&decoded)?;
            current_offset += block.compressed_size as u64;
        }
    }

    Ok(())
}
