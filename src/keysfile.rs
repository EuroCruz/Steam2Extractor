use std::fs;

use crate::error::Result;
use crate::keys::decode_hex;

fn parse_line(line: &str) -> Option<(u32, [u8; 16])> {
    let mut quoted = line.split('"').skip(1).step_by(2);
    let depot_str = quoted.next()?;
    let key_str = quoted.next()?;
    let depot: u32 = depot_str.trim().parse().ok()?;
    let key = decode_hex(key_str.trim()).ok()?;
    Some((depot, key))
}

pub fn parse(path: &str) -> Result<Vec<(u32, [u8; 16])>> {
    let text = fs::read_to_string(path)?;
    Ok(text.lines().filter_map(parse_line).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vdf_style_lines() {
        assert_eq!(
            parse_line("\t\t\"481\"\t\t\"49684128b36ed12d85bc17e8bbbf6b22\""),
            Some((
                481,
                [
                    0x49, 0x68, 0x41, 0x28, 0xb3, 0x6e, 0xd1, 0x2d, 0x85, 0xbc, 0x17, 0xe8, 0xbb,
                    0xbf, 0x6b, 0x22
                ]
            ))
        );
        assert_eq!(parse_line("\t\"depots\"\n"), None);
        assert_eq!(parse_line("{"), None);
    }
}
