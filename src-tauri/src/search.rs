use crate::ia::FileEntry;
use serde::Serialize;
use std::collections::HashSet;

pub const MAX_RESULTS: usize = 500;

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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hit {
    pub item_id: String,
    pub name: String,
    pub size: u64,
    pub original: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct SearchResult {
    pub results: Vec<Hit>,
    pub total: usize,
}

/// I file il cui nome contiene tutte le parole della query; i primi MAX_RESULTS in ordine di nome.
pub fn search(sources: &[&SourceIndex], query: &str, originals_only: bool) -> SearchResult {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut seen = HashSet::new();
    let mut hits: Vec<(&SourceIndex, usize)> = Vec::new();
    for s in sources {
        if !seen.insert(s.item_id.as_str()) {
            continue;
        }
        for (i, lower) in s.lower.iter().enumerate() {
            if originals_only && !s.files[i].original {
                continue;
            }
            if words.iter().all(|w| lower.contains(w.as_str())) {
                hits.push((s, i));
            }
        }
    }
    let total = hits.len();
    let order = |(a, i): &(&SourceIndex, usize), (b, j): &(&SourceIndex, usize)| a.lower[*i].cmp(&b.lower[*j]).then_with(|| a.item_id.cmp(&b.item_id));
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
            Hit { item_id: s.item_id.clone(), name: f.name.clone(), size: f.size, original: f.original }
        })
        .collect();
    SearchResult { results, total }
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
}
