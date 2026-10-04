//! Messaggi del motore in italiano o inglese. La lingua la decide l'interfaccia (che risolve
//! "Automatico" con la lingua di sistema) e la comunica con il comando `set_ui_language`.

use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn is_english_code(lang: &str) -> bool {
    lang == "en"
}

pub fn set_language(lang: &str) {
    ENGLISH.store(is_english_code(lang), Ordering::Relaxed);
}

pub fn pick<'a>(english: bool, it: &'a str, en: &'a str) -> &'a str {
    if english { en } else { it }
}

/// Il testo nella lingua attiva.
pub fn m<'a>(it: &'a str, en: &'a str) -> &'a str {
    pick(ENGLISH.load(Ordering::Relaxed), it, en)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_text_for_the_language() {
        assert_eq!(pick(false, "Raccolta non trovata", "Collection not found"), "Raccolta non trovata");
        assert_eq!(pick(true, "Raccolta non trovata", "Collection not found"), "Collection not found");
    }

    #[test]
    fn only_en_switches_to_english() {
        assert!(is_english_code("en"));
        assert!(!is_english_code("it"));
        assert!(!is_english_code("system"));
    }
}
