//! List files to import and export a collection.
//!
//! Format: UTF-8 text, one source per line (archive.org link or identifier). The first line
//! starting with `#` is the collection name; other `#` lines and blank lines are comments.
//! CSV "title,link" rows are accepted too (`,` `;` or tab separators): the link field counts.

use crate::error::{AppError, AppResult};
use crate::ia;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

/// A subcollection in the file, opened by a `# --- Name ---` separator line.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Section {
    pub name: String,
    pub inputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParsedList {
    pub name: Option<String>,
    /// Valid lines before any separator, in file order and without duplicates.
    pub inputs: Vec<String>,
    /// Lines without a recognizable source.
    pub invalid: Vec<String>,
    /// Subcollections, in file order.
    pub sections: Vec<Section>,
}

fn clean(field: &str) -> &str {
    field.trim().trim_matches('"').trim()
}

/// `# --- Name ---` (two dashes per side are enough) → Some("Name").
fn separator(comment: &str) -> Option<&str> {
    let c = comment.trim();
    if !(c.starts_with("--") && c.ends_with("--")) {
        return None;
    }
    let name = c.trim_matches('-').trim();
    (!name.is_empty()).then_some(name)
}

pub fn parse_list(text: &str) -> ParsedList {
    let mut name = None;
    let mut inputs = Vec::new();
    let mut invalid = Vec::new();
    let mut sections: Vec<Section> = Vec::new();
    // Duplicates are per section: the same source may appear in different sections.
    let mut seen: Vec<HashSet<String>> = vec![HashSet::new()];
    let mut first_data = true;
    for raw in text.trim_start_matches('\u{feff}').lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix('#') {
            if let Some(sec) = separator(rest) {
                sections.push(Section { name: sec.to_string(), inputs: Vec::new() });
                seen.push(HashSet::new());
            } else if name.is_none() && first_data && sections.is_empty() && !rest.trim().is_empty() {
                name = Some(rest.trim().to_string());
            }
            continue;
        }
        let fields: Vec<&str> = line.split([',', ';', '\t']).map(clean).filter(|f| !f.is_empty()).collect();
        let is_first = std::mem::replace(&mut first_data, false);
        // An explicit archive.org link wins (a title may be a URL too: "http://site/, https://archive.org/…");
        // a bare identifier counts only if it is the only field on the line.
        let is_url = |f: &&&str| f.starts_with("http://") || f.starts_with("https://");
        let candidate = fields.iter().filter(is_url).find(|f| ia::parse_link(f).is_some()).or_else(|| fields.iter().find(is_url)).copied().or(if fields.len() == 1 { Some(fields[0]) } else { None });
        match candidate.and_then(|c| ia::parse_link(c).map(|p| (c, p))) {
            Some((c, p)) => {
                let target = match sections.last_mut() {
                    Some(sec) => &mut sec.inputs,
                    None => &mut inputs,
                };
                if seen.last_mut().expect("always at least one set").insert(p.item_id) {
                    target.push(c.to_string());
                }
            }
            // CSV header ("title,link"): first line with several fields and no link.
            None if is_first && fields.len() > 1 && candidate.is_none() => {}
            None => invalid.push(line.to_string()),
        }
    }
    ParsedList { name, inputs, invalid, sections }
}

/// Largest list file read: far beyond any real list, small enough to never strain memory.
const MAX_LIST_MB: u64 = 16;

/// Reads a .txt/.csv list; without a `# name` line, the name is the file name. The path may come
/// from the window (a dropped file), so anything else is refused: other file types, folders and
/// devices, network paths, huge files.
pub fn read_list_file(path: &Path) -> AppResult<ParsedList> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
    let network = cfg!(windows) && ["\\\\", "//"].iter().any(|p| path.to_string_lossy().starts_with(p));
    if network || !matches!(ext.as_deref(), Some("txt" | "csv")) {
        return Err(AppError::NotAListFile);
    }
    let meta = std::fs::metadata(path).map_err(|e| AppError::ReadFileFailed { detail: e.to_string() })?;
    if !meta.is_file() {
        return Err(AppError::NotAListFile);
    }
    if meta.len() > MAX_LIST_MB << 20 {
        return Err(AppError::FileTooLarge { max_mb: MAX_LIST_MB });
    }
    let bytes = std::fs::read(path).map_err(|e| AppError::ReadFileFailed { detail: e.to_string() })?;
    let mut list = parse_list(&String::from_utf8_lossy(&bytes));
    if list.name.is_none() {
        list.name = path.file_stem().map(|s| s.to_string_lossy().into_owned());
    }
    Ok(list)
}

fn push_sources(out: &mut String, sources: &[(String, Option<String>)]) {
    for (id, title) in sources {
        let url = format!("https://archive.org/details/{id}");
        match title.as_deref().map(|t| t.replace(['\n', '\r', '\t'], " ")) {
            Some(t) if !t.trim().is_empty() && !t.trim_start().starts_with('#') => out.push_str(&format!("{}, {url}\n", t.trim())),
            _ => out.push_str(&format!("{url}\n")),
        }
    }
}

/// Sources of a collection: (identifier, title).
pub type SourceList = Vec<(String, Option<String>)>;

/// A collection in the `parse_list` format: title (if any) and link of each source.
pub fn format_list(name: &str, sources: &[(String, Option<String>)]) -> String {
    format_list_with(name, sources, &[])
}

/// Like `format_list`, with subcollections written after a `# --- Name ---` separator.
pub fn format_list_with(name: &str, sources: &[(String, Option<String>)], subs: &[(String, SourceList)]) -> String {
    let mut out = format!("# {}\n# Cain Archive: one source per line, \"title, link\"; \"# --- Name ---\" starts a subcollection\n", name.trim());
    push_sources(&mut out, sources);
    for (sub, list) in subs {
        out.push_str(&format!("\n# --- {} ---\n", sub.trim()));
        push_sources(&mut out, list);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_name_links_ids_and_comments() {
        let text = "\u{feff}# Xbox 360\n\n# commento\nhttps://archive.org/details/uno\n  due_2  \nhttps://archive.org/download/tre/file.zip\n";
        let l = parse_list(text);
        assert_eq!(l.name.as_deref(), Some("Xbox 360"));
        assert_eq!(l.inputs, ["https://archive.org/details/uno", "due_2", "https://archive.org/download/tre/file.zip"]);
        assert!(l.invalid.is_empty());
    }

    #[test]
    fn reads_csv_title_link_and_skips_header() {
        let text = "titolo,link\nHalo, https://archive.org/details/halo_item\n\"Gears; edizione\";https://archive.org/details/gears\nNASA\thttps://archive.org/details/nasa\n";
        let l = parse_list(text);
        assert_eq!(l.name, None);
        assert_eq!(l.inputs, ["https://archive.org/details/halo_item", "https://archive.org/details/gears", "https://archive.org/details/nasa"]);
        assert!(l.invalid.is_empty(), "{:?}", l.invalid);
    }

    #[test]
    fn reports_invalid_lines_and_drops_duplicates() {
        let text = "# Lista\nhttps://example.com/x\ndue parole\nnasa\nhttps://archive.org/details/nasa\nTitolo, senza link\n";
        let l = parse_list(text);
        assert_eq!(l.inputs, ["nasa"]);
        assert_eq!(l.invalid, ["https://example.com/x", "due parole", "Titolo, senza link"]);
    }

    #[test]
    fn exported_list_reads_back_the_same() {
        let text = format_list("Xbox 360", &[("uno".into(), Some("Primo, con virgola".into())), ("due".into(), None)]);
        assert!(text.starts_with("# Xbox 360\n"));
        let l = parse_list(&text);
        assert_eq!(l.name.as_deref(), Some("Xbox 360"));
        assert_eq!(l.inputs, ["https://archive.org/details/uno", "https://archive.org/details/due"]);
        assert!(l.invalid.is_empty());
    }
    #[test]
    fn list_file_takes_name_from_file_when_missing() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("Mia lista.txt");
        std::fs::write(&f, "nasa
").unwrap();
        let l = read_list_file(&f).unwrap();
        assert_eq!((l.name.as_deref(), l.inputs.len()), (Some("Mia lista"), 1));
        std::fs::write(&f, "# Dal file
nasa
").unwrap();
        assert_eq!(read_list_file(&f).unwrap().name.as_deref(), Some("Dal file"));
        let e = read_list_file(&d.path().join("missing.txt")).unwrap_err();
        assert!(matches!(e, AppError::ReadFileFailed { .. }), "{e:?}");
    }

    #[test]
    fn only_list_files_are_read() {
        let d = tempfile::tempdir().unwrap();
        let key = d.path().join("id_ed25519");
        std::fs::write(&key, "secret").unwrap();
        assert_eq!(read_list_file(&key).unwrap_err(), AppError::NotAListFile);
        let folder = d.path().join("folder.txt");
        std::fs::create_dir(&folder).unwrap();
        assert_eq!(read_list_file(&folder).unwrap_err(), AppError::NotAListFile);
        let big = d.path().join("big.csv");
        std::fs::File::create(&big).unwrap().set_len((MAX_LIST_MB << 20) + 1).unwrap();
        assert_eq!(read_list_file(&big).unwrap_err(), AppError::FileTooLarge { max_mb: MAX_LIST_MB });
        if cfg!(windows) {
            assert_eq!(read_list_file(Path::new(r"\\attacker\share\x.txt")).unwrap_err(), AppError::NotAListFile);
        }
    }

    #[test]
    fn separators_open_sections() {
        let text = "# x360\nnasa\n\n# --- DLC ---\nuno\ndue\n#--XBLA--\ntre\n# commento\nquattro\n";
        let l = parse_list(text);
        assert_eq!(l.name.as_deref(), Some("x360"));
        assert_eq!(l.inputs, ["nasa"]);
        let secs: Vec<(String, Vec<String>)> = l.sections.iter().map(|s| (s.name.clone(), s.inputs.clone())).collect();
        assert_eq!(secs, [("DLC".to_string(), vec!["uno".to_string(), "due".to_string()]), ("XBLA".to_string(), vec!["tre".to_string(), "quattro".to_string()])]);
    }

    #[test]
    fn a_separator_first_is_a_section_not_the_name() {
        let l = parse_list("# --- DLC ---\nuno\n");
        assert_eq!(l.name, None);
        assert!(l.inputs.is_empty());
        assert_eq!(l.sections[0].name, "DLC");
    }

    #[test]
    fn the_same_source_may_appear_in_different_sections() {
        let l = parse_list("# x\nuno\nuno\n# --- A ---\nuno\n");
        assert_eq!(l.inputs, ["uno"]);
        assert_eq!(l.sections[0].inputs, ["uno"]);
    }

    #[test]
    fn url_titles_read_back() {
        let text = format_list("Web", &[("capture1".into(), Some("http://example.com/".into()))]);
        let l = parse_list(&text);
        assert_eq!(l.inputs, ["https://archive.org/details/capture1"]);
        assert!(l.invalid.is_empty(), "{:?}", l.invalid);
    }

    #[test]
    fn exported_sections_read_back() {
        let subs = vec![("DLC".to_string(), vec![("d1".to_string(), Some("Primo DLC".to_string()))])];
        let text = format_list_with("x360", &[("g1".into(), None)], &subs);
        assert!(text.contains("\n# --- DLC ---\n"), "{text}");
        let l = parse_list(&text);
        assert_eq!(l.inputs, ["https://archive.org/details/g1"]);
        assert_eq!((l.sections[0].name.as_str(), l.sections[0].inputs.clone()), ("DLC", vec!["https://archive.org/details/d1".to_string()]));
    }

}
