import { useState } from "react";
import { useT } from "../i18n";

type Props = { title: string; message: string; confirm: string; danger?: boolean; onConfirm: () => Promise<void>; onClose: () => void };

/** Asks before an action that can't be undone; onConfirm errors appear in the dialog. */
export function ConfirmDialog({ title, message, confirm, danger, onConfirm, onClose }: Props) {
  const { t, te } = useT();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const run = async () => {
    setBusy(true);
    try {
      await onConfirm();
    } catch (err) {
      setError(te(err));
      setBusy(false);
    }
  };
  return (
    <div className="overlay" onClick={onClose} onKeyDown={(e) => e.key === "Escape" && onClose()}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <b className="title">{title}</b>
        <span className="muted">{message}</span>
        {error && <span className="err small">{error}</span>}
        <div className="row">
          <div className="grow" />
          <button type="button" className="btn ghost" onClick={onClose}>{t("common.cancel")}</button>
          <button className={`btn ${danger ? "danger" : ""}`} autoFocus disabled={busy} onClick={run}>{confirm}</button>
        </div>
      </div>
    </div>
  );
}
