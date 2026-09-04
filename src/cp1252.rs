const HIGH_MAP: [u32; 32] = [
    0x20AC, 0x0000, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0x0000, 0x017D, 0x0000, 0x0000, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x0000, 0x017E, 0x0178,
];

pub fn decode_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            0x00..=0x7f => out.push(b as char),
            0x80..=0x9f => {
                let code = HIGH_MAP[(b - 0x80) as usize];
                if code != 0 {
                    out.push(char::from_u32(code).unwrap());
                }
            }
            0xa0..=0xff => out.push(char::from_u32(b as u32).unwrap()),
        }
    }
    out
}

pub fn sanitize_path(raw: &str) -> String {
    raw.replace('\\', "/")
        .split('/')
        .map(|part| {
            let lower = part.to_lowercase();
            if lower.is_empty() || lower == "." || lower == ".." {
                "_".to_string()
            } else {
                lower
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_passthrough() {
        assert_eq!(decode_string(b"Hello.txt"), "Hello.txt");
    }

    #[test]
    fn high_byte_latin1() {
        assert_eq!(decode_string(&[0xE9]), "\u{00e9}");
    }

    #[test]
    fn high_byte_special() {
        assert_eq!(decode_string(&[0x93, 0x94]), "\u{201c}\u{201d}");
    }

    #[test]
    fn sanitize_traversal() {
        assert_eq!(sanitize_path("A\\B\\..\\C"), "a/b/_/c");
        assert_eq!(sanitize_path("Normal/Path.DAT"), "normal/path.dat");
    }
}
