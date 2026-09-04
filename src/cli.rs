use crate::bail;
use crate::error::Result;
use crate::keysource::KeySource;

pub struct Args {
    pub depot: u32,
    pub version: u32,
    pub blob_dir: String,
    pub dat_dir: String,
    pub blobcrc: Option<String>,
    pub filter: Option<String>,
    pub keys: KeySource,
    pub out: Option<String>,
    pub offline: bool,
}

pub const DEFAULT_BLOB_DIR: &str = "steam2_cache/blobs";
pub const DEFAULT_DAT_DIR: &str = "steam2_cache/dats";

pub fn parse(argv: &[String]) -> Result<Args> {
    let mut positionals = Vec::new();
    let mut blob_dir = None;
    let mut dat_dir = None;
    let mut blobcrc = None;
    let mut filter = None;
    let mut keys = KeySource::default();
    let mut out = None;
    let mut offline = false;

    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        let slot = match arg.as_str() {
            "--blob-dir" => Some(&mut blob_dir),
            "--dat-dir" => Some(&mut dat_dir),
            "--blobcrc" => Some(&mut blobcrc),
            "--filter" => Some(&mut filter),
            "--out" => Some(&mut out),
            _ => None,
        };
        if let Some(slot) = slot {
            i += 1;
            let value = argv
                .get(i)
                .ok_or_else(|| crate::error::Error::new(format!("missing value for {arg}")))?;
            *slot = Some(value.clone());
            i += 1;
        } else if arg == "--key" {
            i += 1;
            let value = argv
                .get(i)
                .ok_or_else(|| crate::error::Error::new("missing value for --key"))?;
            keys.add(value)?;
            i += 1;
        } else if arg == "--keys-file" {
            i += 1;
            let value = argv
                .get(i)
                .ok_or_else(|| crate::error::Error::new("missing value for --keys-file"))?;
            keys.add_file(value)?;
            i += 1;
        } else if arg == "--offline" {
            offline = true;
            i += 1;
        } else if arg.starts_with("--") {
            bail!("unknown option {}", arg);
        } else {
            positionals.push(arg.clone());
            i += 1;
        }
    }

    if positionals.len() < 2 {
        bail!("missing required arguments: depot, version");
    }
    if positionals.len() > 2 {
        bail!("too many arguments");
    }

    let depot: u32 = positionals[0]
        .parse()
        .map_err(|_| crate::error::Error::new("depot must be an integer"))?;
    let version: u32 = positionals[1]
        .parse()
        .map_err(|_| crate::error::Error::new("version must be an integer"))?;

    Ok(Args {
        depot,
        version,
        blob_dir: blob_dir.unwrap_or_else(|| DEFAULT_BLOB_DIR.to_string()),
        dat_dir: dat_dir.unwrap_or_else(|| DEFAULT_DAT_DIR.to_string()),
        blobcrc,
        filter,
        keys,
        out,
        offline,
    })
}
