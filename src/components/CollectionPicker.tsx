import { useState } from "react";
import type { LibraryState } from "../api";
import { useT } from "../i18n";

type Props = { lib: LibraryState; onPick: (collectionId: string) => Promise<void>; onCreate: (name: string) => Promise<void>; onClose: () => void };

/** Sceglie la raccolta in cui salvare una sorgente Non salvata, o ne crea una nuova. */
export function CollectionPicker({ lib, onPick, onCreate, onClose }: Props) {
  const [name, setName] = useState("");
  const { t } = useT();
  const [error, setError] = useState<string | null>(null);
  const run = (p: Promise<void>) => p.catch((e) => setError(String(e)));
  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <b className="title">{t("Salva in una raccolta")}</b>
        {lib.collections.map((c) => (
          <button key={c.id} className="btn ghost" style={{ textAlign: "left" }} onClick={() => run(onPick(c.id))}>{c.name}</button>
        ))}
        <div className="row">
          <input className="field grow" placeholder={t("Nuova raccolta")} value={name} onChange={(e) => setName(e.target.value)} />
          <button className="btn" disabled={!name.trim()} onClick={() => run(onCreate(name))}>{t("Crea e salva")}</button>
        </div>
        {error && <span className="err small">{error}</span>}
        <div className="row"><div className="grow" /><button className="btn ghost" onClick={onClose}>{t("Annulla")}</button></div>
      </div>
    </div>
  );
}
