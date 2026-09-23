use steam2_core::endian::Endian;
use std::io::{Read, Write};

const SEGMENT_SIZE: usize = 65535;

pub fn is_sges(data: &[u8]) -> bool {
    data.len() >= 16 && (data[0..4] == *b"segs" || data[0..4] == *b"sges")
}

pub fn decompress(data: &[u8], expected_size: usize) -> Result<Vec<u8>, String> {
    if !is_sges(data) {
        return Err("sges: bad magic".into());
    }
    let be = data[0..4] == *b"sges";
    let u16_at = |o: usize| -> u16 {
        let b: [u8; 2] = data[o..o + 2].try_into().unwrap();
        if be { u16::from_be_bytes(b) } else { u16::from_le_bytes(b) }
    };
    let u32_at = |o: usize| -> u32 {
        let b: [u8; 4] = data[o..o + 4].try_into().unwrap();
        if be { u32::from_be_bytes(b) } else { u32::from_le_bytes(b) }
    };

    let segment_count = u16_at(6) as usize;
    let total_unc = u32_at(8) as usize;
    let desc_start = 16;
    let desc_end = desc_start + segment_count * 8;
    if desc_end > data.len() {
        return Err("sges: truncated segment table".into());
    }

    let mut out = Vec::with_capacity(total_unc.max(expected_size));
    for i in 0..segment_count {
        let o = desc_start + i * 8;
        let comp_size = u16_at(o) as usize;
        let raw_unc = match u16_at(o + 2) {
            0 => 65536,
            n => n as usize,
        };
        let off_flag = u32_at(o + 4);
        let is_compressed = off_flag & 1 != 0;
        let data_off = (off_flag & !1) as usize;
        let data_end = data_off
            .checked_add(comp_size)
            .ok_or_else(|| format!("sges: segment {i} offset overflow"))?;
        if data_end > data.len() {
            return Err(format!("sges: segment {i} [{data_off}..{data_end}] overruns input"));
        }
        let seg = &data[data_off..data_end];
        if is_compressed {
            let mut dec = flate2::read::DeflateDecoder::new(seg);
            let mut buf = Vec::with_capacity(raw_unc);
            dec.read_to_end(&mut buf).map_err(|e| format!("sges: segment {i} inflate failed: {e}"))?;
            out.extend_from_slice(&buf);
        } else {
            out.extend_from_slice(seg);
        }
    }
    Ok(out)
}

pub fn compress(data: &[u8], endian: Endian) -> Vec<u8> {
    let be = matches!(endian, Endian::Big);
    let put_u16 = |out: &mut Vec<u8>, v: u16| out.extend_from_slice(&if be { v.to_be_bytes() } else { v.to_le_bytes() });
    let put_u32 = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&if be { v.to_be_bytes() } else { v.to_le_bytes() });

    let chunks: Vec<&[u8]> = data.chunks(SEGMENT_SIZE).collect();
    let desc_start = 16usize;
    let desc_end = desc_start + chunks.len() * 8;

    let mut payload = Vec::new();
    let mut descs = Vec::with_capacity(chunks.len());
    let mut offset = desc_end as u32;
    for raw in &chunks {
        assert_eq!(offset & 1, 0, "sges: segment offset must stay even for the flag bit to pack cleanly");
        let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
        enc.write_all(raw).unwrap();
        let compressed = enc.finish().unwrap();
        let (bytes, is_compressed): (&[u8], bool) =
            if compressed.len() < raw.len() { (&compressed, true) } else { (raw, false) };
        descs.push((bytes.len() as u16, raw.len() as u16, offset | (is_compressed as u32)));
        payload.extend_from_slice(bytes);
        offset += bytes.len() as u32;
        if bytes.len() % 2 != 0 {
            payload.push(0);
            offset += 1;
        }
    }

    let mut out = Vec::with_capacity(desc_end + payload.len());
    out.extend_from_slice(if be { b"sges" } else { b"segs" });
    out.extend_from_slice(&[0, 0]);
    put_u16(&mut out, chunks.len() as u16);
    put_u32(&mut out, data.len() as u32);
    out.extend_from_slice(&[0, 0, 0, 0]);
    for (comp_size, raw_unc, off_flag) in descs {
        put_u16(&mut out, comp_size);
        put_u16(&mut out, raw_unc);
        put_u32(&mut out, off_flag);
    }
    out.extend_from_slice(&payload);
    out
}
