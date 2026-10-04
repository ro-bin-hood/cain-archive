use crate::ia::{self, FileEntry, Item};
use crate::queue::Queue;
use crate::store;
use crate::types::{Job, NewFile, Settings};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

pub struct AppState {
    pub queue: Queue,
    pub data_dir: PathBuf,
}

#[derive(Serialize)]
pub struct Analyzed {
    pub input: String,
    pub item: Option<Item>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct FullState {
    pub settings: Settings,
    pub user: Option<String>,
    pub jobs: Vec<Job>,
    pub running: bool,
}

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> FullState {
    let q = &state.queue;
    FullState {
        settings: q.settings.lock().unwrap().clone(),
        user: q.auth.lock().unwrap().as_ref().map(|a| a.user.clone()),
        jobs: q.jobs(),
        running: q.running(),
    }
}

#[tauri::command]
pub async fn analyze_links(state: State<'_, AppState>, text: String) -> Result<Vec<Analyzed>, String> {
    let q = &state.queue;
    let auth = q.auth.lock().unwrap().clone();
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Some(link) = ia::parse_link(line) else {
            out.push(Analyzed { input: line.into(), item: None, error: Some("Link non riconosciuto".into()) });
            continue;
        };
        match ia::fetch_item(q.client(), q.base_url(), auth.as_ref(), &link.item_id).await {
            Ok(mut item) => {
                if let Some(f) = &link.file {
                    let found = item.files.iter().find(|x| &x.name == f).cloned();
                    item.files = vec![found.unwrap_or(FileEntry { name: f.clone(), size: 0, format: String::new(), original: true })];
                }
                out.push(Analyzed { input: line.into(), item: Some(item), error: None });
            }
            Err(e) => out.push(Analyzed { input: line.into(), item: None, error: Some(e) }),
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn enqueue(state: State<'_, AppState>, files: Vec<NewFile>) { state.queue.enqueue(files) }
#[tauri::command]
pub fn start(state: State<'_, AppState>) { state.queue.start() }
#[tauri::command]
pub fn stop(state: State<'_, AppState>) { state.queue.stop() }
#[tauri::command]
pub fn pause_job(state: State<'_, AppState>, id: u64) { state.queue.pause_job(id) }
#[tauri::command]
pub fn resume_job(state: State<'_, AppState>, id: u64) { state.queue.resume_job(id) }
#[tauri::command]
pub fn retry_job(state: State<'_, AppState>, id: u64) { state.queue.resume_job(id) }
#[tauri::command]
pub fn remove_job(state: State<'_, AppState>, id: u64) { state.queue.remove_job(id) }
#[tauri::command]
pub fn clear_completed(state: State<'_, AppState>) { state.queue.clear_completed() }

#[tauri::command]
pub fn set_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    let mut s = settings;
    s.workers = s.workers.clamp(1, 8);
    store::save_settings(&state.data_dir, &s).map_err(|e| format!("Impossibile salvare le impostazioni: {e}"))?;
    *state.queue.settings.lock().unwrap() = s;
    state.queue.pump();
    Ok(())
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    Ok(app.dialog().file().blocking_pick_folder().map(|p| p.to_string()))
}

#[tauri::command]
pub fn open_folder(app: AppHandle, state: State<'_, AppState>, id: u64) -> Result<(), String> {
    let job = state.queue.jobs().into_iter().find(|j| j.id == id).ok_or("File non più in coda")?;
    if job.dest.exists() {
        app.opener().reveal_item_in_dir(&job.dest).map_err(|e| e.to_string())
    } else {
        let dir = job.dest.parent().ok_or("Cartella non valida")?;
        app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn login(state: State<'_, AppState>, email: String, password: String) -> Result<String, String> {
    let q = &state.queue;
    let a = ia::login(q.client(), q.base_url(), email.trim(), &password).await?;
    store::save_auth(&state.data_dir, &a)?;
    let user = a.user.clone();
    *q.auth.lock().unwrap() = Some(a);
    Ok(user)
}

#[tauri::command]
pub fn logout(state: State<'_, AppState>) {
    store::clear_auth(&state.data_dir);
    *state.queue.auth.lock().unwrap() = None;
}
