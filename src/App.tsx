import { useCallback, useEffect, useReducer, useRef, useState, type MouseEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { api, events, type CollectionView, type LibraryState, type ParsedList, type Settings } from "./api";
import { initialQueue, queueReducer, type View } from "./state";
import { LangContext, SOURCE_LANG, resolveLang, translator } from "./i18n";
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
  | { kind: "new"; then?: (collectionId: string) => Promise<void> }
  | { kind: "newsub"; parent: string } | { kind: "rename"; collection: CollectionView }
  | { kind: "save"; itemId: string };

export default function App() {
  const [q, dispatch] = useReducer(queueReducer, initialQueue);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [user, setUser] = useState<string | null>(null);
  const [lib, setLib] = useState<LibraryState>({ collections: [], unsaved: [] });
  const [view, setView] = useState<View>({ kind: "search", scope: { kind: "all" } });
  const [query, setQuery] = useState("");
  // A translated text or an engine error, translated when shown so it follows the language.
  const [alert, setAlert] = useState<unknown>(null);
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [dragging, setDragging] = useState(false);
  const [pending, setPending] = useState<string[]>([]);
  const [reading, setReading] = useState(false);
  const [sidebarWidth, setSidebarWidth] = useState(() => {
    try { return Number(localStorage.getItem("sidebarWidth")) || 250; } catch { return 250; }
  });
  const dialogRef = useRef<Dialog | null>(null);
  // Last search view, to go back to when the queue is closed.
  const lastSearch = useRef<View>({ kind: "search", scope: { kind: "all" } });
  if (view.kind === "search") lastSearch.current = view;
  const toggleQueue = () => setView((v) => (v.kind === "queue" ? lastSearch.current : { kind: "queue" }));
  const langRef = useRef<string>(SOURCE_LANG);
  dialogRef.current = dialog;

  // List files dropped on the window: imported one at a time, each in its own dialog.
  useEffect(() => {
    const un = getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === "enter" || p.type === "over") setDragging(true);
      else if (p.type === "leave") setDragging(false);
      else if (p.type === "drop") {
        setDragging(false);
        const lists = p.paths.filter((f) => /\.(txt|csv)$/i.test(f));
        if (lists.length < p.paths.length) setAlert(translator(langRef.current).t("app.onlyTxtCsv"));
        if (lists.length && !dialogRef.current) setPending(lists);
      }
    });
    return () => { un.then((f) => f()); };
  }, []);

  useEffect(() => {
    if (dialog || reading || !pending.length) return;
    const [next, ...rest] = pending;
    setPending(rest);
    setReading(true);
    api.readImportFile(next)
      .then((list) => {
        if (!list.inputs.length && !list.sections.some((s) => s.inputs.length)) setAlert(t("app.noValidSourceIn", { file: next }));
        else setDialog({ kind: "add", collectionId: null, prefill: list });
      })
      .catch(setAlert)
      .finally(() => setReading(false));
  }, [dialog, reading, pending]);

  // Draggable sidebar edge (180–520 px); double-click = initial width.
  const saveWidth = (w: number) => {
    setSidebarWidth(w);
    try { localStorage.setItem("sidebarWidth", String(w)); } catch { /* convenience only */ }
  };
  const startResize = (e: MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startW = sidebarWidth;
    let w = startW;
    const move = (ev: globalThis.MouseEvent) => {
      w = Math.min(520, Math.max(180, startW + ev.clientX - startX));
      setSidebarWidth(w);
    };
    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      document.body.classList.remove("resizing");
      saveWidth(w);
    };
    document.body.classList.add("resizing");
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  };

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

  // Theme: "system" lets prefers-color-scheme decide, otherwise light/dark is forced.
  // The native window background follows too, so resizing doesn't flash white.
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

  // If the open collection or source disappears (deleted, removed, orphaned), go back to
  // "All sources" instead of staying on a view that no longer exists.
  useEffect(() => {
    if (view.kind !== "search") return;
    const s = view.scope;
    const exists =
      s.kind === "collection" ? lib.collections.some((c) => c.id === s.id)
      : s.kind === "source" ? lib.unsaved.some((m) => m.item_id === s.item_id) || lib.collections.some((c) => c.sources.some((x) => x.item_id === s.item_id))
      : true;
    if (!exists) setView({ kind: "search", scope: { kind: "all" } });
  }, [lib, view]);

  const onError = useCallback((err: unknown) => setAlert(err), []);

  // Language: "Automatic" follows the system languages, falling back to English.
  const lang = resolveLang(settings?.language ?? "system");
  const { t, te } = translator(lang);
  langRef.current = lang;
  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);

  const updateSettings = (s: Settings) => {
    setSettings(s);
    api.setSettings(s).catch(setAlert);
  };

  // Links pasted into the search bar open as unsaved sources; the last one opened is selected.
  const openLinks = async (links: string[]) => {
    const errors: string[] = [];
    let last: string | null = null;
    for (const l of links) {
      try {
        last = (await api.openUnsaved(l)).item_id;
      } catch (e) {
        errors.push(`${l}: ${te(e)}`);
      }
    }
    setLib(await api.libraryState());
    if (last) setView({ kind: "search", scope: { kind: "source", item_id: last } });
    if (errors.length) setAlert(errors.join("\n"));
  };

  // "Import collection": the file read in Rust prefills the add dialog.
  const importList = () => {
    api.pickImportFile(t("app.listFileType")).then((list) => {
      if (!list) return;
      if (!list.inputs.length && !list.sections.some((s) => s.inputs.length)) setAlert(t("app.noValidSource"));
      else setDialog({ kind: "add", collectionId: null, prefill: list });
    }).catch(setAlert);
  };

  const queueCount = q.jobs.filter((j) => ["Queued", "Downloading", "Retrying"].includes(j.status.kind)).length;
  const close = () => setDialog(null);

  if (!settings) return null;
  return (
    <LangContext.Provider value={lang}>
      <TopBar user={user} onLogin={() => setDialog({ kind: "login" })} onLogout={() => { api.logout(); setUser(null); }} onSettings={() => setDialog({ kind: "settings" })} />
      <div className="layout">
        <Sidebar
          width={sidebarWidth}
          lib={lib}
          view={view}
          queueCount={queueCount}
          onSelect={setView}
          onLibrary={setLib}
          onError={onError}
          onNewCollection={() => setDialog({ kind: "new" })}
          onImport={importList}
          onNewCollectionThen={(then) => setDialog({ kind: "new", then })}
          onNewSubcollection={(parent) => setDialog({ kind: "newsub", parent })}
          onToggleQueue={toggleQueue}
          onRename={(c) => setDialog({ kind: "rename", collection: c })}
          onAddSources={(id) => setDialog({ kind: "add", collectionId: id })}
          onSaveUnsaved={(id) => setDialog({ kind: "save", itemId: id })}
        />
        <div className="resizer" title={t("app.resizeHint")} onMouseDown={startResize} onDoubleClick={() => saveWidth(250)} />
        <main className="main-pane">
          {alert != null && (
            <div className="banner row">
              <span className="grow" style={{ whiteSpace: "pre-line" }}>{te(alert)}</span>
              <button className="icon-btn" title={t("common.close")} onClick={() => setAlert(null)}>✕</button>
            </div>
          )}
          {view.kind === "queue" ? (
            <QueueView jobs={q.jobs} running={q.running} progress={q.progress} onClose={() => setView(lastSearch.current)} />
          ) : (
            <SearchView
              scope={view.scope}
              lib={lib}
              jobs={q.jobs}
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
      {dragging && <div className="drop-overlay"><div>⤓ {t("app.dropToImport")}</div></div>}
      <QueueStrip jobs={q.jobs} running={q.running} progress={q.progress} onOpen={toggleQueue} />

      {dialog?.kind === "settings" && <SettingsPanel settings={settings} onChange={updateSettings} onClose={close} />}
      {dialog?.kind === "login" && <LoginDialog onDone={(u) => { setUser(u); close(); }} onClose={close} />}
      {dialog?.kind === "add" && <AddSourcesDialog lib={lib} initialCollection={dialog.collectionId} prefill={dialog.prefill} onLibrary={setLib} onClose={close} />}
      {dialog?.kind === "new" && (
        <NameDialog title={t("dialog.newCollection")} confirm={t("common.create")} onClose={close} onSubmit={async (name) => {
          const st = await api.createCollection(name);
          setLib(st);
          const id = st.collections[st.collections.length - 1].id;
          if (dialog.then) await dialog.then(id);
          else setView({ kind: "search", scope: { kind: "collection", id } });
          close();
        }} />
      )}
      {dialog?.kind === "newsub" && (
        <NameDialog title={t("dialog.newSubcollection")} confirm={t("common.create")} onClose={close} onSubmit={async (name) => {
          const st = await api.createSubcollection(dialog.parent, name);
          setLib(st);
          const sub = st.collections.find((c) => c.parent === dialog.parent && c.name.toLowerCase() === name.trim().toLowerCase());
          if (sub) setView({ kind: "search", scope: { kind: "collection", id: sub.id } });
          close();
        }} />
      )}
      {dialog?.kind === "rename" && (
        <NameDialog title={t("dialog.renameCollection")} initial={dialog.collection.name} confirm={t("common.rename")} onClose={close} onSubmit={async (name) => {
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
    </LangContext.Provider>
  );
}
