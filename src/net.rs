use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::bail;
use crate::error::Result;

const MIRRORS: [&str; 3] = ["de", "us", "ro"];
const SCHEMES: [&str; 2] = ["https", "http"];

const CYCLE_SLEEP_START: Duration = Duration::from_secs(15);
const CYCLE_SLEEP_CAP: Duration = Duration::from_secs(180);

fn probe_latency(mirror: &str) -> Option<u128> {
    for scheme in SCHEMES {
        let url = format!("{scheme}://{mirror}.steam2.download/blobs_dates.txt");
        let start = Instant::now();
        let ok = Command::new("curl")
            .args(["-s", "--max-time", "3", "-I", &url])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Some(start.elapsed().as_millis());
        }
    }
    None
}

fn ordered_mirrors() -> &'static [&'static str] {
    static ORDER: OnceLock<Vec<&'static str>> = OnceLock::new();
    ORDER.get_or_init(|| {
        let mut timed: Vec<(&'static str, Option<u128>)> = std::thread::scope(|scope| {
            let handles: Vec<_> = MIRRORS
                .iter()
                .map(|&m| (m, scope.spawn(move || probe_latency(m))))
                .collect();
            handles
                .into_iter()
                .map(|(m, h)| (m, h.join().unwrap_or(None)))
                .collect()
        });
        timed.sort_by_key(|(_, ms)| ms.unwrap_or(u128::MAX));
        timed.into_iter().map(|(m, _)| m).collect()
    })
}

fn curl_fetch(url: &str, tmp: &Path) -> Result<bool> {
    let status = Command::new("curl")
        .args([
            "-fsSL",
            "-C",
            "-",
            "--retry",
            "5",
            "--retry-connrefused",
            "--connect-timeout",
            "10",
            "-o",
        ])
        .arg(tmp)
        .arg(url)
        .status();
    match status {
        Ok(s) => Ok(s.success()),
        Err(e) => bail!("net: failed to run curl ({e}), is it installed and on PATH?"),
    }
}

fn download(remote_path: &str, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("part");
    let mut sleep = CYCLE_SLEEP_START;
    loop {
        for mirror in ordered_mirrors() {
            for scheme in SCHEMES {
                let url = format!("{scheme}://{mirror}.steam2.download/{remote_path}");
                eprintln!("downloading {url}");
                if curl_fetch(&url, &tmp)? {
                    fs::rename(&tmp, dest)?;
                    return Ok(());
                }
                eprintln!("{url} failed, trying next");
            }
        }
        eprintln!(
            "all mirrors unreachable, retrying in {}s (partial download kept)",
            sleep.as_secs()
        );
        std::thread::sleep(sleep);
        sleep = (sleep * 2).min(CYCLE_SLEEP_CAP);
    }
}

pub fn ensure_file(cache_dir: &Path, remote_subdir: &str, filename: &str) -> Result<PathBuf> {
    let dest = cache_dir.join(filename);
    if !dest.exists() {
        download(&format!("{remote_subdir}/{filename}"), &dest)?;
    }
    Ok(dest)
}

pub fn remote_size(remote_subdir: &str, filename: &str) -> Result<u64> {
    for mirror in ordered_mirrors() {
        for scheme in SCHEMES {
            let url = format!("{scheme}://{mirror}.steam2.download/{remote_subdir}/{filename}");
            let output = Command::new("curl")
                .args([
                    "-sI",
                    "--retry",
                    "5",
                    "--retry-connrefused",
                    "--connect-timeout",
                    "10",
                ])
                .arg(&url)
                .output();
            let Ok(output) = output else { continue };
            if !output.status.success() {
                continue;
            }
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let lower = line.to_ascii_lowercase();
                if let Some(rest) = lower.strip_prefix("content-length:")
                    && let Ok(n) = rest.trim().parse()
                {
                    return Ok(n);
                }
            }
        }
    }
    bail!("net: failed to get remote size for {}", filename);
}
