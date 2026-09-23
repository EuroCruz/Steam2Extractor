use crate::database;

pub fn lookup(depot: u32) -> Option<[u8; 16]> {
    database::lookup_key(depot)
}

pub fn decode_hex(s: &str) -> Result<[u8; 16], String> {
    let s = s.trim();
    if s.len() != 32 {
        return Err(format!("key: expected 32 hex characters, got {}", s.len()));
    }
    let mut key = [0u8; 16];
    for i in 0..16 {
        let byte_str = &s[i * 2..i * 2 + 2];
        key[i] = u8::from_str_radix(byte_str, 16).map_err(|_| "key: invalid hex digit".to_string())?;
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let key = decode_hex("000102030405060708090a0b0c0d0e0f").unwrap();
        assert_eq!(key, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
    }
}
