mod common;
use cain_archive_lib::{download::{self, Finish, Request}, ia};
use std::{path::Path, sync::Mutex, time::Duration};
use tokio_util::sync::CancellationToken;

async fn run(base: &str, item: &str, name: &str, dir: &Path, expected: Option<u64>, cancel: CancellationToken) -> (Finish, Vec<(u32, u64)>) {
    let client = ia::client();
    let url = format!("{base}/download/{item}/{name}");
    let headers = ia::auth_headers(None);
    let dest = dir.join(name);
    let req = Request { client: &client, url: &url, headers: &headers, authed: false, dest: &dest, expected, retry_base_s: 0 };
    let retries = Mutex::new(vec![]);
    let f = download::download(&req, &cancel, &|_, _| {}, &|a, w| retries.lock().unwrap().push((a, w))).await;
    (f, retries.into_inner().unwrap())
}

fn len(b: &[u8]) -> Option<u64> { Some(b.len() as u64) }

#[tokio::test]
async fn downloads_full_file() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let body = common::content("ok", "f.bin");
    let (f, _) = run(&base, "ok", "f.bin", d.path(), len(&body), CancellationToken::new()).await;
    assert_eq!(f, Finish::Done(body.len() as u64));
    assert_eq!(std::fs::read(d.path().join("f.bin")).unwrap(), body);
    assert!(!d.path().join("f.bin.part").exists());
}

#[tokio::test]
async fn resumes_from_part_file() {
    let (base, srv) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let body = common::content("ok", "f.bin");
    std::fs::write(d.path().join("f.bin.part"), &body[..100]).unwrap();
    let (f, _) = run(&base, "ok", "f.bin", d.path(), len(&body), CancellationToken::new()).await;
    assert_eq!(f, Finish::Done(body.len() as u64));
    assert_eq!(std::fs::read(d.path().join("f.bin")).unwrap(), body);
    assert_eq!(*srv.ranges.lock().unwrap(), vec!["bytes=100-".to_string()]);
}

#[tokio::test]
async fn restarts_when_server_ignores_range() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let body = common::content("norange", "f.bin");
    std::fs::write(d.path().join("f.bin.part"), vec![b'x'; 100]).unwrap();
    let (f, _) = run(&base, "norange", "f.bin", d.path(), len(&body), CancellationToken::new()).await;
    assert_eq!(f, Finish::Done(body.len() as u64));
    assert_eq!(std::fs::read(d.path().join("f.bin")).unwrap(), body);
}

#[tokio::test]
async fn complete_part_is_finalized_on_416() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let body = common::content("ok", "f.bin");
    std::fs::write(d.path().join("f.bin.part"), &body).unwrap();
    let (f, _) = run(&base, "ok", "f.bin", d.path(), None, CancellationToken::new()).await;
    assert_eq!(f, Finish::Done(body.len() as u64));
    assert_eq!(std::fs::read(d.path().join("f.bin")).unwrap(), body);
}

#[tokio::test]
async fn existing_complete_file_is_skipped() {
    let (base, srv) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let body = common::content("ok", "f.bin");
    std::fs::write(d.path().join("f.bin"), &body).unwrap();
    let (f, _) = run(&base, "ok", "f.bin", d.path(), len(&body), CancellationToken::new()).await;
    assert_eq!(f, Finish::Done(body.len() as u64));
    assert!(srv.hits.lock().unwrap().is_empty());
}

#[tokio::test]
async fn retries_on_429_respecting_retry_after() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let body = common::content("busy", "f.bin");
    let (f, retries) = run(&base, "busy", "f.bin", d.path(), len(&body), CancellationToken::new()).await;
    assert_eq!(f, Finish::Done(body.len() as u64));
    assert_eq!(retries, vec![(1, 0)]);
}

#[tokio::test]
async fn resumes_after_connection_drop() {
    let (base, srv) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let body = common::content("drop", "f.bin");
    let (f, retries) = run(&base, "drop", "f.bin", d.path(), len(&body), CancellationToken::new()).await;
    assert_eq!(f, Finish::Done(body.len() as u64));
    assert_eq!(std::fs::read(d.path().join("f.bin")).unwrap(), body);
    assert_eq!(retries.len(), 1);
    assert_eq!(srv.ranges.lock().unwrap().len(), 1, "il secondo tentativo deve riprendere con Range");
}

/// A large file that drops more than 3 times but makes progress each time must not fail.
#[tokio::test]
async fn attempts_reset_when_a_drop_still_made_progress() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let body = common::content("dropmany", "f.bin");
    let (f, retries) = run(&base, "dropmany", "f.bin", d.path(), len(&body), CancellationToken::new()).await;
    assert_eq!(f, Finish::Done(body.len() as u64));
    assert_eq!(std::fs::read(d.path().join("f.bin")).unwrap(), body);
    assert_eq!(retries.len(), 4);
    assert!(retries.iter().all(|(a, _)| *a == 1), "ogni caduta con progressi riparte dal tentativo 1: {retries:?}");
}

#[tokio::test]
async fn forbidden_and_not_found_fail_immediately() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let (f, retries) = run(&base, "denied", "f.bin", d.path(), None, CancellationToken::new()).await;
    assert_eq!(f, Finish::Failed("Accesso negato: serve accedere".into()));
    assert!(retries.is_empty());
    let (f, _) = run(&base, "notfound", "f.bin", d.path(), None, CancellationToken::new()).await;
    assert_eq!(f, Finish::Failed("File non trovato".into()));
}

#[tokio::test]
async fn too_long_file_fails_after_retries() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let body = common::content("ok", "f.bin");
    let (f, retries) = run(&base, "ok", "f.bin", d.path(), Some(body.len() as u64 - 10), CancellationToken::new()).await;
    assert!(matches!(&f, Finish::Failed(m) if m.starts_with("Dimensione errata")), "{f:?}");
    assert_eq!(retries.len(), 2);
    assert!(!d.path().join("f.bin").exists() && !d.path().join("f.bin.part").exists());
}

#[tokio::test]
async fn cancel_keeps_partial_file() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let body = common::content("slow", "f.bin");
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move { tokio::time::sleep(Duration::from_millis(500)).await; c2.cancel(); });
    let (f, _) = run(&base, "slow", "f.bin", d.path(), len(&body), cancel).await;
    assert_eq!(f, Finish::Cancelled);
    let part = std::fs::metadata(d.path().join("f.bin.part")).unwrap().len();
    assert!(part > 0 && part < body.len() as u64, "part = {part}");
}
