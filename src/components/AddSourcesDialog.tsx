import { useState } from "react";
import { api, type LibraryState } from "../api";

type Row = { input: string; status: "wait" | "ok" | "err"; msg: string };
type Props = { lib: LibraryState; initialCollection: string | null; onLibrary: (s: LibraryState) => void; onClose: () => void };

/** Aggiunge una o più sorgenti a una raccolta, una riga alla volta, mostrando l'esito di ognuna. */
export function AddSourcesDialog({ lib, initialCollection, onLibrary, onClose }: Props) {
  const [text, setText] = useState("");
  const [cid, setCid] = useState(initialCollection ?? lib.collections[0]?.id ?? "");
  const [newName, setNewName] = useState("");
  const [rows, setRows] = useState<Row[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const add = async () => {
    setError(null);
    let target = cid;
    if (!lib.collections.length) {
      try {
        const st = await api.createCollection(newName);
        onLibrary(st);
        target = st.collections[st.collections.length - 1].id;
        setCid(target);
      } catch (e) { setError(String(e)); return; }
    }
    const done = new Set(rows.filter((r) => r.status === "ok").map((r) => r.input));
    const inputs = [...new Set(text.split(/\s+/).filter(Boolean))].filter((i) => !done.has(i));
    if (!inputs.length) return;
    setBusy(true);
    setRows((r) => [...r.filter((x) => x.status === "ok"), ...inputs.map((input) => ({ input, status: "wait" as const, msg: "In attesa…" }))]);
    for (const input of inputs) {
      const update = (status: Row["status"], msg: string) => setRows((r) => r.map((x) => (x.input === input ? { ...x, status, msg } : x)));
      await api.addSource(target, input).then((m) => update("ok", `Aggiunta · ${m.file_count.toLocaleString("it-IT")} file`)).catch((e) => update("err", String(e)));
    }
    onLibrary(await api.libraryState());
    setBusy(false);
  };

  return (
    <div className="overlay" onClick={busy ? undefined : onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <b className="title">Aggiungi sorgenti</b>
        <textarea className="field" rows={5} autoFocus placeholder="Un link archive.org o un identificatore per riga" value={text} onChange={(e) => setText(e.target.value)} />
        {lib.collections.length ? (
          <label className="stack">
            <span className="section-title">Raccolta</span>
            <select className="field" value={cid} onChange={(e) => setCid(e.target.value)}>
              {lib.collections.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
            </select>
          </label>
        ) : (
          <input className="field" placeholder="Nome della nuova raccolta" value={newName} onChange={(e) => setNewName(e.target.value)} />
        )}
        {rows.length > 0 && (
          <div className="card" style={{ maxHeight: 200, overflow: "auto" }}>
            {rows.map((r) => (
              <div key={r.input} className="item-row">
                <span className="grow name" title={r.input}>{r.input}</span>
                <span className={`small ${r.status === "ok" ? "ok" : r.status === "err" ? "err" : "muted"}`}>{r.msg}</span>
              </div>
            ))}
          </div>
        )}
        {error && <span className="err small">{error}</span>}
        <div className="row">
          <div className="grow" />
          <button className="btn ghost" disabled={busy} onClick={onClose}>Chiudi</button>
          <button className="btn" disabled={busy || !text.trim()} onClick={add}>{busy ? "Aggiunta…" : "Aggiungi"}</button>
        </div>
      </div>
    </div>
  );
}
