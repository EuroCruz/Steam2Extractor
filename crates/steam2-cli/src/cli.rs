pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use steam2_formats::depot::DepotArgs;
pub use steam2_formats::sim::SidArgs;

pub enum Command {
    ExtractDepot(DepotArgs),
    ExtractSid(SidArgs),
    Hash { text: String },
    Version,
    Donate,
    #[cfg(feature = "debug-tools")]
    DebugDictBin { keys_path: String, blobs_path: String, dats_path: String, out_dir: std::path::PathBuf, compression: String, endian: String },
    #[cfg(feature = "debug-tools")]
    DebugDictBinRecompress { compression: String, endian: String, out_dir: std::path::PathBuf },
}

fn parse_common_flag(
    arg: &str,
    argv: &[String],
    i: &mut usize,
    keys: &mut steam2_formats::keysource::KeySource,
    filter: &mut Option<String>,
    out: &mut Option<String>,
) -> Result<bool, String> {
    match arg {
        "--key" => {
            *i += 1;
            let value = argv.get(*i).ok_or("missing value for --key")?;
            keys.add(value)?;
            *i += 1;
            Ok(true)
        }
        "--keys-file" => {
            *i += 1;
            let value = argv.get(*i).ok_or("missing value for --keys-file")?;
            keys.add_file(value)?;
            *i += 1;
            Ok(true)
        }
        "--filter" => {
            *i += 1;
            let value = argv.get(*i).ok_or("missing value for --filter")?;
            *filter = Some(value.clone());
            *i += 1;
            Ok(true)
        }
        "--out" => {
            *i += 1;
            let value = argv.get(*i).ok_or("missing value for --out")?;
            *out = Some(value.clone());
            *i += 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}

const DEFAULT_BLOB_DIR: &str = "steam2_cache/blobs";
const DEFAULT_DAT_DIR: &str = "steam2_cache/dats";

fn is_sim_file(a: &str) -> bool {
    !a.starts_with("--") && a.to_ascii_lowercase().ends_with(".sim")
}

fn parse_depot(argv: &[String]) -> Result<DepotArgs, String> {
    let mut positionals = Vec::new();
    let mut blob_dir = None;
    let mut dat_dir = None;
    let mut blobcrc = None;
    let mut filter = None;
    let mut keys = steam2_formats::keysource::KeySource::default();
    let mut out = None;

    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        if parse_common_flag(arg, argv, &mut i, &mut keys, &mut filter, &mut out)? {
            continue;
        }
        let slot = match arg.as_str() {
            "--blob-dir" => Some(&mut blob_dir),
            "--dat-dir" => Some(&mut dat_dir),
            "--blobcrc" => Some(&mut blobcrc),
            _ => None,
        };
        if let Some(slot) = slot {
            i += 1;
            let value = argv.get(i).ok_or_else(|| format!("missing value for {arg}"))?;
            *slot = Some(value.clone());
            i += 1;
        } else if arg.starts_with("--") {
            return Err(format!("unknown option {}", arg));
        } else {
            positionals.push(arg.clone());
            i += 1;
        }
    }

    if positionals.len() < 2 {
        return Err("missing required arguments: depot, version".to_string());
    }
    if positionals.len() > 2 {
        return Err("too many arguments".to_string());
    }

    let depot: u32 = positionals[0].parse().map_err(|_| "depot must be an integer".to_string())?;
    let version: u32 = positionals[1].parse().map_err(|_| "version must be an integer".to_string())?;

    Ok(steam2_formats::depot::DepotArgs {
        depot,
        version,
        blob_dir: blob_dir.unwrap_or_else(|| DEFAULT_BLOB_DIR.to_string()),
        dat_dir: dat_dir.unwrap_or_else(|| DEFAULT_DAT_DIR.to_string()),
        blobcrc,
        filter,
        keys,
        out,
    })
}

fn parse_sid(argv: &[String]) -> Result<SidArgs, String> {
    let mut sim_files = Vec::new();
    let mut keys = steam2_formats::keysource::KeySource::default();
    let mut filter = None;
    let mut out = None;

    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        if parse_common_flag(arg, argv, &mut i, &mut keys, &mut filter, &mut out)? {
            continue;
        }
        if arg.starts_with("--") {
            return Err(format!("unknown option {}", arg));
        }
        sim_files.push(arg.clone());
        i += 1;
    }

    if sim_files.is_empty() {
        return Err("missing required argument: file.sim".to_string());
    }

    Ok(steam2_formats::sim::SidArgs { sim_files, keys, filter, out })
}

#[cfg(feature = "debug-tools")]
fn is_dictbin_compression(s: &str) -> bool {
    matches!(s, "raw" | "lzxd" | "sges")
}

#[cfg(feature = "debug-tools")]
fn is_endian(s: &str) -> bool {
    matches!(s, "le" | "be")
}

pub fn parse(args: &[String]) -> Option<Result<Command, String>> {
    match args {
        [cmd, text] if cmd == "hash" => Some(Ok(Command::Hash { text: text.clone() })),
        [cmd] if cmd == "version" || cmd == "--version" || cmd == "-v" => Some(Ok(Command::Version)),
        [cmd] if cmd == "donate" => Some(Ok(Command::Donate)),
        #[cfg(feature = "debug-tools")]
        [cmd, sub, keys, blobs, dats, out_dir] if cmd == "debug" && sub == "dictbin" => Some(Ok(Command::DebugDictBin {
            keys_path: keys.clone(),
            blobs_path: blobs.clone(),
            dats_path: dats.clone(),
            out_dir: out_dir.into(),
            compression: "lzxd".into(),
            endian: "le".into(),
        })),
        #[cfg(feature = "debug-tools")]
        [cmd, sub, keys, blobs, dats, out_dir, comp] if cmd == "debug" && sub == "dictbin" && is_dictbin_compression(comp) => {
            Some(Ok(Command::DebugDictBin {
                keys_path: keys.clone(),
                blobs_path: blobs.clone(),
                dats_path: dats.clone(),
                out_dir: out_dir.into(),
                compression: comp.clone(),
                endian: "le".into(),
            }))
        }
        #[cfg(feature = "debug-tools")]
        [cmd, sub, keys, blobs, dats, out_dir, comp, endian]
            if cmd == "debug" && sub == "dictbin" && is_dictbin_compression(comp) && is_endian(endian) =>
        {
            Some(Ok(Command::DebugDictBin {
                keys_path: keys.clone(),
                blobs_path: blobs.clone(),
                dats_path: dats.clone(),
                out_dir: out_dir.into(),
                compression: comp.clone(),
                endian: endian.clone(),
            }))
        }
        #[cfg(feature = "debug-tools")]
        [cmd, sub, out_dir] if cmd == "debug" && sub == "dictbin-recompress" => {
            Some(Ok(Command::DebugDictBinRecompress { compression: "lzxd".into(), endian: "le".into(), out_dir: out_dir.into() }))
        }
        #[cfg(feature = "debug-tools")]
        [cmd, sub, out_dir, comp, endian]
            if cmd == "debug" && sub == "dictbin-recompress" && is_dictbin_compression(comp) && is_endian(endian) =>
        {
            Some(Ok(Command::DebugDictBinRecompress { compression: comp.clone(), endian: endian.clone(), out_dir: out_dir.into() }))
        }
        [cmd, ..] if cmd == "extract" => {
            let rest = &args[1..];
            if rest.iter().any(|a| is_sim_file(a)) {
                Some(parse_sid(rest).map(Command::ExtractSid))
            } else {
                Some(parse_depot(rest).map(Command::ExtractDepot))
            }
        }
        _ => None,
    }
}

#[cfg(feature = "debug-tools")]
fn debug_usage_lines() -> Vec<String> {
    vec![
        String::new(),
        "DEBUG".to_string(),
        "  debug dictbin <keys.txt> <blobs.txt> <dats.txt> <out_dir> [comp] [endian]".to_string(),
        "                                                    build steam2_keys.bin / steam2_blobs.bin / steam2_dats.bin".to_string(),
        "  debug dictbin-recompress <out_dir> [comp] [endian]".to_string(),
        "                                                    repack the running dictionaries with new options".to_string(),
        String::new(),
        "  compression     raw (default) | lzxd | sges".to_string(),
        "  endian          le (default) | be".to_string(),
    ]
}

pub fn usage() -> String {
    let lines = vec![
        format!("Steam2ExtractV2 v{VERSION} — offline Steam2 depot/blob/dat/sid extractor"),
        String::new(),
        "USAGE".to_string(),
        "  steam2extract extract <depot> <version> [options]     extract a depot from blob/dat files".to_string(),
        "  steam2extract extract <file.sim> [file2.sim ...] [options]  extract from a .sim/.sid set".to_string(),
        String::new(),
        "OPTIONS".to_string(),
        format!("  --blob-dir dir    blob directory (default: {DEFAULT_BLOB_DIR})"),
        format!("  --dat-dir dir     dat directory (default: {DEFAULT_DAT_DIR})"),
        "  --blobcrc crc     blob crc, only needed after a depot reset".to_string(),
        "  --key key         depot key, hex or depot:hex, repeatable".to_string(),
        "  --keys-file file  \"depot\" \"key\" pairs, one per line, repeatable".to_string(),
        "  --filter regex    only extract matching paths".to_string(),
        "  --out dir         output directory".to_string(),
        String::new(),
        "UTILITIES".to_string(),
        "  hash      <string>                               print pandemic_hash(string) in hex".to_string(),
        "  version                                          print version and crate details".to_string(),
        "  donate                                           show ways to support development".to_string(),
    ];

    #[cfg(feature = "debug-tools")]
    let lines = lines.into_iter().chain(debug_usage_lines()).collect::<Vec<_>>();

    lines.join("\n")
}
