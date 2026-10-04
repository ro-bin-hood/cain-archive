import { api, type Settings, type Theme } from "../api";

const THEMES: [Theme, string][] = [["system", "Automatico"], ["light", "Chiaro"], ["dark", "Scuro"]];

type Props = { settings: Settings; onChange: (s: Settings) => void; onClose: () => void };

export function SettingsPanel({ settings, onChange, onClose }: Props) {
  const set = (patch: Partial<Settings>) => onChange({ ...settings, ...patch });
  const browse = async () => {
    const p = await api.pickFolder();
    if (p) set({ out_dir: p });
  };
  return (
    <div className="overlay" onClick={onClose}>
      <aside className="drawer" onClick={(e) => e.stopPropagation()}>
        <div className="row">
          <b className="title">Impostazioni</b>
          <div className="grow" />
          <button className="icon-btn" title="Chiudi" onClick={onClose}>✕</button>
        </div>
        <div className="stack">
          <span className="section-title">Tema</span>
          <div className="segmented">
            {THEMES.map(([v, label]) => (
              <button key={v} className={settings.theme === v ? "on" : ""} onClick={() => set({ theme: v })}>{label}</button>
            ))}
          </div>
        </div>
        <div className="stack">
          <span className="section-title">Cartella di destinazione</span>
          <div className="row">
            <input className="field grow" readOnly value={settings.out_dir} title={settings.out_dir} />
            <button className="btn small ghost" onClick={browse}>Sfoglia…</button>
          </div>
        </div>
        <label className="stack">
          <span className="section-title">Download in parallelo</span>
          <select className="field" value={settings.workers} onChange={(e) => set({ workers: Number(e.target.value) })}>
            {[1, 2, 3, 4, 5, 6, 7, 8].map((n) => <option key={n} value={n}>{n}</option>)}
          </select>
          <span className="muted small">3–5 va bene; se compaiono molti "Riprovo", abbassa.</span>
        </label>
        <div className="stack">
          <span className="section-title">Filtri predefiniti</span>
          <label className="row">
            <input type="checkbox" className="check" checked={settings.default_originals} onChange={(e) => set({ default_originals: e.target.checked })} />
            Solo originali
          </label>
          <input className="field" placeholder="Estensioni, es. pdf, mp3 (vuoto = tutte)" value={settings.default_exts} onChange={(e) => set({ default_exts: e.target.value })} />
        </div>
        <span className="muted small">La cartella vale per i file aggiunti da ora in poi; i filtri per i prossimi item analizzati.</span>
      </aside>
    </div>
  );
}
