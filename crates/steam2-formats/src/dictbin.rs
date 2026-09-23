use steam2_compression::{lzx, sges};
use steam2_core::endian::Endian;
use steam2_hash::pandemic::pandemic;

const HEADER_SIZE: usize = 16;
const FLAG_LZXD: u32 = 1;
const FLAG_SGES: u32 = 2;

const MAGIC_KEYS: &[u8; 4] = b"WSDK";
const MAGIC_BLOBS: &[u8; 4] = b"WSDB";
const MAGIC_DATS: &[u8; 4] = b"WSDD";

pub fn bin_dir() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("executable has no parent directory")?;
    Ok(dir.join("bin"))
}

pub fn load(name: &str) -> Result<Vec<u8>, String> {
    let path = bin_dir()?.join(name);
    std::fs::read(&path).map_err(|_| format!("/bin/{name} not found"))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Raw,
    Lzxd,
    Sges,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Keys,
    Blobs,
    Dats,
}

impl Kind {
    fn magic(self) -> &'static [u8; 4] {
        match self {
            Kind::Keys => MAGIC_KEYS,
            Kind::Blobs => MAGIC_BLOBS,
            Kind::Dats => MAGIC_DATS,
        }
    }

    fn from_magic(magic: &[u8]) -> Option<Kind> {
        match magic {
            m if m == MAGIC_KEYS || m == reversed(MAGIC_KEYS) => Some(Kind::Keys),
            m if m == MAGIC_BLOBS || m == reversed(MAGIC_BLOBS) => Some(Kind::Blobs),
            m if m == MAGIC_DATS || m == reversed(MAGIC_DATS) => Some(Kind::Dats),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Keys => "keys",
            Kind::Blobs => "blobs",
            Kind::Dats => "dats",
        }
    }

    pub fn file_name(self) -> &'static str {
        match self {
            Kind::Keys => "steam2_keys.bin",
            Kind::Blobs => "steam2_blobs.bin",
            Kind::Dats => "steam2_dats.bin",
        }
    }
}

fn reversed(magic: &[u8; 4]) -> [u8; 4] {
    let mut r = *magic;
    r.reverse();
    r
}

fn magic_for(kind: Kind, endian: Endian) -> [u8; 4] {
    match endian {
        Endian::Little => *kind.magic(),
        Endian::Big => reversed(kind.magic()),
    }
}

#[derive(Clone)]
pub struct KeyRecord {
    pub depot: u32,
    pub key: [u8; 16],
    pub pandemic: u32,
}

#[derive(Clone)]
pub struct NameRecord {
    pub depot: u32,
    pub version: u32,
    pub crc: u32,
    pub hash: [u8; 32],
    pub pandemic: u32,
}

impl NameRecord {
    pub fn filename(&self, suffix: &str) -> String {
        format!("{}_{}_{:08x}_{}{suffix}", self.depot, self.version, self.crc, hex_encode(&self.hash))
    }

    fn compute_pandemic(depot: u32, version: u32, crc: u32, hash: &[u8; 32], suffix: &str) -> u32 {
        pandemic(&format!("{depot}_{version}_{crc:08x}_{}{suffix}", hex_encode(hash)))
    }
}

impl KeyRecord {
    fn compute_pandemic(depot: u32) -> u32 {
        pandemic(&format!("depot:{depot}"))
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0xf) as usize] as char);
    }
    s
}

pub fn new_key_record(depot: u32, key: [u8; 16]) -> KeyRecord {
    KeyRecord { depot, key, pandemic: KeyRecord::compute_pandemic(depot) }
}

pub fn new_blob_record(depot: u32, version: u32, crc: u32, hash: [u8; 32]) -> NameRecord {
    let pandemic = NameRecord::compute_pandemic(depot, version, crc, &hash, ".blob");
    NameRecord { depot, version, crc, hash, pandemic }
}

pub fn new_dat_record(depot: u32, version: u32, crc: u32, hash: [u8; 32]) -> NameRecord {
    let pandemic = NameRecord::compute_pandemic(depot, version, crc, &hash, ".dat");
    NameRecord { depot, version, crc, hash, pandemic }
}

fn decode_hex_hash(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn decode_hex_key(hex: &str) -> Option<[u8; 16]> {
    if hex.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

pub fn parse_keys_text(text: &str) -> Result<Vec<KeyRecord>, String> {
    let mut keys = Vec::new();
    for (lineno, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (depot, hexkey) = line.split_once('\t').ok_or_else(|| format!("line {}: expected depot\\tkey", lineno + 1))?;
        let depot: u32 = depot.parse().map_err(|_| format!("line {}: bad depot", lineno + 1))?;
        let key = decode_hex_key(hexkey).ok_or_else(|| format!("line {}: bad key hex", lineno + 1))?;
        keys.push(new_key_record(depot, key));
    }
    Ok(keys)
}

fn parse_names_text(text: &str, new_record: impl Fn(u32, u32, u32, [u8; 32]) -> NameRecord) -> Result<Vec<NameRecord>, String> {
    let mut names = Vec::new();
    for (lineno, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [depot, version, crc, hash] = fields.as_slice() else {
            return Err(format!("line {}: expected depot\\tversion\\tcrc\\thash", lineno + 1));
        };
        let depot: u32 = depot.parse().map_err(|_| format!("line {}: bad depot", lineno + 1))?;
        let version: u32 = version.parse().map_err(|_| format!("line {}: bad version", lineno + 1))?;
        let crc = u32::from_str_radix(crc, 16).map_err(|_| format!("line {}: bad crc", lineno + 1))?;
        let hash_bytes = decode_hex_hash(hash).ok_or_else(|| format!("line {}: bad hash hex", lineno + 1))?;
        names.push(new_record(depot, version, crc, hash_bytes));
    }
    Ok(names)
}

pub fn parse_blobs_text(text: &str) -> Result<Vec<NameRecord>, String> {
    parse_names_text(text, new_blob_record)
}

pub fn parse_dats_text(text: &str) -> Result<Vec<NameRecord>, String> {
    parse_names_text(text, new_dat_record)
}

fn encode_keys(records: &[KeyRecord], endian: Endian) -> Vec<u8> {
    let mut out = Vec::with_capacity(records.len() * 24);
    for k in records {
        endian.write(&mut out, 4, k.depot as u64);
        out.extend_from_slice(&k.key);
        endian.write(&mut out, 4, k.pandemic as u64);
    }
    out
}

fn encode_names(records: &[NameRecord], endian: Endian) -> Vec<u8> {
    let mut out = Vec::with_capacity(records.len() * 48);
    for n in records {
        endian.write(&mut out, 4, n.depot as u64);
        endian.write(&mut out, 4, n.version as u64);
        endian.write(&mut out, 4, n.crc as u64);
        out.extend_from_slice(&n.hash);
        endian.write(&mut out, 4, n.pandemic as u64);
    }
    out
}

fn decode_keys(payload: &[u8], count: usize, endian: Endian) -> Result<Vec<KeyRecord>, String> {
    let mut keys = Vec::with_capacity(count);
    for i in 0..count {
        let off = i * 24;
        let rec = payload.get(off..off + 24).ok_or("dictbin: truncated key record")?;
        let depot = endian.read(&rec[0..4]) as u32;
        let key: [u8; 16] = rec[4..20].try_into().unwrap();
        let pandemic = endian.read(&rec[20..24]) as u32;
        keys.push(KeyRecord { depot, key, pandemic });
    }
    Ok(keys)
}

fn decode_names(payload: &[u8], count: usize, endian: Endian) -> Result<Vec<NameRecord>, String> {
    let mut names = Vec::with_capacity(count);
    for i in 0..count {
        let off = i * 48;
        let rec = payload.get(off..off + 48).ok_or("dictbin: truncated name record")?;
        let depot = endian.read(&rec[0..4]) as u32;
        let version = endian.read(&rec[4..8]) as u32;
        let crc = endian.read(&rec[8..12]) as u32;
        let hash: [u8; 32] = rec[12..44].try_into().unwrap();
        let pandemic = endian.read(&rec[44..48]) as u32;
        names.push(NameRecord { depot, version, crc, hash, pandemic });
    }
    Ok(names)
}

fn build_container(kind: Kind, count: usize, payload: Vec<u8>, compression: Compression, endian: Endian) -> Vec<u8> {
    let (flags, stored) = match compression {
        Compression::Raw => (0u32, payload.clone()),
        Compression::Lzxd => (FLAG_LZXD, lzx::compress(&payload)),
        Compression::Sges => (FLAG_SGES, sges::compress(&payload, endian)),
    };

    let mut out = Vec::with_capacity(HEADER_SIZE + stored.len());
    out.extend_from_slice(&magic_for(kind, endian));
    endian.write(&mut out, 4, count as u64);
    endian.write(&mut out, 4, flags as u64);
    endian.write(&mut out, 4, payload.len() as u64);
    out.extend_from_slice(&stored);
    out
}

pub fn build_keys(records: &[KeyRecord], compression: Compression, endian: Endian) -> Vec<u8> {
    build_container(Kind::Keys, records.len(), encode_keys(records, endian), compression, endian)
}

pub fn build_blobs(records: &[NameRecord], compression: Compression, endian: Endian) -> Vec<u8> {
    build_container(Kind::Blobs, records.len(), encode_names(records, endian), compression, endian)
}

pub fn build_dats(records: &[NameRecord], compression: Compression, endian: Endian) -> Vec<u8> {
    build_container(Kind::Dats, records.len(), encode_names(records, endian), compression, endian)
}

struct Header {
    kind: Kind,
    count: usize,
    flags: u32,
    uncompressed_size: usize,
    endian: Endian,
}

fn parse_header(data: &[u8]) -> Result<Header, String> {
    if data.len() < HEADER_SIZE {
        return Err("dictbin: truncated header".to_string());
    }
    let kind = Kind::from_magic(&data[0..4]).ok_or("dictbin: not a Steam2ExtractV2 dictionary")?;
    let endian = if data[0..4] == *kind.magic() { Endian::Little } else { Endian::Big };
    let count = endian.read(&data[4..8]) as usize;
    let flags = endian.read(&data[8..12]) as u32;
    let uncompressed_size = endian.read(&data[12..16]) as usize;
    Ok(Header { kind, count, flags, uncompressed_size, endian })
}

fn payload_of(data: &[u8], header: &Header) -> Result<Vec<u8>, String> {
    let stored = &data[HEADER_SIZE..];
    let payload = if header.flags & FLAG_LZXD != 0 {
        lzx::decompress(stored, header.uncompressed_size)?
    } else if header.flags & FLAG_SGES != 0 {
        sges::decompress(stored, header.uncompressed_size)?
    } else {
        stored.to_vec()
    };
    if payload.len() != header.uncompressed_size {
        return Err(format!("dictbin: decoded payload size {} does not match header ({})", payload.len(), header.uncompressed_size));
    }
    Ok(payload)
}

pub struct ParsedKeys {
    pub records: Vec<KeyRecord>,
    pub endian: Endian,
    pub flags: u32,
}

pub struct ParsedNames {
    pub records: Vec<NameRecord>,
    pub endian: Endian,
    pub flags: u32,
}

pub fn parse_keys(data: &[u8]) -> Result<ParsedKeys, String> {
    let header = parse_header(data)?;
    if header.kind != Kind::Keys {
        return Err("dictbin: expected a keys dictionary".to_string());
    }
    let payload = payload_of(data, &header)?;
    let records = decode_keys(&payload, header.count, header.endian)?;
    Ok(ParsedKeys { records, endian: header.endian, flags: header.flags })
}

pub fn parse_blobs(data: &[u8]) -> Result<ParsedNames, String> {
    let header = parse_header(data)?;
    if header.kind != Kind::Blobs {
        return Err("dictbin: expected a blobs dictionary".to_string());
    }
    let payload = payload_of(data, &header)?;
    let records = decode_names(&payload, header.count, header.endian)?;
    Ok(ParsedNames { records, endian: header.endian, flags: header.flags })
}

pub fn parse_dats(data: &[u8]) -> Result<ParsedNames, String> {
    let header = parse_header(data)?;
    if header.kind != Kind::Dats {
        return Err("dictbin: expected a dats dictionary".to_string());
    }
    let payload = payload_of(data, &header)?;
    let records = decode_names(&payload, header.count, header.endian)?;
    Ok(ParsedNames { records, endian: header.endian, flags: header.flags })
}

pub fn peek_kind(data: &[u8]) -> Option<Kind> {
    data.get(0..4).and_then(Kind::from_magic)
}

pub fn compression_label(flags: u32) -> &'static str {
    if flags & FLAG_LZXD != 0 {
        "lzxd"
    } else if flags & FLAG_SGES != 0 {
        "sges"
    } else {
        "raw"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_compressions_and_endians() {
        let keys = vec![new_key_record(1, [0xab; 16]), new_key_record(2, [0xcd; 16])];
        let blobs = vec![new_blob_record(1, 0, 0x1234, [1u8; 32])];
        let dats = vec![new_dat_record(1, 0, 0x1234, [2u8; 32])];

        for endian in [Endian::Little, Endian::Big] {
            for compression in [Compression::Raw, Compression::Lzxd, Compression::Sges] {
                let keys_bytes = build_keys(&keys, compression, endian);
                let parsed_keys = parse_keys(&keys_bytes).unwrap();
                assert_eq!(parsed_keys.endian, endian);
                assert_eq!(parsed_keys.records.len(), 2);
                assert_eq!(parsed_keys.records[1].key, [0xcd; 16]);

                let blobs_bytes = build_blobs(&blobs, compression, endian);
                let parsed_blobs = parse_blobs(&blobs_bytes).unwrap();
                assert_eq!(
                    parsed_blobs.records[0].filename(".blob"),
                    "1_0_00001234_0101010101010101010101010101010101010101010101010101010101010101.blob"
                );

                let dats_bytes = build_dats(&dats, compression, endian);
                assert!(parse_dats(&dats_bytes).is_ok());
                assert_eq!(peek_kind(&keys_bytes), Some(Kind::Keys));
                assert_eq!(peek_kind(&blobs_bytes), Some(Kind::Blobs));
                assert_eq!(peek_kind(&dats_bytes), Some(Kind::Dats));
            }
        }
    }

    #[test]
    fn pandemic_field_is_deterministic() {
        let a = new_blob_record(5, 1, 0xdead, [0u8; 32]);
        let b = new_blob_record(5, 1, 0xdead, [0u8; 32]);
        assert_eq!(a.pandemic, b.pandemic);
        let c = new_blob_record(5, 1, 0xdead, [1u8; 32]);
        assert_ne!(a.pandemic, c.pandemic);
    }

    #[test]
    fn wrong_kind_is_rejected() {
        let keys_bytes = build_keys(&[new_key_record(1, [0xab; 16])], Compression::Raw, Endian::Little);
        assert!(parse_blobs(&keys_bytes).is_err());
    }
}
