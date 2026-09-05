use crate::error::{Error, Result};
use crate::keysource::KeySource;

pub fn parse_common_flag(
    arg: &str,
    argv: &[String],
    i: &mut usize,
    keys: &mut KeySource,
    filter: &mut Option<String>,
    out: &mut Option<String>,
) -> Result<bool> {
    match arg {
        "--key" => {
            *i += 1;
            let value = argv
                .get(*i)
                .ok_or_else(|| Error::new("missing value for --key"))?;
            keys.add(value)?;
            *i += 1;
            Ok(true)
        }
        "--keys-file" => {
            *i += 1;
            let value = argv
                .get(*i)
                .ok_or_else(|| Error::new("missing value for --keys-file"))?;
            keys.add_file(value)?;
            *i += 1;
            Ok(true)
        }
        "--filter" => {
            *i += 1;
            let value = argv
                .get(*i)
                .ok_or_else(|| Error::new("missing value for --filter"))?;
            *filter = Some(value.clone());
            *i += 1;
            Ok(true)
        }
        "--out" => {
            *i += 1;
            let value = argv
                .get(*i)
                .ok_or_else(|| Error::new("missing value for --out"))?;
            *out = Some(value.clone());
            *i += 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}
