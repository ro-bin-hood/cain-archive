import { useEffect, useState, type MouseEvent, type ReactNode } from "react";
import { api, type CollectionView, type LibraryState, type SourceMeta } from "../api";
import type { View } from "../state";
import { scopeKey } from "../state";
import { useT } from "../i18n";
import { collectionLabel, orderedCollections, withChildren } from "../util";
import { useDragReorder } from "../dragReorder";

type Props = {
  width: number;
  lib: LibraryState;
  view: View;
  queueCount: number;
  onSelect: (v: View) => void;
  onLibrary: (s: LibraryState) => void;
  onError: (err: unknown) => void;
  onNewCollection: () => void;
  onImport: () => void;
  onRename: (c: CollectionView) => void;
  onAddSources: (collectionId: string) => void;
  onSaveUnsaved: (itemId: string) => void;
  /** Asks for confirmation, then deletes the collection. */
  onDelete: (c: CollectionView) => void;
  /** Asks for a new collection name, creates it, then runs `then` with its id. */
  onNewCollectionThen: (then: (collectionId: string) => Promise<void>) => void;
  onNewSubcollection: (parentId: string) => void;
  /** Opens the queue, or goes back to the previous search if the queue is already open. */
  onToggleQueue: () => void;
};

/** Selected sources (always within one collection) and the open context menu. */
type Picked = { cid: string; ids: Set<string>; anchor: string };
type Ctx = { x: number; y: number; cid: string; ids: string[] };
/** Selected collections (Ctrl/Shift+click), for actions on several at once. */
type PickedColls = { ids: Set<string>; anchor: string };

export function Sidebar({ width, lib, view, queueCount, onSelect, onLibrary, onError, onNewCollection, onImport, onRename, onAddSources, onSaveUnsaved, onDelete, onNewCollectionThen, onNewSubcollection, onToggleQueue }: Props) {
  const { t, te } = useT();
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [menu, setMenu] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState<Set<string>>(new Set());
  const [picked, setPicked] = useState<Picked | null>(null);
  const [ctx, setCtx] = useState<Ctx | null>(null);
  const [pickedColls, setPickedColls] = useState<PickedColls | null>(null);

  // Esc closes the menus and clears the selection.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") { setCtx(null); setMenu(null); setPicked(null); setPickedColls(null); } };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Clicking a source: Ctrl adds/removes, Shift selects the range from the last clicked one,
  // a plain click opens the source and makes it the only selected one.
  const clickSource = (e: MouseEvent, c: CollectionView, id: string) => {
    setPickedColls(null);
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

  // Clicking a collection: Ctrl adds/removes it, Shift selects the range (in tree order) from the
  // last clicked one, a plain click opens it.
  const clickCollection = (e: MouseEvent, c: CollectionView) => {
    setMenu(null);
    if (e.ctrlKey || e.metaKey) {
      const ids = new Set(pickedColls?.ids ?? (current.startsWith("c:") ? [current.slice(2)] : []));
      if (ids.has(c.id)) ids.delete(c.id); else ids.add(c.id);
      setPickedColls({ ids, anchor: c.id });
      setPicked(null);
      return;
    }
    if (e.shiftKey) {
      const anchor = pickedColls?.anchor ?? (current.startsWith("c:") ? current.slice(2) : c.id);
      const order = orderedCollections(lib).map((x) => x.id);
      const [a, b] = [order.indexOf(anchor), order.indexOf(c.id)].sort((x, y) => x - y);
      setPickedColls({ ids: new Set(a < 0 ? [c.id] : order.slice(a, b + 1)), anchor: a < 0 ? c.id : anchor });
      setPicked(null);
      return;
    }
    setPickedColls({ ids: new Set([c.id]), anchor: c.id });
    setPicked(null);
    select({ kind: "search", scope: { kind: "collection", id: c.id } });
  };

  // Right-click on a collection opens its ⋯ menu; inside a multiple selection it keeps it.
  const collectionMenu = (e: MouseEvent, c: CollectionView) => {
    e.preventDefault();
    if (!pickedColls?.ids.has(c.id)) setPickedColls({ ids: new Set([c.id]), anchor: c.id });
    setCtx(null);
    setMenu(`c:${c.id}`);
  };

  // Exports the selected collections, one file each; a subcollection whose parent is also
  // selected is already inside the parent's file.
  const exportPicked = (ids: Set<string>) => {
    setMenu(null);
    const roots = lib.collections.filter((c) => ids.has(c.id) && !(c.parent && ids.has(c.parent))).map((c) => c.id);
    api.exportCollections(roots).then((p) => p && onError(t("sidebar.exportedMany", { n: roots.length, path: p }))).catch(fail);
  };

  const selectAll = (c: CollectionView) => {
    setMenu(null);
    setCtx(null);
    setPicked(c.sources.length ? { cid: c.id, ids: new Set(c.sources.map((s) => s.item_id)), anchor: c.sources[0].item_id } : null);
  };

  // Dragging a collection row: it moves among its siblings (top-level collections, or
  // subcollections of the same parent), taking its sources and subcollections along.
  const grabCollection = useDragReorder({
    rows: (id) => {
      const parent = lib.collections.find((c) => c.id === id)?.parent ?? "";
      return [...document.querySelectorAll<HTMLElement>(`.sidebar [data-drag-id][data-parent="${parent}"]`)];
    },
    onMove: (id, before) => { api.moveCollection(id, before).then(onLibrary).catch(fail); },
    orderKey: lib.collections.map((c) => c.id).join(),
  });

  const bulk = async (op: () => Promise<LibraryState>) => {
    setCtx(null);
    try { onLibrary(await op()); setPicked(null); } catch (e) { fail(e); }
  };

  const current = view.kind === "search" ? scopeKey(view.scope) : "queue";
  const select = (v: View) => { setMenu(null); onSelect(v); };
  const reload = () => api.libraryState().then(onLibrary);
  const fail = (e: unknown) => onError(e);
  const toggle = (id: string) => setCollapsed((s) => { const n = new Set(s); if (n.has(id)) n.delete(id); else n.add(id); return n; });

  const refresh = async (ids: string[]) => {
    setMenu(null);
    setRefreshing((s) => new Set([...s, ...ids]));
    const errors: string[] = [];
    for (const id of ids) {
      await api.refreshSource(id).catch((e) => errors.push(`${id}: ${te(e)}`));
      setRefreshing((s) => { const n = new Set(s); n.delete(id); return n; });
    }
    await reload();
    if (errors.length) onError(t("sidebar.refreshFailed", { list: errors.join("; ") }));
  };

  // Enables or disables (in the collection search) every source of the collection and its subcollections.
  const includeAll = async (c: CollectionView, included: boolean) => {
    setMenu(null);
    try {
      for (const k of withChildren(lib, c.id)) {
        for (const src of k.sources) if (src.included !== included) onLibrary(await api.setIncluded(k.id, src.item_id, included));
      }
    } catch (e) { fail(e); }
  };

  const sourceLabel = (m: SourceMeta) => m.title || m.item_id;
  const warn = (m: SourceMeta) => m.error && <span className="warn" title={te(m.error)}>⚠</span>;
  const spin = (id: string) => refreshing.has(id) && <span className="muted small">↻</span>;

  const tops = lib.collections.filter((c) => !c.parent);
  const childrenOf = (id: string) => lib.collections.filter((c) => c.parent === id);
  const targets = orderedCollections(lib);
  const multi = pickedColls !== null && pickedColls.ids.size > 1;

  // A collection with its sources and, for a parent, its indented subcollections.
  const renderCollection = (c: CollectionView): ReactNode => {
    const kids = childrenOf(c.id);
    const allSources = [...c.sources, ...kids.flatMap((k) => k.sources)];
    return (
      <div key={c.id} className="coll-block" data-drag-id={c.id} data-parent={c.parent ?? ""}>
        <div className={`side-item ${c.parent ? "child" : ""} ${current === `c:${c.id}` ? "on" : ""} ${multi && pickedColls.ids.has(c.id) ? "picked" : ""}`}
          onMouseDown={(e) => grabCollection(e, c.id)} onClick={(e) => clickCollection(e, c)} onContextMenu={(e) => collectionMenu(e, c)}>
          <span className="grab" title={t("common.dragToReorder")}>⋮⋮</span>
          <button className="small-btn" onClick={(e) => { e.stopPropagation(); toggle(c.id); }}>{collapsed.has(c.id) ? "▸" : "▾"}</button>
          <span className="grow name">{c.name}</span>
          <span className="muted small">{allSources.length}</span>
          <span className={`actions ${menu === `c:${c.id}` ? "open" : ""}`}>
            <button className="small-btn" title={t("common.more")} onClick={(e) => { e.stopPropagation(); setMenu(menu === `c:${c.id}` ? null : `c:${c.id}`); }}>⋯</button>
          </span>
          {menu === `c:${c.id}` && (
            <div className="menu" onClick={(e) => e.stopPropagation()}>
              {multi && pickedColls.ids.has(c.id) && <>
                <button onClick={() => exportPicked(pickedColls.ids)}>{t("sidebar.exportSelected", { n: pickedColls.ids.size })}</button>
                <div className="menu-sep" />
              </>}
              <button onClick={() => { setMenu(null); onAddSources(c.id); }}>{t("sidebar.addSources")}</button>
              <button disabled={allSources.every((s) => s.included)} onClick={() => includeAll(c, true)}>{t("sidebar.enableAll")}</button>
              <button disabled={!allSources.some((s) => s.included)} onClick={() => includeAll(c, false)}>{t("sidebar.disableAll")}</button>
              <button disabled={!c.sources.length} onClick={() => selectAll(c)}>{t("sidebar.selectAllSources")}</button>
              {!c.parent && <button onClick={() => { setMenu(null); onNewSubcollection(c.id); }}>{t("sidebar.newSubcollection")}</button>}
              <button disabled={!allSources.length} onClick={() => refresh(allSources.map((s) => s.item_id))}>{t("sidebar.refreshAll")}</button>
              <button onClick={() => { setMenu(null); onRename(c); }}>{t("common.rename")}</button>
              <button disabled={!allSources.length} onClick={() => {
                setMenu(null);
                api.exportCollection(c.id, t("app.listFileType")).then((p) => p && onError(t("sidebar.exported", { path: p }))).catch(fail);
              }}>{t("sidebar.export")}</button>
              <button className="err" onClick={() => { setMenu(null); onDelete(c); }}>{t("sidebar.delete")}</button>
            </div>
          )}
        </div>
        {!collapsed.has(c.id) && c.sources.map((s) => {
          const key = `${c.id}:${s.item_id}`;
          return (
            <div key={key} className={`side-item sub ${c.parent ? "deep" : ""} ${current === `s:${s.item_id}` ? "on" : ""} ${picked?.cid === c.id && picked.ids.has(s.item_id) && picked.ids.size > 1 ? "picked" : ""}`}
              onClick={(e) => clickSource(e, c, s.item_id)} onContextMenu={(e) => openCtx(e, c, s.item_id)}>
              <input type="checkbox" className="check" title={t("sidebar.includeInSearch")} checked={s.included} onClick={(e) => e.stopPropagation()}
                onChange={(e) => api.setIncluded(c.id, s.item_id, e.target.checked).then(onLibrary).catch(fail)} />
              <span className={`grow name ${s.included ? "" : "muted"}`} title={s.item_id}>{sourceLabel(s)}</span>
              {spin(s.item_id)}
              {warn(s)}
              <span className={`actions ${menu === key ? "open" : ""}`}>
                <button className="small-btn" title={t("common.more")} onClick={(e) => { e.stopPropagation(); setMenu(menu === key ? null : key); }}>⋯</button>
              </span>
              {menu === key && (
                <div className="menu" onClick={(e) => e.stopPropagation()}>
                  <button onClick={() => refresh([s.item_id])}>{t("common.refresh")}</button>
                  <button onClick={() => {
                    setMenu(null);
                    api.removeSource(c.id, s.item_id).then(onLibrary).catch(fail);
                    if (current === `s:${s.item_id}`) onSelect({ kind: "search", scope: { kind: "collection", id: c.id } });
                  }}>{t("sidebar.removeFromCollection")}</button>
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
    <nav className="sidebar" style={{ width }} onMouseLeave={() => { setMenu(null); }}>
      {lib.unsaved.length > 0 && (
        <>
          <div className="side-head">{t("sidebar.unsaved")}</div>
          {lib.unsaved.map((m) => (
            <div key={m.item_id} className={`side-item ${current === `s:${m.item_id}` ? "on" : ""}`} onClick={() => select({ kind: "search", scope: { kind: "source", item_id: m.item_id } })}>
              <span className="grow name" title={m.item_id}>{sourceLabel(m)}</span>
              {warn(m)}
              <span className="actions">
                <button className="small-btn" title={t("sidebar.saveToCollection")} onClick={(e) => { e.stopPropagation(); onSaveUnsaved(m.item_id); }}>★</button>
                <button className="small-btn" title={t("common.close")} onClick={(e) => {
                  e.stopPropagation();
                  api.closeUnsaved(m.item_id).then(onLibrary).catch(fail);
                  if (current === `s:${m.item_id}`) onSelect({ kind: "search", scope: { kind: "all" } });
                }}>✕</button>
              </span>
            </div>
          ))}
        </>
      )}

      <div className={`side-item ${current === "all" ? "on" : ""}`} onClick={() => select({ kind: "search", scope: { kind: "all" } })}>🌐 {t("sidebar.allSources")}</div>

      <div className="side-head">
        {t("sidebar.collections")}
        <span className="row" style={{ gap: 2 }}>
          <button className="small-btn" title={t("sidebar.importFromFile")} onClick={onImport}>⤓</button>
          <button className="small-btn" title={t("sidebar.newCollection")} onClick={onNewCollection}>＋</button>
        </span>
      </div>
      {tops.map(renderCollection)}

      <div className="grow" />
      <div className={`side-item ${current === "queue" ? "on" : ""}`} onClick={() => { setMenu(null); onToggleQueue(); }}>
        ⬇ {t("sidebar.queue")} <span className="grow" />{queueCount > 0 && <span className="badge">{queueCount}</span>}
      </div>
      {ctx && (
        <div className="ctx-backdrop" onClick={() => setCtx(null)} onContextMenu={(e) => { e.preventDefault(); setCtx(null); }}>
          <div className="menu ctx-menu" style={{ left: Math.min(ctx.x, window.innerWidth - 240), top: Math.min(ctx.y, window.innerHeight - 320) }} onClick={(e) => e.stopPropagation()}>
            {(() => { const c = lib.collections.find((x) => x.id === ctx.cid); return c && ctx.ids.length < c.sources.length ? <button onClick={() => selectAll(c)}>{t("sidebar.selectAllSources")}</button> : null; })()}
            <button onClick={() => bulk(() => api.removeSources(ctx.cid, ctx.ids))}>{t("sidebar.removeSelected", { n: ctx.ids.length })}</button>
            <div className="menu-head">{t("sidebar.moveTo")}</div>
            {targets.filter((c) => c.id !== ctx.cid).map((c) => (
              <button key={`m${c.id}`} onClick={() => bulk(() => api.moveSources(ctx.cid, c.id, ctx.ids))}>{collectionLabel(lib, c)}</button>
            ))}
            <button className="muted" onClick={() => { const { cid, ids } = ctx; setCtx(null); onNewCollectionThen(async (to) => { onLibrary(await api.moveSources(cid, to, ids)); setPicked(null); }); }}>＋ {t("common.newCollectionEllipsis")}</button>
            <div className="menu-head">{t("sidebar.copyTo")}</div>
            {targets.filter((c) => c.id !== ctx.cid).map((c) => (
              <button key={`c${c.id}`} onClick={() => bulk(() => api.copySources(ctx.cid, c.id, ctx.ids))}>{collectionLabel(lib, c)}</button>
            ))}
            <button className="muted" onClick={() => { const { cid, ids } = ctx; setCtx(null); onNewCollectionThen(async (to) => { onLibrary(await api.copySources(cid, to, ids)); setPicked(null); }); }}>＋ {t("common.newCollectionEllipsis")}</button>
          </div>
        </div>
      )}
    </nav>
  );
}
