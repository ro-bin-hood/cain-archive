use crate::ia;
use crate::i18n::m;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, RANGE, RETRY_AFTER};
use reqwest::{Client, StatusCode};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

pub const MAX_ATTEMPTS: u32 = 3;

#[derive(Debug, PartialEq)]
pub enum Finish {
    Done(u64),
    Cancelled,
    Failed(String),
    Disk(String),
}

#[derive(Debug, PartialEq)]
enum AttemptError {
    Retryable { msg: String, retry_after: Option<u64> },
    Fatal(String),
    Disk(String),
    Cancelled,
}

pub struct Request<'a> {
    pub client: &'a Client,
    pub url: &'a str,
    pub headers: &'a HeaderMap,
    pub authed: bool,
    pub dest: &'a Path,
    pub expected: Option<u64>,
    pub retry_base_s: u64,
}

pub fn retry_delay(attempt: u32, retry_after: Option<u64>, base_s: u64) -> u64 {
    match retry_after {
        Some(s) => s.min(60),
        None => base_s * attempt as u64,
    }
}

pub fn part_path(dest: &Path) -> PathBuf {
    let mut s = dest.as_os_str().to_owned();
    s.push(".part");
    PathBuf::from(s)
}

fn sanitize(part: &str) -> String {
    let s: String = part
        .chars()
        .map(|c| if matches!(c, '<' | '>' | ':' | '"' | '\\' | '|' | '?' | '*') || c.is_control() { '_' } else { c })
        .collect();
    let s = s.trim_end_matches(['.', ' ']);
    // CON, AUX, COM1… are devices on Windows, even with an extension (aux.h).
    let (stem, rest) = s.split_at(s.find('.').unwrap_or(s.len()));
    let up = stem.to_ascii_uppercase();
    let reserved = matches!(up.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((up.starts_with("COM") || up.starts_with("LPT")) && up.len() == 4 && up.as_bytes()[3].is_ascii_digit() && up.as_bytes()[3] != b'0');
    if reserved { format!("{stem}_{rest}") } else { s.to_string() }
}

/// `<out_dir>/<item>/<path>`, with Windows-safe names and no way out of the folder.
pub fn dest_for(out_dir: &Path, item_id: &str, name: &str) -> PathBuf {
    let mut p = out_dir.join(sanitize(item_id));
    for seg in name.split('/') {
        if seg == ".." || seg == "." {
            continue;
        }
        let s = sanitize(seg);
        if !s.is_empty() {
            p.push(s);
        }
    }
    p
}

fn disk(e: std::io::Error) -> AttemptError {
    AttemptError::Disk(e.to_string())
}

fn parse_retry_after(h: &HeaderMap) -> Option<u64> {
    h.get(RETRY_AFTER)?.to_str().ok()?.trim().parse().ok()
}

async fn finalize(part: &Path, dest: &Path, expected: Option<u64>) -> Result<u64, AttemptError> {
    let len = tokio::fs::metadata(part).await.map_err(disk)?.len();
    match expected {
        Some(e) if len < e => {
            // Kept: the next attempt resumes from here with Range.
            return Err(AttemptError::Retryable { msg: format!("{} ({len}/{e} byte)", m("File incompleto", "Incomplete file")), retry_after: None });
        }
        Some(e) if len > e => {
            tokio::fs::remove_file(part).await.map_err(disk)?;
            return Err(AttemptError::Retryable { msg: format!("{} ({len} ≠ {e} byte)", m("Dimensione errata", "Wrong size")), retry_after: None });
        }
        _ => {}
    }
    tokio::fs::rename(part, dest).await.map_err(disk)?;
    Ok(len)
}

async fn attempt(req: &Request<'_>, cancel: &CancellationToken, on_progress: &(dyn Fn(u64, Option<u64>) + Send + Sync)) -> Result<u64, AttemptError> {
    if let Ok(m) = tokio::fs::metadata(req.dest).await {
        if req.expected.is_none_or(|e| e == m.len()) {
            return Ok(m.len());
        }
        tokio::fs::remove_file(req.dest).await.map_err(disk)?;
    }
    if let Some(parent) = req.dest.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(disk)?;
    }
    let part = part_path(req.dest);
    let mut have = tokio::fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
    let mut headers = req.headers.clone();
    if have > 0 {
        headers.insert(RANGE, HeaderValue::from_str(&format!("bytes={have}-")).expect("header Range"));
    }
    let resp = tokio::select! {
        r = ia::get(req.client, req.url, &headers) => r.map_err(|msg| AttemptError::Retryable { msg: format!("{}: {msg}", m("Errore di rete", "Network error")), retry_after: None })?,
        _ = cancel.cancelled() => return Err(AttemptError::Cancelled),
    };
    let status = resp.status();
    match status {
        StatusCode::RANGE_NOT_SATISFIABLE if have > 0 => return finalize(&part, req.dest, req.expected).await,
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            let why = if req.authed { m("Accesso negato: item riservato", "Access denied: restricted item") } else { m("Accesso negato: serve accedere", "Access denied: log in required") };
            return Err(AttemptError::Fatal(why.to_string()));
        }
        StatusCode::NOT_FOUND => return Err(AttemptError::Fatal(m("File non trovato", "File not found").into())),
        s if s == StatusCode::TOO_MANY_REQUESTS || s.is_server_error() => {
            return Err(AttemptError::Retryable { msg: format!("{} (HTTP {})", m("Server occupato", "Server busy"), s.as_u16()), retry_after: parse_retry_after(resp.headers()) });
        }
        s if !s.is_success() => return Err(AttemptError::Fatal(format!("{} {}", m("Errore HTTP", "HTTP error"), s.as_u16()))),
        _ => {}
    }
    if status != StatusCode::PARTIAL_CONTENT {
        have = 0; // the server ignored Range: start over
    }
    let total = resp.content_length().map(|n| n + have).or(req.expected);
    let mut file = if have > 0 {
        tokio::fs::OpenOptions::new().append(true).open(&part).await
    } else {
        tokio::fs::File::create(&part).await
    }
    .map_err(disk)?;
    let mut done = have;
    on_progress(done, total);
    let mut stream = resp.bytes_stream();
    loop {
        let next = tokio::select! {
            n = stream.next() => n,
            _ = cancel.cancelled() => {
                file.flush().await.map_err(disk)?;
                return Err(AttemptError::Cancelled);
            }
        };
        match next {
            None => break,
            Some(Err(e)) => {
                file.flush().await.map_err(disk)?;
                return Err(AttemptError::Retryable { msg: format!("{}: {e}", m("Connessione interrotta", "Connection dropped")), retry_after: None });
            }
            Some(Ok(chunk)) => {
                file.write_all(&chunk).await.map_err(disk)?;
                done += chunk.len() as u64;
                on_progress(done, total);
            }
        }
    }
    file.flush().await.map_err(disk)?;
    drop(file);
    finalize(&part, req.dest, req.expected).await
}

pub async fn download(
    req: &Request<'_>,
    cancel: &CancellationToken,
    on_progress: &(dyn Fn(u64, Option<u64>) + Send + Sync),
    on_retry: &(dyn Fn(u32, u64) + Send + Sync),
) -> Finish {
    let part = part_path(req.dest);
    let part_len = || std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    // Consecutive failures without progress: a drop that still downloaded some bytes
    // starts again from the first attempt, so a large file doesn't fail after three drops in an hour.
    let mut failures = 0;
    loop {
        let before = part_len();
        match attempt(req, cancel, on_progress).await {
            Ok(len) => return Finish::Done(len),
            Err(AttemptError::Cancelled) => return Finish::Cancelled,
            Err(AttemptError::Fatal(m)) => return Finish::Failed(m),
            Err(AttemptError::Disk(m)) => return Finish::Disk(m),
            Err(AttemptError::Retryable { msg, retry_after }) => {
                failures = if part_len() > before { 1 } else { failures + 1 };
                if failures >= MAX_ATTEMPTS {
                    return Finish::Failed(msg);
                }
                let wait = retry_delay(failures, retry_after, req.retry_base_s);
                on_retry(failures, wait);
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
                    _ = cancel.cancelled() => return Finish::Cancelled,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_delay_grows_and_respects_retry_after() {
        assert_eq!(retry_delay(1, None, 3), 3);
        assert_eq!(retry_delay(2, None, 3), 6);
        assert_eq!(retry_delay(1, Some(10), 3), 10);
        assert_eq!(retry_delay(1, Some(500), 3), 60);
    }

    #[test]
    fn part_path_appends_suffix() {
        assert_eq!(part_path(Path::new("C:/x/a.tar.gz")), PathBuf::from("C:/x/a.tar.gz.part"));
    }

    #[test]
    fn dest_for_builds_subfolders() {
        assert_eq!(dest_for(Path::new("/out"), "item", "dir/sub/a.flac"), PathBuf::from("/out/item/dir/sub/a.flac"));
    }

    #[test]
    fn dest_for_sanitizes_windows_chars() {
        assert_eq!(dest_for(Path::new("/out"), "item", "a:b?<c>|\"d*.txt"), PathBuf::from("/out/item/a_b__c___d_.txt"));
        assert_eq!(dest_for(Path::new("/out"), "item", "name. "), PathBuf::from("/out/item/name"));
    }

    #[test]
    fn dest_for_avoids_windows_reserved_names() {
        assert_eq!(dest_for(Path::new("/out"), "item", "src/aux.h"), PathBuf::from("/out/item/src/aux_.h"));
        assert_eq!(dest_for(Path::new("/out"), "item", "CON"), PathBuf::from("/out/item/CON_"));
        assert_eq!(dest_for(Path::new("/out"), "item", "com1.tar.gz"), PathBuf::from("/out/item/com1_.tar.gz"));
        assert_eq!(dest_for(Path::new("/out"), "item", "Lpt9.txt"), PathBuf::from("/out/item/Lpt9_.txt"));
        assert_eq!(dest_for(Path::new("/out"), "item", "console.txt"), PathBuf::from("/out/item/console.txt"));
        assert_eq!(dest_for(Path::new("/out"), "nul", "a.txt"), PathBuf::from("/out/nul_/a.txt"));
    }

    #[test]
    fn dest_for_blocks_traversal() {
        assert_eq!(dest_for(Path::new("/out"), "item", "../../evil/./x.txt"), PathBuf::from("/out/item/evil/x.txt"));
        assert_eq!(dest_for(Path::new("/out"), "item", "a\\..\\b.txt"), PathBuf::from("/out/item/a_.._b.txt"));
    }
}
