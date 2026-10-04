import { useState, type FormEvent } from "react";
import { useT } from "../i18n";

type Props = { title: string; initial?: string; confirm: string; onSubmit: (name: string) => Promise<void>; onClose: () => void };

/** Dialog with a single text field; onSubmit errors appear below the field. */
export function NameDialog({ title, initial = "", confirm, onSubmit, onClose }: Props) {
  const [name, setName] = useState(initial);
  const { t } = useT();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    try {
      await onSubmit(name);
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  };
  return (
    <div className="overlay" onClick={onClose}>
      <form className="modal" onClick={(e) => e.stopPropagation()} onSubmit={submit}>
        <b className="title">{title}</b>
        <input className="field" autoFocus value={name} onChange={(e) => setName(e.target.value)} />
        {error && <span className="err small">{error}</span>}
        <div className="row">
          <div className="grow" />
          <button type="button" className="btn ghost" onClick={onClose}>{t("Annulla")}</button>
          <button className="btn" disabled={busy}>{confirm}</button>
        </div>
      </form>
    </div>
  );
}
