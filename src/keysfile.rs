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
            parse_line("\t\t\"201701\"\t\t\"f0ed49ba492fbb4d7bf288f24150fa2b\""),
            Some((
                201701,
                [
                    0xf0, 0xed, 0x49, 0xba, 0x49, 0x2f, 0xbb, 0x4d, 0x7b, 0xf2, 0x88, 0xf2, 0x41,
                    0x50, 0xfa, 0x2b
                ]
            ))
        );
        assert_eq!(parse_line("\t\"depots\"\n"), None);
        assert_eq!(parse_line("{"), None);
    }
}
