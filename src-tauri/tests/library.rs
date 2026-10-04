mod common;
use cain_archive_lib::{
    ia,
    library::{self, Library, Scope},
};
use std::sync::Mutex;

fn lib(dir: &std::path::Path) -> Mutex<Library> {
    Mutex::new(Library::load(dir))
}

fn meta_hits(srv: &common::Srv, item: &str) -> u32 {
    *srv.hits.lock().unwrap().get(&format!("meta:{item}")).unwrap_or(&0)
}

#[tokio::test]
async fn add_source_downloads_and_persists() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let l = lib(d.path());
    let c = l.lock().unwrap().create_collection("Uno").unwrap();
    let m = library::add_source(&l, &ia::client(), &base, None, &c, "https://archive.org/details/nasa").await.unwrap();
    assert_eq!((m.item_id.as_str(), m.title.as_deref(), m.file_count, m.total_size), ("nasa", Some("Titolo nasa"), 3, 1550));
    assert!(d.path().join("sources").join("nasa.json").exists());
    let r = l.lock().unwrap().search(&Scope::Collection { id: c }, "flac", false).unwrap();
    assert_eq!(r.total, 1);
}

#[tokio::test]
async fn add_source_reports_errors_without_changes() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let l = lib(d.path());
    let c = l.lock().unwrap().create_collection("Uno").unwrap();
    let client = ia::client();
    assert_eq!(library::add_source(&l, &client, &base, None, &c, "https://example.com/x").await.unwrap_err(), "Link non riconosciuto");
    let e = library::add_source(&l, &client, &base, None, &c, "missing").await.unwrap_err();
    assert!(e.contains("prova ad accedere"), "{e}");
    assert_eq!(library::add_source(&l, &client, &base, None, "c99", "nasa").await.unwrap_err(), "Raccolta non trovata");
    assert!(l.lock().unwrap().state().collections[0].sources.is_empty());
}

#[tokio::test]
async fn known_source_is_linked_without_refetching() {
    let (base, srv) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let l = lib(d.path());
    let (c1, c2) = { let mut g = l.lock().unwrap(); (g.create_collection("Uno").unwrap(), g.create_collection("Due").unwrap()) };
    let client = ia::client();
    library::add_source(&l, &client, &base, None, &c1, "nasa").await.unwrap();
    library::add_source(&l, &client, &base, None, &c2, "nasa").await.unwrap();
    library::add_source(&l, &client, &base, None, &c2, "nasa").await.unwrap();
    assert_eq!(meta_hits(&srv, "nasa"), 1);
    assert_eq!(l.lock().unwrap().state().collections[1].sources.len(), 1);
}

#[tokio::test]
async fn concurrent_adds_create_one_entry() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let l = lib(d.path());
    let c = l.lock().unwrap().create_collection("Uno").unwrap();
    let client = ia::client();
    let (a, b) = tokio::join!(
        library::add_source(&l, &client, &base, None, &c, "nasa"),
        library::add_source(&l, &client, &base, None, &c, "https://archive.org/details/nasa"),
    );
    a.unwrap();
    b.unwrap();
    assert_eq!(l.lock().unwrap().state().collections[0].sources.len(), 1);
    assert_eq!(l.lock().unwrap().search(&Scope::All, "", false).unwrap().total, 3);
}

#[tokio::test]
async fn refresh_failure_keeps_the_list() {
    let (base, _) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let l = lib(d.path());
    let c = l.lock().unwrap().create_collection("Uno").unwrap();
    let client = ia::client();
    library::add_source(&l, &client, &base, None, &c, "flaky").await.unwrap();
    let e = library::refresh_source(&l, &client, &base, None, "flaky").await.unwrap_err();
    assert!(e.contains("Item vuoto"), "{e}");
    let g = l.lock().unwrap();
    assert!(g.known("flaky").unwrap().error.is_some());
    assert_eq!(g.search(&Scope::All, "", false).unwrap().total, 3);
    drop(g);
    assert_eq!(library::refresh_source(&l, &client, &base, None, "nope").await.unwrap_err(), "Sorgente non trovata");
}

#[tokio::test]
async fn open_unsaved_reuses_saved_source() {
    let (base, srv) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let l = lib(d.path());
    let c = l.lock().unwrap().create_collection("Uno").unwrap();
    let client = ia::client();
    library::add_source(&l, &client, &base, None, &c, "nasa").await.unwrap();
    let m = library::open_unsaved(&l, &client, &base, None, "https://archive.org/download/nasa/a.flac").await.unwrap();
    assert_eq!(m.item_id, "nasa");
    assert!(l.lock().unwrap().state().unsaved.is_empty());
    assert_eq!(meta_hits(&srv, "nasa"), 1);
}

#[tokio::test]
async fn unsaved_then_added_becomes_saved() {
    let (base, srv) = common::start().await;
    let d = tempfile::tempdir().unwrap();
    let l = lib(d.path());
    let c = l.lock().unwrap().create_collection("Uno").unwrap();
    let client = ia::client();
    library::open_unsaved(&l, &client, &base, None, "https://archive.org/details/other").await.unwrap();
    assert_eq!(l.lock().unwrap().state().unsaved.len(), 1);
    library::add_source(&l, &client, &base, None, &c, "other").await.unwrap();
    let st = l.lock().unwrap().state();
    assert!(st.unsaved.is_empty());
    assert_eq!(st.collections[0].sources[0].meta.item_id, "other");
    assert_eq!(meta_hits(&srv, "other"), 1);
    assert!(d.path().join("sources").join("other.json").exists());
}
