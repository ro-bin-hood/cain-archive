use crate::error::{AppError, AppResult};
use crate::ia;
use crate::import::{self, ParsedList};
use crate::library::{self, Library, LibraryState, Scope, SourceMeta};
use crate::search::{self, Filters, SearchResult};
use crate::queue::Queue;
use crate::store;
use crate::types::{Job, NewFile, Settings};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

pub struct AppState {
    pub queue: Queue,
    pub library: Mutex<Library>,
    pub data_dir: PathBuf,
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
pub fn set_settings(state: State<'_, AppState>, settings: Settings) -> AppResult<()> {
    let mut s = settings;
    s.workers = s.workers.clamp(1, 8);
    store::save_settings(&state.data_dir, &s)?;
    *state.queue.settings.lock().unwrap() = s;
    state.queue.pump();
    Ok(())
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> AppResult<Option<String>> {
    Ok(app.dialog().file().blocking_pick_folder().map(|p| p.to_string()))
}

#[tauri::command]
pub fn open_folder(app: AppHandle, state: State<'_, AppState>, id: u64) -> AppResult<()> {
    let job = state.queue.jobs().into_iter().find(|j| j.id == id).ok_or(AppError::JobNotInQueue)?;
    if job.dest.exists() {
        app.opener().reveal_item_in_dir(&job.dest).map_err(|e| AppError::OpenFolderFailed { detail: e.to_string() })
    } else {
        let dir = job.dest.parent().ok_or(AppError::InvalidFolder)?;
        app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| AppError::OpenFolderFailed { detail: e.to_string() })
    }
}

#[tauri::command]
pub async fn login(state: State<'_, AppState>, email: String, password: String) -> AppResult<String> {
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

// Library commands run off the UI thread (`async`): searching hundreds of thousands of
// files, or a lock held while saving, never freezes the window.
fn lib_state(state: &State<'_, AppState>) -> LibraryState {
    state.library.lock().unwrap().state()
}

#[tauri::command(async)]
pub fn library_state(state: State<'_, AppState>) -> LibraryState {
    lib_state(&state)
}

#[tauri::command(async)]
pub fn take_library_warning(state: State<'_, AppState>) -> Option<AppError> {
    state.library.lock().unwrap().take_warning()
}

#[tauri::command(async)]
pub fn create_collection(state: State<'_, AppState>, name: String) -> AppResult<LibraryState> {
    state.library.lock().unwrap().create_collection(&name)?;
    Ok(lib_state(&state))
}

#[tauri::command(async)]
pub fn rename_collection(state: State<'_, AppState>, id: String, name: String) -> AppResult<LibraryState> {
    state.library.lock().unwrap().rename_collection(&id, &name)?;
    Ok(lib_state(&state))
}

#[tauri::command(async)]
pub fn delete_collection(state: State<'_, AppState>, id: String) -> AppResult<LibraryState> {
    state.library.lock().unwrap().delete_collection(&id)?;
    Ok(lib_state(&state))
}

#[tauri::command]
pub async fn add_source(state: State<'_, AppState>, collection_id: String, input: String) -> AppResult<SourceMeta> {
    let auth = state.queue.auth.lock().unwrap().clone();
    library::add_source(&state.library, state.queue.client(), state.queue.base_url(), auth.as_ref(), &collection_id, &input).await
}

#[tauri::command(async)]
pub fn remove_source(state: State<'_, AppState>, collection_id: String, item_id: String) -> AppResult<LibraryState> {
    state.library.lock().unwrap().remove_source(&collection_id, &item_id)?;
    Ok(lib_state(&state))
}

#[tauri::command(async)]
pub fn set_included(state: State<'_, AppState>, collection_id: String, item_id: String, included: bool) -> AppResult<LibraryState> {
    state.library.lock().unwrap().set_included(&collection_id, &item_id, included)?;
    Ok(lib_state(&state))
}

#[tauri::command]
pub async fn refresh_source(state: State<'_, AppState>, item_id: String) -> AppResult<SourceMeta> {
    let auth = state.queue.auth.lock().unwrap().clone();
    library::refresh_source(&state.library, state.queue.client(), state.queue.base_url(), auth.as_ref(), &item_id).await
}

#[tauri::command]
pub async fn open_unsaved(state: State<'_, AppState>, input: String) -> AppResult<SourceMeta> {
    let auth = state.queue.auth.lock().unwrap().clone();
    library::open_unsaved(&state.library, state.queue.client(), state.queue.base_url(), auth.as_ref(), &input).await
}

#[tauri::command(async)]
pub fn close_unsaved(state: State<'_, AppState>, item_id: String) -> LibraryState {
    state.library.lock().unwrap().close_unsaved(&item_id);
    lib_state(&state)
}

#[tauri::command(async)]
pub fn save_unsaved(state: State<'_, AppState>, item_id: String, collection_id: String) -> AppResult<LibraryState> {
    state.library.lock().unwrap().save_unsaved(&item_id, &collection_id)?;
    Ok(lib_state(&state))
}

#[tauri::command(async)]
pub fn search(state: State<'_, AppState>, scope: Scope, query: String, filters: Filters) -> AppResult<SearchResult> {
    let mut result = state.library.lock().unwrap().search_with(&scope, &query, &filters)?;
    let out_dir = state.queue.settings.lock().unwrap().out_dir.clone();
    search::mark_local(&mut result.results, &state.queue.jobs(), &out_dir);
    Ok(result)
}

/// Picks a list file and reads it; a missing name becomes the file name.
/// `filter_name` labels the file type in the dialog, in the UI language.
#[tauri::command]
pub async fn pick_import_file(app: AppHandle, filter_name: String) -> AppResult<Option<ParsedList>> {
    let Some(file) = app.dialog().file().add_filter(filter_name, &["txt", "csv"]).blocking_pick_file() else { return Ok(None) };
    let path = file.into_path().map_err(AppError::other)?;
    import::read_list_file(&path).map(Some)
}

/// Reads a list file dropped on the window.
#[tauri::command(async)]
pub fn read_import_file(path: String) -> AppResult<ParsedList> {
    import::read_list_file(std::path::Path::new(&path))
}

/// Saves a collection as a list file; returns the chosen path (None if cancelled).
#[tauri::command]
pub async fn export_collection(app: AppHandle, state: State<'_, AppState>, id: String, filter_name: String) -> AppResult<Option<String>> {
    let (name, sources, subs) = state.library.lock().unwrap().export_tree(&id)?;
    let Some(file) = app.dialog().file().add_filter(filter_name, &["txt"]).set_file_name(format!("{}.txt", safe_file_name(&name))).blocking_save_file() else { return Ok(None) };
    let path = file.into_path().map_err(AppError::other)?;
    std::fs::write(&path, import::format_list_with(&name, &sources, &subs)).map_err(|e| AppError::SaveFileFailed { detail: e.to_string() })?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Saves several collections as list files, one each, in a folder the user picks; returns the
/// folder (None if cancelled). Existing files are kept: a clashing name gets " (2)", " (3)"...
#[tauri::command]
pub async fn export_collections(app: AppHandle, state: State<'_, AppState>, ids: Vec<String>) -> AppResult<Option<String>> {
    let lists = {
        let lib = state.library.lock().unwrap();
        ids.iter().map(|id| lib.export_tree(id)).collect::<AppResult<Vec<_>>>()?
    };
    let Some(dir) = app.dialog().file().blocking_pick_folder() else { return Ok(None) };
    let dir = dir.into_path().map_err(AppError::other)?;
    for (name, sources, subs) in lists {
        let base = safe_file_name(&name);
        let path = (1..)
            .map(|n| dir.join(if n == 1 { format!("{base}.txt") } else { format!("{base} ({n}).txt") }))
            .find(|p| !p.exists())
            .unwrap();
        std::fs::write(&path, import::format_list_with(&name, &sources, &subs)).map_err(|e| AppError::SaveFileFailed { detail: e.to_string() })?;
    }
    Ok(Some(dir.to_string_lossy().into_owned()))
}

fn safe_file_name(name: &str) -> String {
    name.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) { '_' } else { c }).collect()
}

#[tauri::command(async)]
pub fn remove_sources(state: State<'_, AppState>, collection_id: String, item_ids: Vec<String>) -> AppResult<LibraryState> {
    state.library.lock().unwrap().remove_sources(&collection_id, &item_ids)?;
    Ok(lib_state(&state))
}

#[tauri::command(async)]
pub fn copy_sources(state: State<'_, AppState>, to: String, item_ids: Vec<String>) -> AppResult<LibraryState> {
    state.library.lock().unwrap().copy_sources(&to, &item_ids)?;
    Ok(lib_state(&state))
}

#[tauri::command(async)]
pub fn move_sources(state: State<'_, AppState>, from: String, to: String, item_ids: Vec<String>) -> AppResult<LibraryState> {
    state.library.lock().unwrap().move_sources(&from, &to, &item_ids)?;
    Ok(lib_state(&state))
}

#[tauri::command(async)]
pub fn create_subcollection(state: State<'_, AppState>, parent: String, name: String) -> AppResult<LibraryState> {
    state.library.lock().unwrap().create_subcollection(&parent, &name)?;
    Ok(lib_state(&state))
}

#[tauri::command]
pub fn move_job(state: State<'_, AppState>, id: u64, before: Option<u64>) {
    state.queue.move_job(id, before)
}

#[tauri::command(async)]
pub fn move_collection(state: State<'_, AppState>, id: String, before: Option<String>) -> AppResult<LibraryState> {
    state.library.lock().unwrap().move_collection(&id, before.as_deref())?;
    Ok(lib_state(&state))
}
