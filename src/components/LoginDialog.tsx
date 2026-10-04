import { useState, type FormEvent } from "react";
import { api } from "../api";
import { useT } from "../i18n";

type Props = { onDone: (user: string) => void; onClose: () => void };

export function LoginDialog({ onDone, onClose }: Props) {
  const [email, setEmail] = useState("");
  const { t, te } = useT();
  const [pw, setPw] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!email || !pw) return;
    setBusy(true);
    setError(null);
    try {
      onDone(await api.login(email, pw));
    } catch (err) {
      setError(te(err));
      setBusy(false);
    }
  };
  return (
    <div className="overlay" onClick={onClose}>
      <form className="modal" onClick={(e) => e.stopPropagation()} onSubmit={submit}>
        <b className="title">{t("login.title")}</b>
        <span className="muted small">{t("login.hint")}</span>
        <input className="field" type="email" placeholder={t("login.email")} autoFocus value={email} onChange={(e) => setEmail(e.target.value)} />
        <input className="field" type="password" placeholder={t("login.password")} value={pw} onChange={(e) => setPw(e.target.value)} />
        {error && <span className="err small">{error}</span>}
        <div className="row">
          <div className="grow" />
          <button type="button" className="btn ghost" onClick={onClose}>{t("common.cancel")}</button>
          <button className="btn" disabled={busy}>{busy ? t("login.loggingIn") : t("titlebar.logIn")}</button>
        </div>
      </form>
    </div>
  );
}
