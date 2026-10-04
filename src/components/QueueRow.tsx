import { memo, useEffect, useState } from "react";
import { api, type Job, type JobProgress } from "../api";
import { human } from "../util";

function statusText(j: Job, p: JobProgress | undefined, left: number): { text: string; cls: string } {
  const s = j.status;
  switch (s.kind) {
    case "Queued": return { text: `${j.size ? human(j.size) + " · " : ""}In coda`, cls: "" };
    case "Downloading":
      return p
        ? { text: `${human(p.done)} / ${p.total ? human(p.total) : "?"} · ${human(p.speed)}/s`, cls: "" }
        : { text: "Avvio…", cls: "" };
    case "Retrying": return { text: `${left > 0 ? `Riprovo tra ${left} s` : "Riprovo…"} (${s.attempt}/3)`, cls: "warn" };
    case "Paused": return { text: "In pausa", cls: "" };
    case "Done": return { text: `✔ Completato · ${human(j.size)}`, cls: "ok" };
    case "Failed": return { text: `✘ ${s.reason}`, cls: "err" };
  }
}

function Row({ job, live }: { job: Job; live?: JobProgress }) {
  // Conto alla rovescia del nuovo tentativo, che riparte a ogni nuovo stato Retrying.
  const retryKey = job.status.kind === "Retrying" ? `${job.status.attempt}:${job.status.wait_s}` : "";
  const [left, setLeft] = useState(0);
  useEffect(() => {
    if (job.status.kind !== "Retrying") return;
    setLeft(job.status.wait_s);
    const t = setInterval(() => setLeft((l) => Math.max(0, l - 1)), 1000);
    return () => clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [retryKey]);
  const { text, cls } = statusText(job, live, left);
  const k = job.status.kind;
  const pct = live && live.total ? (live.done / live.total) * 100 : 0;
  const act = (title: string, icon: string, fn: () => void) => (
    <button className="icon-btn" title={title} onClick={fn}>{icon}</button>
  );
  return (
    <div className="item-row">
      <div className="grow">
        <div className="name" title={`${job.item_id}/${job.name}`}>{job.name}</div>
        {k === "Downloading" && <div className="bar" style={{ margin: "5px 0 3px" }}><i style={{ width: `${pct}%` }} /></div>}
        <div className={`small muted ${cls}`}>{text}</div>
      </div>
      {(k === "Downloading" || k === "Retrying") && act("Pausa", "⏸", () => api.pauseJob(job.id))}
      {k === "Paused" && act("Riprendi", "▶", () => api.resumeJob(job.id))}
      {k === "Failed" && act("Riprova", "↻", () => api.retryJob(job.id))}
      {k === "Done" && act("Apri cartella", "📂", () => api.openFolder(job.id))}
      {act("Rimuovi", "✕", () => api.removeJob(job.id))}
    </div>
  );
}

/** Con migliaia di file in coda, ridisegna solo le righe che sono cambiate davvero. */
export const QueueRow = memo(
  Row,
  (a, b) => a.live === b.live && JSON.stringify(a.job) === JSON.stringify(b.job),
);
