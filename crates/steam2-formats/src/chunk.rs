use steam2_compression::inflate::zlib_decompress;
use steam2_crypto::aes::cfb_decrypt;

use crate::manifest::CompressionType;

pub fn handle_chunk(data: &[u8], cmptype: CompressionType, key: &[u8; 16]) -> Result<Vec<u8>, String> {
    if data.len() > 0x10000 {
        return Err("handle_chunk: impossible size chunk".to_string());
    }
    if data.is_empty() {
        return Ok(Vec::new());
    }

    match cmptype {
        CompressionType::Uncompressed => Ok(data.to_vec()),
        CompressionType::Compressed => zlib_decompress(data, 0x8000),
        CompressionType::CompressedAndEncrypted => {
            if data.len() < 8 {
                return Err("handle_chunk: chunk too small for header (filemode 3)".to_string());
            }
            let decompressed_size = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
            if decompressed_size > 0x8000 {
                return Err("handle_chunk: impossible size chunk (filemode 3)".to_string());
            }
            let mut payload = data[8..].to_vec();
            cfb_decrypt(key, &mut payload);
            zlib_decompress(&payload, decompressed_size)
        }
        CompressionType::Encrypted => {
            let mut payload = data.to_vec();
            cfb_decrypt(key, &mut payload);
            Ok(payload)
        }
    }
}
