use crate::bail;
use crate::error::Result;
use crate::keysource::KeySource;

pub struct SidArgs {
    pub sim_files: Vec<String>,
    pub keys: KeySource,
    pub filter: Option<String>,
    pub out: Option<String>,
}

pub fn parse(argv: &[String]) -> Result<SidArgs> {
    let mut sim_files = Vec::new();
    let mut keys = KeySource::default();
    let mut filter = None;
    let mut out = None;

    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        match arg.as_str() {
            "--key" => {
                i += 1;
                let value = argv
                    .get(i)
                    .ok_or_else(|| crate::error::Error::new("missing value for --key"))?;
                keys.add(value)?;
                i += 1;
            }
            "--keys-file" => {
                i += 1;
                let value = argv
                    .get(i)
                    .ok_or_else(|| crate::error::Error::new("missing value for --keys-file"))?;
                keys.add_file(value)?;
                i += 1;
            }
            "--filter" => {
                i += 1;
                let value = argv
                    .get(i)
                    .ok_or_else(|| crate::error::Error::new("missing value for --filter"))?;
                filter = Some(value.clone());
                i += 1;
            }
            "--out" => {
                i += 1;
                let value = argv
                    .get(i)
                    .ok_or_else(|| crate::error::Error::new("missing value for --out"))?;
                out = Some(value.clone());
                i += 1;
            }
            _ if arg.starts_with("--") => bail!("unknown option {}", arg),
            _ => {
                sim_files.push(arg.clone());
                i += 1;
            }
        }
    }

    if sim_files.is_empty() {
        bail!("missing required argument: file.sim");
    }

    Ok(SidArgs {
        sim_files,
        keys,
        filter,
        out,
    })
}
