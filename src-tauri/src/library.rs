use crate::ia::{self, Auth, FileEntry, Item};
use crate::i18n::m;
use crate::search::{self, Filters, SearchResult, SourceIndex};
use crate::store::write_atomic;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

fn missing() -> &'static str {
    m("Elenco mancante, aggiorna la sorgente", "File list missing, refresh the source")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceMeta {
    pub item_id: String,
    pub title: Option<String>,
    pub file_count: usize,
    pub total_size: u64,
    pub updated_at: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SourceRef {
    item_id: String,
    included: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Collection {
    id: String,
    name: String,
    sources: Vec<SourceRef>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct LibraryFile {
    collections: Vec<Collection>,
    sources: BTreeMap<String, SourceMeta>,
    next_id: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceView {
    #[serde(flatten)]
    pub meta: SourceMeta,
    pub included: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollectionView {
    pub id: String,
    pub name: String,
    pub sources: Vec<SourceView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LibraryState {
    pub collections: Vec<CollectionView>,
    pub unsaved: Vec<SourceMeta>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Scope {
    All,
    Collection { id: String },
    Source { item_id: String },
    Unsaved,
}

/// Raccolte, sorgenti salvate (su disco) e Non salvate (solo in memoria), con gli indici di ricerca.
pub struct Library {
    dir: PathBuf,
    file: LibraryFile,
    index: HashMap<String, SourceIndex>,
    unsaved: Vec<SourceMeta>,
    warning: Option<String>,
    /// Impostato quando library.json esiste ma non si è potuto leggere o mettere da parte:
    /// salvare sovrascriverebbe le raccolte vere con una libreria vuota.
    read_only: Option<String>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn meta_of(item: &Item) -> SourceMeta {
    SourceMeta {
        item_id: item.id.clone(),
        title: item.title.clone(),
        file_count: item.files.len(),
        total_size: item.files.iter().map(|f| f.size).sum(),
        updated_at: now(),
        error: None,
    }
}

impl Library {
    pub fn load(dir: &Path) -> Library {
        let path = dir.join("library.json");
        let mut warning = None;
        let mut read_only = None;
        let file = match std::fs::read(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => LibraryFile::default(),
            Err(e) => {
                warning = Some(format!("{} ({e}): {}", m("Impossibile leggere la libreria", "Could not read the library"), m("le modifiche non verranno salvate finché non riavvii l'app", "changes will not be saved until you restart the app")));
                read_only = Some(format!("{}: {e}", m("Libreria in sola lettura", "Library is read-only")));
                LibraryFile::default()
            }
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| {
                match std::fs::rename(&path, dir.join("library.bak")) {
                    Ok(()) => warning = Some(m("Libreria illeggibile: è stata messa da parte come library.bak", "Unreadable library: it was set aside as library.bak").to_string()),
                    Err(e) => {
                        warning = Some(format!("{} ({e}): {}", m("Libreria illeggibile e non spostabile", "Unreadable library that could not be moved"), m("le modifiche non verranno salvate", "changes will not be saved")));
                        read_only = Some(format!("{}: {e}", m("Libreria in sola lettura", "Library is read-only")));
                    }
                }
                LibraryFile::default()
            }),
        };
        let mut lib = Library { dir: dir.to_path_buf(), file, index: HashMap::new(), unsaved: Vec::new(), warning, read_only };
        let ids: Vec<String> = lib.file.sources.keys().cloned().collect();
        for id in ids {
            let files: Option<Vec<FileEntry>> = std::fs::read(lib.source_path(&id)).ok().and_then(|b| serde_json::from_slice(&b).ok());
            let files = files.unwrap_or_else(|| {
                let m = lib.file.sources.get_mut(&id).expect("id preso dalle chiavi");
                m.file_count = 0;
                m.total_size = 0;
                m.error = Some(missing().to_string());
                Vec::new()
            });
            lib.index.insert(id.clone(), SourceIndex::new(&id, &files));
        }
        lib
    }

    fn source_path(&self, item_id: &str) -> PathBuf {
        self.dir.join("sources").join(format!("{item_id}.json"))
    }

    fn writable(&self) -> Result<(), String> {
        self.read_only.clone().map_or(Ok(()), Err)
    }

    fn save(&self) -> Result<(), String> {
        self.writable()?;
        let json = serde_json::to_vec_pretty(&self.file).map_err(|e| e.to_string())?;
        write_atomic(&self.dir.join("library.json"), &json).map_err(|e| format!("{}: {e}", m("Impossibile salvare la libreria", "Could not save the library")))
    }

    fn save_files(&self, item_id: &str, files: &[FileEntry]) -> Result<(), String> {
        self.writable()?;
        let json = serde_json::to_vec(files).map_err(|e| e.to_string())?;
        write_atomic(&self.source_path(item_id), &json).map_err(|e| format!("{}: {e}", m("Impossibile salvare la sorgente", "Could not save the source")))
    }

    pub fn take_warning(&mut self) -> Option<String> {
        self.warning.take()
    }

    pub fn state(&self) -> LibraryState {
        let collections = self
            .file
            .collections
            .iter()
            .map(|c| CollectionView {
                id: c.id.clone(),
                name: c.name.clone(),
                sources: c
                    .sources
                    .iter()
                    .filter_map(|r| self.file.sources.get(&r.item_id).map(|m| SourceView { meta: m.clone(), included: r.included }))
                    .collect(),
            })
            .collect();
        LibraryState { collections, unsaved: self.unsaved.clone() }
    }

    fn clean_name(&self, name: &str, except: Option<&str>) -> Result<String, String> {
        let n = name.trim();
        if n.is_empty() {
            return Err(m("Nome vuoto", "Empty name").into());
        }
        let lower = n.to_lowercase();
        if self.file.collections.iter().any(|c| Some(c.id.as_str()) != except && c.name.to_lowercase() == lower) {
            return Err(m("Esiste già una raccolta con questo nome", "A collection with this name already exists").into());
        }
        Ok(n.to_string())
    }

    /// Usata da tutte le operazioni che modificano una raccolta: in sola lettura si fermano qui,
    /// prima di toccare lo stato in memoria.
    fn collection_mut(&mut self, id: &str) -> Result<&mut Collection, String> {
        self.writable()?;
        self.file.collections.iter_mut().find(|c| c.id == id).ok_or_else(|| m("Raccolta non trovata", "Collection not found").to_string())
    }

    pub fn create_collection(&mut self, name: &str) -> Result<String, String> {
        self.writable()?;
        let name = self.clean_name(name, None)?;
        self.file.next_id += 1;
        let id = format!("c{}", self.file.next_id);
        self.file.collections.push(Collection { id: id.clone(), name, sources: Vec::new() });
        self.save()?;
        Ok(id)
    }

    pub fn rename_collection(&mut self, id: &str, name: &str) -> Result<(), String> {
        self.collection_mut(id)?;
        let name = self.clean_name(name, Some(id))?;
        self.collection_mut(id)?.name = name;
        self.save()
    }

    pub fn delete_collection(&mut self, id: &str) -> Result<(), String> {
        let before = self.file.collections.len();
        self.file.collections.retain(|c| c.id != id);
        if self.file.collections.len() == before {
            return Err(m("Raccolta non trovata", "Collection not found").into());
        }
        self.drop_orphans();
        self.save()
    }

    pub fn remove_source(&mut self, collection_id: &str, item_id: &str) -> Result<(), String> {
        self.collection_mut(collection_id)?.sources.retain(|r| r.item_id != item_id);
        self.drop_orphans();
        self.save()
    }

    pub fn set_included(&mut self, collection_id: &str, item_id: &str, included: bool) -> Result<(), String> {
        let c = self.collection_mut(collection_id)?;
        let r = c.sources.iter_mut().find(|r| r.item_id == item_id).ok_or_else(|| m("Sorgente non trovata", "Source not found").to_string())?;
        r.included = included;
        self.save()
    }

    /// Toglie più sorgenti da una raccolta; quelle rimaste orfane vengono cancellate.
    pub fn remove_sources(&mut self, collection_id: &str, ids: &[String]) -> Result<(), String> {
        self.collection_mut(collection_id)?.sources.retain(|r| !ids.contains(&r.item_id));
        self.drop_orphans();
        self.save()
    }

    /// Riferimenti (con la spunta) alle sorgenti indicate: da `from` se dato, altrimenti dalla
    /// prima raccolta che le contiene.
    fn refs_for(&self, from: Option<&str>, ids: &[String]) -> Result<Vec<SourceRef>, String> {
        ids.iter()
            .map(|id| {
                if !self.file.sources.contains_key(id) {
                    return Err(m("Sorgente non trovata", "Source not found").to_string());
                }
                let found = self.file.collections.iter().filter(|c| from.is_none_or(|f| c.id == f)).flat_map(|c| c.sources.iter()).find(|r| &r.item_id == id);
                match (found, from) {
                    (Some(r), _) => Ok(r.clone()),
                    (None, Some(_)) => Err(m("Sorgente non trovata", "Source not found").to_string()),
                    (None, None) => Ok(SourceRef { item_id: id.clone(), included: true }),
                }
            })
            .collect()
    }

    fn push_refs(&mut self, to: &str, refs: Vec<SourceRef>) -> Result<(), String> {
        let c = self.collection_mut(to)?;
        for r in refs {
            if !c.sources.iter().any(|x| x.item_id == r.item_id) {
                c.sources.push(r);
            }
        }
        Ok(())
    }

    /// Copia sorgenti salvate in un'altra raccolta, conservando la spunta; niente doppioni.
    pub fn copy_sources(&mut self, to: &str, ids: &[String]) -> Result<(), String> {
        self.collection_mut(to)?;
        let refs = self.refs_for(None, ids)?;
        self.push_refs(to, refs)?;
        self.save()
    }

    /// Sposta sorgenti da una raccolta a un'altra, conservando la spunta.
    pub fn move_sources(&mut self, from: &str, to: &str, ids: &[String]) -> Result<(), String> {
        if from == to {
            return Err(m("Origine e destinazione coincidono", "Source and destination are the same").into());
        }
        self.collection_mut(from)?;
        self.collection_mut(to)?;
        let refs = self.refs_for(Some(from), ids)?;
        self.push_refs(to, refs)?;
        self.collection_mut(from)?.sources.retain(|r| !ids.contains(&r.item_id));
        self.save()
    }

    /// Cancella le sorgenti salvate che non sono più in nessuna raccolta.
    fn drop_orphans(&mut self) {
        let used: HashSet<&str> = self.file.collections.iter().flat_map(|c| c.sources.iter().map(|r| r.item_id.as_str())).collect();
        let orphans: Vec<String> = self.file.sources.keys().filter(|k| !used.contains(k.as_str())).cloned().collect();
        for id in orphans {
            self.file.sources.remove(&id);
            self.index.remove(&id);
            let _ = std::fs::remove_file(self.source_path(&id));
        }
    }

    pub fn known(&self, item_id: &str) -> Option<SourceMeta> {
        self.file.sources.get(item_id).cloned().or_else(|| self.unsaved.iter().find(|m| m.item_id == item_id).cloned())
    }

    fn link(&mut self, collection_id: &str, item_id: &str) -> Result<(), String> {
        let c = self.collection_mut(collection_id)?;
        if !c.sources.iter().any(|r| r.item_id == item_id) {
            c.sources.push(SourceRef { item_id: item_id.to_string(), included: true });
        }
        Ok(())
    }

    /// Collega alla raccolta una sorgente già nota, salvando una Non salvata se serve.
    /// `None` = sorgente sconosciuta, da scaricare.
    pub fn link_known(&mut self, collection_id: &str, item_id: &str) -> Result<Option<SourceMeta>, String> {
        self.collection_mut(collection_id)?;
        if let Some(pos) = self.unsaved.iter().position(|m| m.item_id == item_id) {
            let files = self.index.get(item_id).map(|i| i.files.clone()).unwrap_or_default();
            self.save_files(item_id, &files)?;
            let meta = self.unsaved.remove(pos);
            self.file.sources.insert(item_id.to_string(), meta);
        }
        let Some(meta) = self.file.sources.get(item_id).cloned() else { return Ok(None) };
        self.link(collection_id, item_id)?;
        self.save()?;
        Ok(Some(meta))
    }

    pub fn add_fetched(&mut self, collection_id: &str, item: Item) -> Result<SourceMeta, String> {
        self.collection_mut(collection_id)?;
        let meta = meta_of(&item);
        self.save_files(&item.id, &item.files)?;
        self.index.insert(item.id.clone(), SourceIndex::new(&item.id, &item.files));
        self.unsaved.retain(|m| m.item_id != item.id);
        self.file.sources.insert(item.id.clone(), meta.clone());
        self.link(collection_id, &item.id)?;
        self.save()?;
        Ok(meta)
    }

    pub fn apply_refresh(&mut self, item_id: &str, fetched: Result<Item, String>) -> Result<SourceMeta, String> {
        let saved = self.file.sources.contains_key(item_id);
        match fetched {
            Ok(item) => {
                let meta = meta_of(&item);
                if saved {
                    self.save_files(item_id, &item.files)?;
                    self.file.sources.insert(item_id.to_string(), meta.clone());
                    self.save()?;
                } else if let Some(m) = self.unsaved.iter_mut().find(|m| m.item_id == item_id) {
                    *m = meta.clone();
                } else {
                    return Err(m("Sorgente non trovata", "Source not found").into());
                }
                self.index.insert(item_id.to_string(), SourceIndex::new(item_id, &item.files));
                Ok(meta)
            }
            Err(e) => {
                if let Some(m) = self.file.sources.get_mut(item_id) {
                    m.error = Some(e.clone());
                    self.save()?;
                } else if let Some(m) = self.unsaved.iter_mut().find(|m| m.item_id == item_id) {
                    m.error = Some(e.clone());
                }
                Err(e)
            }
        }
    }

    pub fn add_unsaved(&mut self, item: Item) -> SourceMeta {
        // Salvata nel frattempo (per esempio da "Aggiungi sorgenti"): vale quella.
        if let Some(saved) = self.file.sources.get(&item.id) {
            return saved.clone();
        }
        let meta = meta_of(&item);
        self.index.insert(item.id.clone(), SourceIndex::new(&item.id, &item.files));
        self.unsaved.retain(|m| m.item_id != item.id);
        self.unsaved.push(meta.clone());
        meta
    }

    pub fn close_unsaved(&mut self, item_id: &str) {
        if let Some(pos) = self.unsaved.iter().position(|m| m.item_id == item_id) {
            self.unsaved.remove(pos);
            if !self.file.sources.contains_key(item_id) {
                self.index.remove(item_id);
            }
        }
    }

    pub fn save_unsaved(&mut self, item_id: &str, collection_id: &str) -> Result<(), String> {
        if !self.unsaved.iter().any(|m| m.item_id == item_id) {
            return Err(m("Sorgente non trovata", "Source not found").into());
        }
        self.link_known(collection_id, item_id).map(|_| ())
    }

    /// Nome e sorgenti (identificatore, titolo) di una raccolta, nell'ordine in cui sono state aggiunte.
    pub fn export_data(&self, id: &str) -> Result<(String, Vec<(String, Option<String>)>), String> {
        let c = self.file.collections.iter().find(|c| c.id == id).ok_or_else(|| m("Raccolta non trovata", "Collection not found").to_string())?;
        let sources = c.sources.iter().map(|r| (r.item_id.clone(), self.file.sources.get(&r.item_id).and_then(|m| m.title.clone()))).collect();
        Ok((c.name.clone(), sources))
    }

    pub fn search(&self, scope: &Scope, query: &str, originals_only: bool) -> Result<SearchResult, String> {
        self.search_with(scope, query, &Filters { originals_only, ..Filters::default() })
    }

    pub fn search_with(&self, scope: &Scope, query: &str, filters: &Filters) -> Result<SearchResult, String> {
        let ids: Vec<&str> = match scope {
            Scope::All => self.file.collections.iter().flat_map(|c| c.sources.iter().map(|r| r.item_id.as_str())).collect(),
            Scope::Collection { id } => self
                .file
                .collections
                .iter()
                .find(|c| &c.id == id)
                .ok_or_else(|| m("Raccolta non trovata", "Collection not found").to_string())?
                .sources
                .iter()
                .filter(|r| r.included)
                .map(|r| r.item_id.as_str())
                .collect(),
            Scope::Source { item_id } => {
                if self.known(item_id).is_none() {
                    return Err(m("Sorgente non trovata", "Source not found").into());
                }
                vec![item_id.as_str()]
            }
            Scope::Unsaved => self.unsaved.iter().map(|m| m.item_id.as_str()).collect(),
        };
        let sources: Vec<&SourceIndex> = ids.iter().filter_map(|id| self.index.get(*id)).collect();
        Ok(search::search_with(&sources, query, filters))
    }
}

/// Aggiunge una sorgente a una raccolta: la collega se già nota, altrimenti ne scarica l'elenco.
pub async fn add_source(lib: &Mutex<Library>, client: &reqwest::Client, base: &str, auth: Option<&Auth>, collection_id: &str, input: &str) -> Result<SourceMeta, String> {
    let link = ia::parse_link(input).ok_or_else(|| m("Link non riconosciuto", "Link not recognized").to_string())?;
    let known = lib.lock().unwrap().link_known(collection_id, &link.item_id)?;
    if let Some(meta) = known {
        return Ok(meta);
    }
    let item = ia::fetch_item(client, base, auth, &link.item_id).await?;
    lib.lock().unwrap().add_fetched(collection_id, item)
}

/// Riscarica l'elenco; se fallisce resta quello vecchio e l'errore viene registrato.
pub async fn refresh_source(lib: &Mutex<Library>, client: &reqwest::Client, base: &str, auth: Option<&Auth>, item_id: &str) -> Result<SourceMeta, String> {
    let exists = lib.lock().unwrap().known(item_id).is_some();
    if !exists {
        return Err(m("Sorgente non trovata", "Source not found").into());
    }
    let fetched = ia::fetch_item(client, base, auth, item_id).await;
    lib.lock().unwrap().apply_refresh(item_id, fetched)
}

/// Apre una sorgente come Non salvata; se è già nota (salvata o aperta) la riusa.
pub async fn open_unsaved(lib: &Mutex<Library>, client: &reqwest::Client, base: &str, auth: Option<&Auth>, input: &str) -> Result<SourceMeta, String> {
    let link = ia::parse_link(input).ok_or_else(|| m("Link non riconosciuto", "Link not recognized").to_string())?;
    let known = lib.lock().unwrap().known(&link.item_id);
    if let Some(meta) = known {
        return Ok(meta);
    }
    let item = ia::fetch_item(client, base, auth, &link.item_id).await?;
    Ok(lib.lock().unwrap().add_unsaved(item))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ia::{FileEntry, Item};

    fn item(id: &str, names: &[&str]) -> Item {
        Item {
            id: id.into(),
            title: Some(format!("Titolo {id}")),
            files: names.iter().map(|n| FileEntry { name: (*n).into(), size: 100, format: String::new(), original: true }).collect(),
        }
    }
    fn total(lib: &Library, scope: Scope) -> usize {
        lib.search(&scope, "", false).unwrap().total
    }

    #[test]
    fn collection_names_are_validated() {
        let d = tempfile::tempdir().unwrap();
        let mut lib = Library::load(d.path());
        let a = lib.create_collection("  Xbox 360 ").unwrap();
        assert_eq!(lib.state().collections[0].name, "Xbox 360");
        assert_eq!(lib.create_collection("   ").unwrap_err(), "Nome vuoto");
        assert_eq!(lib.create_collection("xbox 360").unwrap_err(), "Esiste già una raccolta con questo nome");
        let b = lib.create_collection("Manuali").unwrap();
        assert_ne!(a, b);
        assert_eq!(lib.rename_collection(&b, "XBOX 360").unwrap_err(), "Esiste già una raccolta con questo nome");
        lib.rename_collection(&a, "Xbox 360 ").unwrap();
        lib.rename_collection(&b, "Manuali PS2").unwrap();
        assert_eq!(lib.state().collections[1].name, "Manuali PS2");
        assert_eq!(lib.rename_collection("c99", "x").unwrap_err(), "Raccolta non trovata");
        lib.delete_collection(&a).unwrap();
        assert_eq!(lib.state().collections.len(), 1);
        assert_eq!(lib.delete_collection(&a).unwrap_err(), "Raccolta non trovata");
    }

    #[test]
    fn shared_source_is_stored_once_and_orphans_are_removed() {
        let d = tempfile::tempdir().unwrap();
        let mut lib = Library::load(d.path());
        let c1 = lib.create_collection("Uno").unwrap();
        let c2 = lib.create_collection("Due").unwrap();
        lib.add_fetched(&c1, item("a", &["x.zip", "y.zip"])).unwrap();
        assert_eq!(lib.link_known(&c2, "a").unwrap().map(|m| m.file_count), Some(2));
        assert_eq!(lib.link_known(&c2, "zzz").unwrap(), None);
        let file = d.path().join("sources").join("a.json");
        assert!(file.exists());
        lib.delete_collection(&c1).unwrap();
        assert!(file.exists(), "ancora usata da Due");
        lib.remove_source(&c2, "a").unwrap();
        assert!(!file.exists(), "orfana: cancellata");
        assert!(lib.known("a").is_none());
    }

    #[test]
    fn adding_twice_to_same_collection_does_not_duplicate() {
        let d = tempfile::tempdir().unwrap();
        let mut lib = Library::load(d.path());
        let c = lib.create_collection("Uno").unwrap();
        lib.add_fetched(&c, item("a", &["x"])).unwrap();
        lib.add_fetched(&c, item("a", &["x", "y"])).unwrap();
        lib.link_known(&c, "a").unwrap();
        let st = lib.state();
        assert_eq!(st.collections[0].sources.len(), 1);
        assert_eq!(st.collections[0].sources[0].meta.file_count, 2);
    }

    #[test]
    fn refresh_failure_keeps_old_list_and_records_error() {
        let d = tempfile::tempdir().unwrap();
        let mut lib = Library::load(d.path());
        let c = lib.create_collection("Uno").unwrap();
        lib.add_fetched(&c, item("a", &["x", "y", "z"])).unwrap();
        assert_eq!(lib.apply_refresh("a", Err("Errore HTTP 503".into())).unwrap_err(), "Errore HTTP 503");
        assert_eq!(lib.known("a").unwrap().error.as_deref(), Some("Errore HTTP 503"));
        assert_eq!(total(&lib, Scope::All), 3);
        let m = lib.apply_refresh("a", Ok(item("a", &["w"]))).unwrap();
        assert_eq!((m.file_count, m.error), (1, None));
        assert_eq!(total(&lib, Scope::All), 1);
    }

    #[test]
    fn unsaved_sources_open_close_and_save() {
        let d = tempfile::tempdir().unwrap();
        let mut lib = Library::load(d.path());
        let c = lib.create_collection("Uno").unwrap();
        lib.add_unsaved(item("u", &["p", "q"]));
        lib.add_unsaved(item("v", &["r"]));
        assert_eq!(lib.state().unsaved.len(), 2);
        assert_eq!(total(&lib, Scope::Unsaved), 3);
        assert_eq!(total(&lib, Scope::Source { item_id: "u".into() }), 2);
        assert!(!d.path().join("sources").join("u.json").exists(), "le Non salvate non si scrivono su disco");
        lib.close_unsaved("v");
        assert_eq!(lib.state().unsaved.len(), 1);
        lib.save_unsaved("u", &c).unwrap();
        let st = lib.state();
        assert!(st.unsaved.is_empty());
        assert_eq!(st.collections[0].sources[0].meta.item_id, "u");
        assert!(d.path().join("sources").join("u.json").exists());
        assert_eq!(total(&lib, Scope::Collection { id: c.clone() }), 2);
        assert_eq!(lib.save_unsaved("nope", &c).unwrap_err(), "Sorgente non trovata");
    }

    #[test]
    fn scopes_respect_inclusion_and_deduplicate() {
        let d = tempfile::tempdir().unwrap();
        let mut lib = Library::load(d.path());
        let c1 = lib.create_collection("Uno").unwrap();
        let c2 = lib.create_collection("Due").unwrap();
        lib.add_fetched(&c1, item("a", &["1", "2"])).unwrap();
        lib.add_fetched(&c1, item("b", &["3"])).unwrap();
        lib.link_known(&c2, "b").unwrap();
        lib.set_included(&c1, "b", false).unwrap();
        lib.add_unsaved(item("u", &["4"]));
        assert_eq!(total(&lib, Scope::All), 3, "All ignora le esclusioni e conta b una volta");
        assert_eq!(total(&lib, Scope::Collection { id: c1.clone() }), 2);
        assert_eq!(total(&lib, Scope::Collection { id: c2 }), 1);
        assert_eq!(total(&lib, Scope::Source { item_id: "b".into() }), 1);
        assert_eq!(total(&lib, Scope::Unsaved), 1);
        assert_eq!(lib.search(&Scope::Collection { id: "c99".into() }, "", false).unwrap_err(), "Raccolta non trovata");
        assert_eq!(lib.search(&Scope::Source { item_id: "zz".into() }, "", false).unwrap_err(), "Sorgente non trovata");
        assert_eq!(lib.set_included(&c1, "zz", true).unwrap_err(), "Sorgente non trovata");
    }

    #[test]
    fn library_survives_reload() {
        let d = tempfile::tempdir().unwrap();
        let (c, before) = {
            let mut lib = Library::load(d.path());
            let c = lib.create_collection("Uno").unwrap();
            lib.add_fetched(&c, item("a", &["x", "y"])).unwrap();
            lib.add_fetched(&c, item("b", &["z"])).unwrap();
            lib.set_included(&c, "b", false).unwrap();
            lib.add_unsaved(item("u", &["w"]));
            (c, lib.state())
        };
        let mut lib = Library::load(d.path());
        let after = lib.state();
        assert_eq!(after.collections, before.collections);
        assert!(after.unsaved.is_empty(), "le Non salvate non sopravvivono alla chiusura");
        assert_eq!(total(&lib, Scope::Collection { id: c }), 2);
        assert_eq!(lib.take_warning(), None);
        let c2 = lib.create_collection("Due").unwrap();
        assert!(!before.collections.iter().any(|x| x.id == c2), "gli id non si riusano");
    }

    #[test]
    fn corrupt_library_is_moved_aside() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("library.json"), "{rotto").unwrap();
        let mut lib = Library::load(d.path());
        assert!(lib.state().collections.is_empty());
        assert_eq!(lib.take_warning().as_deref(), Some("Libreria illeggibile: è stata messa da parte come library.bak"));
        assert_eq!(lib.take_warning(), None);
        assert_eq!(std::fs::read_to_string(d.path().join("library.bak")).unwrap(), "{rotto");
    }

    #[test]
    fn missing_source_file_is_flagged() {
        let d = tempfile::tempdir().unwrap();
        {
            let mut lib = Library::load(d.path());
            let c = lib.create_collection("Uno").unwrap();
            lib.add_fetched(&c, item("a", &["x"])).unwrap();
        }
        std::fs::remove_file(d.path().join("sources").join("a.json")).unwrap();
        let lib = Library::load(d.path());
        let m = lib.known("a").unwrap();
        assert_eq!((m.file_count, m.error.as_deref()), (0, Some("Elenco mancante, aggiorna la sorgente")));
        assert_eq!(total(&lib, Scope::All), 0);
    }
    /// Se un open_unsaved finisce dopo che la stessa sorgente è stata salvata, non deve
    /// duplicarla né, chiudendo la copia Non salvata, togliere l'indice a quella salvata.
    #[test]
    fn unsaved_never_shadows_a_saved_source() {
        let d = tempfile::tempdir().unwrap();
        let mut lib = Library::load(d.path());
        let c = lib.create_collection("Uno").unwrap();
        lib.add_fetched(&c, item("x", &["1", "2"])).unwrap();
        let m = lib.add_unsaved(item("x", &["1", "2", "3"]));
        assert_eq!(m.file_count, 2, "restituisce la sorgente salvata");
        assert!(lib.state().unsaved.is_empty());
        lib.close_unsaved("x");
        assert_eq!(total(&lib, Scope::All), 2);
    }

    /// Un errore di lettura che non sia "file inesistente" (file bloccato, permessi) non deve
    /// far partire una libreria vuota che al primo salvataggio sovrascrive quella vera.
    #[test]
    fn unreadable_library_is_never_overwritten() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("library.json")).unwrap();
        let mut lib = Library::load(d.path());
        let w = lib.take_warning().expect("avviso");
        assert!(w.starts_with("Impossibile leggere la libreria"), "{w}");
        assert!(lib.create_collection("Uno").unwrap_err().starts_with("Libreria in sola lettura"));
        assert!(d.path().join("library.json").is_dir(), "il file originale resta intatto");
    }

    #[test]
    fn export_data_lists_sources_in_order() {
        let d = tempfile::tempdir().unwrap();
        let mut lib = Library::load(d.path());
        let c = lib.create_collection("Uno").unwrap();
        lib.add_fetched(&c, item("b", &["x"])).unwrap();
        lib.add_fetched(&c, item("a", &["y"])).unwrap();
        let (name, sources) = lib.export_data(&c).unwrap();
        assert_eq!(name, "Uno");
        assert_eq!(sources, vec![("b".to_string(), Some("Titolo b".to_string())), ("a".to_string(), Some("Titolo a".to_string()))]);
        assert_eq!(lib.export_data("c99").unwrap_err(), "Raccolta non trovata");
    }

    fn ids_of(lib: &Library, cid: &str) -> Vec<(String, bool)> {
        let st = lib.state();
        st.collections.iter().find(|c| c.id == cid).unwrap().sources.iter().map(|s| (s.meta.item_id.clone(), s.included)).collect()
    }

    #[test]
    fn sources_move_copy_and_remove_in_bulk() {
        let d = tempfile::tempdir().unwrap();
        let mut lib = Library::load(d.path());
        let a = lib.create_collection("A").unwrap();
        let b = lib.create_collection("B").unwrap();
        for id in ["x", "y", "z"] {
            lib.add_fetched(&a, item(id, &["f"])).unwrap();
        }
        lib.set_included(&a, "y", false).unwrap();
        lib.link_known(&b, "z").unwrap();

        lib.copy_sources(&b, &["x".into(), "y".into(), "z".into()]).unwrap();
        assert_eq!(ids_of(&lib, &b), [("z".into(), true), ("x".into(), true), ("y".into(), false)], "copia: niente doppioni, spunta conservata");
        assert_eq!(ids_of(&lib, &a).len(), 3);

        let c = lib.create_collection("C").unwrap();
        lib.move_sources(&a, &c, &["x".into(), "y".into()]).unwrap();
        assert_eq!(ids_of(&lib, &a), [("z".into(), true)]);
        assert_eq!(ids_of(&lib, &c), [("x".into(), true), ("y".into(), false)]);
        assert!(d.path().join("sources").join("x.json").exists(), "spostata, non orfana");

        lib.remove_sources(&c, &["x".into(), "y".into()]).unwrap();
        assert!(ids_of(&lib, &c).is_empty());
        assert!(lib.known("x").is_some(), "ancora in B");
        lib.remove_sources(&b, &["x".into()]).unwrap();
        assert!(lib.known("x").is_none() && !d.path().join("sources").join("x.json").exists(), "ora orfana: cancellata");

        assert_eq!(lib.move_sources(&a, &a, &["z".into()]).unwrap_err(), "Origine e destinazione coincidono");
        assert_eq!(lib.copy_sources("c99", &["z".into()]).unwrap_err(), "Raccolta non trovata");
        assert_eq!(lib.copy_sources(&b, &["nope".into()]).unwrap_err(), "Sorgente non trovata");
    }

}
