import { memo, useEffect, useState } from "react";
import { api, type Job, type JobProgress } from "../api";
import { human } from "../util";
import { useT, type T, type TE } from "../i18n";

function statusText(t: T, te: TE, j: Job, p: JobProgress | undefined, left: number): { text: string; cls: string } {
  const s = j.status;
  switch (s.kind) {
    case "Queued": return { text: `${j.size ? human(j.size) + " · " : ""}${t("queue.statusQueued")}`, cls: "" };
    case "Downloading":
      return p
        ? { text: `${human(p.done)} / ${p.total ? human(p.total) : "?"} · ${human(p.speed)}/s`, cls: "" }
        : { text: t("queue.statusStarting"), cls: "" };
    case "Retrying": return { text: `${left > 0 ? t("queue.statusRetryIn", { n: left }) : t("queue.statusRetrying")} (${s.attempt}/3)`, cls: "warn" };
    case "Paused": return { text: t("queue.statusPaused"), cls: "" };
    case "Done": return { text: `✔ ${t("queue.statusDone")} · ${human(j.size)}`, cls: "ok" };
    case "Failed": return { text: `✘ ${te(s.reason)}`, cls: "err" };
  }
}

function Row({ job, live }: { job: Job; live?: JobProgress }) {
  const { t, te } = useT();
  // Countdown to the next attempt, restarting on every new Retrying state.
  const retryKey = job.status.kind === "Retrying" ? `${job.status.attempt}:${job.status.wait_s}` : "";
  const [left, setLeft] = useState(0);
  useEffect(() => {
    if (job.status.kind !== "Retrying") return;
    setLeft(job.status.wait_s);
    const t = setInterval(() => setLeft((l) => Math.max(0, l - 1)), 1000);
    return () => clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [retryKey]);
  const { text, cls } = statusText(t, te, job, live, left);
  const k = job.status.kind;
  const pct = live && live.total ? (live.done / live.total) * 100 : 0;
  const act = (title: string, icon: string, fn: () => void) => (
    <button className="icon-btn" title={title} onClick={fn}>{icon}</button>
  );
  return (
    <div className="item-row" data-job={job.id}>
      <span className="grab" title={t("common.dragToReorder")}>⋮⋮</span>
      <div className="grow">
        <div className="name" title={`${job.item_id}/${job.name}`}>{job.name}</div>
        {k === "Downloading" && <div className="bar" style={{ margin: "5px 0 3px" }}><i style={{ width: `${pct}%` }} /></div>}
        <div className={`small muted ${cls}`}>{text}</div>
      </div>
      {(k === "Downloading" || k === "Retrying") && act(t("queue.pause"), "⏸", () => api.pauseJob(job.id))}
      {k === "Paused" && act(t("queue.resume"), "▶", () => api.resumeJob(job.id))}
      {k === "Failed" && act(t("queue.retry"), "↻", () => api.retryJob(job.id))}
      {k === "Done" && act(t("queue.openFolder"), "📂", () => api.openFolder(job.id))}
      {act(t("queue.remove"), "✕", () => api.removeJob(job.id))}
    </div>
  );
}

/** With thousands of queued files, only rows that really changed are redrawn. */
export const QueueRow = memo(
  Row,
  (a, b) => a.live === b.live && JSON.stringify(a.job) === JSON.stringify(b.job),
);
