use std::sync::OnceLock;

use crate::dbcrypt;

const MAGIC: &[u8; 8] = b"S2DBv001";
const KEY_RECORD: usize = 20;
const NAME_RECORD: usize = 44;

const RAW: &[u8] = include_bytes!("../assets/database.bin");

struct Database {
    bytes: Vec<u8>,
    keys: (usize, usize),
    blobs: (usize, usize),
    dats: (usize, usize),
}

fn load() -> Database {
    let mut bytes = RAW.to_vec();
    dbcrypt::decrypt(&mut bytes);
    assert_eq!(&bytes[0..8], MAGIC, "db: corrupt embedded database");
    let keys_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let blobs_count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let dats_count = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;

    let keys_start = 20;
    let keys_end = keys_start + keys_count * KEY_RECORD;
    let blobs_end = keys_end + blobs_count * NAME_RECORD;
    let dats_end = blobs_end + dats_count * NAME_RECORD;
    assert_eq!(bytes.len(), dats_end, "db: corrupt embedded database");

    Database {
        bytes,
        keys: (keys_start, keys_end),
        blobs: (keys_end, blobs_end),
        dats: (blobs_end, dats_end),
    }
}

fn database() -> &'static Database {
    static DB: OnceLock<Database> = OnceLock::new();
    DB.get_or_init(load)
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

fn record_depot(section: &[u8], record_size: usize, idx: usize) -> u32 {
    let off = idx * record_size;
    u32::from_le_bytes(section[off..off + 4].try_into().unwrap())
}

fn depot_range(section: &[u8], record_size: usize, depot: u32) -> std::ops::Range<usize> {
    let n = section.len() / record_size;
    let mut lo = 0;
    let mut hi = n;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if record_depot(section, record_size, mid) < depot {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let start = lo;
    let mut end = start;
    while end < n && record_depot(section, record_size, end) == depot {
        end += 1;
    }
    start..end
}

pub fn lookup_key(depot: u32) -> Option<[u8; 16]> {
    let db = database();
    let section = &db.bytes[db.keys.0..db.keys.1];
    let range = depot_range(section, KEY_RECORD, depot);
    range.into_iter().next().map(|idx| {
        let off = idx * KEY_RECORD;
        section[off + 4..off + 20].try_into().unwrap()
    })
}

fn filenames(section: &[u8], depot: u32, suffix: &str) -> Vec<String> {
    depot_range(section, NAME_RECORD, depot)
        .map(|i| {
            let off = i * NAME_RECORD;
            let record = &section[off..off + NAME_RECORD];
            let version = u32::from_le_bytes(record[4..8].try_into().unwrap());
            let crc = u32::from_le_bytes(record[8..12].try_into().unwrap());
            let hash = hex_encode(&record[12..44]);
            format!("{depot}_{version}_{crc:08x}_{hash}{suffix}")
        })
        .collect()
}

pub fn blob_filenames(depot: u32) -> Vec<String> {
    let db = database();
    filenames(&db.bytes[db.blobs.0..db.blobs.1], depot, ".blob")
}

pub fn dat_filenames(depot: u32) -> Vec<String> {
    let db = database();
    filenames(&db.bytes[db.dats.0..db.dats.1], depot, ".dat")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depot_zero_has_entries() {
        let blobs = blob_filenames(0);
        let dats = dat_filenames(0);
        assert!(!blobs.is_empty());
        assert!(!dats.is_empty());
        assert!(blobs.iter().any(|f| f == "0_0_1b04cb6e_8487005ce5fcfea91a0cb64015da6b5076e35463afb81baf0f086ed57fff6b90.blob"));
        assert!(
            dats.iter()
                .all(|f| f.starts_with("0_") && f.ends_with(".dat"))
        );
    }

    #[test]
    fn unknown_depot_is_empty() {
        assert!(blob_filenames(0xffff_fffe).is_empty());
        assert!(lookup_key(0xffff_fffe).is_none());
    }

    #[test]
    fn depot_zero_key_known() {
        assert!(lookup_key(0).is_some());
    }
}
