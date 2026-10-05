use crate::download::{self, dest_for, part_path, Finish, Request};
use crate::error::AppError;
use crate::ia::{self, Auth};
use crate::types::{Job, JobStatus, NewFile, Settings};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct JobProgress {
    pub id: u64,
    pub done: u64,
    pub total: Option<u64>,
    pub speed: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Snapshot {
    pub jobs: Vec<JobProgress>,
    pub done: u64,
    pub total: u64,
    pub speed: f64,
    pub eta_s: Option<u64>,
}

/// Done and total bytes of the current work: completed, active and queued. Paused or
/// failed jobs don't count, otherwise ETA and percentage are skewed.
fn totals(jobs: &[Job], live_done: u64) -> (u64, u64) {
    let counted = |j: &&Job| !matches!(j.status, JobStatus::Paused | JobStatus::Failed { .. });
    let total = jobs.iter().filter(counted).map(|j| j.size).sum();
    let done = jobs.iter().filter(|j| j.status == JobStatus::Done).map(|j| j.size).sum::<u64>() + live_done;
    (done, total)
}

pub trait Sink: Send + Sync + 'static {
    fn changed(&self, jobs: &[Job], running: bool);
    fn progress(&self, snap: &Snapshot);
    fn alert(&self, err: &AppError);
}

struct Live {
    done: u64,
    total: Option<u64>,
    speed: f64,
    mark_done: u64,
    mark_t: Instant,
}

struct Inner {
    jobs: Vec<Job>,
    next_id: u64,
    running: bool,
    active: HashMap<u64, CancellationToken>,
    live: HashMap<u64, Live>,
    /// Jobs stopped (token cancelled) but restarted before their task noticed:
    /// when the task ends they go back to the queue instead of being paused.
    restart: HashSet<u64>,
    dirty: bool,
    changed: bool,
}

/// Different names can map to the same file (NTFS ignores case, and `a?`/`a*` both become
/// `a_`): in that case the new file becomes `name (2).ext`, `name (3).ext`, …
fn unique_dest(dest: PathBuf, jobs: &[Job]) -> PathBuf {
    let key = |p: &Path| p.to_string_lossy().to_lowercase();
    let taken = |p: &Path| jobs.iter().any(|j| key(&j.dest) == key(p));
    if !taken(&dest) {
        return dest;
    }
    let stem = dest.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = dest.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..)
        .map(|n| dest.with_file_name(format!("{stem} ({n}){ext}")))
        .find(|p| !taken(p))
        .expect("unbounded range")
}

#[derive(Clone)]
pub struct Queue {
    inner: Arc<Mutex<Inner>>,
    sink: Arc<dyn Sink>,
    emit_lock: Arc<Mutex<()>>,
    client: reqwest::Client,
    base_url: Arc<String>,
    pub settings: Arc<Mutex<Settings>>,
    pub auth: Arc<Mutex<Option<Auth>>>,
    pub retry_base_s: u64,
}

impl Queue {
    pub fn new(jobs: Vec<Job>, settings: Settings, auth: Option<Auth>, sink: Arc<dyn Sink>, base_url: &str) -> Self {
        let next_id = jobs.iter().map(|j| j.id).max().unwrap_or(0) + 1;
        let inner = Inner { jobs, next_id, running: false, active: HashMap::new(), live: HashMap::new(), restart: HashSet::new(), dirty: false, changed: false };
        Self {
            inner: Arc::new(Mutex::new(inner)),
            sink,
            emit_lock: Arc::new(Mutex::new(())),
            client: ia::client(),
            base_url: Arc::new(base_url.to_string()),
            settings: Arc::new(Mutex::new(settings)),
            auth: Arc::new(Mutex::new(auth)),
            retry_base_s: 3,
        }
    }

    pub fn client(&self) -> &reqwest::Client { &self.client }
    pub fn base_url(&self) -> &str { &self.base_url }
    pub fn jobs(&self) -> Vec<Job> { self.inner.lock().unwrap().jobs.clone() }
    pub fn running(&self) -> bool { self.inner.lock().unwrap().running }

    /// Marks the queue as changed: the ticker saves and emits the state at most every 250 ms,
    /// so thousands of files don't rewrite queue.json and the list on every single change.
    fn notify(&self) {
        self.inner.lock().unwrap().changed = true;
    }

    /// Emits the current state if it changed (always, with `force`). The copy is taken inside
    /// `emit_lock`, so states go out in order: never an old one after a newer one.
    fn flush(&self, force: bool) {
        let _g = self.emit_lock.lock().unwrap();
        let (jobs, running) = {
            let mut i = self.inner.lock().unwrap();
            if !i.changed && !force {
                return;
            }
            i.changed = false;
            (i.jobs.clone(), i.running)
        };
        self.sink.changed(&jobs, running);
    }

    pub fn enqueue(&self, files: Vec<NewFile>) {
        let out_dir = self.settings.lock().unwrap().out_dir.clone();
        {
            let mut i = self.inner.lock().unwrap();
            for f in files {
                // Only real archive.org identifiers: the id becomes a folder name.
                if !ia::is_ident(&f.item_id) {
                    continue;
                }
                // Already in the queue: added again, a failed or paused file goes back in line.
                if let Some(j) = i.jobs.iter_mut().find(|j| j.item_id == f.item_id && j.name == f.name) {
                    if matches!(j.status, JobStatus::Failed { .. } | JobStatus::Paused) {
                        j.status = JobStatus::Queued;
                    }
                    continue;
                }
                let id = i.next_id;
                i.next_id += 1;
                let dest = unique_dest(dest_for(&out_dir, &f.item_id, &f.name), &i.jobs);
                i.jobs.push(Job { id, item_id: f.item_id, name: f.name, size: f.size, dest, status: JobStatus::Queued });
            }
        }
        self.pump();
    }

    pub fn start(&self) {
        {
            let mut g = self.inner.lock().unwrap();
            let i = &mut *g;
            i.running = true;
            for j in &mut i.jobs {
                if j.status == JobStatus::Paused {
                    j.status = JobStatus::Queued;
                }
            }
            for (id, t) in &i.active {
                if t.is_cancelled() {
                    i.restart.insert(*id);
                }
            }
        }
        self.pump();
    }

    /// Stops everything: active downloads become Paused (when their task ends), queued ones right away.
    pub fn stop(&self) {
        {
            let mut i = self.inner.lock().unwrap();
            i.running = false;
            i.restart.clear();
            for t in i.active.values() {
                t.cancel();
            }
            for j in &mut i.jobs {
                if j.status == JobStatus::Queued {
                    j.status = JobStatus::Paused;
                }
            }
        }
        self.notify();
    }

    /// On app close: like stop, but also marks active jobs Paused right away and saves.
    pub fn shutdown(&self) {
        {
            let mut g = self.inner.lock().unwrap();
            let i = &mut *g;
            i.running = false;
            for t in i.active.values() {
                t.cancel();
            }
            for j in &mut i.jobs {
                if i.active.contains_key(&j.id) || j.status == JobStatus::Queued {
                    j.status = JobStatus::Paused;
                }
            }
        }
        self.flush(true);
    }

    pub fn pause_job(&self, id: u64) {
        {
            let mut i = self.inner.lock().unwrap();
            i.restart.remove(&id);
            if let Some(t) = i.active.get(&id) {
                t.cancel();
            } else if let Some(j) = i.jobs.iter_mut().find(|j| j.id == id && j.status == JobStatus::Queued) {
                j.status = JobStatus::Paused;
            }
        }
        self.notify();
    }

    /// Resumes a paused job or retries a failed one.
    pub fn resume_job(&self, id: u64) {
        {
            let mut i = self.inner.lock().unwrap();
            if i.active.get(&id).is_some_and(|t| t.is_cancelled()) {
                i.restart.insert(id);
                i.running = true;
            } else if let Some(j) = i.jobs.iter_mut().find(|j| j.id == id && matches!(j.status, JobStatus::Paused | JobStatus::Failed { .. })) {
                j.status = JobStatus::Queued;
                i.running = true;
            }
        }
        self.pump();
    }

    pub fn remove_job(&self, id: u64) {
        let removed = {
            let mut i = self.inner.lock().unwrap();
            let was_active = match i.active.get(&id) {
                Some(t) => { t.cancel(); true }
                None => false,
            };
            let pos = i.jobs.iter().position(|j| j.id == id);
            pos.map(|p| (i.jobs.remove(p), was_active))
        };
        // If it was active, its task deletes the .part when it notices the removal.
        if let Some((job, false)) = removed {
            let _ = std::fs::remove_file(part_path(&job.dest));
        }
        self.notify();
    }

    /// Moves a job before `before` (or to the end if None). The order decides which starts first.
    pub fn move_job(&self, id: u64, before: Option<u64>) {
        {
            let mut i = self.inner.lock().unwrap();
            if before == Some(id) || before.is_some_and(|b| !i.jobs.iter().any(|j| j.id == b)) {
                return;
            }
            let Some(from) = i.jobs.iter().position(|j| j.id == id) else { return };
            let job = i.jobs.remove(from);
            let to = before.and_then(|b| i.jobs.iter().position(|j| j.id == b)).unwrap_or(i.jobs.len());
            i.jobs.insert(to, job);
        }
        self.notify();
    }

    pub fn clear_completed(&self) {
        self.inner.lock().unwrap().jobs.retain(|j| j.status != JobStatus::Done);
        self.notify();
    }

    /// Starts queued jobs while workers are free; turns `running` off when nothing is left to do.
    pub fn pump(&self) {
        let to_start = {
            let workers = self.settings.lock().unwrap().workers.clamp(1, 8) as usize;
            let mut g = self.inner.lock().unwrap();
            let i = &mut *g;
            let mut v = Vec::new();
            if i.running {
                while i.active.len() < workers {
                    let Some(j) = i.jobs.iter_mut().find(|j| j.status == JobStatus::Queued) else { break };
                    j.status = JobStatus::Downloading;
                    let token = CancellationToken::new();
                    i.active.insert(j.id, token.clone());
                    v.push((j.id, token));
                }
                if i.active.is_empty() {
                    i.running = false;
                }
            }
            v
        };
        for (id, token) in to_start {
            let q = self.clone();
            tauri::async_runtime::spawn(async move { q.run_job(id, token).await });
        }
        self.notify();
    }

    async fn run_job(&self, id: u64, cancel: CancellationToken) {
        let found = self.inner.lock().unwrap().jobs.iter().find(|j| j.id == id).cloned();
        let Some(job) = found else {
            // Removed between pump and here: free its slot, or the queue would never stop "running".
            self.inner.lock().unwrap().active.remove(&id);
            self.pump();
            return;
        };
        let auth = self.auth.lock().unwrap().clone();
        let path: Vec<String> = job.name.split('/').map(|s| urlencoding::encode(s).into_owned()).collect();
        let url = format!("{}/download/{}/{}", self.base_url, job.item_id, path.join("/"));
        let headers = ia::auth_headers(auth.as_ref());
        let req = Request {
            client: &self.client,
            url: &url,
            headers: &headers,
            authed: auth.is_some(),
            dest: &job.dest,
            expected: (job.size > 0).then_some(job.size),
            retry_base_s: self.retry_base_s,
        };
        let on_progress = |done: u64, total: Option<u64>| {
            let resumed = {
                let mut g = self.inner.lock().unwrap();
                let i = &mut *g;
                let live = i.live.entry(id).or_insert_with(|| Live { done, total, speed: 0.0, mark_done: done, mark_t: Instant::now() });
                live.done = done;
                live.total = total;
                i.dirty = true;
                match i.jobs.iter_mut().find(|j| j.id == id) {
                    Some(j) if matches!(j.status, JobStatus::Retrying { .. }) => {
                        j.status = JobStatus::Downloading;
                        true
                    }
                    _ => false,
                }
            };
            if resumed {
                self.notify();
            }
        };
        let on_retry = |attempt: u32, wait_s: u64| {
            if let Some(j) = self.inner.lock().unwrap().jobs.iter_mut().find(|j| j.id == id) {
                j.status = JobStatus::Retrying { attempt, wait_s };
            }
            self.notify();
        };
        let finish = download::download(&req, &cancel, &on_progress, &on_retry).await;

        let mut alert = None;
        {
            let mut g = self.inner.lock().unwrap();
            let i = &mut *g;
            i.active.remove(&id);
            i.live.remove(&id);
            i.dirty = true;
            match (finish, i.jobs.iter().position(|j| j.id == id)) {
                (Finish::Cancelled, None) => {
                    let _ = std::fs::remove_file(part_path(&job.dest));
                }
                (_, None) => {}
                (Finish::Done(n), Some(p)) => {
                    let j = &mut i.jobs[p];
                    j.status = JobStatus::Done;
                    if j.size == 0 {
                        j.size = n;
                    }
                }
                (Finish::Cancelled, Some(p)) => {
                    i.jobs[p].status = if i.restart.remove(&id) { JobStatus::Queued } else { JobStatus::Paused };
                }
                (Finish::Failed(reason), Some(p)) => i.jobs[p].status = JobStatus::Failed { reason },
                (Finish::Disk(detail), Some(p)) => {
                    i.jobs[p].status = JobStatus::Paused;
                    i.running = false;
                    for t in i.active.values() {
                        t.cancel();
                    }
                    for j in &mut i.jobs {
                        if j.status == JobStatus::Queued {
                            j.status = JobStatus::Paused;
                        }
                    }
                    alert = Some(AppError::DiskProblem { detail });
                }
            }
        }
        if let Some(err) = alert {
            self.sink.alert(&err);
        }
        self.pump();
    }

    /// Emits progress every 250 ms, only when something changed.
    pub async fn ticker(self) {
        let mut iv = tokio::time::interval(Duration::from_millis(250));
        loop {
            iv.tick().await;
            self.flush(false);
            if let Some(s) = self.snapshot_if_changed() {
                self.sink.progress(&s);
            }
        }
    }

    fn snapshot_if_changed(&self) -> Option<Snapshot> {
        let mut g = self.inner.lock().unwrap();
        let i = &mut *g;
        if !i.dirty && i.live.is_empty() {
            return None;
        }
        i.dirty = false;
        let now = Instant::now();
        let mut jobs = Vec::new();
        for (&id, l) in i.live.iter_mut() {
            let dt = now.duration_since(l.mark_t).as_secs_f64();
            if dt >= 0.2 {
                let inst = l.done.saturating_sub(l.mark_done) as f64 / dt;
                l.speed = if l.speed == 0.0 { inst } else { 0.7 * l.speed + 0.3 * inst };
                l.mark_done = l.done;
                l.mark_t = now;
            }
            jobs.push(JobProgress { id, done: l.done, total: l.total, speed: l.speed });
        }
        let (done, total) = totals(&i.jobs, jobs.iter().map(|p| p.done).sum());
        let speed: f64 = jobs.iter().map(|p| p.speed).sum();
        let eta_s = (i.running && speed > 0.0 && total > done).then(|| ((total - done) as f64 / speed) as u64);
        Some(Snapshot { jobs, done, total, speed, eta_s })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn job(id: u64, size: u64, status: JobStatus) -> Job {
        Job { id, item_id: "i".into(), name: format!("f{id}"), size, dest: PathBuf::from(format!("C:/x/f{id}")), status }
    }

    /// Paused or failed files are not part of the current work: they don't count in the total.
    #[test]
    fn totals_ignore_paused_and_failed_jobs() {
        let jobs = vec![
            job(1, 100, JobStatus::Done),
            job(2, 1000, JobStatus::Paused),
            job(3, 50, JobStatus::Queued),
            job(4, 10, JobStatus::Failed { reason: AppError::FileNotFound }),
            job(5, 200, JobStatus::Downloading),
        ];
        assert_eq!(totals(&jobs, 20), (120, 350));
    }
    struct NullSink;
    impl Sink for NullSink {
        fn changed(&self, _: &[Job], _: bool) {}
        fn progress(&self, _: &Snapshot) {}
        fn alert(&self, _: &AppError) {}
    }

    fn order(q: &Queue) -> Vec<u64> {
        q.jobs().iter().map(|j| j.id).collect()
    }

    /// Dragging a file in the queue puts it before another one, or at the end.
    #[test]
    fn jobs_can_be_reordered() {
        let jobs = vec![job(1, 1, JobStatus::Queued), job(2, 1, JobStatus::Queued), job(3, 1, JobStatus::Done)];
        let q = Queue::new(jobs, Settings::default(), None, std::sync::Arc::new(NullSink), "http://x");
        q.move_job(3, Some(1));
        assert_eq!(order(&q), [3, 1, 2]);
        q.move_job(3, None);
        assert_eq!(order(&q), [1, 2, 3]);
        q.move_job(1, Some(3));
        assert_eq!(order(&q), [2, 1, 3]);
        q.move_job(2, Some(2));
        q.move_job(99, Some(1));
        q.move_job(1, Some(99));
        assert_eq!(order(&q), [2, 1, 3], "meaningless moves change nothing");
    }

}
