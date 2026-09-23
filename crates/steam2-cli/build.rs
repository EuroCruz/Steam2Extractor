use std::path::{Path, PathBuf};

const KEYS_SOURCE: &str = "../steam2-formats/data/steam2_keys.txt";
const BLOBS_SOURCE: &str = "../steam2-formats/data/steam2_blobs.txt";
const DATS_SOURCE: &str = "../steam2-formats/data/steam2_dats.txt";

fn target_dir(out_dir: &Path) -> Option<PathBuf> {
    out_dir.parent()?.parent()?.parent().map(Path::to_path_buf)
}

fn main() {
    println!("cargo:rerun-if-changed={KEYS_SOURCE}");
    println!("cargo:rerun-if-changed={BLOBS_SOURCE}");
    println!("cargo:rerun-if-changed={DATS_SOURCE}");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR set by cargo"));
    let Some(target_dir) = target_dir(&out_dir) else {
        return;
    };

    let bin_dir = target_dir.join("bin");
    if std::fs::create_dir_all(&bin_dir).is_err() {
        return;
    }

    let endian = steam2_core::endian::Endian::Little;
    let compression = steam2_formats::dictbin::Compression::Lzxd;

    if let Ok(text) = std::fs::read_to_string(KEYS_SOURCE) {
        if let Ok(records) = steam2_formats::dictbin::parse_keys_text(&text) {
            let bytes = steam2_formats::dictbin::build_keys(&records, compression, endian);
            let _ = std::fs::write(bin_dir.join(steam2_formats::dictbin::Kind::Keys.file_name()), bytes);
        }
    }

    if let Ok(text) = std::fs::read_to_string(BLOBS_SOURCE) {
        if let Ok(records) = steam2_formats::dictbin::parse_blobs_text(&text) {
            let bytes = steam2_formats::dictbin::build_blobs(&records, compression, endian);
            let _ = std::fs::write(bin_dir.join(steam2_formats::dictbin::Kind::Blobs.file_name()), bytes);
        }
    }

    if let Ok(text) = std::fs::read_to_string(DATS_SOURCE) {
        if let Ok(records) = steam2_formats::dictbin::parse_dats_text(&text) {
            let bytes = steam2_formats::dictbin::build_dats(&records, compression, endian);
            let _ = std::fs::write(bin_dir.join(steam2_formats::dictbin::Kind::Dats.file_name()), bytes);
        }
    }
}
