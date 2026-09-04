const SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

const RCON: [u8; 10] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];

const fn build_inv_sbox() -> [u8; 256] {
    let mut inv = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        inv[SBOX[i] as usize] = i as u8;
        i += 1;
    }
    inv
}
const INV_SBOX: [u8; 256] = build_inv_sbox();

pub struct Aes128 {
    round_keys: [[u8; 4]; 44],
}

pub struct Aes256 {
    round_keys: [[u8; 4]; 60],
}

fn xtime(a: u8) -> u8 {
    if a & 0x80 != 0 {
        (a << 1) ^ 0x1b
    } else {
        a << 1
    }
}

fn gmul(a: u8, b: u8) -> u8 {
    let mut a = a;
    let mut b = b;
    let mut p = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            p ^= a;
        }
        a = xtime(a);
        b >>= 1;
    }
    p
}

impl Aes128 {
    pub fn new(key: &[u8; 16]) -> Self {
        let mut round_keys = [[0u8; 4]; 44];
        for i in 0..4 {
            round_keys[i] = [key[4 * i], key[4 * i + 1], key[4 * i + 2], key[4 * i + 3]];
        }
        for i in 4..44 {
            let mut temp = round_keys[i - 1];
            if i % 4 == 0 {
                temp = [temp[1], temp[2], temp[3], temp[0]];
                for b in temp.iter_mut() {
                    *b = SBOX[*b as usize];
                }
                temp[0] ^= RCON[i / 4 - 1];
            }
            round_keys[i] = [
                round_keys[i - 4][0] ^ temp[0],
                round_keys[i - 4][1] ^ temp[1],
                round_keys[i - 4][2] ^ temp[2],
                round_keys[i - 4][3] ^ temp[3],
            ];
        }
        Aes128 { round_keys }
    }

    fn add_round_key(&self, state: &mut [u8; 16], round: usize) {
        for c in 0..4 {
            let rk = self.round_keys[round * 4 + c];
            for r in 0..4 {
                state[c * 4 + r] ^= rk[r];
            }
        }
    }

    pub fn encrypt_block(&self, block: &[u8; 16]) -> [u8; 16] {
        let mut state = *block;
        self.add_round_key(&mut state, 0);
        for round in 1..10 {
            sub_bytes(&mut state);
            shift_rows(&mut state);
            mix_columns(&mut state);
            self.add_round_key(&mut state, round);
        }
        sub_bytes(&mut state);
        shift_rows(&mut state);
        self.add_round_key(&mut state, 10);
        state
    }
}

fn sub_bytes(state: &mut [u8; 16]) {
    for b in state.iter_mut() {
        *b = SBOX[*b as usize];
    }
}

fn shift_rows(state: &mut [u8; 16]) {
    let s = *state;
    for r in 1..4 {
        for c in 0..4 {
            state[c * 4 + r] = s[((c + r) % 4) * 4 + r];
        }
    }
}

fn mix_columns(state: &mut [u8; 16]) {
    for c in 0..4 {
        let a0 = state[c * 4];
        let a1 = state[c * 4 + 1];
        let a2 = state[c * 4 + 2];
        let a3 = state[c * 4 + 3];
        state[c * 4] = gmul(a0, 2) ^ gmul(a1, 3) ^ a2 ^ a3;
        state[c * 4 + 1] = a0 ^ gmul(a1, 2) ^ gmul(a2, 3) ^ a3;
        state[c * 4 + 2] = a0 ^ a1 ^ gmul(a2, 2) ^ gmul(a3, 3);
        state[c * 4 + 3] = gmul(a0, 3) ^ a1 ^ a2 ^ gmul(a3, 2);
    }
}

fn inv_sub_bytes(state: &mut [u8; 16]) {
    for b in state.iter_mut() {
        *b = INV_SBOX[*b as usize];
    }
}

fn inv_shift_rows(state: &mut [u8; 16]) {
    let s = *state;
    for r in 1..4 {
        for c in 0..4 {
            state[c * 4 + r] = s[((c + 4 - r) % 4) * 4 + r];
        }
    }
}

fn inv_mix_columns(state: &mut [u8; 16]) {
    for c in 0..4 {
        let a0 = state[c * 4];
        let a1 = state[c * 4 + 1];
        let a2 = state[c * 4 + 2];
        let a3 = state[c * 4 + 3];
        state[c * 4] = gmul(a0, 14) ^ gmul(a1, 11) ^ gmul(a2, 13) ^ gmul(a3, 9);
        state[c * 4 + 1] = gmul(a0, 9) ^ gmul(a1, 14) ^ gmul(a2, 11) ^ gmul(a3, 13);
        state[c * 4 + 2] = gmul(a0, 13) ^ gmul(a1, 9) ^ gmul(a2, 14) ^ gmul(a3, 11);
        state[c * 4 + 3] = gmul(a0, 11) ^ gmul(a1, 13) ^ gmul(a2, 9) ^ gmul(a3, 14);
    }
}

impl Aes256 {
    pub fn new(key: &[u8; 32]) -> Self {
        let mut round_keys = [[0u8; 4]; 60];
        for i in 0..8 {
            round_keys[i] = [key[4 * i], key[4 * i + 1], key[4 * i + 2], key[4 * i + 3]];
        }
        for i in 8..60 {
            let mut temp = round_keys[i - 1];
            if i % 8 == 0 {
                temp = [temp[1], temp[2], temp[3], temp[0]];
                for b in temp.iter_mut() {
                    *b = SBOX[*b as usize];
                }
                temp[0] ^= RCON[i / 8 - 1];
            } else if i % 8 == 4 {
                for b in temp.iter_mut() {
                    *b = SBOX[*b as usize];
                }
            }
            round_keys[i] = [
                round_keys[i - 8][0] ^ temp[0],
                round_keys[i - 8][1] ^ temp[1],
                round_keys[i - 8][2] ^ temp[2],
                round_keys[i - 8][3] ^ temp[3],
            ];
        }
        Aes256 { round_keys }
    }

    fn add_round_key(&self, state: &mut [u8; 16], round: usize) {
        for c in 0..4 {
            let rk = self.round_keys[round * 4 + c];
            for r in 0..4 {
                state[c * 4 + r] ^= rk[r];
            }
        }
    }

    pub fn encrypt_block(&self, block: &[u8; 16]) -> [u8; 16] {
        let mut state = *block;
        self.add_round_key(&mut state, 0);
        for round in 1..14 {
            sub_bytes(&mut state);
            shift_rows(&mut state);
            mix_columns(&mut state);
            self.add_round_key(&mut state, round);
        }
        sub_bytes(&mut state);
        shift_rows(&mut state);
        self.add_round_key(&mut state, 14);
        state
    }

    pub fn decrypt_block(&self, block: &[u8; 16]) -> [u8; 16] {
        let mut state = *block;
        self.add_round_key(&mut state, 14);
        for round in (1..14).rev() {
            inv_shift_rows(&mut state);
            inv_sub_bytes(&mut state);
            self.add_round_key(&mut state, round);
            inv_mix_columns(&mut state);
        }
        inv_shift_rows(&mut state);
        inv_sub_bytes(&mut state);
        self.add_round_key(&mut state, 0);
        state
    }
}

pub fn ecb_decrypt_block(key: &[u8; 32], block: &[u8; 16]) -> [u8; 16] {
    Aes256::new(key).decrypt_block(block)
}

pub fn cbc_decrypt(key: &[u8; 32], iv: &[u8; 16], data: &mut [u8]) {
    let cipher = Aes256::new(key);
    let mut feedback = *iv;
    for chunk in data.chunks_mut(16) {
        let mut block = [0u8; 16];
        block[..chunk.len()].copy_from_slice(chunk);
        let decrypted = cipher.decrypt_block(&block);
        for i in 0..chunk.len() {
            chunk[i] = decrypted[i] ^ feedback[i];
        }
        feedback = block;
    }
}

pub fn cfb_decrypt(key: &[u8; 16], data: &mut [u8]) {
    let cipher = Aes128::new(key);
    let mut feedback = [0u8; 16];
    for chunk in data.chunks_mut(16) {
        let keystream = cipher.encrypt_block(&feedback);
        let mut next_feedback = [0u8; 16];
        next_feedback[..chunk.len()].copy_from_slice(chunk);
        for (b, k) in chunk.iter_mut().zip(keystream.iter()) {
            *b ^= k;
        }
        feedback = next_feedback;
    }
}

pub fn cfb_encrypt(key: &[u8; 16], data: &mut [u8]) {
    let cipher = Aes128::new(key);
    let mut feedback = [0u8; 16];
    for chunk in data.chunks_mut(16) {
        let keystream = cipher.encrypt_block(&feedback);
        for (b, k) in chunk.iter_mut().zip(keystream.iter()) {
            *b ^= k;
        }
        let mut next_feedback = [0u8; 16];
        next_feedback[..chunk.len()].copy_from_slice(chunk);
        feedback = next_feedback;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fips197_test_vector() {
        let key: [u8; 16] = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ];
        let plaintext: [u8; 16] = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        let expected: [u8; 16] = [
            0x69, 0xc4, 0xe0, 0xd8, 0x6a, 0x7b, 0x04, 0x30, 0xd8, 0xcd, 0xb7, 0x80, 0x70, 0xb4,
            0xc5, 0x5a,
        ];
        let cipher = Aes128::new(&key);
        assert_eq!(cipher.encrypt_block(&plaintext), expected);
    }

    #[test]
    fn cfb_roundtrip() {
        let key = [7u8; 16];
        for len in [0, 1, 15, 16, 17, 31, 32, 100, 1000] {
            let original: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            let mut buf = original.clone();
            cfb_encrypt(&key, &mut buf);
            if len > 0 {
                assert_ne!(buf, original);
            }
            cfb_decrypt(&key, &mut buf);
            assert_eq!(buf, original);
        }
    }

    #[test]
    fn aes256_fips197_test_vector() {
        let key: [u8; 32] = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
            0x1c, 0x1d, 0x1e, 0x1f,
        ];
        let plaintext: [u8; 16] = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        let expected: [u8; 16] = [
            0x8e, 0xa2, 0xb7, 0xca, 0x51, 0x67, 0x45, 0xbf, 0xea, 0xfc, 0x49, 0x90, 0x4b, 0x49,
            0x60, 0x89,
        ];
        let cipher = Aes256::new(&key);
        let ct = cipher.encrypt_block(&plaintext);
        assert_eq!(ct, expected);
        assert_eq!(cipher.decrypt_block(&ct), plaintext);
    }

    #[test]
    fn cbc_nist_sp800_38a_vector() {
        let key: [u8; 32] = [
            0x60, 0x3d, 0xeb, 0x10, 0x15, 0xca, 0x71, 0xbe, 0x2b, 0x73, 0xae, 0xf0, 0x85, 0x7d,
            0x77, 0x81, 0x1f, 0x35, 0x2c, 0x07, 0x3b, 0x61, 0x08, 0xd7, 0x2d, 0x98, 0x10, 0xa3,
            0x09, 0x14, 0xdf, 0xf4,
        ];
        let iv: [u8; 16] = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ];
        let mut data: Vec<u8> = vec![
            0xf5, 0x8c, 0x4c, 0x04, 0xd6, 0xe5, 0xf1, 0xba, 0x77, 0x9e, 0xab, 0xfb, 0x5f, 0x7b,
            0xfb, 0xd6, 0x9c, 0xfc, 0x4e, 0x96, 0x7e, 0xdb, 0x80, 0x8d, 0x67, 0x9f, 0x77, 0x7b,
            0xc6, 0x70, 0x2c, 0x7d, 0x39, 0xf2, 0x33, 0x69, 0xa9, 0xd9, 0xba, 0xcf, 0xa5, 0x30,
            0xe2, 0x63, 0x04, 0x23, 0x14, 0x61, 0xb2, 0xeb, 0x05, 0xe2, 0xc3, 0x9b, 0xe9, 0xfc,
            0xda, 0x6c, 0x19, 0x07, 0x8c, 0x6a, 0x9d, 0x1b,
        ];
        let expected: Vec<u8> = vec![
            0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93,
            0x17, 0x2a, 0xae, 0x2d, 0x8a, 0x57, 0x1e, 0x03, 0xac, 0x9c, 0x9e, 0xb7, 0x6f, 0xac,
            0x45, 0xaf, 0x8e, 0x51, 0x30, 0xc8, 0x1c, 0x46, 0xa3, 0x5c, 0xe4, 0x11, 0xe5, 0xfb,
            0xc1, 0x19, 0x1a, 0x0a, 0x52, 0xef, 0xf6, 0x9f, 0x24, 0x45, 0xdf, 0x4f, 0x9b, 0x17,
            0xad, 0x2b, 0x41, 0x7b, 0xe6, 0x6c, 0x37, 0x10,
        ];
        cbc_decrypt(&key, &iv, &mut data);
        assert_eq!(data, expected);
    }
}
