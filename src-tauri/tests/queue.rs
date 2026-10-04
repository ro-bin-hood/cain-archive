mod common;
use cain_archive_lib::{
    ia::Auth,
    queue::{Queue, Sink, Snapshot},
    types::{Job, JobStatus, NewFile, Settings},
};
use std::{path::Path, sync::{Arc, Mutex}, time::Duration};

#[derive(Default)]
struct RecSink { alerts: Mutex<Vec<String>> }
impl Sink for RecSink {
    fn changed(&self, _: &[Job], _: bool) {}
    fn progress(&self, _: &Snapshot) {}
    fn alert(&self, m: &str) { self.alerts.lock().unwrap().push(m.into()); }
}

fn queue(base: &str, dir: &Path, auth: Option<Auth>) -> Queue {
    let s = Settings { out_dir: dir.to_path_buf(), workers: 2, ..Settings::default() };
    let mut q = Queue::new(vec![], s, auth, Arc::new(RecSink::default()), base);
    q.retry_base_s = 0;
    q
}

fn nf(item: &str, name: &str) -> NewFile {
    NewFile { item_id: item.into(), name: name.into(), size: common::content(item, name).len() as u64 }
}

async fn wait_for(q: &Queue, what: &str, pred: impl Fn(&[Job]) -> bool) {
    for _ in 0..200 {
        if pred(&q.jobs()) { return; }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("timeout aspettando {what}: {:?}", q.jobs());
}

fn all(st: JobStatus) -> impl Fn(&[Job]) -> bool {
    move |js| !js.is_empty() && js.iter().all(|j| j.status == st)
}

#[tokio::test]
async fn downloads_all_and_stops_running() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let q = queue(&base, d.path(), None);
    q.enqueue(vec![nf("ok", "a.bin"), nf("ok", "b.bin"), nf("ok", "sub/c.bin")]);
    q.start();
    wait_for(&q, "tutti Done", all(JobStatus::Done)).await;
    for n in ["a.bin", "b.bin", "sub/c.bin"] {
        assert_eq!(std::fs::read(d.path().join("ok").join(n)).unwrap(), common::content("ok", n));
    }
    wait_for(&q, "running=false", |_| !q.running()).await;
}

#[tokio::test]
async fn special_names_are_encoded() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let q = queue(&base, d.path(), None);
    let name = "dir/a b#1%?.txt";
    q.enqueue(vec![nf("ok", name)]);
    q.start();
    wait_for(&q, "Done", all(JobStatus::Done)).await;
    let saved = d.path().join("ok/dir/a b#1%_.txt");
    assert_eq!(std::fs::read(saved).unwrap(), common::content("ok", name));
}

#[tokio::test]
async fn duplicates_are_ignored() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let q = queue(&base, d.path(), None);
    q.enqueue(vec![nf("ok", "a.bin"), nf("ok", "a.bin")]);
    q.enqueue(vec![nf("ok", "a.bin")]);
    assert_eq!(q.jobs().len(), 1);
}

#[tokio::test]
async fn stop_pauses_and_start_resumes_with_range() {
    let (base, srv) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let q = queue(&base, d.path(), None);
    q.enqueue(vec![nf("slow", "f.bin")]);
    q.start();
    let part = d.path().join("slow/f.bin.part");
    wait_for(&q, ".part creato", |_| std::fs::metadata(&part).map(|m| m.len() > 0).unwrap_or(false)).await;
    q.stop();
    wait_for(&q, "Paused", all(JobStatus::Paused)).await;
    q.start();
    wait_for(&q, "Done", all(JobStatus::Done)).await;
    assert_eq!(std::fs::read(d.path().join("slow/f.bin")).unwrap(), common::content("slow", "f.bin"));
    assert_eq!(srv.ranges.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn denied_fails_and_retry_requeues() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let q = queue(&base, d.path(), None);
    q.enqueue(vec![nf("denied", "f.bin")]);
    q.start();
    wait_for(&q, "Failed", |js| matches!(&js[0].status, JobStatus::Failed { reason } if reason == "Accesso negato: serve accedere")).await;
    q.resume_job(q.jobs()[0].id);
    wait_for(&q, "Failed di nuovo", |js| matches!(js[0].status, JobStatus::Failed { .. })).await;
}

#[tokio::test]
async fn remove_deletes_partial_file() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let q = queue(&base, d.path(), None);
    q.enqueue(vec![nf("slow", "f.bin")]);
    q.start();
    let part = d.path().join("slow/f.bin.part");
    wait_for(&q, ".part creato", |_| part.exists()).await;
    q.remove_job(q.jobs()[0].id);
    assert!(q.jobs().is_empty());
    for _ in 0..40 {
        if !part.exists() { return; }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(".part non cancellato");
}

#[tokio::test]
async fn logged_in_downloads_restricted_item() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let auth = Auth { user: "u".into(), cookie_user: "u".into(), cookie_sig: "SIG".into(), access: "a".into(), secret: "s".into() };
    let q = queue(&base, d.path(), Some(auth));
    q.enqueue(vec![nf("authonly", "f.bin")]);
    q.start();
    wait_for(&q, "Done", all(JobStatus::Done)).await;
}

#[tokio::test]
async fn clear_completed_keeps_others() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let q = queue(&base, d.path(), None);
    q.enqueue(vec![nf("ok", "a.bin")]);
    q.start();
    wait_for(&q, "Done", all(JobStatus::Done)).await;
    q.enqueue(vec![nf("ok", "b.bin")]);
    q.pause_job(q.jobs()[1].id);
    q.clear_completed();
    let js = q.jobs();
    assert_eq!(js.len(), 1);
    assert_eq!(js[0].name, "b.bin");
}

#[tokio::test]
async fn colliding_destinations_get_distinct_files() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let q = queue(&base, d.path(), None);
    // Su NTFS Track01/track01 sono lo stesso file; a?/a* diventano entrambi a_.txt.
    q.enqueue(vec![nf("ok", "Track01.mp3"), nf("ok", "track01.mp3"), nf("ok", "a?.txt"), nf("ok", "a*.txt")]);
    let js = q.jobs();
    let dests: std::collections::HashSet<String> = js.iter().map(|j| j.dest.to_string_lossy().to_lowercase()).collect();
    assert_eq!(dests.len(), 4, "{:?}", js.iter().map(|j| &j.dest).collect::<Vec<_>>());
    q.start();
    wait_for(&q, "tutti Done", all(JobStatus::Done)).await;
    for j in q.jobs() {
        assert_eq!(std::fs::read(&j.dest).unwrap(), common::content("ok", &j.name), "{}", j.name);
    }
}

#[derive(Default)]
struct ChangeSink { changes: Mutex<Vec<(Vec<Job>, bool)>> }
impl Sink for ChangeSink {
    fn changed(&self, jobs: &[Job], running: bool) { self.changes.lock().unwrap().push((jobs.to_vec(), running)); }
    fn progress(&self, _: &Snapshot) {}
    fn alert(&self, _: &str) {}
}

/// Ogni cambio di stato riscrive queue.json e ridisegna la lista: con migliaia di file deve
/// essere raggruppato, e l'ultimo stato emesso deve essere quello finale (mai uno vecchio dopo).
#[tokio::test]
async fn changes_are_batched_and_last_one_is_final() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let sink = Arc::new(ChangeSink::default());
    let s = Settings { out_dir: d.path().to_path_buf(), workers: 4, ..Settings::default() };
    let mut q = Queue::new(vec![], s, None, sink.clone() as Arc<dyn Sink>, &base);
    q.retry_base_s = 0;
    tauri::async_runtime::spawn(q.clone().ticker());
    q.enqueue((0..60).map(|i| nf("ok", &format!("f{i}.bin"))).collect());
    q.start();
    wait_for(&q, "tutti Done", all(JobStatus::Done)).await;
    wait_for(&q, "running=false", |_| !q.running()).await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    let ch = sink.changes.lock().unwrap();
    assert!(ch.len() < 30, "{} notifiche per 60 file", ch.len());
    let (jobs, running) = ch.last().unwrap();
    assert!(!running && jobs.len() == 60 && jobs.iter().all(|j| j.status == JobStatus::Done));
}
