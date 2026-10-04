import { useCallback, useEffect, useMemo, useState } from "react";
import { api, type Hit, type LibraryState, type Scope, type SearchResult } from "../api";
import { extractLinks, formatDate, hitKey, human, tagLabels } from "../util";
import { ResultRow } from "./ResultRow";

type Props = {
  scope: Scope;
  lib: LibraryState;
  query: string;
  onQuery: (q: string) => void;
  defaultOriginals: boolean;
  onLibrary: (s: LibraryState) => void;
  onOpenLinks: (links: string[]) => void;
  onSaveUnsaved: (itemId: string) => void;
  onError: (msg: string) => void;
};

export function SearchView({ scope, lib, query, onQuery, defaultOriginals, onLibrary, onOpenLinks, onSaveUnsaved, onError }: Props) {
  const [originals, setOriginals] = useState(defaultOriginals);
  const [result, setResult] = useState<SearchResult | null>(null);
  const [selected, setSelected] = useState<Map<string, Hit>>(new Map());
  const [refreshing, setRefreshing] = useState(false);

  const words = useMemo(() => query.toLowerCase().split(/\s+/).filter(Boolean), [query]);

  // La selezione vale per un ambito e una query: cambiandoli si svuota.
  useEffect(() => setSelected(new Map()), [scope, query]);

  // Ricerca 150 ms dopo l'ultima modifica; si ripete anche quando la libreria cambia (sorgenti aggiornate).
  useEffect(() => {
    let alive = true;
    const t = setTimeout(() => {
      api.search(scope, query, originals).then((r) => alive && setResult(r)).catch((e) => alive && onError(String(e)));
    }, 150);
    return () => { alive = false; clearTimeout(t); };
  }, [scope, query, originals, lib, onError]);

  const toggle = useCallback((h: Hit) => setSelected((s) => {
    const n = new Map(s);
    const k = hitKey(h);
    if (n.has(k)) n.delete(k); else n.set(k, h);
    return n;
  }), []);

  const onInput = (text: string) => {
    const links = extractLinks(text);
    if (links.length) { onOpenLinks(links); onQuery(""); } else onQuery(text);
  };

  const enqueue = () => {
    const files = [...selected.values()].map((h) => ({ item_id: h.item_id, name: h.name, size: h.size }));
    api.enqueue(files).then(() => setSelected(new Map())).catch((e) => onError(String(e)));
  };

  // Intestazione: di cosa si sta parlando.
  const collection = scope.kind === "collection" ? lib.collections.find((c) => c.id === scope.id) : undefined;
  const sourceId = scope.kind === "source" ? scope.item_id : null;
  const source = sourceId
    ? lib.unsaved.find((m) => m.item_id === sourceId) ?? lib.collections.flatMap((c) => c.sources).find((s) => s.item_id === sourceId)
    : undefined;
  const unsaved = !!sourceId && lib.unsaved.some((m) => m.item_id === sourceId);
  const allCount = new Set(lib.collections.flatMap((c) => c.sources.map((s) => s.item_id))).size;
  const title =
    scope.kind === "all" ? `Tutte le sorgenti · ${allCount} sorgenti`
    : scope.kind === "unsaved" ? "Non salvate"
    : collection ? `${collection.name} · ${collection.sources.filter((s) => s.included).length} sorgenti`
    : source ? source.title || source.item_id : "";

  const refresh = async () => {
    if (!sourceId) return;
    setRefreshing(true);
    await api.refreshSource(sourceId).catch((e) => onError(String(e)));
    setRefreshing(false);
    onLibrary(await api.libraryState());
  };

  let empty: string | null = null;
  if (!lib.collections.length && !lib.unsaved.length) empty = "Crea una raccolta o incolla un link archive.org per iniziare";
  else if (collection && !collection.sources.length) empty = "Aggiungi sorgenti con ⋯ → Aggiungi sorgenti";
  else if (result && result.total === 0 && words.length) empty = "Nessun file contiene tutte le parole";

  const shown = result?.results ?? [];
  // Sigle calcolate sulle sorgenti dell'ambito (non sui risultati), così non cambiano mentre si scrive.
  const scopeIds =
    scope.kind === "collection" ? (collection?.sources.filter((s) => s.included).map((s) => s.item_id) ?? [])
    : scope.kind === "source" ? [scope.item_id]
    : scope.kind === "unsaved" ? lib.unsaved.map((m) => m.item_id)
    : lib.collections.flatMap((c) => c.sources.map((s) => s.item_id));
  const tags = tagLabels([...scopeIds, ...shown.map((h) => h.item_id)]);
  return (
    <>
      <input className="field search" autoFocus placeholder="Cerca nei file… oppure incolla un link archive.org" value={query} onChange={(e) => onInput(e.target.value)} />
      <div className="row" style={{ flexWrap: "wrap" }}>
        <b className="name">{title}</b>
        {result && (
          <span className="muted small">
            · {result.total.toLocaleString("it-IT")} risultati{result.total > shown.length ? ` · mostrati ${shown.length}` : ""}
          </span>
        )}
        <div className="grow" />
        <button className={`chip ${originals ? "on" : ""}`} onClick={() => setOriginals((o) => !o)}>Solo originali</button>
        <button className="muted small" onClick={() => setSelected(new Map(shown.map((h) => [hitKey(h), h])))}>Tutti</button>
        <span className="muted small">·</span>
        <button className="muted small" onClick={() => setSelected(new Map())}>Nessuno</button>
        <button className="btn small" disabled={!selected.size} onClick={enqueue}>Aggiungi {selected.size} alla coda</button>
      </div>
      {source && (
        <div className="row muted small">
          <span>{source.file_count.toLocaleString("it-IT")} file · {human(source.total_size)} · aggiornata il {formatDate(source.updated_at)}</span>
          {source.error && <span className="warn" title={source.error}>⚠ {source.error}</span>}
          <div className="grow" />
          <button className="btn small ghost" disabled={refreshing} onClick={refresh}>{refreshing ? "Aggiornamento…" : "↻ Aggiorna"}</button>
          {unsaved && <button className="btn small" onClick={() => onSaveUnsaved(source.item_id)}>★ Salva</button>}
        </div>
      )}
      <div className="card results">
        {empty ? <div className="empty">{empty}</div>
          : shown.map((h) => <ResultRow key={hitKey(h)} hit={h} tag={tags.get(h.item_id) ?? h.item_id} checked={selected.has(hitKey(h))} words={words} onToggle={toggle} />)}
      </div>
    </>
  );
}
