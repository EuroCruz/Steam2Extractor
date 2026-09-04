use crate::adler32::adler32;
use crate::bail;
use crate::error::Result;

struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    fn read_bit(&mut self) -> Result<u32> {
        if self.byte_pos >= self.data.len() {
            bail!("inflate: unexpected end of stream");
        }
        let bit = (self.data[self.byte_pos] >> self.bit_pos) & 1;
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
        Ok(bit as u32)
    }

    fn read_bits(&mut self, n: u32) -> Result<u32> {
        let mut v = 0u32;
        for i in 0..n {
            v |= self.read_bit()? << i;
        }
        Ok(v)
    }

    fn align_byte(&mut self) {
        if self.bit_pos != 0 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
    }

    fn read_u8(&mut self) -> Result<u8> {
        if self.byte_pos >= self.data.len() {
            bail!("inflate: unexpected end of stream");
        }
        let b = self.data[self.byte_pos];
        self.byte_pos += 1;
        Ok(b)
    }

    fn trailer_offset(&self) -> usize {
        if self.bit_pos == 0 {
            self.byte_pos
        } else {
            self.byte_pos + 1
        }
    }
}

struct HuffTable {
    counts: Vec<u16>,
    symbols: Vec<u16>,
}

impl HuffTable {
    fn build(lengths: &[u8]) -> HuffTable {
        let max_bits = *lengths.iter().max().unwrap_or(&0) as usize;
        let mut counts = vec![0u16; max_bits + 1];
        for &l in lengths {
            if l > 0 {
                counts[l as usize] += 1;
            }
        }
        let mut offsets = vec![0u16; max_bits + 2];
        for i in 1..=max_bits {
            offsets[i + 1] = offsets[i] + counts[i];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l > 0 {
                symbols[offsets[l as usize] as usize] = sym as u16;
                offsets[l as usize] += 1;
            }
        }
        HuffTable { counts, symbols }
    }

    fn decode(&self, br: &mut BitReader) -> Result<u16> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for len in 1..self.counts.len() {
            code |= br.read_bit()? as i32;
            let count = self.counts[len] as i32;
            if code - first < count {
                return Ok(self.symbols[(index + (code - first)) as usize]);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        bail!("inflate: invalid huffman code")
    }
}

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const CLEN_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

fn fixed_tables() -> (HuffTable, HuffTable) {
    let mut lit_lengths = [0u8; 288];
    lit_lengths[0..144].fill(8);
    lit_lengths[144..256].fill(9);
    lit_lengths[256..280].fill(7);
    lit_lengths[280..288].fill(8);
    let dist_lengths = [5u8; 30];
    (
        HuffTable::build(&lit_lengths),
        HuffTable::build(&dist_lengths),
    )
}

fn dynamic_tables(br: &mut BitReader) -> Result<(HuffTable, HuffTable)> {
    let hlit = br.read_bits(5)? as usize + 257;
    let hdist = br.read_bits(5)? as usize + 1;
    let hclen = br.read_bits(4)? as usize + 4;

    let mut clen_lengths = [0u8; 19];
    for i in 0..hclen {
        clen_lengths[CLEN_ORDER[i]] = br.read_bits(3)? as u8;
    }
    let clen_table = HuffTable::build(&clen_lengths);

    let mut lengths = Vec::with_capacity(hlit + hdist);
    while lengths.len() < hlit + hdist {
        let sym = clen_table.decode(br)?;
        match sym {
            0..=15 => lengths.push(sym as u8),
            16 => {
                if lengths.is_empty() {
                    bail!("inflate: repeat with no previous length");
                }
                let prev = *lengths.last().unwrap();
                let rep = br.read_bits(2)? + 3;
                for _ in 0..rep {
                    lengths.push(prev);
                }
            }
            17 => {
                let rep = br.read_bits(3)? + 3;
                lengths.extend(std::iter::repeat_n(0, rep as usize));
            }
            18 => {
                let rep = br.read_bits(7)? + 11;
                lengths.extend(std::iter::repeat_n(0, rep as usize));
            }
            _ => bail!("inflate: invalid code length symbol"),
        }
    }
    if lengths.len() != hlit + hdist {
        bail!("inflate: code length overrun");
    }
    let lit_table = HuffTable::build(&lengths[0..hlit]);
    let dist_table = HuffTable::build(&lengths[hlit..hlit + hdist]);
    Ok((lit_table, dist_table))
}

fn inflate_block(
    br: &mut BitReader,
    lit_table: &HuffTable,
    dist_table: &HuffTable,
    out: &mut Vec<u8>,
) -> Result<()> {
    loop {
        let sym = lit_table.decode(br)?;
        if sym < 256 {
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let idx = (sym - 257) as usize;
            if idx >= LENGTH_BASE.len() {
                bail!("inflate: invalid length symbol");
            }
            let length =
                LENGTH_BASE[idx] as usize + br.read_bits(LENGTH_EXTRA[idx] as u32)? as usize;
            let dsym = dist_table.decode(br)? as usize;
            if dsym >= DIST_BASE.len() {
                bail!("inflate: invalid distance symbol");
            }
            let distance =
                DIST_BASE[dsym] as usize + br.read_bits(DIST_EXTRA[dsym] as u32)? as usize;
            if distance > out.len() {
                bail!("inflate: distance too far back");
            }
            let start = out.len() - distance;
            for i in 0..length {
                let byte = out[start + i];
                out.push(byte);
            }
        }
    }
}

fn inflate_stream(br: &mut BitReader, out: &mut Vec<u8>) -> Result<()> {
    loop {
        let bfinal = br.read_bits(1)?;
        let btype = br.read_bits(2)?;
        match btype {
            0 => {
                br.align_byte();
                let len_lo = br.read_u8()? as u16;
                let len_hi = br.read_u8()? as u16;
                let len = len_lo | (len_hi << 8);
                let nlen_lo = br.read_u8()? as u16;
                let nlen_hi = br.read_u8()? as u16;
                let nlen = nlen_lo | (nlen_hi << 8);
                if len != !nlen {
                    bail!("inflate: stored block length mismatch");
                }
                for _ in 0..len {
                    out.push(br.read_u8()?);
                }
            }
            1 => {
                let (lit_table, dist_table) = fixed_tables();
                inflate_block(br, &lit_table, &dist_table, out)?;
            }
            2 => {
                let (lit_table, dist_table) = dynamic_tables(br)?;
                inflate_block(br, &lit_table, &dist_table, out)?;
            }
            _ => bail!("inflate: invalid block type"),
        }
        if bfinal == 1 {
            return Ok(());
        }
    }
}

pub fn raw_inflate(data: &[u8], size_hint: usize) -> Result<Vec<u8>> {
    let mut br = BitReader::new(data);
    let mut out = Vec::with_capacity(size_hint);
    inflate_stream(&mut br, &mut out)?;
    Ok(out)
}

pub fn zlib_decompress(data: &[u8], size_hint: usize) -> Result<Vec<u8>> {
    if data.len() < 6 {
        bail!("inflate: zlib stream too short");
    }
    let cmf = data[0];
    let flg = data[1];
    if cmf & 0x0f != 8 {
        bail!("inflate: unsupported compression method");
    }
    if !(cmf as u16 * 256 + flg as u16).is_multiple_of(31) {
        bail!("inflate: zlib header checksum mismatch");
    }
    if flg & 0x20 != 0 {
        bail!("inflate: zlib preset dictionary not supported");
    }

    let mut br = BitReader::new(&data[2..]);
    let mut out = Vec::with_capacity(size_hint);
    inflate_stream(&mut br, &mut out)?;

    let trailer_start = 2 + br.trailer_offset();
    if data.len() < trailer_start + 4 {
        bail!("inflate: missing adler32 trailer");
    }
    let expected = u32::from_be_bytes([
        data[trailer_start],
        data[trailer_start + 1],
        data[trailer_start + 2],
        data[trailer_start + 3],
    ]);
    let actual = adler32(1, &out);
    if expected != actual {
        bail!("inflate: adler32 checksum mismatch");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_stored_block() {
        let mut raw = vec![0x01u8, 0x05, 0x00, 0xfa, 0xff];
        raw.extend_from_slice(b"hello");
        let mut zlib = vec![0x78, 0x01];
        zlib.extend_from_slice(&raw);
        let adler = adler32(1, b"hello");
        zlib.extend_from_slice(&adler.to_be_bytes());
        let out = zlib_decompress(&zlib, 5).unwrap();
        assert_eq!(out, b"hello");
    }

    #[test]
    fn raw_inflate_stream() {
        let compressed: [u8; 57] = [
            0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0x28, 0x4a, 0x2c, 0x57, 0x48, 0x49, 0x4d, 0xcb,
            0x49, 0x2c, 0x49, 0x55, 0x28, 0xcf, 0x2f, 0xca, 0x49, 0xd1, 0x41, 0x11, 0xca, 0x48,
            0x2c, 0x56, 0xc8, 0xcb, 0x57, 0xc8, 0x48, 0x4d, 0x4c, 0x49, 0x2d, 0x52, 0xc8, 0x2f,
            0x52, 0x28, 0x29, 0x4a, 0xcc, 0xcc, 0x49, 0x2d, 0x52, 0x04, 0x0a, 0x8d, 0x5c, 0xed,
            0x00,
        ];
        let expected = b"hello raw deflate world, raw deflate has no header or trailer! ".repeat(5);
        let out = raw_inflate(&compressed, expected.len()).unwrap();
        assert_eq!(out, expected);
    }

    #[test]
    fn roundtrip_real_zlib_stream() {
        let compressed: [u8; 61] = [
            0x78, 0xda, 0x0b, 0xc9, 0x48, 0x55, 0x28, 0x2c, 0xcd, 0x4c, 0xce, 0x56, 0x48, 0x2a,
            0xca, 0x2f, 0xcf, 0x53, 0x48, 0xcb, 0xaf, 0x50, 0xc8, 0x2a, 0xcd, 0x2d, 0x28, 0x56,
            0xc8, 0x2f, 0x4b, 0x2d, 0x52, 0x28, 0x01, 0x4a, 0xe7, 0x24, 0x56, 0x55, 0x2a, 0xa4,
            0xe4, 0xa7, 0xeb, 0x29, 0x84, 0x8c, 0x2a, 0x1e, 0x55, 0x3c, 0xaa, 0x98, 0xda, 0x8a,
            0x01, 0x47, 0xa5, 0x43, 0x1c,
        ];
        let expected = b"The quick brown fox jumps over the lazy dog. ".repeat(20);
        let out = zlib_decompress(&compressed, expected.len()).unwrap();
        assert_eq!(out, expected);
    }

    #[test]
    fn roundtrip_dynamic_huffman() {
        let compressed = include_bytes!("../testdata/sample.zlib");
        let expected = include_bytes!("../testdata/sample.bin");
        let out = zlib_decompress(compressed, expected.len()).unwrap();
        assert_eq!(out, expected);
    }
}
