use crate::bail;
use crate::db;
use crate::error::Result;

pub fn lookup(depot: u32) -> Option<[u8; 16]> {
    db::lookup_key(depot)
}

pub fn decode_hex(s: &str) -> Result<[u8; 16]> {
    let s = s.trim();
    if s.len() != 32 {
        bail!("key: expected 32 hex characters, got {}", s.len());
    }
    let mut key = [0u8; 16];
    for i in 0..16 {
        let byte_str = &s[i * 2..i * 2 + 2];
        key[i] = u8::from_str_radix(byte_str, 16)
            .map_err(|_| crate::error::Error::new("key: invalid hex digit"))?;
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_depot_lookup() {
        assert!(lookup(0).is_some());
        assert_eq!(lookup(999_999_999), None);
    }

    #[test]
    fn hex_roundtrip() {
        let key = decode_hex("000102030405060708090a0b0c0d0e0f").unwrap();
        assert_eq!(key, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
    }
}
