import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import logo from "../assets/logo.svg";
import { useT } from "../i18n";

type Props = { user: string | null; onLogin: () => void; onLogout: () => void; onSettings: () => void };

const win = getCurrentWindow();

/** The app's title bar: the window has no Windows title bar. Empty areas drag
 *  the window (double-click = maximize), the buttons on the right replace the system ones. */
export function TopBar({ user, onLogin, onLogout, onSettings }: Props) {
  const [menu, setMenu] = useState(false);
  const { t } = useT();
  return (
    <header className="titlebar" data-tauri-drag-region>
      <img src={logo} className="logo" alt="" data-tauri-drag-region />
      <span className="title" data-tauri-drag-region>Cain Archive</span>
      <div className="grow" data-tauri-drag-region />
      <div style={{ position: "relative" }}>
        {user ? (
          <button className="pill" onClick={() => setMenu((m) => !m)}>● {user}</button>
        ) : (
          <button className="pill" onClick={onLogin}>{t("titlebar.logIn")}</button>
        )}
        {menu && user && (
          <div className="menu">
            <button onClick={() => { setMenu(false); onLogout(); }}>{t("titlebar.logOut")}</button>
          </div>
        )}
      </div>
      <button className="icon-btn" title={t("titlebar.settings")} onClick={onSettings}>⚙</button>
      <div className="win-controls">
        <button title={t("titlebar.minimize")} onClick={() => win.minimize()}>
          <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 5h10" stroke="currentColor" /></svg>
        </button>
        <button title={t("titlebar.maximize")} onClick={() => win.toggleMaximize()}>
          <svg width="10" height="10" viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" /></svg>
        </button>
        <button className="close" title={t("common.close")} onClick={() => win.close()}>
          <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 0l10 10M10 0L0 10" stroke="currentColor" /></svg>
        </button>
      </div>
    </header>
  );
}
