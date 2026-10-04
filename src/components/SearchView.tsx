import { useCallback, useEffect, useMemo, useState, type ClipboardEvent, type KeyboardEvent } from "react";
import { api, type Hit, type Job, type LibraryState, type Scope, type SearchResult, type Sort } from "../api";
import { extractLinks, formatDate, hitKey, human, tagLabels } from "../util";
import { ResultRow } from "./ResultRow";

type Props = {
  scope: Scope;
  lib: LibraryState;
  jobs: Job[];
  query: string;
  onQuery: (q: string) => void;
  defaultOriginals: boolean;
  onLibrary: (s: LibraryState) => void;
  onOpenLinks: (links: string[]) => void;
  onSaveUnsaved: (itemId: string) => void;
  onError: (msg: string) => void;
};

export function SearchView({ scope, lib, jobs, query, onQuery, defaultOriginals, onLibrary, onOpenLinks, onSaveUnsaved, onError }: Props) {
  const [originals, setOriginals] = useState(defaultOriginals);
  const [exts, setExts] = useState<string[]>([]);
  const [sort, setSort] = useState<Sort>("name");
  const [result, setResult] = useState<SearchResult | null>(null);
  const [pending, setPending] = useState(true);
  const [selected, setSelected] = useState<Map<string, Hit>>(new Map());
  const [refreshing, setRefreshing] = useState(false);

  const words = useMemo(() => query.toLowerCase().split(/\s+/).filter(Boolean), [query]);

  // La selezione vale per un ambito e una query: cambiandoli si svuota. I filtri per estensione
  // dipendono dall'ambito, quindi ripartono da zero quando lo si cambia.
  useEffect(() => setSelected(new Map()), [scope, query]);
  useEffect(() => setExts([]), [scope]);

  // Ricerca 150 ms dopo l'ultima modifica; si ripete quando cambiano la libreria (sorgenti
  // aggiornate) o la coda (per lo stato "in coda" / "scaricato" dei risultati).
  useEffect(() => {
    let alive = true;
    setPending(true);
    const t = setTimeout(() => {
      api.search(scope, query, { originals_only: originals, exts, sort })
        .then((r) => {
          if (!alive) return;
          setResult(r);
          setPending(false);
          // Restano selezionati solo i file ancora visibili (filtri, sorgenti escluse…).
          const keys = new Set(r.results.map(hitKey));
          setSelected((s) => (s.size && [...s.keys()].some((k) => !keys.has(k)) ? new Map([...s].filter(([k]) => keys.has(k))) : s));
        })
        .catch((e) => alive && onError(String(e)));
    }, 150);
    return () => { alive = false; clearTimeout(t); };
  }, [scope, query, originals, exts, sort, lib, jobs, onError]);

  const toggle = useCallback((h: Hit) => setSelected((s) => {
    const n = new Map(s);
    const k = hitKey(h);
    if (n.has(k)) n.delete(k); else n.set(k, h);
    return n;
  }), []);

  // I link si aprono solo incollandoli o premendo Invio: mentre li si scrive a mano sono incompleti.
  const openLinksIn = (text: string) => {
    const links = extractLinks(text);
    if (!links.length) return false;
    onOpenLinks(links);
    onQuery("");
    return true;
  };
  const onPaste = (e: ClipboardEvent<HTMLInputElement>) => {
    if (openLinksIn(e.clipboardData.getData("text"))) e.preventDefault();
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") openLinksIn(query);
  };

  const toggleExt = (ext: string) => setExts((x) => (x.includes(ext) ? x.filter((e) => e !== ext) : [...x, ext]));

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
  else if (collection && !collection.sources.some((s) => s.included)) empty = "Tutte le sorgenti di questa raccolta sono escluse dalla ricerca: spunta quelle da includere";
  else if (scope.kind === "all" && allCount === 0)
    empty = lib.unsaved.length ? "Nessuna sorgente salvata: le Non salvate si cercano selezionandole a sinistra" : "Le raccolte sono vuote: aggiungi sorgenti con ⋯ → Aggiungi sorgenti";
  else if (result && result.total === 0 && (words.length || exts.length)) empty = "Nessun file corrisponde alla ricerca";

  const shown = result?.results ?? [];
  // Sigle calcolate sulle sorgenti dell'ambito (non sui risultati), così non cambiano mentre si scrive.
  const scopeIds =
    scope.kind === "collection" ? (collection?.sources.filter((s) => s.included).map((s) => s.item_id) ?? [])
    : scope.kind === "source" ? [scope.item_id]
    : scope.kind === "unsaved" ? lib.unsaved.map((m) => m.item_id)
    : lib.collections.flatMap((c) => c.sources.map((s) => s.item_id));
  const tags = tagLabels([...scopeIds, ...shown.map((h) => h.item_id)]);
  // Chip: le estensioni presenti, più quelle attive anche se ora non ne compaiono.
  const extChips = [...(result?.extensions ?? []).map((e) => ({ ext: e.ext, count: e.count as number | null }))];
  for (const e of exts) if (!extChips.some((c) => c.ext === e)) extChips.push({ ext: e, count: null });
  // "Tutti" salta i file già scaricati o già in coda.
  const selectable = shown.filter((h) => !h.local);

  return (
    <>
      <input className="field search" autoFocus placeholder="Cerca nei file… oppure incolla un link archive.org" value={query}
        onChange={(e) => onQuery(e.target.value)} onPaste={onPaste} onKeyDown={onKeyDown} />
      <div className="row" style={{ flexWrap: "wrap" }}>
        <b className="name">{title}</b>
        {result && (
          <span className="muted small">
            · {result.total.toLocaleString("it-IT")} risultati{result.total > shown.length ? ` · mostrati ${shown.length}` : ""}
          </span>
        )}
        <div className="grow" />
        <button className="muted small" disabled={pending || !selectable.length} title="Seleziona i risultati non ancora scaricati né in coda"
          onClick={() => setSelected(new Map(selectable.map((h) => [hitKey(h), h])))}>Tutti</button>
        <span className="muted small">·</span>
        <button className="muted small" onClick={() => setSelected(new Map())}>Nessuno</button>
        <button className="btn small" disabled={!selected.size || pending} onClick={enqueue}>Aggiungi {selected.size} alla coda</button>
      </div>
      <div className="row" style={{ flexWrap: "wrap", gap: 6 }}>
        <button className={`chip ${originals ? "on" : ""}`} onClick={() => setOriginals((o) => !o)}>Solo originali</button>
        {extChips.map(({ ext, count }) => (
          <button key={ext} className={`chip ${exts.includes(ext) ? "on" : ""}`} onClick={() => toggleExt(ext)}>
            {ext}{count != null && <span className="muted"> {count.toLocaleString("it-IT")}</span>}
          </button>
        ))}
        <div className="grow" />
        <select className="field sort" value={sort} onChange={(e) => setSort(e.target.value as Sort)} title="Ordina i risultati">
          <option value="name">Nome</option>
          <option value="size_desc">Più grandi prima</option>
          <option value="size_asc">Più piccoli prima</option>
        </select>
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
