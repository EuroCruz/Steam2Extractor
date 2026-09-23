#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Endian {
    Little,
    Big,
}

impl Endian {
    pub fn read(&self, bytes: &[u8]) -> u64 {
        let mut buf = [0u8; 8];
        match self {
            Endian::Little => {
                buf[..bytes.len()].copy_from_slice(bytes);
                u64::from_le_bytes(buf)
            }
            Endian::Big => {
                buf[8 - bytes.len()..].copy_from_slice(bytes);
                u64::from_be_bytes(buf)
            }
        }
    }

    pub fn write(&self, out: &mut Vec<u8>, width: usize, value: u64) {
        match self {
            Endian::Little => out.extend_from_slice(&value.to_le_bytes()[..width]),
            Endian::Big => out.extend_from_slice(&value.to_be_bytes()[8 - width..]),
        }
    }
}
