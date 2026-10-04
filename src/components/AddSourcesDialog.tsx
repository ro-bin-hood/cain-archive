import { useState } from "react";
import { api, type LibraryState, type ParsedList } from "../api";
import { useT } from "../i18n";

type Row = { input: string; status: "wait" | "ok" | "err"; msg: string };
type Props = { lib: LibraryState; initialCollection: string | null; prefill?: ParsedList; onLibrary: (s: LibraryState) => void; onClose: () => void };

const NEW = "__new__";

/** Aggiunge una o più sorgenti a una raccolta, una riga alla volta, mostrando l'esito di ognuna.
 *  Con `prefill` (import da file) parte con le righe e il nome della raccolta già compilati. */
export function AddSourcesDialog({ lib, initialCollection, prefill, onLibrary, onClose }: Props) {
  const { t, locale } = useT();
  const existing = prefill?.name ? lib.collections.find((c) => c.name.toLowerCase() === prefill.name!.trim().toLowerCase()) : undefined;
  const [text, setText] = useState(prefill ? prefill.inputs.join("\n") : "");
  const [cid, setCid] = useState(existing?.id ?? (prefill ? NEW : initialCollection ?? lib.collections[0]?.id ?? NEW));
  const [newName, setNewName] = useState(existing ? "" : prefill?.name ?? "");
  const [rows, setRows] = useState<Row[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const add = async () => {
    setError(null);
    let target = cid;
    if (cid === NEW) {
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
    setRows((r) => [...r.filter((x) => x.status === "ok"), ...inputs.map((input) => ({ input, status: "wait" as const, msg: t("In attesa…") }))]);
    for (const input of inputs) {
      const update = (status: Row["status"], msg: string) => setRows((r) => r.map((x) => (x.input === input ? { ...x, status, msg } : x)));
      await api.addSource(target, input).then((m) => update("ok", t("Aggiunta · {n} file", { n: m.file_count.toLocaleString(locale) }))).catch((e) => update("err", String(e)));
    }
    onLibrary(await api.libraryState());
    setBusy(false);
  };

  return (
    <div className="overlay" onClick={busy ? undefined : onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <b className="title">{prefill ? t("Importa raccolta") : t("Aggiungi sorgenti")}</b>
        <textarea className="field" rows={5} autoFocus placeholder={t("Un link archive.org o un identificatore per riga")} value={text} onChange={(e) => setText(e.target.value)} />
        {prefill && prefill.invalid.length > 0 && (
          <span className="warn small" title={prefill.invalid.join("\n")}>
            {t(prefill.invalid.length === 1 ? "{n} riga non riconosciuta nel file, ignorata" : "{n} righe non riconosciute nel file, ignorate", { n: prefill.invalid.length })}
          </span>
        )}
        <label className="stack">
          <span className="section-title">{t("Raccolta")}</span>
          <select className="field" value={cid} onChange={(e) => setCid(e.target.value)}>
            {lib.collections.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
            <option value={NEW}>＋ {t("Nuova raccolta…")}</option>
          </select>
        </label>
        {cid === NEW && <input className="field" placeholder={t("Nome della nuova raccolta")} value={newName} onChange={(e) => setNewName(e.target.value)} />}
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
          <button className="btn ghost" disabled={busy} onClick={onClose}>{t("Chiudi")}</button>
          <button className="btn" disabled={busy || !text.trim()} onClick={add}>{busy ? t("Aggiunta…") : prefill ? t("Importa") : t("Aggiungi")}</button>
        </div>
      </div>
    </div>
  );
}
