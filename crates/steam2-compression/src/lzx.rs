use lzxc::{Encoder, WindowSize as EncodeWindowSize, MAX_CHUNK_SIZE};
use lzxd::{Lzxd, WindowSize};

const CHUNK_SIZE: usize = 32768;

pub fn decompress(compressed: &[u8], uncompressed_size: usize) -> Result<Vec<u8>, String> {
    let mut lzxd = Lzxd::new(WindowSize::KB128);
    let mut out = Vec::with_capacity(uncompressed_size);
    let mut pos = 0usize;

    while out.len() < uncompressed_size {
        if pos + 2 > compressed.len() {
            return Err("truncated chunk header".into());
        }
        let (header_len, chunk_uncompressed_len, chunk_compressed_len) = if compressed[pos] == 0xff {
            if pos + 5 > compressed.len() {
                return Err("truncated extended chunk header".into());
            }
            let ulen = u16::from_be_bytes([compressed[pos + 1], compressed[pos + 2]]) as usize;
            let clen = u16::from_be_bytes([compressed[pos + 3], compressed[pos + 4]]) as usize;
            (5, ulen, clen)
        } else {
            let clen = u16::from_be_bytes([compressed[pos], compressed[pos + 1]]) as usize;
            (2, CHUNK_SIZE.min(uncompressed_size - out.len()), clen)
        };

        let payload_start = pos + header_len;
        let payload_end = payload_start + chunk_compressed_len;
        if payload_end > compressed.len() {
            return Err("chunk payload overruns compressed data".into());
        }

        let chunk = lzxd
            .decompress_next(&compressed[payload_start..payload_end], chunk_uncompressed_len)
            .map_err(|e| format!("{e:?}"))?;
        out.extend_from_slice(chunk);
        pos = payload_end;
    }

    Ok(out)
}

pub fn compress(data: &[u8]) -> Vec<u8> {
    assert_eq!(MAX_CHUNK_SIZE, CHUNK_SIZE, "lzxc chunk size must match the container's framing");

    let mut encoder = Encoder::new(EncodeWindowSize::KB128);
    let mut out = Vec::new();
    let slabs: Vec<&[u8]> = data.chunks(CHUNK_SIZE).collect();

    for (i, slab) in slabs.iter().enumerate() {
        let compressed = encoder.encode_chunk(slab);
        if i == slabs.len() - 1 && slab.len() != CHUNK_SIZE {
            out.push(0xff);
            out.extend_from_slice(&(slab.len() as u16).to_be_bytes());
            out.extend_from_slice(&(compressed.len() as u16).to_be_bytes());
        } else {
            out.extend_from_slice(&(compressed.len() as u16).to_be_bytes());
        }
        out.extend_from_slice(&compressed);
    }

    out
}
