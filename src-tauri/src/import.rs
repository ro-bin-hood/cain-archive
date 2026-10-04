//! File di lista per importare ed esportare una raccolta.
//!
//! Formato: testo UTF-8, una sorgente per riga (link archive.org o identificatore). La prima riga
//! che inizia con `#` è il nome della raccolta; le altre `#` e le righe vuote sono commenti.
//! Sono accettate anche righe CSV "titolo,link" (separatori `,` `;` o tab): conta il campo link.

use crate::i18n::m;
use crate::ia;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

/// Una sotto-raccolta del file, aperta da una riga separatore `# --- Nome ---`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Section {
    pub name: String,
    pub inputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParsedList {
    pub name: Option<String>,
    /// Righe valide prima di qualsiasi separatore, nell'ordine del file e senza doppioni.
    pub inputs: Vec<String>,
    /// Righe che non contengono una sorgente riconoscibile.
    pub invalid: Vec<String>,
    /// Sotto-raccolte, nell'ordine del file.
    pub sections: Vec<Section>,
}

fn clean(field: &str) -> &str {
    field.trim().trim_matches('"').trim()
}

/// `# --- Nome ---` (bastano due trattini per lato) → Some("Nome").
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
    // Doppioni per sezione: la stessa sorgente può stare in sezioni diverse.
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
        // Un link esplicito vince; un identificatore nudo vale solo se è l'unico campo della riga.
        let candidate = fields.iter().find(|f| f.starts_with("http://") || f.starts_with("https://")).copied().or(if fields.len() == 1 { Some(fields[0]) } else { None });
        match candidate.and_then(|c| ia::parse_link(c).map(|p| (c, p))) {
            Some((c, p)) => {
                let target = match sections.last_mut() {
                    Some(sec) => &mut sec.inputs,
                    None => &mut inputs,
                };
                if seen.last_mut().expect("sempre almeno un insieme").insert(p.item_id) {
                    target.push(c.to_string());
                }
            }
            // Intestazione CSV ("titolo,link"): prima riga con più campi e nessun link.
            None if is_first && fields.len() > 1 && candidate.is_none() => {}
            None => invalid.push(line.to_string()),
        }
    }
    ParsedList { name, inputs, invalid, sections }
}

/// Legge un file di lista; se manca la riga `# nome`, il nome è quello del file.
pub fn read_list_file(path: &Path) -> Result<ParsedList, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", m("Impossibile leggere il file", "Could not read the file")))?;
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

/// Sorgenti di una raccolta: (identificatore, titolo).
pub type SourceList = Vec<(String, Option<String>)>;

/// Una raccolta nel formato di `parse_list`: titolo (se c'è) e link di ogni sorgente.
pub fn format_list(name: &str, sources: &[(String, Option<String>)]) -> String {
    format_list_with(name, sources, &[])
}

/// Come `format_list`, con le sotto-raccolte scritte dopo un separatore `# --- Nome ---`.
pub fn format_list_with(name: &str, sources: &[(String, Option<String>)], subs: &[(String, SourceList)]) -> String {
    let mut out = format!("# {}\n# Cain Archive: una sorgente per riga, \"titolo, link\"; \"# --- Nome ---\" apre una sotto-raccolta\n", name.trim());
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
        let e = read_list_file(&d.path().join("manca.txt")).unwrap_err();
        assert!(e.starts_with("Impossibile leggere il file"), "{e}");
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
    fn exported_sections_read_back() {
        let subs = vec![("DLC".to_string(), vec![("d1".to_string(), Some("Primo DLC".to_string()))])];
        let text = format_list_with("x360", &[("g1".into(), None)], &subs);
        assert!(text.contains("\n# --- DLC ---\n"), "{text}");
        let l = parse_list(&text);
        assert_eq!(l.inputs, ["https://archive.org/details/g1"]);
        assert_eq!((l.sections[0].name.as_str(), l.sections[0].inputs.clone()), ("DLC", vec!["https://archive.org/details/d1".to_string()]));
    }

}
