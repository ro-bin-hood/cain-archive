import { api, type Settings, type Theme } from "../api";
import { LANGUAGES, useT, type MessageKey } from "../i18n";

const THEMES: [Theme, MessageKey][] = [["system", "settings.themeSystem"], ["light", "settings.themeLight"], ["dark", "settings.themeDark"]];

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
          <b className="title">{t("titlebar.settings")}</b>
          <div className="grow" />
          <button className="icon-btn" title={t("common.close")} onClick={onClose}>✕</button>
        </div>
        <div className="stack">
          <span className="section-title">{t("settings.theme")}</span>
          <div className="segmented">
            {THEMES.map(([v, label]) => (
              <button key={v} className={settings.theme === v ? "on" : ""} onClick={() => set({ theme: v })}>{t(label)}</button>
            ))}
          </div>
        </div>
        <div className="stack">
          <span className="section-title">{t("settings.language")}</span>
          {/* Language names stay in their own language, so they are always recognizable. */}
          <select className="field" value={settings.language} onChange={(e) => set({ language: e.target.value })}>
            <option value="system">{t("settings.languageSystem")}</option>
            {LANGUAGES.map(([code, name]) => <option key={code} value={code}>{name}</option>)}
          </select>
        </div>
        <div className="stack">
          <span className="section-title">{t("settings.outDir")}</span>
          <div className="row">
            <input className="field grow" readOnly value={settings.out_dir} title={settings.out_dir} />
            <button className="btn small ghost" onClick={browse}>{t("settings.browse")}</button>
          </div>
        </div>
        <label className="stack">
          <span className="section-title">{t("settings.workers")}</span>
          <select className="field" value={settings.workers} onChange={(e) => set({ workers: Number(e.target.value) })}>
            {[1, 2, 3, 4, 5, 6, 7, 8].map((n) => <option key={n} value={n}>{n}</option>)}
          </select>
          <span className="muted small">{t("settings.workersHint")}</span>
        </label>
        <div className="stack">
          <span className="section-title">{t("settings.defaultFilters")}</span>
          <label className="row">
            <input type="checkbox" className="check" checked={settings.default_originals} onChange={(e) => set({ default_originals: e.target.checked })} />
            {t("search.originalsOnly")}
          </label>
        </div>
        <span className="muted small">{t("settings.footnote")}</span>
      </aside>
    </div>
  );
}
