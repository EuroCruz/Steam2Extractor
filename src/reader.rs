use crate::bail;
use crate::error::{Error, Result};

pub fn read_u32_at(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(data[off..off + 4].try_into().unwrap())
}

pub fn read_u64_at(data: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(data[off..off + 8].try_into().unwrap())
}

pub fn cstr_at(data: &[u8], off: usize) -> Result<&[u8]> {
    let bytes = data
        .get(off..)
        .ok_or_else(|| Error::new("reader: string offset out of bounds"))?;
    let end = bytes
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| Error::new("reader: unterminated string"))?;
    Ok(&bytes[..end])
}

pub struct ByteReader<'a> {
    data: &'a [u8],
    pub pos: usize,
}

impl<'a> ByteReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        ByteReader { data, pos: 0 }
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        if self.pos + 4 > self.data.len() {
            bail!("reader: out of bounds read");
        }
        let v = u32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }

    pub fn read_u64(&mut self) -> Result<u64> {
        if self.pos + 8 > self.data.len() {
            bail!("reader: out of bounds read");
        }
        let v = u64::from_le_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Ok(v)
    }
}
