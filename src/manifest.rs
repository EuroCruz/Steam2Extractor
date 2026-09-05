use std::collections::HashMap;

use crate::adler32::adler32;
use crate::bail;
use crate::cp1252;
use crate::error::Result;
use crate::reader::{cstr_at, read_u32_at as read_u32};

const HEADER_SIZE: usize = 0x38;
const NODE_SIZE: usize = 0x1c;

#[derive(Clone, Copy)]
pub struct Header {
    pub app_id: u32,
    pub ver_id: u32,
}

#[derive(Clone, Copy)]
pub struct DirNode {
    pub name_offset: u32,
    pub file_id: u32,
    pub flags: u32,
    pub parent: u32,
}

pub struct Manifest {
    pub header: Header,
    pub nodes: Vec<DirNode>,
    pub id_to_path: HashMap<u32, String>,
}

impl Manifest {
    pub fn parse(data: &[u8]) -> Result<Manifest> {
        if data.len() < HEADER_SIZE {
            bail!("manifest: too small for header");
        }
        let mst_version = read_u32(data, 0);
        if mst_version != 3 && mst_version != 4 {
            bail!("manifest: version isn't 3 or 4");
        }
        let binary_size = read_u32(data, 24);
        if binary_size as usize != data.len() {
            bail!("manifest: manifest size is wrong");
        }
        let checksum = read_u32(data, 52);

        let mut checked = data.to_vec();
        checked[48..52].copy_from_slice(&0u32.to_le_bytes());
        checked[52..56].copy_from_slice(&0u32.to_le_bytes());
        let computed = adler32(0, &checked);
        if checksum != computed {
            bail!("manifest: checksum is wrong");
        }

        let num_of_nodes = read_u32(data, 12);
        let header = Header {
            app_id: read_u32(data, 4),
            ver_id: read_u32(data, 8),
        };

        let nodes_start = HEADER_SIZE;
        let nodes_end = nodes_start + NODE_SIZE * num_of_nodes as usize;
        if nodes_end > data.len() {
            bail!("manifest: node table out of bounds");
        }
        let mut nodes = Vec::with_capacity(num_of_nodes as usize);
        for i in 0..num_of_nodes as usize {
            let off = nodes_start + i * NODE_SIZE;
            nodes.push(DirNode {
                name_offset: read_u32(data, off),
                file_id: read_u32(data, off + 8),
                flags: read_u32(data, off + 12),
                parent: read_u32(data, off + 16),
            });
        }

        let string_table = nodes_end;
        let mut id_to_path = HashMap::with_capacity(nodes.len());
        for node in &nodes {
            let mut path = String::new();
            let mut current = *node;
            let mut hops = 0usize;
            while current.parent != 0xffffffff {
                hops += 1;
                if hops > nodes.len() {
                    bail!("manifest: cyclic node parent chain");
                }
                let name_bytes = cstr_at(data, string_table + current.name_offset as usize)?;
                let name = cp1252::decode_string(name_bytes);
                if current.parent != 0 {
                    path = format!("/{}{}", name, path);
                } else {
                    path = format!("{}{}", name, path);
                }
                let parent_idx = current.parent as usize;
                if parent_idx >= nodes.len() {
                    bail!("manifest: node parent index out of bounds");
                }
                current = nodes[parent_idx];
            }
            id_to_path.insert(node.file_id, path);
        }

        Ok(Manifest {
            header,
            nodes,
            id_to_path,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CompressionType {
    Uncompressed = 0,
    Compressed = 1,
    CompressedAndEncrypted = 2,
    Encrypted = 3,
}

impl CompressionType {
    pub fn from_u8(v: u8) -> Result<CompressionType> {
        match v {
            0 => Ok(CompressionType::Uncompressed),
            1 => Ok(CompressionType::Compressed),
            2 => Ok(CompressionType::CompressedAndEncrypted),
            3 => Ok(CompressionType::Encrypted),
            _ => bail!("manifest: invalid compression type {}", v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_paths_and_checksum() {
        let mut data = vec![0u8; HEADER_SIZE + NODE_SIZE * 3 + 12];

        data[0..4].copy_from_slice(&4u32.to_le_bytes());
        data[4..8].copy_from_slice(&1000u32.to_le_bytes());
        data[8..12].copy_from_slice(&5u32.to_le_bytes());
        data[12..16].copy_from_slice(&3u32.to_le_bytes());
        let len = data.len() as u32;
        data[24..28].copy_from_slice(&len.to_le_bytes());

        let node = |off: usize, name_offset: u32, file_id: u32, flags: u32, parent: u32| {
            (off, name_offset, file_id, flags, parent)
        };
        let nodes = [
            node(0, 0, 0, 0, 0xffffffff),
            node(1, 1, 1, 1, 0),
            node(2, 6, 2, 1, 1),
        ];
        for (i, name_offset, file_id, flags, parent) in nodes {
            let off = HEADER_SIZE + NODE_SIZE * i;
            data[off..off + 4].copy_from_slice(&name_offset.to_le_bytes());
            data[off + 8..off + 12].copy_from_slice(&file_id.to_le_bytes());
            data[off + 12..off + 16].copy_from_slice(&flags.to_le_bytes());
            data[off + 16..off + 20].copy_from_slice(&parent.to_le_bytes());
        }

        let string_table = HEADER_SIZE + NODE_SIZE * 3;
        data[string_table..string_table + 12].copy_from_slice(b"\0dirA\0fileX\0");

        let checksum = adler32(0, &data);
        data[52..56].copy_from_slice(&checksum.to_le_bytes());

        let manifest = Manifest::parse(&data).unwrap();
        assert_eq!(manifest.header.app_id, 1000);
        assert_eq!(manifest.header.ver_id, 5);
        assert_eq!(manifest.id_to_path[&1], "dirA");
        assert_eq!(manifest.id_to_path[&2], "dirA/fileX");
    }

    fn build_manifest(nodes: &[(u32, u32, u32, u32)], string_table_bytes: &[u8]) -> Vec<u8> {
        let mut data = vec![0u8; HEADER_SIZE + NODE_SIZE * nodes.len() + string_table_bytes.len()];
        data[0..4].copy_from_slice(&4u32.to_le_bytes());
        data[12..16].copy_from_slice(&(nodes.len() as u32).to_le_bytes());
        let len = data.len() as u32;
        data[24..28].copy_from_slice(&len.to_le_bytes());

        for (i, &(name_offset, file_id, flags, parent)) in nodes.iter().enumerate() {
            let off = HEADER_SIZE + NODE_SIZE * i;
            data[off..off + 4].copy_from_slice(&name_offset.to_le_bytes());
            data[off + 8..off + 12].copy_from_slice(&file_id.to_le_bytes());
            data[off + 12..off + 16].copy_from_slice(&flags.to_le_bytes());
            data[off + 16..off + 20].copy_from_slice(&parent.to_le_bytes());
        }

        let string_table = HEADER_SIZE + NODE_SIZE * nodes.len();
        data[string_table..string_table + string_table_bytes.len()]
            .copy_from_slice(string_table_bytes);

        let checksum = adler32(0, &data);
        data[52..56].copy_from_slice(&checksum.to_le_bytes());
        data
    }

    #[test]
    fn rejects_cyclic_parent_chain() {
        let nodes = [(0u32, 0u32, 1u32, 1u32), (0u32, 1u32, 1u32, 0u32)];
        let data = build_manifest(&nodes, b"\0");
        assert!(Manifest::parse(&data).is_err());
    }

    #[test]
    fn rejects_out_of_bounds_parent() {
        let nodes = [(0u32, 0u32, 1u32, 999u32)];
        let data = build_manifest(&nodes, b"\0");
        assert!(Manifest::parse(&data).is_err());
    }
}
