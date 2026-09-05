use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::bail;
use crate::error::{Error, Result};

const MIRRORS: [&str; 3] = ["de", "us", "ro"];
const SCHEMES: [&str; 2] = ["https", "http"];

const CYCLE_SLEEP_START: Duration = Duration::from_secs(15);
const CYCLE_SLEEP_CAP: Duration = Duration::from_secs(180);

const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const MAX_ATTEMPTS_PER_CYCLE: usize = MIRRORS.len() * SCHEMES.len() * 2;
const SWITCH_MARGIN_MS: u128 = 50;
const CURL_TIMEOUT_ARGS: [&str; 3] = ["--retry-connrefused", "--connect-timeout", "10"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Backend {
    Curl,
    Wget,
    PowerShell,
}

fn runs(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn find_backend() -> Option<Backend> {
    static BACKEND: OnceLock<Option<Backend>> = OnceLock::new();
    *BACKEND.get_or_init(|| {
        if runs("curl", &["--version"]) {
            Some(Backend::Curl)
        } else if runs("wget", &["--version"]) {
            Some(Backend::Wget)
        } else if cfg!(windows) && runs("powershell", &["-NoProfile", "-Command", "exit"]) {
            Some(Backend::PowerShell)
        } else {
            None
        }
    })
}

fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn probe_latency(backend: Backend, mirror: &str) -> Option<u128> {
    if backend == Backend::PowerShell {
        return None;
    }
    for scheme in SCHEMES {
        let url = format!("{scheme}://{mirror}.steam2.download/blobs_dates.txt");
        let start = Instant::now();
        let ok = match backend {
            Backend::Curl => runs("curl", &["-s", "--max-time", "3", "-I", &url]),
            Backend::Wget => runs("wget", &["--spider", "-q", "--timeout=3", &url]),
            Backend::PowerShell => unreachable!(),
        };
        if ok {
            return Some(start.elapsed().as_millis());
        }
    }
    None
}

fn probe_all_mirrors(backend: Backend) -> Vec<(&'static str, Option<u128>)> {
    let mut timed: Vec<(&'static str, Option<u128>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = MIRRORS
            .iter()
            .map(|&m| (m, scope.spawn(move || probe_latency(backend, m))))
            .collect();
        handles
            .into_iter()
            .map(|(m, h)| (m, h.join().unwrap_or(None)))
            .collect()
    });
    timed.sort_by_key(|(_, ms)| ms.unwrap_or(u128::MAX));
    timed
}

struct MirrorCache {
    ranking: Vec<(&'static str, Option<u128>)>,
    at: Instant,
}

fn ranked_mirrors(backend: Backend, force: bool) -> Vec<(&'static str, Option<u128>)> {
    static CACHE: OnceLock<Mutex<MirrorCache>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        Mutex::new(MirrorCache {
            ranking: probe_all_mirrors(backend),
            at: Instant::now(),
        })
    });
    let mut guard = cache.lock().unwrap();
    if force || guard.at.elapsed() >= HEALTH_CHECK_INTERVAL {
        guard.ranking = probe_all_mirrors(backend);
        guard.at = Instant::now();
    }
    guard.ranking.clone()
}

fn powershell_resume_script(url: &str, tmp: &Path) -> String {
    const TEMPLATE: &str = r#"
$ErrorActionPreference = 'Stop'
$dest = __TMP__
$offset = 0
if (Test-Path $dest) { $offset = (Get-Item $dest).Length }
try {
    $req = [System.Net.HttpWebRequest]::Create(__URL__)
    $req.Method = 'GET'
    $req.Timeout = 15000
    $req.ReadWriteTimeout = 30000
    if ($offset -gt 0) { $req.AddRange([int64]$offset) }
    $resp = $req.GetResponse()
    $stream = $resp.GetResponseStream()
    $fs = [System.IO.File]::Open($dest, [System.IO.FileMode]::Append, [System.IO.FileAccess]::Write)
    $buf = New-Object byte[] 65536
    $total = $offset
    $last = Get-Date
    while (($read = $stream.Read($buf, 0, $buf.Length)) -gt 0) {
        $fs.Write($buf, 0, $read)
        $total += $read
        if ((New-TimeSpan -Start $last -End (Get-Date)).TotalMilliseconds -gt 500) {
            $mb = [Math]::Round($total / 1MB, 1)
            Write-Host -NoNewline "`rdownloaded $mb MB"
            $last = Get-Date
        }
    }
    Write-Host ""
    $fs.Close(); $stream.Close(); $resp.Close()
    exit 0
} catch {
    Write-Host $_.Exception.Message
    exit 1
}
"#;
    TEMPLATE
        .replace("__TMP__", &ps_quote(&tmp.display().to_string()))
        .replace("__URL__", &ps_quote(url))
}

fn spawn_fetch(backend: Backend, url: &str, tmp: &Path) -> Result<Child> {
    match backend {
        Backend::Curl => Command::new("curl")
            .args(["-f", "-L", "-C", "-", "--retry", "2"])
            .args(CURL_TIMEOUT_ARGS)
            .args(["--progress-bar", "-o"])
            .arg(tmp)
            .arg(url)
            .spawn()
            .map_err(|e| Error::new(format!("net: failed to run curl ({e})"))),
        Backend::Wget => Command::new("wget")
            .args(["-c", "--tries=2", "--timeout=10", "-O"])
            .arg(tmp)
            .arg(url)
            .spawn()
            .map_err(|e| Error::new(format!("net: failed to run wget ({e})"))),
        Backend::PowerShell => {
            let script = powershell_resume_script(url, tmp);
            Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .spawn()
                .map_err(|e| Error::new(format!("net: failed to run powershell ({e})")))
        }
    }
}

fn tmp_len(tmp: &Path) -> u64 {
    fs::metadata(tmp).map(|m| m.len()).unwrap_or(0)
}

fn candidate_list(ranking: &[(&'static str, Option<u128>)]) -> Vec<(&'static str, &'static str)> {
    ranking
        .iter()
        .flat_map(|&(m, _)| SCHEMES.iter().map(move |&s| (m, s)))
        .collect()
}

fn race_download(backend: Backend, remote_path: &str, tmp: &Path) -> Result<bool> {
    let mut candidates = candidate_list(&ranked_mirrors(backend, false));
    let mut idx = 0usize;
    let mut attempts = 0usize;

    loop {
        if idx >= candidates.len() || attempts >= MAX_ATTEMPTS_PER_CYCLE {
            return Ok(false);
        }
        attempts += 1;

        let (mirror, scheme) = candidates[idx];
        let url = format!("{scheme}://{mirror}.steam2.download/{remote_path}");
        eprintln!("downloading {url}");
        let mut child = spawn_fetch(backend, &url, tmp)?;
        let mut last_check = Instant::now();
        let mut last_size = tmp_len(tmp);

        loop {
            std::thread::sleep(POLL_INTERVAL);

            if let Some(status) = child.try_wait()? {
                if status.success() {
                    return Ok(true);
                }
                eprintln!("{url} failed, trying next mirror");
                idx += 1;
                break;
            }

            if last_check.elapsed() < HEALTH_CHECK_INTERVAL {
                continue;
            }
            last_check = Instant::now();

            let size_now = tmp_len(tmp);
            let stalled = size_now == last_size;
            last_size = size_now;

            if stalled {
                eprintln!("{url} stalled, trying next mirror");
                let _ = child.kill();
                let _ = child.wait();
                idx += 1;
                break;
            }

            let fresh = ranked_mirrors(backend, true);
            let current_ms = fresh.iter().find(|(m, _)| *m == mirror).and_then(|(_, ms)| *ms);
            let switch_to_best = match fresh.first() {
                Some(&(best, _)) if best == mirror => None,
                Some(&(best, Some(best_ms))) => match current_ms {
                    Some(cur_ms) if best_ms + SWITCH_MARGIN_MS < cur_ms => Some(best),
                    None => Some(best),
                    _ => None,
                },
                _ => None,
            };

            if let Some(best) = switch_to_best {
                eprintln!("faster mirror {best} detected, switching");
                let _ = child.kill();
                let _ = child.wait();
                candidates = candidate_list(&fresh);
                idx = 0;
                break;
            }
        }
    }
}

fn download(remote_path: &str, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let backend = match find_backend() {
        Some(b) => b,
        None => bail!(
            "net: no downloader found; install curl (or wget) and make sure it's on PATH \
             (on Windows, PowerShell also works as a fallback)"
        ),
    };
    let tmp = dest.with_extension("part");
    let mut sleep = CYCLE_SLEEP_START;
    loop {
        if race_download(backend, remote_path, &tmp)? {
            fs::rename(&tmp, dest)?;
            return Ok(());
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

fn parse_content_length(text: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let trimmed = line.trim().to_ascii_lowercase();
        trimmed
            .strip_prefix("content-length:")
            .and_then(|rest| rest.trim().parse().ok())
    })
}

fn head_size(backend: Backend, url: &str) -> Option<u64> {
    match backend {
        Backend::Curl => {
            let output = Command::new("curl")
                .args(["-sI", "--retry", "5"])
                .args(CURL_TIMEOUT_ARGS)
                .arg(url)
                .output()
                .ok()?;
            output
                .status
                .success()
                .then(|| parse_content_length(&String::from_utf8_lossy(&output.stdout)))
                .flatten()
        }
        Backend::Wget => {
            let output = Command::new("wget")
                .args(["--spider", "-S", "--tries=5", "--timeout=10"])
                .arg(url)
                .output()
                .ok()?;
            parse_content_length(&String::from_utf8_lossy(&output.stderr))
        }
        Backend::PowerShell => {
            let script = format!(
                "try {{ (Invoke-WebRequest -Uri {} -Method Head -UseBasicParsing).Headers.'Content-Length' }} catch {{ exit 1 }}",
                ps_quote(url)
            );
            let output = Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .output()
                .ok()?;
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().parse().ok())
                .flatten()
        }
    }
}

pub fn remote_size(remote_subdir: &str, filename: &str) -> Result<u64> {
    let backend = match find_backend() {
        Some(b) => b,
        None => bail!("net: no downloader found; install curl, wget, or (on Windows) PowerShell"),
    };
    for (mirror, _) in ranked_mirrors(backend, false) {
        for scheme in SCHEMES {
            let url = format!("{scheme}://{mirror}.steam2.download/{remote_subdir}/{filename}");
            if let Some(n) = head_size(backend, &url) {
                return Ok(n);
            }
        }
    }
    bail!("net: failed to get remote size for {}", filename);
}
