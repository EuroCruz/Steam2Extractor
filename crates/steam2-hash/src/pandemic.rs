pub fn pandemic(s: &str) -> u32 {
    const PRIME: u32 = 0x0100_0193;
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return 0;
    }
    let mut h: u32 = 0x811C_9DC5;
    for &c in bytes {
        h = ((c | 0x20) as u32 ^ h).wrapping_mul(PRIME);
    }
    (h ^ 0x2A).wrapping_mul(PRIME)
}

#[cfg(test)]
mod tests {
    use super::pandemic;

    #[test]
    fn known_vectors() {
        assert_eq!(pandemic("ANY"), 0xED05_7225);
        assert_eq!(pandemic("AttackAction"), 0x7a9f_0a17);
    }
}
