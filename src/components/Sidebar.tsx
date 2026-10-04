import { useState } from "react";
import { api, type CollectionView, type LibraryState, type SourceMeta } from "../api";
import type { View } from "../state";
import { scopeKey } from "../state";
import { useT } from "../i18n";

type Props = {
  width: number;
  lib: LibraryState;
  view: View;
  queueCount: number;
  onSelect: (v: View) => void;
  onLibrary: (s: LibraryState) => void;
  onError: (msg: string) => void;
  onNewCollection: () => void;
  onImport: () => void;
  onRename: (c: CollectionView) => void;
  onAddSources: (collectionId: string) => void;
  onSaveUnsaved: (itemId: string) => void;
};

export function Sidebar({ width, lib, view, queueCount, onSelect, onLibrary, onError, onNewCollection, onImport, onRename, onAddSources, onSaveUnsaved }: Props) {
  const { t } = useT();
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [menu, setMenu] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState<Set<string>>(new Set());

  const current = view.kind === "search" ? scopeKey(view.scope) : "queue";
  const select = (v: View) => { setMenu(null); onSelect(v); };
  const reload = () => api.libraryState().then(onLibrary);
  const fail = (e: unknown) => onError(String(e));
  const toggle = (id: string) => setCollapsed((s) => { const n = new Set(s); if (n.has(id)) n.delete(id); else n.add(id); return n; });

  const refresh = async (ids: string[]) => {
    setMenu(null);
    setRefreshing((s) => new Set([...s, ...ids]));
    const errors: string[] = [];
    for (const id of ids) {
      await api.refreshSource(id).catch((e) => errors.push(`${id}: ${e}`));
      setRefreshing((s) => { const n = new Set(s); n.delete(id); return n; });
    }
    await reload();
    if (errors.length) onError(t("Aggiornamento non riuscito per {list}", { list: errors.join("; ") }));
  };

  const removeCollection = async (c: CollectionView) => {
    if (confirmDelete !== c.id) { setConfirmDelete(c.id); return; }
    setMenu(null);
    setConfirmDelete(null);
    try {
      onLibrary(await api.deleteCollection(c.id));
      if (current === `c:${c.id}`) onSelect({ kind: "search", scope: { kind: "all" } });
    } catch (e) { fail(e); }
  };

  const sourceLabel = (m: SourceMeta) => m.title || m.item_id;
  const warn = (m: SourceMeta) => m.error && <span className="warn" title={m.error}>⚠</span>;
  const spin = (id: string) => refreshing.has(id) && <span className="muted small">↻</span>;

  return (
    <nav className="sidebar" style={{ width }} onMouseLeave={() => { setMenu(null); setConfirmDelete(null); }}>
      {lib.unsaved.length > 0 && (
        <>
          <div className="side-head">{t("Non salvate")}</div>
          {lib.unsaved.map((m) => (
            <div key={m.item_id} className={`side-item ${current === `s:${m.item_id}` ? "on" : ""}`} onClick={() => select({ kind: "search", scope: { kind: "source", item_id: m.item_id } })}>
              <span className="grow name" title={m.item_id}>{sourceLabel(m)}</span>
              {warn(m)}
              <span className="actions">
                <button className="small-btn" title={t("Salva in una raccolta")} onClick={(e) => { e.stopPropagation(); onSaveUnsaved(m.item_id); }}>★</button>
                <button className="small-btn" title={t("Chiudi")} onClick={(e) => {
                  e.stopPropagation();
                  api.closeUnsaved(m.item_id).then(onLibrary).catch(fail);
                  if (current === `s:${m.item_id}`) onSelect({ kind: "search", scope: { kind: "all" } });
                }}>✕</button>
              </span>
            </div>
          ))}
        </>
      )}

      <div className={`side-item ${current === "all" ? "on" : ""}`} onClick={() => select({ kind: "search", scope: { kind: "all" } })}>🌐 {t("Tutte le sorgenti")}</div>

      <div className="side-head">
        {t("Raccolte")}
        <span className="row" style={{ gap: 2 }}>
          <button className="small-btn" title={t("Importa raccolta da file")} onClick={onImport}>⤓</button>
          <button className="small-btn" title={t("Nuova raccolta")} onClick={onNewCollection}>＋</button>
        </span>
      </div>
      {lib.collections.map((c) => (
        <div key={c.id}>
          <div className={`side-item ${current === `c:${c.id}` ? "on" : ""}`} onClick={() => select({ kind: "search", scope: { kind: "collection", id: c.id } })}>
            <button className="small-btn" onClick={(e) => { e.stopPropagation(); toggle(c.id); }}>{collapsed.has(c.id) ? "▸" : "▾"}</button>
            <span className="grow name">{c.name}</span>
            <span className="muted small">{c.sources.length}</span>
            <span className={`actions ${menu === `c:${c.id}` ? "open" : ""}`}>
              <button className="small-btn" title={t("Altro")} onClick={(e) => { e.stopPropagation(); setConfirmDelete(null); setMenu(menu === `c:${c.id}` ? null : `c:${c.id}`); }}>⋯</button>
            </span>
            {menu === `c:${c.id}` && (
              <div className="menu" onClick={(e) => e.stopPropagation()}>
                <button onClick={() => { setMenu(null); onAddSources(c.id); }}>{t("Aggiungi sorgenti")}</button>
                <button disabled={!c.sources.length} onClick={() => refresh(c.sources.map((s) => s.item_id))}>{t("Aggiorna tutte")}</button>
                <button onClick={() => { setMenu(null); onRename(c); }}>{t("Rinomina")}</button>
                <button disabled={!c.sources.length} onClick={() => {
                  setMenu(null);
                  api.exportCollection(c.id).then((p) => p && onError(t("Raccolta esportata in {path}", { path: p }))).catch(fail);
                }}>{t("Esporta…")}</button>
                <button className="err" onClick={() => removeCollection(c)}>{confirmDelete === c.id ? t("Conferma eliminazione") : t("Elimina")}</button>
              </div>
            )}
          </div>
          {!collapsed.has(c.id) && c.sources.map((s) => {
            const key = `${c.id}:${s.item_id}`;
            return (
              <div key={key} className={`side-item sub ${current === `s:${s.item_id}` ? "on" : ""}`} onClick={() => select({ kind: "search", scope: { kind: "source", item_id: s.item_id } })}>
                <input type="checkbox" className="check" title={t("Includi nella ricerca della raccolta")} checked={s.included} onClick={(e) => e.stopPropagation()}
                  onChange={(e) => api.setIncluded(c.id, s.item_id, e.target.checked).then(onLibrary).catch(fail)} />
                <span className={`grow name ${s.included ? "" : "muted"}`} title={s.item_id}>{sourceLabel(s)}</span>
                {spin(s.item_id)}
                {warn(s)}
                <span className={`actions ${menu === key ? "open" : ""}`}>
                  <button className="small-btn" title={t("Altro")} onClick={(e) => { e.stopPropagation(); setMenu(menu === key ? null : key); }}>⋯</button>
                </span>
                {menu === key && (
                  <div className="menu" onClick={(e) => e.stopPropagation()}>
                    <button onClick={() => refresh([s.item_id])}>{t("Aggiorna")}</button>
                    <button onClick={() => {
                      setMenu(null);
                      api.removeSource(c.id, s.item_id).then(onLibrary).catch(fail);
                      if (current === `s:${s.item_id}`) onSelect({ kind: "search", scope: { kind: "collection", id: c.id } });
                    }}>{t("Togli dalla raccolta")}</button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      ))}

      <div className="grow" />
      <div className={`side-item ${current === "queue" ? "on" : ""}`} onClick={() => select({ kind: "queue" })}>
        ⬇ {t("Coda")} <span className="grow" />{queueCount > 0 && <span className="badge">{queueCount}</span>}
      </div>
    </nav>
  );
}
