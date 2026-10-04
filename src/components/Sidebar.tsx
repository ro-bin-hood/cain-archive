import { useEffect, useState, type MouseEvent, type ReactNode } from "react";
import { api, type CollectionView, type LibraryState, type SourceMeta } from "../api";
import type { View } from "../state";
import { scopeKey } from "../state";
import { useT } from "../i18n";
import { collectionLabel, orderedCollections } from "../util";

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
  /** Asks for a new collection name, creates it, then runs `then` with its id. */
  onNewCollectionThen: (then: (collectionId: string) => Promise<void>) => void;
  onNewSubcollection: (parentId: string) => void;
  /** Opens the queue, or goes back to the previous search if the queue is already open. */
  onToggleQueue: () => void;
};

/** Selected sources (always within one collection) and the open context menu. */
type Picked = { cid: string; ids: Set<string>; anchor: string };
type Ctx = { x: number; y: number; cid: string; ids: string[] };

export function Sidebar({ width, lib, view, queueCount, onSelect, onLibrary, onError, onNewCollection, onImport, onRename, onAddSources, onSaveUnsaved, onNewCollectionThen, onNewSubcollection, onToggleQueue }: Props) {
  const { t } = useT();
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [menu, setMenu] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState<Set<string>>(new Set());
  const [picked, setPicked] = useState<Picked | null>(null);
  const [ctx, setCtx] = useState<Ctx | null>(null);

  // Esc closes the context menu and clears the selection.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") { setCtx(null); setPicked(null); } };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Clicking a source: Ctrl adds/removes, Shift selects the range from the last clicked one,
  // a plain click opens the source and makes it the only selected one.
  const clickSource = (e: MouseEvent, c: CollectionView, id: string) => {
    if ((e.ctrlKey || e.metaKey) && picked?.cid === c.id) {
      const ids = new Set(picked.ids);
      if (ids.has(id)) ids.delete(id); else ids.add(id);
      setPicked({ cid: c.id, ids, anchor: id });
      return;
    }
    if (e.shiftKey && picked?.cid === c.id) {
      const order = c.sources.map((s) => s.item_id);
      const [a, b] = [order.indexOf(picked.anchor), order.indexOf(id)].sort((x, y) => x - y);
      setPicked({ cid: c.id, ids: new Set(order.slice(a, b + 1)), anchor: picked.anchor });
      return;
    }
    setPicked({ cid: c.id, ids: new Set([id]), anchor: id });
    select({ kind: "search", scope: { kind: "source", item_id: id } });
  };

  const openCtx = (e: MouseEvent, c: CollectionView, id: string) => {
    e.preventDefault();
    let ids = picked?.cid === c.id && picked.ids.has(id) ? picked.ids : new Set([id]);
    if (ids !== picked?.ids) setPicked({ cid: c.id, ids, anchor: id });
    setMenu(null);
    setCtx({ x: e.clientX, y: e.clientY, cid: c.id, ids: [...ids] });
  };

  const selectAll = (c: CollectionView) => {
    setMenu(null);
    setCtx(null);
    if (c.sources.length) setPicked({ cid: c.id, ids: new Set(c.sources.map((s) => s.item_id)), anchor: c.sources[0].item_id });
  };

  // Dragging a collection by its handle: it moves among its siblings (top-level collections,
  // or subcollections of the same parent). `over` is the sibling it will land before.
  const [collDrag, setCollDrag] = useState<{ id: string; over: string | null } | null>(null);
  const grabCollection = (e: MouseEvent, c: CollectionView) => {
    e.preventDefault();
    e.stopPropagation();
    const siblings = () => [...document.querySelectorAll<HTMLElement>(`.sidebar [data-coll][data-parent="${c.parent ?? ""}"]`)];
    const targetAt = (y: number) => siblings().find((r) => { const b = r.getBoundingClientRect(); return y < b.top + b.height / 2; })?.dataset.coll ?? null;
    let over = targetAt(e.clientY);
    setCollDrag({ id: c.id, over });
    const move = (ev: globalThis.MouseEvent) => { over = targetAt(ev.clientY); setCollDrag({ id: c.id, over }); };
    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      document.body.classList.remove("dragging-row");
      setCollDrag(null);
      const ids = siblings().map((r) => r.dataset.coll);
      const from = ids.indexOf(c.id);
      // Dropped on itself or right after itself: no move.
      if (over === c.id || (over !== null && ids.indexOf(over) === from + 1) || (over === null && from === ids.length - 1)) return;
      api.moveCollection(c.id, over).then(onLibrary).catch(fail);
    };
    document.body.classList.add("dragging-row");
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  };

  const bulk = async (op: () => Promise<LibraryState>) => {
    setCtx(null);
    try { onLibrary(await op()); setPicked(null); } catch (e) { fail(e); }
  };

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

  const tops = lib.collections.filter((c) => !c.parent);
  const childrenOf = (id: string) => lib.collections.filter((c) => c.parent === id);
  const targets = orderedCollections(lib);

  // A collection with its sources and, for a parent, its indented subcollections.
  const renderCollection = (c: CollectionView): ReactNode => {
    const kids = childrenOf(c.id);
    const allSources = [...c.sources, ...kids.flatMap((k) => k.sources)];
    return (
      <div key={c.id}>
        <div data-coll={c.id} data-parent={c.parent ?? ""}
          className={`side-item ${c.parent ? "child" : ""} ${current === `c:${c.id}` ? "on" : ""} ${collDrag?.over === c.id && collDrag.id !== c.id ? "drop-before" : ""} ${collDrag?.id === c.id ? "dragged" : ""}`}
          onClick={() => select({ kind: "search", scope: { kind: "collection", id: c.id } })}>
          <span className="grab" title={t("Trascina per riordinare")} onMouseDown={(e) => grabCollection(e, c)} onClick={(e) => e.stopPropagation()}>⋮⋮</span>
          <button className="small-btn" onClick={(e) => { e.stopPropagation(); toggle(c.id); }}>{collapsed.has(c.id) ? "▸" : "▾"}</button>
          <span className="grow name">{c.name}</span>
          <span className="muted small">{allSources.length}</span>
          <span className={`actions ${menu === `c:${c.id}` ? "open" : ""}`}>
            <button className="small-btn" title={t("Altro")} onClick={(e) => { e.stopPropagation(); setConfirmDelete(null); setMenu(menu === `c:${c.id}` ? null : `c:${c.id}`); }}>⋯</button>
          </span>
          {menu === `c:${c.id}` && (
            <div className="menu" onClick={(e) => e.stopPropagation()}>
              <button onClick={() => { setMenu(null); onAddSources(c.id); }}>{t("Aggiungi sorgenti")}</button>
              <button disabled={!c.sources.length} onClick={() => selectAll(c)}>{t("Seleziona tutte le sorgenti")}</button>
              {!c.parent && <button onClick={() => { setMenu(null); onNewSubcollection(c.id); }}>{t("Nuova sotto-raccolta")}</button>}
              <button disabled={!allSources.length} onClick={() => refresh(allSources.map((s) => s.item_id))}>{t("Aggiorna tutte")}</button>
              <button onClick={() => { setMenu(null); onRename(c); }}>{t("Rinomina")}</button>
              <button disabled={!allSources.length} onClick={() => {
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
            <div key={key} className={`side-item sub ${c.parent ? "deep" : ""} ${current === `s:${s.item_id}` ? "on" : ""} ${picked?.cid === c.id && picked.ids.has(s.item_id) && picked.ids.size > 1 ? "picked" : ""}`}
              onClick={(e) => clickSource(e, c, s.item_id)} onContextMenu={(e) => openCtx(e, c, s.item_id)}>
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
        {!collapsed.has(c.id) && kids.map(renderCollection)}
      </div>
    );
  };

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
      {tops.map(renderCollection)}
      {collDrag && collDrag.over === null && <div className="drop-end-line" />}

      <div className="grow" />
      <div className={`side-item ${current === "queue" ? "on" : ""}`} onClick={() => { setMenu(null); onToggleQueue(); }}>
        ⬇ {t("Coda")} <span className="grow" />{queueCount > 0 && <span className="badge">{queueCount}</span>}
      </div>
      {ctx && (
        <div className="ctx-backdrop" onClick={() => setCtx(null)} onContextMenu={(e) => { e.preventDefault(); setCtx(null); }}>
          <div className="menu ctx-menu" style={{ left: Math.min(ctx.x, window.innerWidth - 240), top: Math.min(ctx.y, window.innerHeight - 320) }} onClick={(e) => e.stopPropagation()}>
            {(() => { const c = lib.collections.find((x) => x.id === ctx.cid); return c && ctx.ids.length < c.sources.length ? <button onClick={() => selectAll(c)}>{t("Seleziona tutte le sorgenti")}</button> : null; })()}
            <button onClick={() => bulk(() => api.removeSources(ctx.cid, ctx.ids))}>{t("Togli dalla raccolta ({n})", { n: ctx.ids.length })}</button>
            <div className="menu-head">{t("Sposta in")}</div>
            {targets.filter((c) => c.id !== ctx.cid).map((c) => (
              <button key={`m${c.id}`} onClick={() => bulk(() => api.moveSources(ctx.cid, c.id, ctx.ids))}>{collectionLabel(lib, c)}</button>
            ))}
            <button className="muted" onClick={() => { const { cid, ids } = ctx; setCtx(null); onNewCollectionThen(async (to) => { onLibrary(await api.moveSources(cid, to, ids)); setPicked(null); }); }}>＋ {t("Nuova raccolta…")}</button>
            <div className="menu-head">{t("Copia in")}</div>
            {targets.filter((c) => c.id !== ctx.cid).map((c) => (
              <button key={`c${c.id}`} onClick={() => bulk(() => api.copySources(c.id, ctx.ids))}>{collectionLabel(lib, c)}</button>
            ))}
            <button className="muted" onClick={() => { const { ids } = ctx; setCtx(null); onNewCollectionThen(async (to) => { onLibrary(await api.copySources(to, ids)); setPicked(null); }); }}>＋ {t("Nuova raccolta…")}</button>
          </div>
        </div>
      )}
    </nav>
  );
}
