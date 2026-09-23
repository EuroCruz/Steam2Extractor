use std::path::{Path, PathBuf};

use crate::depot;
use crate::dictbin;
use crate::keysource::KeySource;
use crate::sim;
use crate::Entry;

pub struct Target {
    pub depot: u32,
    pub version: u32,
    pub blob_dir: String,
    pub dat_dir: String,
}

fn parse_depot_version(stem: &str) -> Option<(u32, u32)> {
    let mut parts = stem.splitn(3, '_');
    let depot: u32 = parts.next()?.parse().ok()?;
    let version: u32 = parts.next()?.parse().ok()?;
    Some((depot, version))
}

pub fn resolve_target(path: &Path, blob_dir: Option<&str>, dat_dir: Option<&str>) -> Result<Target, String> {
    let stem = path.file_stem().and_then(|s| s.to_str()).ok_or("could not read file name")?;
    let (depot, version) = parse_depot_version(stem)
        .ok_or_else(|| format!("{}: expected a depot_version_crc_hash.dat/.blob filename", path.display()))?;

    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let sibling = |name: &str| -> Option<PathBuf> {
        let candidate = parent.join(name);
        candidate.exists().then_some(candidate)
    };

    let blob_dir = match blob_dir {
        Some(dir) => dir.to_string(),
        None => {
            if path.extension().and_then(|e| e.to_str()) == Some("blob") {
                parent.to_string_lossy().into_owned()
            } else {
                sibling("blob").map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| depot::DEFAULT_BLOB_DIR.to_string())
            }
        }
    };
    let dat_dir = match dat_dir {
        Some(dir) => dir.to_string(),
        None => {
            if path.extension().and_then(|e| e.to_str()) == Some("dat") {
                parent.to_string_lossy().into_owned()
            } else {
                sibling("dat").map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| depot::DEFAULT_DAT_DIR.to_string())
            }
        }
    };

    Ok(Target { depot, version, blob_dir, dat_dir })
}

pub fn list_depot(target: &Target, keys: &KeySource) -> Result<Vec<Entry>, String> {
    let loaded = depot::load(&target.blob_dir, &target.dat_dir, target.depot, target.version, &None, keys)?;
    Ok(loaded
        .manifest
        .nodes
        .iter()
        .filter(|n| n.flags != 0)
        .map(|n| {
            let name = loaded.manifest.id_to_path.get(&n.file_id).cloned().unwrap_or_default();
            let blocks = loaded.fileids.get(&n.file_id).map(|f| f.block_count()).unwrap_or(0);
            Entry { name: crate::cp1252::sanitize_path(&name), size: blocks as u64, detail: format!("file_id {}, {blocks} block(s)", n.file_id) }
        })
        .collect())
}

pub fn verify_depot(target: &Target, keys: &KeySource) -> Result<String, String> {
    let loaded = depot::load(&target.blob_dir, &target.dat_dir, target.depot, target.version, &None, keys)?;
    let entries = loaded.manifest.nodes.iter().filter(|n| n.flags != 0).count();
    Ok(format!(
        "depot {} version {}, app {} ver {}, {entries} entries",
        target.depot, target.version, loaded.manifest.header.app_id, loaded.manifest.header.ver_id
    ))
}

pub fn list_sim(path: &Path) -> Result<Vec<Entry>, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    sim::list_entries(&data)
}

pub fn verify_sim(path: &Path) -> Result<String, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    sim::verify(&data)
}

pub enum DictContent {
    Keys(Vec<dictbin::KeyRecord>),
    Names(Vec<dictbin::NameRecord>, &'static str),
}

pub fn list_dict(path: &Path, pandemic: Option<u32>) -> Result<Vec<Entry>, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    match dict_content(&data)? {
        DictContent::Keys(records) => Ok(records
            .into_iter()
            .filter(|k| pandemic.is_none_or(|h| k.pandemic == h))
            .map(|k| Entry { name: format!("depot {}", k.depot), size: 16, detail: format!("pandemic 0x{:08x}", k.pandemic) })
            .collect()),
        DictContent::Names(records, suffix) => Ok(records
            .into_iter()
            .filter(|n| pandemic.is_none_or(|h| n.pandemic == h))
            .map(|n| Entry { name: n.filename(suffix), size: 0, detail: format!("pandemic 0x{:08x}", n.pandemic) })
            .collect()),
    }
}

pub fn verify_dict(path: &Path) -> Result<String, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    match dict_content(&data)? {
        DictContent::Keys(records) => Ok(format!("keys dictionary, {} entries", records.len())),
        DictContent::Names(records, suffix) => Ok(format!("{} dictionary, {} entries", suffix.trim_start_matches('.'), records.len())),
    }
}

fn dict_content(data: &[u8]) -> Result<DictContent, String> {
    match dictbin::peek_kind(data) {
        Some(dictbin::Kind::Keys) => Ok(DictContent::Keys(dictbin::parse_keys(data)?.records)),
        Some(dictbin::Kind::Blobs) => Ok(DictContent::Names(dictbin::parse_blobs(data)?.records, ".blob")),
        Some(dictbin::Kind::Dats) => Ok(DictContent::Names(dictbin::parse_dats(data)?.records, ".dat")),
        None => Err("not a Steam2ExtractV2 dictionary".to_string()),
    }
}
