pub mod download;
pub mod i18n;
pub mod ia;
pub mod import;
pub mod library;
pub mod queue;
pub mod search;
pub mod store;
pub mod types;
mod commands;

use commands::AppState;
use queue::{Queue, Sink, Snapshot};
use serde::Serialize;
use std::{path::PathBuf, sync::{Arc, Mutex}};
use tauri::{Emitter, Manager};
use types::Job;

struct TauriSink {
    app: tauri::AppHandle,
    dir: PathBuf,
    save_lock: Mutex<()>,
}

#[derive(Serialize, Clone)]
struct Changed<'a> {
    jobs: &'a [Job],
    running: bool,
}

impl Sink for TauriSink {
    fn changed(&self, jobs: &[Job], running: bool) {
        {
            let _g = self.save_lock.lock().unwrap();
            let _ = store::save_queue(&self.dir, jobs);
        }
        let _ = self.app.emit("queue-changed", Changed { jobs, running });
    }
    fn progress(&self, snap: &Snapshot) {
        let _ = self.app.emit("queue-progress", snap);
    }
    fn alert(&self, msg: &str) {
        let _ = self.app.emit("queue-alert", msg);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let sink = TauriSink { app: app.handle().clone(), dir: dir.clone(), save_lock: Mutex::new(()) };
            let queue = Queue::new(store::load_queue(&dir), store::load_settings(&dir), store::load_auth(&dir), Arc::new(sink), ia::BASE_URL);
            tauri::async_runtime::spawn(queue.clone().ticker());
            let library = std::sync::Mutex::new(library::Library::load(&dir));
            app.manage(AppState { queue, library, data_dir: dir });
            Ok(())
        })
        .on_window_event(|w, e| {
            if let tauri::WindowEvent::CloseRequested { .. } = e {
                w.state::<AppState>().queue.shutdown();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::enqueue,
            commands::start,
            commands::stop,
            commands::pause_job,
            commands::resume_job,
            commands::retry_job,
            commands::remove_job,
            commands::clear_completed,
            commands::set_settings,
            commands::pick_folder,
            commands::open_folder,
            commands::login,
            commands::logout,
            commands::library_state,
            commands::take_library_warning,
            commands::create_collection,
            commands::rename_collection,
            commands::delete_collection,
            commands::add_source,
            commands::remove_source,
            commands::set_included,
            commands::refresh_source,
            commands::open_unsaved,
            commands::close_unsaved,
            commands::save_unsaved,
            commands::search,
            commands::pick_import_file,
            commands::read_import_file,
            commands::set_ui_language,
            commands::remove_sources,
            commands::copy_sources,
            commands::move_sources,
            commands::create_subcollection,
            commands::move_job,
            commands::move_collection,
            commands::export_collection,
        ])
        .run(tauri::generate_context!())
        .expect("errore all'avvio di Cain Archive");
}
