import { api, type Language, type Settings, type Theme } from "../api";
import { useT } from "../i18n";

const THEMES: [Theme, string][] = [["system", "Automatico"], ["light", "Chiaro"], ["dark", "Scuro"]];
// Language names stay in their own language, so they are always recognizable.
const LANGUAGES: [Language, string][] = [["system", "Automatico"], ["it", "Italiano"], ["en", "English"]];

type Props = { settings: Settings; onChange: (s: Settings) => void; onClose: () => void };

export function SettingsPanel({ settings, onChange, onClose }: Props) {
  const set = (patch: Partial<Settings>) => onChange({ ...settings, ...patch });
  const { t } = useT();
  const browse = async () => {
    const p = await api.pickFolder();
    if (p) set({ out_dir: p });
  };
  return (
    <div className="overlay" onClick={onClose}>
      <aside className="drawer" onClick={(e) => e.stopPropagation()}>
        <div className="row">
          <b className="title">{t("Impostazioni")}</b>
          <div className="grow" />
          <button className="icon-btn" title={t("Chiudi")} onClick={onClose}>✕</button>
        </div>
        <div className="stack">
          <span className="section-title">{t("Tema")}</span>
          <div className="segmented">
            {THEMES.map(([v, label]) => (
              <button key={v} className={settings.theme === v ? "on" : ""} onClick={() => set({ theme: v })}>{t(label)}</button>
            ))}
          </div>
        </div>
        <div className="stack">
          <span className="section-title">{t("Lingua")}</span>
          <div className="segmented">
            {LANGUAGES.map(([v, label]) => (
              <button key={v} className={settings.language === v ? "on" : ""} onClick={() => set({ language: v })}>{v === "system" ? t(label) : label}</button>
            ))}
          </div>
        </div>
        <div className="stack">
          <span className="section-title">{t("Cartella di destinazione")}</span>
          <div className="row">
            <input className="field grow" readOnly value={settings.out_dir} title={settings.out_dir} />
            <button className="btn small ghost" onClick={browse}>{t("Sfoglia…")}</button>
          </div>
        </div>
        <label className="stack">
          <span className="section-title">{t("Download in parallelo")}</span>
          <select className="field" value={settings.workers} onChange={(e) => set({ workers: Number(e.target.value) })}>
            {[1, 2, 3, 4, 5, 6, 7, 8].map((n) => <option key={n} value={n}>{n}</option>)}
          </select>
          <span className="muted small">{t("3–5 va bene; se compaiono molti \"Riprovo\", abbassa.")}</span>
        </label>
        <div className="stack">
          <span className="section-title">{t("Filtri predefiniti")}</span>
          <label className="row">
            <input type="checkbox" className="check" checked={settings.default_originals} onChange={(e) => set({ default_originals: e.target.checked })} />
            {t("Solo originali")}
          </label>
        </div>
        <span className="muted small">{t("\"Solo originali\" decide lo stato iniziale del filtro nella ricerca. La cartella vale per i file aggiunti da ora in poi.")}</span>
      </aside>
    </div>
  );
}
