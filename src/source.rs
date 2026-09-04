use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use crate::bail;
use crate::db;
use crate::error::Result;
use crate::net;

enum Kind {
    Blob,
    Dat,
}

pub struct Source {
    cache_dir: PathBuf,
    kind: Kind,
    offline: bool,
}

impl Source {
    pub fn blobs(cache_dir: PathBuf, offline: bool) -> Source {
        Source {
            cache_dir,
            kind: Kind::Blob,
            offline,
        }
    }

    pub fn dats(cache_dir: PathBuf, offline: bool) -> Source {
        Source {
            cache_dir,
            kind: Kind::Dat,
            offline,
        }
    }

    fn remote_subdir(&self) -> &'static str {
        match self.kind {
            Kind::Blob => "blobs",
            Kind::Dat => "dats",
        }
    }

    pub fn list(&self, depot: u32, prefix: &str, suffix: &str) -> Result<Vec<String>> {
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
            Kind::Blob => db::blob_filenames(depot),
            Kind::Dat => db::dat_filenames(depot),
        };
        names.extend(indexed);
        Ok(names.into_iter().collect())
    }

    pub fn size(&self, filename: &str) -> Result<u64> {
        let local = self.cache_dir.join(filename);
        if let Ok(meta) = fs::metadata(&local) {
            return Ok(meta.len());
        }
        if self.offline {
            bail!("{}: not found locally and offline mode is set", filename);
        }
        net::remote_size(self.remote_subdir(), filename)
    }

    pub fn resolve(&self, filename: &str) -> Result<PathBuf> {
        let local = self.cache_dir.join(filename);
        if local.exists() {
            return Ok(local);
        }
        if self.offline {
            bail!("{}: not found locally and offline mode is set", filename);
        }
        net::ensure_file(&self.cache_dir, self.remote_subdir(), filename)
    }
}
