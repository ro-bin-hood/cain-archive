use crate::ia::Auth;
use crate::i18n::m;
use crate::types::{Job, JobStatus, Settings};
use std::{fs, io, path::Path};

pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data)?;
    fs::rename(tmp, path)
}

fn read_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> T {
    fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn load_settings(dir: &Path) -> Settings {
    let mut s: Settings = read_json(&dir.join("settings.json"));
    s.workers = s.workers.clamp(1, 8);
    if !matches!(s.theme.as_str(), "system" | "light" | "dark") {
        s.theme = "system".into();
    }
    if !matches!(s.language.as_str(), "system" | "it" | "en") {
        s.language = "system".into();
    }
    s
}

pub fn save_settings(dir: &Path, s: &Settings) -> io::Result<()> {
    write_atomic(&dir.join("settings.json"), &serde_json::to_vec_pretty(s)?)
}

/// Jobs that were running at shutdown come back Paused: they will resume from the .part.
pub fn load_queue(dir: &Path) -> Vec<Job> {
    let mut jobs: Vec<Job> = read_json(&dir.join("queue.json"));
    for j in &mut jobs {
        if matches!(j.status, JobStatus::Downloading | JobStatus::Retrying { .. }) {
            j.status = JobStatus::Paused;
        }
    }
    jobs
}

pub fn save_queue(dir: &Path, jobs: &[Job]) -> io::Result<()> {
    write_atomic(&dir.join("queue.json"), &serde_json::to_vec_pretty(jobs)?)
}

/// The session (never the password) lives in `session.bin`, encrypted with DPAPI: only the current
/// Windows user can decrypt it. Credential Manager doesn't fit here: on some systems
/// it already rejects ~300-character entries, while an archive.org session is ~700.
#[cfg(windows)]
mod secret {
    use crate::i18n::m;
    use std::path::Path;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};

    fn dpapi(data: &[u8], protect: bool) -> Option<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        // SAFETY: input points to `data`, valid for the whole call; out is allocated by
        // Windows and freed with LocalFree after copying.
        unsafe {
            let ok = if protect {
                CryptProtectData(&input, std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null(), 0, &mut out)
            } else {
                CryptUnprotectData(&input, std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), 0, &mut out)
            };
            if ok == 0 {
                return None;
            }
            let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
            LocalFree(out.pbData as _);
            Some(v)
        }
    }

    pub fn load(dir: &Path) -> Option<String> {
        String::from_utf8(dpapi(&std::fs::read(dir.join("session.bin")).ok()?, false)?).ok()
    }

    pub fn save(dir: &Path, json: &str) -> Result<(), String> {
        let enc = dpapi(json.as_bytes(), true).ok_or(m("cifratura non riuscita", "encryption failed"))?;
        super::write_atomic(&dir.join("session.bin"), &enc).map_err(|e| e.to_string())
    }

    pub fn clear(dir: &Path) {
        let _ = std::fs::remove_file(dir.join("session.bin"));
    }
}

/// Outside Windows: the system keyring (Keychain, keyutils).
#[cfg(not(windows))]
mod secret {
    use crate::i18n::m;
    use std::path::Path;
    const SERVICE: &str = "com.cainarchive.app";

    fn entry() -> Option<keyring::Entry> {
        keyring::Entry::new(SERVICE, "session").ok()
    }

    pub fn load(_dir: &Path) -> Option<String> {
        entry()?.get_password().ok()
    }

    pub fn save(_dir: &Path, json: &str) -> Result<(), String> {
        entry().ok_or(m("portachiavi non disponibile", "keyring unavailable"))?.set_password(json).map_err(|e| e.to_string())
    }

    pub fn clear(_dir: &Path) {
        if let Some(e) = entry() {
            let _ = e.delete_credential();
        }
    }
}

pub fn load_auth(dir: &Path) -> Option<Auth> {
    serde_json::from_str(&secret::load(dir)?).ok()
}

pub fn save_auth(dir: &Path, a: &Auth) -> Result<(), String> {
    let json = serde_json::to_string(a).map_err(|e| e.to_string())?;
    secret::save(dir, &json).map_err(|e| format!("{}: {e}", m("Impossibile salvare la sessione", "Could not save the session")))
}

pub fn clear_auth(dir: &Path) {
    secret::clear(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Job, JobStatus, Settings};
    use std::path::PathBuf;

    fn job(id: u64, status: JobStatus) -> Job {
        Job { id, item_id: "i".into(), name: format!("f{id}"), size: 10, dest: PathBuf::from(format!("C:/x/f{id}")), status }
    }

    #[test]
    fn missing_or_corrupt_settings_give_defaults() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(load_settings(d.path()), Settings::default());
        std::fs::write(d.path().join("settings.json"), "{not json").unwrap();
        assert_eq!(load_settings(d.path()), Settings::default());
    }

    #[test]
    fn settings_roundtrip_and_clamp_workers() {
        let d = tempfile::tempdir().unwrap();
        let s = Settings { out_dir: "D:/dl".into(), workers: 5, default_originals: false, default_exts: "pdf,mp3".into(), theme: "dark".into(), language: "it".into() };
        save_settings(d.path(), &s).unwrap();
        assert_eq!(load_settings(d.path()), s);
        save_settings(d.path(), &Settings { workers: 40, ..s.clone() }).unwrap();
        assert_eq!(load_settings(d.path()).workers, 8);
        std::fs::write(d.path().join("settings.json"), r#"{"workers": 0}"#).unwrap();
        let partial = load_settings(d.path());
        assert_eq!((partial.workers, partial.default_originals, partial.theme.as_str()), (1, true, "system"));
        std::fs::write(d.path().join("settings.json"), r#"{"theme": "fucsia"}"#).unwrap();
        assert_eq!(load_settings(d.path()).theme, "system");
        std::fs::write(d.path().join("settings.json"), r#"{"language": "en"}"#).unwrap();
        assert_eq!(load_settings(d.path()).language, "en");
        std::fs::write(d.path().join("settings.json"), r#"{"language": "klingon"}"#).unwrap();
        assert_eq!(load_settings(d.path()).language, "system");
    }

    #[test]
    fn load_queue_turns_active_into_paused() {
        let d = tempfile::tempdir().unwrap();
        let jobs = vec![
            job(1, JobStatus::Downloading),
            job(2, JobStatus::Retrying { attempt: 1, wait_s: 3 }),
            job(3, JobStatus::Queued),
            job(4, JobStatus::Done),
            job(5, JobStatus::Failed { reason: "x".into() }),
        ];
        save_queue(d.path(), &jobs).unwrap();
        let st: Vec<_> = load_queue(d.path()).into_iter().map(|j| j.status).collect();
        assert_eq!(st, vec![JobStatus::Paused, JobStatus::Paused, JobStatus::Queued, JobStatus::Done, JobStatus::Failed { reason: "x".into() }]);
    }

    /// A real archive.org session is ~700 characters: it must be saved, read back identical,
    /// never appear in plain text on disk, and disappear on logout.
    #[test]
    fn long_session_roundtrips_encrypted() {
        let d = tempfile::tempdir().unwrap();
        let a = Auth { user: "tester".into(), cookie_user: "me%40x.it".into(), cookie_sig: "SIGSECRET".repeat(54), access: "A".repeat(16), secret: "S".repeat(16) };
        save_auth(d.path(), &a).unwrap();
        assert_eq!(load_auth(d.path()), Some(a));
        let on_disk: Vec<u8> = std::fs::read_dir(d.path()).unwrap().flat_map(|e| std::fs::read(e.unwrap().path()).unwrap()).collect();
        assert!(!String::from_utf8_lossy(&on_disk).contains("SIGSECRET"), "sessione salvata in chiaro");
        clear_auth(d.path());
        assert_eq!(load_auth(d.path()), None);
    }

    #[test]
    fn missing_queue_is_empty() {
        let d = tempfile::tempdir().unwrap();
        assert!(load_queue(d.path()).is_empty());
    }
}
