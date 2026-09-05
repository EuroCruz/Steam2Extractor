use crate::argflags::parse_common_flag;
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
        if parse_common_flag(arg, argv, &mut i, &mut keys, &mut filter, &mut out)? {
            continue;
        }
        if arg.starts_with("--") {
            bail!("unknown option {}", arg);
        }
        sim_files.push(arg.clone());
        i += 1;
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
