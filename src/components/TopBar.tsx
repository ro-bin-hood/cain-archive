import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import logo from "../assets/logo.svg";

type Props = { user: string | null; onLogin: () => void; onLogout: () => void; onSettings: () => void };

const win = getCurrentWindow();

/** Barra del titolo dell'app: la finestra non ha la barra di Windows. Le zone vuote trascinano
 *  la finestra (doppio clic = ingrandisci), i pulsanti a destra sostituiscono quelli di sistema. */
export function TopBar({ user, onLogin, onLogout, onSettings }: Props) {
  const [menu, setMenu] = useState(false);
  return (
    <header className="titlebar" data-tauri-drag-region>
      <img src={logo} className="logo" alt="" data-tauri-drag-region />
      <span className="title" data-tauri-drag-region>Cain Archive</span>
      <div className="grow" data-tauri-drag-region />
      <div style={{ position: "relative" }}>
        {user ? (
          <button className="pill" onClick={() => setMenu((m) => !m)}>● {user}</button>
        ) : (
          <button className="pill" onClick={onLogin}>Accedi</button>
        )}
        {menu && user && (
          <div className="menu">
            <button onClick={() => { setMenu(false); onLogout(); }}>Esci</button>
          </div>
        )}
      </div>
      <button className="icon-btn" title="Impostazioni" onClick={onSettings}>⚙</button>
      <div className="win-controls">
        <button title="Riduci a icona" onClick={() => win.minimize()}>
          <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 5h10" stroke="currentColor" /></svg>
        </button>
        <button title="Ingrandisci" onClick={() => win.toggleMaximize()}>
          <svg width="10" height="10" viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" /></svg>
        </button>
        <button className="close" title="Chiudi" onClick={() => win.close()}>
          <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 0l10 10M10 0L0 10" stroke="currentColor" /></svg>
        </button>
      </div>
    </header>
  );
}
