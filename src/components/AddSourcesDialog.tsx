import { useState } from "react";
import { api, type LibraryState, type ParsedList } from "../api";
import { useT } from "../i18n";
import { collectionLabel, orderedCollections } from "../util";

type Row = { key: string; input: string; section: string | null; status: "wait" | "ok" | "err"; msg: string };
type Props = { lib: LibraryState; initialCollection: string | null; prefill?: ParsedList; onLibrary: (s: LibraryState) => void; onClose: () => void };

const NEW = "__new__";

/** Adds one or more sources to a collection, one line at a time, showing each outcome.
 *  With `prefill` (file import) it starts with the lines and collection name filled in. */
export function AddSourcesDialog({ lib, initialCollection, prefill, onLibrary, onClose }: Props) {
  const { t, locale } = useT();
  // The file name is matched against top-level collections only.
  const existing = prefill?.name ? lib.collections.find((c) => !c.parent && c.name.toLowerCase() === prefill.name!.trim().toLowerCase()) : undefined;
  const sections = prefill?.sections ?? [];
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
    // Lines to add: those in the text field go to the chosen collection, then each section of the
    // file goes to the subcollection with the same name (created if missing). The key keeps the
    // same source in different sections apart; lines that already succeeded are not repeated.
    const done = new Set(rows.filter((r) => r.status === "ok").map((r) => r.key));
    const jobs: { key: string; input: string; section: string | null }[] = [
      ...[...new Set(text.split(/\s+/).filter(Boolean))].map((input) => ({ key: input, input, section: null })),
      ...sections.flatMap((s) => s.inputs.map((input) => ({ key: `${s.name}\u0000${input}`, input, section: s.name }))),
    ].filter((j) => !done.has(j.key));
    if (!jobs.length) return;
    setBusy(true);
    setRows((r) => [...r.filter((x) => x.status === "ok"), ...jobs.map((j) => ({ ...j, status: "wait" as const, msg: t("In attesa…") }))]);
    let state = await api.libraryState();
    const subIds = new Map<string, string>();
    for (const j of jobs) {
      const update = (status: Row["status"], msg: string) => setRows((r) => r.map((x) => (x.key === j.key ? { ...x, status, msg } : x)));
      let dest = target;
      if (j.section) {
        const name = j.section.toLowerCase();
        let id = subIds.get(name) ?? state.collections.find((c) => c.parent === target && c.name.toLowerCase() === name)?.id;
        if (!id) {
          try {
            state = await api.createSubcollection(target, j.section);
            id = state.collections.find((c) => c.parent === target && c.name.toLowerCase() === name)?.id;
          } catch (e) { update("err", String(e)); continue; }
        }
        if (!id) { update("err", t("Raccolta non trovata")); continue; }
        subIds.set(name, id);
        dest = id;
      }
      await api.addSource(dest, j.input).then((m) => update("ok", t("Aggiunta · {n} file", { n: m.file_count.toLocaleString(locale) }))).catch((e) => update("err", String(e)));
    }
    onLibrary(await api.libraryState());
    setBusy(false);
  };

  return (
    <div className="overlay" onClick={busy ? undefined : onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <b className="title">{prefill ? t("Importa raccolta") : t("Aggiungi sorgenti")}</b>
        <textarea className="field" rows={5} autoFocus placeholder={t("Un link archive.org o un identificatore per riga")} value={text} onChange={(e) => setText(e.target.value)} />
        {sections.length > 0 && (
          <span className="muted small">
            {t("Sotto-raccolte: {list}", { list: sections.map((s) => `${s.name} (${s.inputs.length})`).join(", ") })}
          </span>
        )}
        {prefill && prefill.invalid.length > 0 && (
          <span className="warn small" title={prefill.invalid.join("\n")}>
            {t(prefill.invalid.length === 1 ? "{n} riga non riconosciuta nel file, ignorata" : "{n} righe non riconosciute nel file, ignorate", { n: prefill.invalid.length })}
          </span>
        )}
        <label className="stack">
          <span className="section-title">{t("Raccolta")}</span>
          <select className="field" value={cid} onChange={(e) => setCid(e.target.value)}>
            {orderedCollections(lib).map((c) => <option key={c.id} value={c.id}>{collectionLabel(lib, c)}</option>)}
            <option value={NEW}>＋ {t("Nuova raccolta…")}</option>
          </select>
        </label>
        {cid === NEW && <input className="field" placeholder={t("Nome della nuova raccolta")} value={newName} onChange={(e) => setNewName(e.target.value)} />}
        {rows.length > 0 && (
          <div className="card" style={{ maxHeight: 200, overflow: "auto" }}>
            {rows.map((r) => (
              <div key={r.key} className="item-row">
                <span className="grow name" title={r.input}>{r.section && <span className="src">{r.section}</span>} {r.input}</span>
                <span className={`small ${r.status === "ok" ? "ok" : r.status === "err" ? "err" : "muted"}`}>{r.msg}</span>
              </div>
            ))}
          </div>
        )}
        {error && <span className="err small">{error}</span>}
        <div className="row">
          <div className="grow" />
          <button className="btn ghost" disabled={busy} onClick={onClose}>{t("Chiudi")}</button>
          <button className="btn" disabled={busy || (!text.trim() && !sections.length)} onClick={add}>{busy ? t("Aggiunta…") : prefill ? t("Importa") : t("Aggiungi")}</button>
        </div>
      </div>
    </div>
  );
}
