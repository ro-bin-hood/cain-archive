import { useMemo, useState } from "react";
import type { Item, NewFile } from "../api";
import { ext, human, parseExts, preselect } from "../util";

type Props = { item: Item; defaultOriginals: boolean; defaultExts: string; onAdd: (files: NewFile[]) => void; onClose: () => void };

export function ItemCard({ item, defaultOriginals, defaultExts, onAdd, onClose }: Props) {
  const single = item.files.length === 1;
  const itemExts = useMemo(() => {
    const count = new Map<string, number>();
    for (const f of item.files) {
      const e = ext(f.name);
      if (e) count.set(e, (count.get(e) ?? 0) + 1);
    }
    return [...count.entries()].sort((a, b) => b[1] - a[1]).map(([e]) => e);
  }, [item]);
  const [originals, setOriginals] = useState(defaultOriginals && !single);
  const [exts, setExts] = useState<string[]>(() => parseExts(defaultExts).filter((e) => itemExts.includes(e)));
  const [selected, setSelected] = useState<Set<string>>(() =>
    single ? new Set(item.files.map((f) => f.name)) : preselect(item.files, originals, exts),
  );

  const apply = (o: boolean, e: string[]) => { setOriginals(o); setExts(e); setSelected(preselect(item.files, o, e)); };
  const toggleExt = (e: string) => apply(originals, exts.includes(e) ? exts.filter((x) => x !== e) : [...exts, e]);
  const toggleFile = (name: string) =>
    setSelected((s) => { const n = new Set(s); if (n.has(name)) n.delete(name); else n.add(name); return n; });
  const chosen = item.files.filter((f) => selected.has(f.name));
  const bytes = chosen.reduce((a, f) => a + f.size, 0);

  return (
    <section className="card">
      <div className="row" style={{ marginBottom: 6 }}>
        <b className="name">{item.id}</b>
        <span className="muted small">· {chosen.length} di {item.files.length} selezionati · {human(bytes)}</span>
        <div className="grow" />
        <button className="icon-btn" title="Chiudi" onClick={onClose}>✕</button>
      </div>
      {!single && (
        <div className="row" style={{ flexWrap: "wrap", gap: 6, marginBottom: 4 }}>
          <button className={`chip ${originals ? "on" : ""}`} onClick={() => apply(!originals, exts)}>Solo originali</button>
          {itemExts.map((e) => (
            <button key={e} className={`chip ${exts.includes(e) ? "on" : ""}`} onClick={() => toggleExt(e)}>{e}</button>
          ))}
          <div className="grow" />
          <button className="muted small" onClick={() => setSelected(new Set(item.files.map((f) => f.name)))}>Tutti</button>
          <span className="muted small">·</span>
          <button className="muted small" onClick={() => setSelected(new Set())}>Nessuno</button>
        </div>
      )}
      <div className="list">
        {item.files.map((f) => (
          <label key={f.name} className="item-row" style={{ cursor: "pointer" }}>
            <input type="checkbox" className="check" checked={selected.has(f.name)} onChange={() => toggleFile(f.name)} />
            <span className="type">{(ext(f.name) || "—").toUpperCase().slice(0, 4)}</span>
            <span className={`grow name ${selected.has(f.name) ? "" : "muted"}`} title={f.name}>{f.name}</span>
            <span className="muted small">{f.size ? human(f.size) : "?"}</span>
          </label>
        ))}
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <div className="grow" />
        <button
          className="btn small"
          disabled={chosen.length === 0}
          onClick={() => onAdd(chosen.map((f) => ({ item_id: item.id, name: f.name, size: f.size })))}
        >
          Aggiungi {chosen.length} file alla coda
        </button>
      </div>
    </section>
  );
}
