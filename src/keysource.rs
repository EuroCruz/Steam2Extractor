use crate::db;
use crate::error::{Error, Result};
use crate::keys::decode_hex;
use crate::keysfile;

#[derive(Default)]
pub struct KeySource {
    entries: Vec<(Option<u32>, [u8; 16])>,
}

impl KeySource {
    pub fn add(&mut self, spec: &str) -> Result<()> {
        match spec.split_once(':') {
            Some((depot, hex)) => {
                let depot: u32 = depot
                    .parse()
                    .map_err(|_| Error::new("--key: depot must be an integer"))?;
                self.entries.push((Some(depot), decode_hex(hex)?));
            }
            None => self.entries.push((None, decode_hex(spec)?)),
        }
        Ok(())
    }

    pub fn add_file(&mut self, path: &str) -> Result<()> {
        self.entries.extend(
            keysfile::parse(path)?
                .into_iter()
                .map(|(d, k)| (Some(d), k)),
        );
        Ok(())
    }

    pub fn resolve(&self, depot: u32) -> Option<[u8; 16]> {
        self.entries
            .iter()
            .find(|(d, _)| *d == Some(depot))
            .or_else(|| self.entries.iter().find(|(d, _)| d.is_none()))
            .map(|(_, k)| *k)
            .or_else(|| db::lookup_key(depot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_depot_wins_over_default() {
        let mut ks = KeySource::default();
        ks.add(&"0".repeat(32)).unwrap();
        ks.add(&format!("5:{}", "1".repeat(32))).unwrap();
        assert_eq!(ks.resolve(5), Some([0x11; 16]));
        assert_eq!(ks.resolve(6), Some([0x00; 16]));
    }

    #[test]
    fn falls_back_to_embedded_db() {
        let ks = KeySource::default();
        assert!(ks.resolve(0).is_some());
        assert_eq!(ks.resolve(999_999_999), None);
    }
}
