use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use crate::database;

enum Kind {
    Blob,
    Dat,
}

pub struct Source {
    cache_dir: PathBuf,
    kind: Kind,
}

impl Source {
    pub fn blobs(cache_dir: PathBuf) -> Source {
        Source { cache_dir, kind: Kind::Blob }
    }

    pub fn dats(cache_dir: PathBuf) -> Source {
        Source { cache_dir, kind: Kind::Dat }
    }

    pub fn list(&self, depot: u32, prefix: &str, suffix: &str) -> Result<Vec<String>, String> {
        let mut names = BTreeSet::new();
        if let Ok(read_dir) = fs::read_dir(&self.cache_dir) {
            for entry in read_dir.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with(prefix) && name.ends_with(suffix) {
                    names.insert(name);
                }
            }
        }
        let indexed = match self.kind {
            Kind::Blob => database::blob_filenames(depot),
            Kind::Dat => database::dat_filenames(depot),
        };
        names.extend(indexed);
        Ok(names.into_iter().collect())
    }

    pub fn exists_locally(&self, filename: &str) -> bool {
        self.cache_dir.join(filename).exists()
    }

    pub fn size(&self, filename: &str) -> Result<u64, String> {
        let local = self.cache_dir.join(filename);
        fs::metadata(&local).map(|m| m.len()).map_err(|_| format!("{filename}: not found locally"))
    }

    pub fn resolve(&self, filename: &str) -> Result<PathBuf, String> {
        let local = self.cache_dir.join(filename);
        if local.exists() {
            return Ok(local);
        }
        Err(format!("{filename}: not found locally"))
    }
}
