import { useEffect, useReducer, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, events, type Item, type Settings } from "./api";
import { initialQueue, queueReducer } from "./state";
import { TopBar } from "./components/TopBar";
import { LinkInput } from "./components/LinkInput";
import { ItemCard } from "./components/ItemCard";
import { QueueView } from "./components/QueueView";
import { SettingsPanel } from "./components/SettingsPanel";
import { LoginDialog } from "./components/LoginDialog";

type Card = { key: number; input: string; item: Item | null; error: string | null };
let nextKey = 1;

export default function App() {
  const [q, dispatch] = useReducer(queueReducer, initialQueue);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [user, setUser] = useState<string | null>(null);
  const [cards, setCards] = useState<Card[]>([]);
  const [busy, setBusy] = useState(false);
  const [alert, setAlert] = useState<string | null>(null);
  const [panel, setPanel] = useState<"settings" | "login" | null>(null);

  useEffect(() => {
    api.getState().then((s) => {
      setSettings(s.settings);
      setUser(s.user);
      dispatch({ type: "changed", jobs: s.jobs, running: s.running });
    });
    const subs = [
      events.onChanged((p) => dispatch({ type: "changed", ...p })),
      events.onProgress((snap) => dispatch({ type: "progress", snap })),
      events.onAlert(setAlert),
    ];
    return () => { subs.forEach((p) => p.then((un) => un())); };
  }, []);

  const analyze = async (text: string) => {
    setBusy(true);
    try {
      const res = await api.analyze(text);
      setCards((c) => [...res.map((r) => ({ key: nextKey++, ...r })), ...c]);
      return true;
    } catch (e) {
      setAlert(String(e));
      return false;
    } finally {
      setBusy(false);
    }
  };
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

  const closeCard = (key: number) => setCards((c) => c.filter((x) => x.key !== key));
  const updateSettings = (s: Settings) => {
    setSettings(s);
    api.setSettings(s).catch((e) => setAlert(String(e)));
  };

  if (!settings) return null;
  return (
    <>
      <TopBar
        user={user}
        onLogin={() => setPanel("login")}
        onLogout={() => { api.logout(); setUser(null); }}
        onSettings={() => setPanel("settings")}
      />
      <div className="app">
        {alert && (
          <div className="banner row">
            <span className="grow">{alert}</span>
            <button className="icon-btn" title="Chiudi" onClick={() => setAlert(null)}>✕</button>
          </div>
        )}
        <LinkInput busy={busy} onAnalyze={analyze} />
        {cards.map((c) =>
          c.item ? (
            <ItemCard
              key={c.key}
              item={c.item}
              defaultOriginals={settings.default_originals}
              defaultExts={settings.default_exts}
              onAdd={(files) => { api.enqueue(files); closeCard(c.key); }}
              onClose={() => closeCard(c.key)}
            />
          ) : (
            <div key={c.key} className="banner row">
              <span className="grow">
                <span className="name">{c.input}</span>
                <br />
                <span className="err small">{c.error}</span>
              </span>
              <button className="icon-btn" title="Chiudi" onClick={() => closeCard(c.key)}>✕</button>
            </div>
          ),
        )}
        <QueueView jobs={q.jobs} running={q.running} progress={q.progress} />
        {panel === "settings" && <SettingsPanel settings={settings} onChange={updateSettings} onClose={() => setPanel(null)} />}
        {panel === "login" && <LoginDialog onDone={(u) => { setUser(u); setPanel(null); }} onClose={() => setPanel(null)} />}
      </div>
    </>
  );
}
