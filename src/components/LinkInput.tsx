import { useState } from "react";

type Props = { busy: boolean; onAnalyze: (text: string) => Promise<boolean> };

export function LinkInput({ busy, onAnalyze }: Props) {
  const [text, setText] = useState("");
  const submit = async () => {
    if (text.trim() && (await onAnalyze(text))) setText("");
  };
  return (
    <div className="row" style={{ alignItems: "flex-start" }}>
      <textarea
        className="field grow"
        rows={Math.min(6, Math.max(1, text.split("\n").length))}
        placeholder="Incolla uno o più link o identificatori archive.org… (Ctrl+Invio per analizzare)"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter" && e.ctrlKey) submit(); }}
      />
      <button className="btn" disabled={busy || !text.trim()} onClick={submit}>
        {busy ? "Analisi…" : "Analizza"}
      </button>
    </div>
  );
}
