import { memo, useEffect, useState, type MouseEvent } from "react";
import { api, type Job, type JobProgress } from "../api";
import { human } from "../util";
import { useT, type T } from "../i18n";

function statusText(t: T, j: Job, p: JobProgress | undefined, left: number): { text: string; cls: string } {
  const s = j.status;
  switch (s.kind) {
    case "Queued": return { text: `${j.size ? human(j.size) + " · " : ""}${t("In coda")}`, cls: "" };
    case "Downloading":
      return p
        ? { text: `${human(p.done)} / ${p.total ? human(p.total) : "?"} · ${human(p.speed)}/s`, cls: "" }
        : { text: t("Avvio…"), cls: "" };
    case "Retrying": return { text: `${left > 0 ? t("Riprovo tra {n} s", { n: left }) : t("Riprovo…")} (${s.attempt}/3)`, cls: "warn" };
    case "Paused": return { text: t("In pausa"), cls: "" };
    case "Done": return { text: `✔ ${t("Completato")} · ${human(j.size)}`, cls: "ok" };
    case "Failed": return { text: `✘ ${s.reason}`, cls: "err" };
  }
}

function Row({ job, live, onGrab }: { job: Job; live?: JobProgress; onGrab: (e: MouseEvent, id: number) => void }) {
  const { t } = useT();
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
  const { text, cls } = statusText(t, job, live, left);
  const k = job.status.kind;
  const pct = live && live.total ? (live.done / live.total) * 100 : 0;
  const act = (title: string, icon: string, fn: () => void) => (
    <button className="icon-btn" title={title} onClick={fn}>{icon}</button>
  );
  return (
    <div className="item-row" data-job={job.id}>
      <span className="grab" title={t("Trascina per riordinare")} onMouseDown={(e) => onGrab(e, job.id)}>⋮⋮</span>
      <div className="grow">
        <div className="name" title={`${job.item_id}/${job.name}`}>{job.name}</div>
        {k === "Downloading" && <div className="bar" style={{ margin: "5px 0 3px" }}><i style={{ width: `${pct}%` }} /></div>}
        <div className={`small muted ${cls}`}>{text}</div>
      </div>
      {(k === "Downloading" || k === "Retrying") && act(t("Pausa"), "⏸", () => api.pauseJob(job.id))}
      {k === "Paused" && act(t("Riprendi"), "▶", () => api.resumeJob(job.id))}
      {k === "Failed" && act(t("Riprova"), "↻", () => api.retryJob(job.id))}
      {k === "Done" && act(t("Apri cartella"), "📂", () => api.openFolder(job.id))}
      {act(t("Rimuovi"), "✕", () => api.removeJob(job.id))}
    </div>
  );
}

/** Con migliaia di file in coda, ridisegna solo le righe che sono cambiate davvero. */
export const QueueRow = memo(
  Row,
  (a, b) => a.live === b.live && a.onGrab === b.onGrab && JSON.stringify(a.job) === JSON.stringify(b.job),
);
