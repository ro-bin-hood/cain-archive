//! File di lista per importare ed esportare una raccolta.
//!
//! Formato: testo UTF-8, una sorgente per riga (link archive.org o identificatore). La prima riga
//! che inizia con `#` è il nome della raccolta; le altre `#` e le righe vuote sono commenti.
//! Sono accettate anche righe CSV "titolo,link" (separatori `,` `;` o tab): conta il campo link.

use crate::ia;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParsedList {
    pub name: Option<String>,
    /// Righe valide, nell'ordine del file e senza doppioni (stesso identificatore).
    pub inputs: Vec<String>,
    /// Righe che non contengono una sorgente riconoscibile.
    pub invalid: Vec<String>,
}

fn clean(field: &str) -> &str {
    field.trim().trim_matches('"').trim()
}

pub fn parse_list(text: &str) -> ParsedList {
    let mut name = None;
    let mut inputs = Vec::new();
    let mut invalid = Vec::new();
    let mut seen = HashSet::new();
    let mut first_data = true;
    for raw in text.trim_start_matches('\u{feff}').lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix('#') {
            if name.is_none() && first_data && !rest.trim().is_empty() {
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
                if seen.insert(p.item_id) {
                    inputs.push(c.to_string());
                }
            }
            // Intestazione CSV ("titolo,link"): prima riga con più campi e nessun link.
            None if is_first && fields.len() > 1 && candidate.is_none() => {}
            None => invalid.push(line.to_string()),
        }
    }
    ParsedList { name, inputs, invalid }
}

/// Una raccolta nel formato di `parse_list`: titolo (se c'è) e link di ogni sorgente.
pub fn format_list(name: &str, sources: &[(String, Option<String>)]) -> String {
    let mut out = format!("# {}\n# Esportata da Cain Archive: una sorgente per riga, \"titolo, link\"\n", name.trim());
    for (id, title) in sources {
        let url = format!("https://archive.org/details/{id}");
        match title.as_deref().map(|t| t.replace(['\n', '\r', '\t'], " ")) {
            Some(t) if !t.trim().is_empty() && !t.trim_start().starts_with('#') => out.push_str(&format!("{}, {url}\n", t.trim())),
            _ => out.push_str(&format!("{url}\n")),
        }
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
}
