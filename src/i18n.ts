import { createContext, useContext } from "react";

/** Lingua effettiva dell'interfaccia. L'impostazione "system" si risolve con la lingua del sistema. */
export type Lang = "it" | "en";

export function resolveLang(setting: string): Lang {
  if (setting === "it" || setting === "en") return setting;
  return navigator.language.toLowerCase().startsWith("it") ? "it" : "en";
}

/** Traduzioni inglesi; la chiave è il testo italiano usato nel codice. `{x}` è un segnaposto. */
export const EN: Record<string, string> = {
  // Barra del titolo
  "Accedi": "Log in",
  "Esci": "Log out",
  "Impostazioni": "Settings",
  "Riduci a icona": "Minimize",
  "Ingrandisci": "Maximize",
  "Chiudi": "Close",
  // Barra laterale
  "Non salvate": "Unsaved",
  "Salva in una raccolta": "Save to a collection",
  "Tutte le sorgenti": "All sources",
  "Raccolte": "Collections",
  "Importa raccolta da file": "Import collection from file",
  "Nuova raccolta": "New collection",
  "Altro": "More",
  "Aggiungi sorgenti": "Add sources",
  "Aggiorna tutte": "Refresh all",
  "Rinomina": "Rename",
  "Esporta…": "Export…",
  "Elimina": "Delete",
  "Conferma eliminazione": "Confirm delete",
  "Includi nella ricerca della raccolta": "Include in the collection search",
  "Aggiorna": "Refresh",
  "Togli dalla raccolta": "Remove from collection",
  "Coda": "Queue",
  "Aggiornamento non riuscito per {list}": "Refresh failed for {list}",
  "Raccolta esportata in {path}": "Collection exported to {path}",
  // Ricerca
  "Cerca nei file… oppure incolla un link archive.org": "Search files… or paste an archive.org link",
  "Tutte le sorgenti · {n} sorgenti": "All sources · {n} sources",
  "{name} · {n} sorgenti": "{name} · {n} sources",
  "{n} risultati": "{n} results",
  "mostrati {n}": "showing {n}",
  "Tutti": "All",
  "Nessuno": "None",
  "Seleziona i risultati non ancora scaricati né in coda": "Select results not yet downloaded or queued",
  "Aggiungi {n} alla coda": "Add {n} to queue",
  "Solo originali": "Originals only",
  "Ordina i risultati": "Sort results",
  "Nome": "Name",
  "Più grandi prima": "Largest first",
  "Più piccoli prima": "Smallest first",
  "{n} file · {size} · aggiornata il {date}": "{n} files · {size} · updated {date}",
  "Aggiornamento…": "Refreshing…",
  "Salva": "Save",
  "Crea una raccolta o incolla un link archive.org per iniziare": "Create a collection or paste an archive.org link to get started",
  "Aggiungi sorgenti con ⋯ → Aggiungi sorgenti": "Add sources with ⋯ → Add sources",
  "Tutte le sorgenti di questa raccolta sono escluse dalla ricerca: spunta quelle da includere": "All sources in this collection are excluded from search: tick the ones to include",
  "Nessuna sorgente salvata: le Non salvate si cercano selezionandole a sinistra": "No saved sources: search unsaved ones by selecting them on the left",
  "Le raccolte sono vuote: aggiungi sorgenti con ⋯ → Aggiungi sorgenti": "Your collections are empty: add sources with ⋯ → Add sources",
  "Nessun file corrisponde alla ricerca": "No file matches your search",
  "scaricato": "downloaded",
  "in coda": "queued",
  "Già presente nella cartella di destinazione": "Already in the destination folder",
  "Già nella coda": "Already in the queue",
  // Aggiunta e importazione
  "Importa raccolta": "Import collection",
  "Un link archive.org o un identificatore per riga": "One archive.org link or identifier per line",
  "{n} riga non riconosciuta nel file, ignorata": "{n} unrecognized line in the file, skipped",
  "{n} righe non riconosciute nel file, ignorate": "{n} unrecognized lines in the file, skipped",
  "Raccolta": "Collection",
  "Nuova raccolta…": "New collection…",
  "Nome della nuova raccolta": "New collection name",
  "In attesa…": "Waiting…",
  "Aggiunta · {n} file": "Added · {n} files",
  "Aggiunta…": "Adding…",
  "Importa": "Import",
  "Aggiungi": "Add",
  "Crea": "Create",
  "Rinomina raccolta": "Rename collection",
  "Annulla": "Cancel",
  "Crea e salva": "Create and save",
  // Coda
  "{n} in corso": "{n} downloading",
  "Coda: {n} file": "Queue: {n} files",
  "Coda vuota": "Queue empty",
  "Apri la coda": "Open the queue",
  "Coda · {n} file": "Queue · {n} files",
  "{done} di {total}": "{done} of {total}",
  "Pulisci completati": "Clear completed",
  "Avvia": "Start",
  "La coda è vuota: cerca dei file e aggiungili con «Aggiungi alla coda».": "The queue is empty: search for files and add them with “Add to queue”.",
  "In coda": "Queued",
  "Avvio…": "Starting…",
  "Riprovo tra {n} s": "Retrying in {n} s",
  "Riprovo…": "Retrying…",
  "In pausa": "Paused",
  "Completato": "Done",
  "Pausa": "Pause",
  "Riprendi": "Resume",
  "Riprova": "Retry",
  "Apri cartella": "Open folder",
  "Rimuovi": "Remove",
  // Impostazioni
  "Tema": "Theme",
  "Automatico": "Automatic",
  "Chiaro": "Light",
  "Scuro": "Dark",
  "Lingua": "Language",
  "Cartella di destinazione": "Destination folder",
  "Sfoglia…": "Browse…",
  "Download in parallelo": "Parallel downloads",
  "3–5 va bene; se compaiono molti \"Riprovo\", abbassa.": "3–5 works well; lower it if you see many \"Retrying\".",
  "Filtri predefiniti": "Default filters",
  "\"Solo originali\" decide lo stato iniziale del filtro nella ricerca. La cartella vale per i file aggiunti da ora in poi.": "\"Originals only\" sets the initial state of the search filter. The folder applies to files added from now on.",
  // Login
  "Accedi a archive.org": "Log in to archive.org",
  "Serve per gli item visibili solo agli utenti registrati. La password non viene salvata.": "Needed for items visible only to registered users. Your password is never stored.",
  "Accesso…": "Logging in…",
  // App
  "Si possono importare solo file .txt o .csv": "Only .txt or .csv files can be imported",
  "Nessuna sorgente valida in {file}": "No valid source in {file}",
  "Nessuna sorgente valida nel file": "No valid source in the file",
  "Trascina per ridimensionare · doppio clic per ripristinare": "Drag to resize · double-click to reset",
  "Rilascia per importare la raccolta": "Drop to import the collection",
  "mai": "never",
};

export function translate(lang: Lang, it: string, vars?: Record<string, string | number>): string {
  let s = lang === "en" ? EN[it] ?? it : it;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.split(`{${k}}`).join(String(v));
  return s;
}

export const LangContext = createContext<Lang>("it");

export type T = (it: string, vars?: Record<string, string | number>) => string;

/** Funzione di traduzione e locale per i numeri/le date nella lingua corrente. */
export function useT(): { t: T; locale: string } {
  const lang = useContext(LangContext);
  return { t: (it, vars) => translate(lang, it, vars), locale: lang === "en" ? "en-US" : "it-IT" };
}
