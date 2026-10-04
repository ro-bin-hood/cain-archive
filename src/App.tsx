import { useCallback, useEffect, useReducer, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, events, type CollectionView, type LibraryState, type ParsedList, type Settings } from "./api";
import { initialQueue, queueReducer, type View } from "./state";
import { TopBar } from "./components/TopBar";
import { Sidebar } from "./components/Sidebar";
import { SearchView } from "./components/SearchView";
import { QueueView } from "./components/QueueView";
import { QueueStrip } from "./components/QueueStrip";
import { AddSourcesDialog } from "./components/AddSourcesDialog";
import { NameDialog } from "./components/NameDialog";
import { CollectionPicker } from "./components/CollectionPicker";
import { SettingsPanel } from "./components/SettingsPanel";
import { LoginDialog } from "./components/LoginDialog";

type Dialog =
  | { kind: "settings" } | { kind: "login" }
  | { kind: "add"; collectionId: string | null; prefill?: ParsedList }
  | { kind: "new" } | { kind: "rename"; collection: CollectionView }
  | { kind: "save"; itemId: string };

export default function App() {
  const [q, dispatch] = useReducer(queueReducer, initialQueue);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [user, setUser] = useState<string | null>(null);
  const [lib, setLib] = useState<LibraryState>({ collections: [], unsaved: [] });
  const [view, setView] = useState<View>({ kind: "search", scope: { kind: "all" } });
  const [query, setQuery] = useState("");
  const [alert, setAlert] = useState<string | null>(null);
  const [dialog, setDialog] = useState<Dialog | null>(null);

  useEffect(() => {
    api.getState().then((s) => {
      setSettings(s.settings);
      setUser(s.user);
      dispatch({ type: "changed", jobs: s.jobs, running: s.running });
    });
    api.libraryState().then(setLib);
    api.takeLibraryWarning().then((w) => w && setAlert(w));
    const subs = [
      events.onChanged((p) => dispatch({ type: "changed", ...p })),
      events.onProgress((snap) => dispatch({ type: "progress", snap })),
      events.onAlert(setAlert),
    ];
    return () => { subs.forEach((p) => p.then((un) => un())); };
  }, []);

  // Tema: "system" lascia decidere a prefers-color-scheme, altrimenti forza chiaro/scuro.
  // Anche lo sfondo nativo della finestra si adegua, così ridimensionando non lampeggia bianco.
  const theme = settings?.theme;
  useEffect(() => {
    if (!theme) return;
    const root = document.documentElement;
    if (theme === "system") delete root.dataset.theme;
    else root.dataset.theme = theme;
    const apply = () => {
      const bg = getComputedStyle(root).getPropertyValue("--bg").trim();
      const [r, g, b] = [1, 3, 5].map((i) => parseInt(bg.slice(i, i + 2), 16));
      getCurrentWindow().setBackgroundColor({ red: r, green: g, blue: b, alpha: 255 }).catch(() => {});
    };
    apply();
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [theme]);

  // Se la raccolta o la sorgente aperta sparisce (eliminata, tolta, rimasta orfana), si torna a
  // "Tutte le sorgenti" invece di restare su una vista che non esiste più.
  useEffect(() => {
    if (view.kind !== "search") return;
    const s = view.scope;
    const exists =
      s.kind === "collection" ? lib.collections.some((c) => c.id === s.id)
      : s.kind === "source" ? lib.unsaved.some((m) => m.item_id === s.item_id) || lib.collections.some((c) => c.sources.some((x) => x.item_id === s.item_id))
      : true;
    if (!exists) setView({ kind: "search", scope: { kind: "all" } });
  }, [lib, view]);

  const onError = useCallback((msg: string) => setAlert(msg), []);

  const updateSettings = (s: Settings) => {
    setSettings(s);
    api.setSettings(s).catch((e) => setAlert(String(e)));
  };

  // Link incollati nella ricerca: si aprono come Non salvate; si seleziona l'ultima aperta.
  const openLinks = async (links: string[]) => {
    const errors: string[] = [];
    let last: string | null = null;
    for (const l of links) {
      try {
        last = (await api.openUnsaved(l)).item_id;
      } catch (e) {
        errors.push(`${l}: ${e}`);
      }
    }
    setLib(await api.libraryState());
    if (last) setView({ kind: "search", scope: { kind: "source", item_id: last } });
    if (errors.length) setAlert(errors.join("\n"));
  };

  // "Importa raccolta": il file letto in Rust precompila la finestra di aggiunta.
  const importList = () => {
    api.pickImportFile().then((list) => {
      if (!list) return;
      if (!list.inputs.length) setAlert("Nessuna sorgente valida nel file");
      else setDialog({ kind: "add", collectionId: null, prefill: list });
    }).catch((e) => setAlert(String(e)));
  };

  const queueCount = q.jobs.filter((j) => ["Queued", "Downloading", "Retrying"].includes(j.status.kind)).length;
  const close = () => setDialog(null);

  if (!settings) return null;
  return (
    <>
      <TopBar user={user} onLogin={() => setDialog({ kind: "login" })} onLogout={() => { api.logout(); setUser(null); }} onSettings={() => setDialog({ kind: "settings" })} />
      <div className="layout">
        <Sidebar
          lib={lib}
          view={view}
          queueCount={queueCount}
          onSelect={setView}
          onLibrary={setLib}
          onError={onError}
          onNewCollection={() => setDialog({ kind: "new" })}
          onImport={importList}
          onRename={(c) => setDialog({ kind: "rename", collection: c })}
          onAddSources={(id) => setDialog({ kind: "add", collectionId: id })}
          onSaveUnsaved={(id) => setDialog({ kind: "save", itemId: id })}
        />
        <main className="main-pane">
          {alert && (
            <div className="banner row">
              <span className="grow" style={{ whiteSpace: "pre-line" }}>{alert}</span>
              <button className="icon-btn" title="Chiudi" onClick={() => setAlert(null)}>✕</button>
            </div>
          )}
          {view.kind === "queue" ? (
            <QueueView jobs={q.jobs} running={q.running} progress={q.progress} />
          ) : (
            <SearchView
              scope={view.scope}
              lib={lib}
              query={query}
              onQuery={setQuery}
              defaultOriginals={settings.default_originals}
              onLibrary={setLib}
              onOpenLinks={openLinks}
              onSaveUnsaved={(id) => setDialog({ kind: "save", itemId: id })}
              onError={onError}
            />
          )}
        </main>
      </div>
      <QueueStrip jobs={q.jobs} running={q.running} progress={q.progress} onOpen={() => setView({ kind: "queue" })} />

      {dialog?.kind === "settings" && <SettingsPanel settings={settings} onChange={updateSettings} onClose={close} />}
      {dialog?.kind === "login" && <LoginDialog onDone={(u) => { setUser(u); close(); }} onClose={close} />}
      {dialog?.kind === "add" && <AddSourcesDialog lib={lib} initialCollection={dialog.collectionId} prefill={dialog.prefill} onLibrary={setLib} onClose={close} />}
      {dialog?.kind === "new" && (
        <NameDialog title="Nuova raccolta" confirm="Crea" onClose={close} onSubmit={async (name) => {
          const st = await api.createCollection(name);
          setLib(st);
          setView({ kind: "search", scope: { kind: "collection", id: st.collections[st.collections.length - 1].id } });
          close();
        }} />
      )}
      {dialog?.kind === "rename" && (
        <NameDialog title="Rinomina raccolta" initial={dialog.collection.name} confirm="Rinomina" onClose={close} onSubmit={async (name) => {
          setLib(await api.renameCollection(dialog.collection.id, name));
          close();
        }} />
      )}
      {dialog?.kind === "save" && (
        <CollectionPicker lib={lib} onClose={close}
          onPick={async (cid) => { setLib(await api.saveUnsaved(dialog.itemId, cid)); close(); }}
          onCreate={async (name) => {
            const st = await api.createCollection(name);
            setLib(await api.saveUnsaved(dialog.itemId, st.collections[st.collections.length - 1].id));
            close();
          }} />
      )}
    </>
  );
}
