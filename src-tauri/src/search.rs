use crate::download::dest_for;
use crate::ia::FileEntry;
use crate::types::{Job, JobStatus};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub const MAX_RESULTS: usize = 500;
/// Quante estensioni proporre come filtro (le più frequenti).
const MAX_EXTENSIONS: usize = 15;

/// Elenco dei file di una sorgente con i nomi già in minuscolo, pronto per la ricerca.
#[derive(Debug, Clone)]
pub struct SourceIndex {
    pub item_id: String,
    pub files: Vec<FileEntry>,
    lower: Vec<String>,
}

impl SourceIndex {
    pub fn new(item_id: &str, files: &[FileEntry]) -> Self {
        Self { item_id: item_id.to_string(), files: files.to_vec(), lower: files.iter().map(|f| f.name.to_lowercase()).collect() }
    }
}

/// Se un risultato è già sul disco o già nella coda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Local {
    Downloaded,
    Queued,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hit {
    pub item_id: String,
    pub name: String,
    pub size: u64,
    pub original: bool,
    pub local: Option<Local>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExtCount {
    pub ext: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct SearchResult {
    pub results: Vec<Hit>,
    pub total: usize,
    /// Estensioni presenti tra le corrispondenze, prima del filtro per estensione: servono come chip.
    pub extensions: Vec<ExtCount>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    #[default]
    Name,
    SizeDesc,
    SizeAsc,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct Filters {
    pub originals_only: bool,
    /// Estensioni ammesse, in minuscolo e senza punto; vuoto = tutte.
    pub exts: Vec<String>,
    pub sort: Sort,
}

/// Estensione in minuscolo dell'ultimo pezzo del percorso, "" se non c'è.
fn ext_of(lower: &str) -> &str {
    let base = lower.rsplit('/').next().unwrap_or(lower);
    match base.rfind('.') {
        Some(i) if i > 0 => &base[i + 1..],
        _ => "",
    }
}

/// Ricerca per nome con l'ordine predefinito (vedi `search_with`).
pub fn search(sources: &[&SourceIndex], query: &str, originals_only: bool) -> SearchResult {
    search_with(sources, query, &Filters { originals_only, ..Filters::default() })
}

/// I file il cui nome contiene tutte le parole della query, filtrati e ordinati; i primi MAX_RESULTS.
pub fn search_with(sources: &[&SourceIndex], query: &str, filters: &Filters) -> SearchResult {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let exts: Vec<String> = filters.exts.iter().map(|e| e.trim().trim_start_matches('.').to_lowercase()).collect();
    let mut seen = HashSet::new();
    let mut ext_counts: HashMap<&str, usize> = HashMap::new();
    let mut hits: Vec<(&SourceIndex, usize)> = Vec::new();
    for s in sources {
        if !seen.insert(s.item_id.as_str()) {
            continue;
        }
        for (i, lower) in s.lower.iter().enumerate() {
            if filters.originals_only && !s.files[i].original {
                continue;
            }
            if !words.iter().all(|w| lower.contains(w.as_str())) {
                continue;
            }
            let ext = ext_of(lower);
            if !ext.is_empty() {
                *ext_counts.entry(ext).or_default() += 1;
            }
            if exts.is_empty() || exts.iter().any(|e| e == ext) {
                hits.push((s, i));
            }
        }
    }
    let total = hits.len();
    let by_name = |(a, i): &(&SourceIndex, usize), (b, j): &(&SourceIndex, usize)| a.lower[*i].cmp(&b.lower[*j]).then_with(|| a.item_id.cmp(&b.item_id));
    let order = |x: &(&SourceIndex, usize), y: &(&SourceIndex, usize)| -> Ordering {
        let (sx, sy) = (x.0.files[x.1].size, y.0.files[y.1].size);
        match filters.sort {
            Sort::Name => by_name(x, y),
            Sort::SizeDesc => sy.cmp(&sx).then_with(|| by_name(x, y)),
            Sort::SizeAsc => sx.cmp(&sy).then_with(|| by_name(x, y)),
        }
    };
    // Con molte corrispondenze basta separare le prime MAX_RESULTS e ordinare solo quelle.
    if hits.len() > MAX_RESULTS {
        hits.select_nth_unstable_by(MAX_RESULTS, order);
        hits.truncate(MAX_RESULTS);
    }
    hits.sort_by(order);
    let results = hits
        .into_iter()
        .map(|(s, i)| {
            let f = &s.files[i];
            Hit { item_id: s.item_id.clone(), name: f.name.clone(), size: f.size, original: f.original, local: None }
        })
        .collect();
    let mut extensions: Vec<ExtCount> = ext_counts.into_iter().map(|(ext, count)| ExtCount { ext: ext.to_string(), count }).collect();
    extensions.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.ext.cmp(&b.ext)));
    extensions.truncate(MAX_EXTENSIONS);
    SearchResult { results, total, extensions }
}

/// Segna i risultati già in coda (anche in pausa) o già scaricati: job completato, oppure file
/// presente in `<out_dir>/<item>/<nome>` con la dimensione attesa. Un job fallito non conta.
pub fn mark_local(hits: &mut [Hit], jobs: &[Job], out_dir: &Path) {
    let by_key: HashMap<(&str, &str), &JobStatus> = jobs.iter().map(|j| ((j.item_id.as_str(), j.name.as_str()), &j.status)).collect();
    for h in hits.iter_mut() {
        h.local = match by_key.get(&(h.item_id.as_str(), h.name.as_str())) {
            Some(JobStatus::Done) => Some(Local::Downloaded),
            Some(JobStatus::Queued | JobStatus::Downloading | JobStatus::Retrying { .. } | JobStatus::Paused) => Some(Local::Queued),
            _ => {
                let on_disk = std::fs::metadata(dest_for(out_dir, &h.item_id, &h.name)).map(|m| m.is_file() && (h.size == 0 || m.len() == h.size)).unwrap_or(false);
                on_disk.then_some(Local::Downloaded)
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(name: &str, original: bool) -> FileEntry {
        FileEntry { name: name.into(), size: 10, format: String::new(), original }
    }
    fn names(r: &SearchResult) -> Vec<String> {
        r.results.iter().map(|h| format!("{}:{}", h.item_id, h.name)).collect()
    }

    #[test]
    fn all_words_must_match_case_insensitive() {
        let a = SourceIndex::new("a", &[f("Call of Duty (Italy).zip", true), f("Call of Juarez (Italy).zip", true), f("Duty Calls.zip", true)]);
        let r = search(&[&a], "duty ITALY", false);
        assert_eq!(names(&r), ["a:Call of Duty (Italy).zip"]);
        assert_eq!(r.total, 1);
    }

    #[test]
    fn empty_or_blank_query_returns_everything() {
        let a = SourceIndex::new("a", &[f("x", true), f("y", true)]);
        assert_eq!(search(&[&a], "", false).total, 2);
        assert_eq!(search(&[&a], "   ", false).total, 2);
    }

    #[test]
    fn results_are_sorted_by_name_then_source() {
        let a = SourceIndex::new("a", &[f("b.zip", true), f("C.zip", true)]);
        let b = SourceIndex::new("b", &[f("a.zip", true), f("b.zip", true)]);
        assert_eq!(names(&search(&[&a, &b], "", false)), ["b:a.zip", "a:b.zip", "b:b.zip", "a:C.zip"]);
    }

    #[test]
    fn at_most_500_results_with_full_total() {
        let files: Vec<FileEntry> = (0..1200).map(|i| f(&format!("file{i:04}.bin"), true)).collect();
        let a = SourceIndex::new("a", &files);
        let r = search(&[&a], "file", false);
        assert_eq!(r.total, 1200);
        assert_eq!(r.results.len(), MAX_RESULTS);
        assert_eq!(r.results[0].name, "file0000.bin");
    }

    #[test]
    fn duplicate_sources_are_counted_once() {
        let a = SourceIndex::new("a", &[f("x", true)]);
        assert_eq!(search(&[&a, &a], "", false).total, 1);
    }

    #[test]
    fn originals_only_filters_derivatives() {
        let a = SourceIndex::new("a", &[f("x.flac", true), f("x.mp3", false)]);
        assert_eq!(names(&search(&[&a], "x", true)), ["a:x.flac"]);
    }

    #[test]
    fn hit_carries_size_and_original() {
        let a = SourceIndex::new("a", &[FileEntry { name: "x".into(), size: 1234, format: "Flac".into(), original: false }]);
        let h = &search(&[&a], "", false).results[0];
        assert_eq!((h.size, h.original), (1234, false));
    }

    #[test]
    fn search_100k_files_is_fast() {
        let files: Vec<FileEntry> = (0..100_000).map(|i| f(&format!("Game {i} (Europe) (En,It).zip"), true)).collect();
        let a = SourceIndex::new("a", &files);
        let t = std::time::Instant::now();
        let r = search(&[&a], "europe it", false);
        assert_eq!(r.total, 100_000);
        assert!(t.elapsed().as_millis() < 1000, "{:?}", t.elapsed());
    }
    fn filters(exts: &[&str], sort: Sort) -> Filters {
        Filters { originals_only: false, exts: exts.iter().map(|e| e.to_string()).collect(), sort }
    }

    #[test]
    fn extension_filter_and_counts() {
        let a = SourceIndex::new("a", &[f("x.ZIP", true), f("y.zip", true), f("z.iso", true), f("dir/readme", true), f("w.flac", false)]);
        let r = search_with(&[&a], "", &filters(&["zip"], Sort::Name));
        assert_eq!(names(&r), ["a:x.ZIP", "a:y.zip"]);
        assert_eq!(r.total, 2);
        let counts: Vec<(String, usize)> = r.extensions.iter().map(|e| (e.ext.clone(), e.count)).collect();
        assert_eq!(counts, [("zip".to_string(), 2), ("flac".to_string(), 1), ("iso".to_string(), 1)], "contate prima del filtro per estensione");
        let only_orig = search_with(&[&a], "", &Filters { originals_only: true, exts: vec![], sort: Sort::Name });
        assert!(only_orig.extensions.iter().all(|e| e.ext != "flac"));
    }

    #[test]
    fn sort_by_size() {
        let files = [("b", 30), ("a", 30), ("c", 10), ("d", 99)].map(|(n, size)| FileEntry { name: n.into(), size, format: String::new(), original: true });
        let a = SourceIndex::new("a", &files);
        assert_eq!(names(&search_with(&[&a], "", &filters(&[], Sort::SizeDesc))), ["a:d", "a:a", "a:b", "a:c"]);
        assert_eq!(names(&search_with(&[&a], "", &filters(&[], Sort::SizeAsc))), ["a:c", "a:a", "a:b", "a:d"]);
    }

    #[test]
    fn hits_are_marked_downloaded_or_queued() {
        use crate::types::{Job, JobStatus};
        let d = tempfile::tempdir().unwrap();
        let a = SourceIndex::new("it", &[
            FileEntry { name: "su_disco.bin".into(), size: 3, format: String::new(), original: true },
            FileEntry { name: "corto.bin".into(), size: 10, format: String::new(), original: true },
            FileEntry { name: "in_coda.bin".into(), size: 5, format: String::new(), original: true },
            FileEntry { name: "fallito.bin".into(), size: 5, format: String::new(), original: true },
            FileEntry { name: "finito.bin".into(), size: 5, format: String::new(), original: true },
        ]);
        std::fs::create_dir_all(d.path().join("it")).unwrap();
        std::fs::write(d.path().join("it/su_disco.bin"), b"abc").unwrap();
        std::fs::write(d.path().join("it/corto.bin"), b"abc").unwrap();
        let job = |name: &str, status: JobStatus| Job { id: 1, item_id: "it".into(), name: name.into(), size: 5, dest: d.path().join("it").join(name), status };
        let jobs = vec![job("in_coda.bin", JobStatus::Paused), job("fallito.bin", JobStatus::Failed { reason: "x".into() }), job("finito.bin", JobStatus::Done)];
        let mut r = search(&[&a], "", false);
        mark_local(&mut r.results, &jobs, d.path());
        let got: Vec<(String, Option<Local>)> = r.results.iter().map(|h| (h.name.clone(), h.local)).collect();
        assert_eq!(got, [
            ("corto.bin".to_string(), None),
            ("fallito.bin".to_string(), None),
            ("finito.bin".to_string(), Some(Local::Downloaded)),
            ("in_coda.bin".to_string(), Some(Local::Queued)),
            ("su_disco.bin".to_string(), Some(Local::Downloaded)),
        ]);
    }

}
