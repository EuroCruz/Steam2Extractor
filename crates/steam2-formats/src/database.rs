use std::collections::HashMap;
use std::sync::OnceLock;

use steam2_hash::pandemic::pandemic;

use crate::dictbin::{self, KeyRecord, NameRecord};

struct KeysIndex {
    records: Vec<KeyRecord>,
    by_pandemic: HashMap<u32, usize>,
}

fn load_keys() -> Option<KeysIndex> {
    let bytes = dictbin::load(dictbin::Kind::Keys.file_name()).ok()?;
    let parsed = dictbin::parse_keys(&bytes).ok()?;
    let by_pandemic = parsed.records.iter().enumerate().map(|(i, k)| (k.pandemic, i)).collect();
    Some(KeysIndex { records: parsed.records, by_pandemic })
}

fn load_blobs() -> Option<Vec<NameRecord>> {
    let bytes = dictbin::load(dictbin::Kind::Blobs.file_name()).ok()?;
    Some(dictbin::parse_blobs(&bytes).ok()?.records)
}

fn load_dats() -> Option<Vec<NameRecord>> {
    let bytes = dictbin::load(dictbin::Kind::Dats.file_name()).ok()?;
    Some(dictbin::parse_dats(&bytes).ok()?.records)
}

fn keys() -> &'static Option<KeysIndex> {
    static KEYS: OnceLock<Option<KeysIndex>> = OnceLock::new();
    KEYS.get_or_init(load_keys)
}

fn blobs() -> &'static Option<Vec<NameRecord>> {
    static BLOBS: OnceLock<Option<Vec<NameRecord>>> = OnceLock::new();
    BLOBS.get_or_init(load_blobs)
}

fn dats() -> &'static Option<Vec<NameRecord>> {
    static DATS: OnceLock<Option<Vec<NameRecord>>> = OnceLock::new();
    DATS.get_or_init(load_dats)
}

pub fn lookup_key(depot: u32) -> Option<[u8; 16]> {
    let idx = keys().as_ref()?;
    let wanted = pandemic(&format!("depot:{depot}"));
    idx.by_pandemic.get(&wanted).map(|&i| idx.records[i].key)
}

pub fn blob_filenames(depot: u32) -> Vec<String> {
    let Some(records) = blobs().as_ref() else { return Vec::new() };
    records.iter().filter(|n| n.depot == depot).map(|n| n.filename(".blob")).collect()
}

pub fn dat_filenames(depot: u32) -> Vec<String> {
    let Some(records) = dats().as_ref() else { return Vec::new() };
    records.iter().filter(|n| n.depot == depot).map(|n| n.filename(".dat")).collect()
}
