const MOD_ADLER: u32 = 65521;

pub fn adler32(seed: u32, data: &[u8]) -> u32 {
    let mut a = seed & 0xffff;
    let mut b = (seed >> 16) & 0xffff;
    for &byte in data {
        a = (a + byte as u32) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}
