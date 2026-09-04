use crate::bail;
use crate::error::Result;
use crate::inflate::zlib_decompress;

pub fn rb32(n: u32) -> [u8; 4] {
    n.to_le_bytes()
}

pub struct Blob<'a> {
    data: &'a [u8],
    total_size: usize,
}

impl<'a> Blob<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Blob<'a>> {
        if data.len() < 10 {
            bail!("blob: too small");
        }
        let magic = u16::from_le_bytes([data[0], data[1]]);
        if magic != 0x5001 {
            bail!("blob: bad magic");
        }
        let total_size = u32::from_le_bytes([data[2], data[3], data[4], data[5]]) as usize;
        Ok(Blob { data, total_size })
    }

    pub fn get(&self, key: &[u8]) -> Option<&'a [u8]> {
        let mut pos = 10usize;
        while pos < self.total_size {
            if pos + 6 > self.data.len() {
                return None;
            }
            let keysize = u16::from_le_bytes([self.data[pos], self.data[pos + 1]]) as usize;
            let valuesize = u32::from_le_bytes([
                self.data[pos + 2],
                self.data[pos + 3],
                self.data[pos + 4],
                self.data[pos + 5],
            ]) as usize;
            pos += 6;
            if pos + keysize + valuesize > self.data.len() {
                return None;
            }
            let keydata = &self.data[pos..pos + keysize];
            if keydata == key {
                return Some(&self.data[pos + keysize..pos + keysize + valuesize]);
            }
            pos += keysize + valuesize;
        }
        None
    }
}

pub fn value_u32(v: &[u8]) -> Result<u32> {
    if v.len() != 4 {
        bail!("blob: tried to read value as u32 with bytes != 4");
    }
    Ok(u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
}

pub fn value_u64(v: &[u8]) -> Result<u64> {
    if v.len() != 8 {
        bail!("blob: tried to read value as u64 with bytes != 8");
    }
    Ok(u64::from_le_bytes(v.try_into().unwrap()))
}

pub fn decompress_blob(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 20 {
        bail!("blob: compressed blob too small");
    }
    let magic = u16::from_le_bytes([data[0], data[1]]);
    if magic != 0x4301 {
        bail!("blob: bad magic in compressed blob");
    }
    let unpacked_size = u64::from_le_bytes(data[10..18].try_into().unwrap()) as usize;
    let out = zlib_decompress(&data[20..], unpacked_size)?;
    if out.len() > unpacked_size {
        bail!("blob: compressed blob failed to decompress");
    }
    let mut buf = vec![0u8; unpacked_size];
    buf[..out.len()].copy_from_slice(&out);
    Ok(buf)
}
