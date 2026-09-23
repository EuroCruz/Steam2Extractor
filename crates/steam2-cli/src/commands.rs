use crate::cli::{DepotArgs, SidArgs};

pub fn extract_depot(args: &DepotArgs) -> Result<(), String> {
    steam2_formats::depot::run(args)
}

pub fn extract_sid(args: &SidArgs) -> Result<(), String> {
    steam2_formats::sim::run(args)
}

pub fn hash(text: &str) -> Result<(), String> {
    println!("0x{:08x}", steam2_hash::pandemic::pandemic(text));
    Ok(())
}

pub fn version() -> Result<(), String> {
    println!("Steam2ExtractV2 v{}", crate::cli::VERSION);
    println!();
    println!("crates:");
    for (name, version) in [
        ("steam2-cli", crate::cli::VERSION),
        ("steam2-formats", steam2_formats::VERSION),
        ("steam2-compression", steam2_compression::VERSION),
        ("steam2-crypto", steam2_crypto::VERSION),
        ("steam2-hash", steam2_hash::VERSION),
        ("steam2-core", steam2_core::VERSION),
    ] {
        println!("  {name:<24} v{version}");
    }
    if let Ok(exe) = std::env::current_exe() {
        println!();
        println!("path: {}", exe.display());
    }
    Ok(())
}

pub fn donate() -> Result<(), String> {
    println!("Steam2ExtractV2 is free and always will be.");
    println!("If it saved you time, you can support development here:");
    println!();
    println!("  Bitcoin  bc1q2e60ws5yy6m5czv5wtc97z28vp8et3quvzx35c");
    println!("  Solana   HgqXV2YMWQggDJNDWzJ3rrczwhhy9AAUq7xZcziBqHbC");
    println!("  Base     0xff7b4d8be072dd36eb221c64cdc6ba48cce83b7e");
    println!("  ETH      0xff7b4d8be072dd36eb221c64cdc6ba48cce83b7e");
    println!("  BNB      0xff7b4d8be072dd36eb221c64cdc6ba48cce83b7e");
    Ok(())
}

#[cfg(feature = "debug-tools")]
fn dictbin_compression(s: &str) -> Result<steam2_formats::dictbin::Compression, String> {
    match s {
        "raw" => Ok(steam2_formats::dictbin::Compression::Raw),
        "lzxd" => Ok(steam2_formats::dictbin::Compression::Lzxd),
        "sges" => Ok(steam2_formats::dictbin::Compression::Sges),
        other => Err(format!("unknown dictbin compression '{other}' (known: raw, lzxd, sges)")),
    }
}

#[cfg(feature = "debug-tools")]
fn dictbin_endian(s: &str) -> Result<steam2_core::endian::Endian, String> {
    match s {
        "le" => Ok(steam2_core::endian::Endian::Little),
        "be" => Ok(steam2_core::endian::Endian::Big),
        other => Err(format!("unknown endian '{other}' (known: le, be)")),
    }
}

#[cfg(feature = "debug-tools")]
pub fn debug_dictbin(
    keys_path: &str,
    blobs_path: &str,
    dats_path: &str,
    out_dir: &std::path::Path,
    compression: &str,
    endian: &str,
) -> Result<(), String> {
    let comp = dictbin_compression(compression)?;
    let end = dictbin_endian(endian)?;

    let keys_text = std::fs::read_to_string(keys_path).map_err(|e| format!("{keys_path}: {e}"))?;
    let blobs_text = std::fs::read_to_string(blobs_path).map_err(|e| format!("{blobs_path}: {e}"))?;
    let dats_text = std::fs::read_to_string(dats_path).map_err(|e| format!("{dats_path}: {e}"))?;

    let keys = steam2_formats::dictbin::parse_keys_text(&keys_text)?;
    let blobs = steam2_formats::dictbin::parse_blobs_text(&blobs_text)?;
    let dats = steam2_formats::dictbin::parse_dats_text(&dats_text)?;

    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;

    let keys_bytes = steam2_formats::dictbin::build_keys(&keys, comp, end);
    let blobs_bytes = steam2_formats::dictbin::build_blobs(&blobs, comp, end);
    let dats_bytes = steam2_formats::dictbin::build_dats(&dats, comp, end);

    write_generated(&out_dir.join(steam2_formats::dictbin::Kind::Keys.file_name()), &keys_bytes)?;
    write_generated(&out_dir.join(steam2_formats::dictbin::Kind::Blobs.file_name()), &blobs_bytes)?;
    write_generated(&out_dir.join(steam2_formats::dictbin::Kind::Dats.file_name()), &dats_bytes)?;

    println!(
        "wrote {} key(s), {} blob(s), {} dat(s) -> {} ({compression}, {endian})",
        keys.len(),
        blobs.len(),
        dats.len(),
        out_dir.display()
    );
    Ok(())
}

#[cfg(feature = "debug-tools")]
pub fn debug_dictbin_recompress(compression: &str, endian: &str, out_dir: &std::path::Path) -> Result<(), String> {
    let comp = dictbin_compression(compression)?;
    let end = dictbin_endian(endian)?;
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;

    let keys_bytes = steam2_formats::dictbin::load(steam2_formats::dictbin::Kind::Keys.file_name())?;
    let keys = steam2_formats::dictbin::parse_keys(&keys_bytes)?;
    let out_keys = steam2_formats::dictbin::build_keys(&keys.records, comp, end);
    write_generated(&out_dir.join(steam2_formats::dictbin::Kind::Keys.file_name()), &out_keys)?;

    let blobs_bytes = steam2_formats::dictbin::load(steam2_formats::dictbin::Kind::Blobs.file_name())?;
    let blobs = steam2_formats::dictbin::parse_blobs(&blobs_bytes)?;
    let out_blobs = steam2_formats::dictbin::build_blobs(&blobs.records, comp, end);
    write_generated(&out_dir.join(steam2_formats::dictbin::Kind::Blobs.file_name()), &out_blobs)?;

    let dats_bytes = steam2_formats::dictbin::load(steam2_formats::dictbin::Kind::Dats.file_name())?;
    let dats = steam2_formats::dictbin::parse_dats(&dats_bytes)?;
    let out_dats = steam2_formats::dictbin::build_dats(&dats.records, comp, end);
    write_generated(&out_dir.join(steam2_formats::dictbin::Kind::Dats.file_name()), &out_dats)?;

    println!("repacked keys/blobs/dats -> {} ({compression}, {endian})", out_dir.display());
    Ok(())
}

#[cfg(feature = "debug-tools")]
fn write_generated(out_path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(out_path, bytes).map_err(|e| format!("{}: {e}", out_path.display()))
}
