import { useMemo, useRef } from "react";
import { api, type Job, type Snapshot } from "../api";
import { duration, human } from "../util";
import { QueueRow } from "./QueueRow";
import { useT } from "../i18n";
import { useDragReorder } from "../dragReorder";

type Props = { jobs: Job[]; running: boolean; progress: Snapshot | null; onClose: () => void };

export function QueueView({ jobs, running, progress, onClose }: Props) {
  const { t } = useT();
  const listRef = useRef<HTMLDivElement>(null);
  const orderKey = useMemo(() => jobs.map((j) => j.id).join(), [jobs]);
  const grab = useDragReorder({
    rows: () => [...(listRef.current?.querySelectorAll<HTMLElement>("[data-drag-id]") ?? [])],
    onMove: (id, before) => { api.moveJob(Number(id), before === null ? null : Number(before)); },
    orderKey,
  });
  const live = new Map((progress?.jobs ?? []).map((p) => [p.id, p]));
  const canStart = jobs.some((j) => j.status.kind === "Queued" || j.status.kind === "Paused");
  const hasDone = jobs.some((j) => j.status.kind === "Done");
  const pct = progress && progress.total ? Math.min(100, (progress.done / progress.total) * 100) : 0;
  return (
    <>
      <div className="row">
        <span className="section-title">{t("queue.title", { n: jobs.length })}</span>
        {running && progress && progress.total > 0 && (
          <span className="muted small">
            · {t("queue.progress", { done: human(progress.done), total: human(progress.total) })} · {human(progress.speed)}/s
            {progress.eta_s != null ? ` · ${duration(progress.eta_s)}` : ""}
          </span>
        )}
        <div className="grow" />
        {hasDone && <button className="btn small ghost" onClick={() => api.clearCompleted()}>{t("queue.clearCompleted")}</button>}
        {running ? (
          <button className="btn small ghost" onClick={() => api.stop()}>■ {t("queue.stop")}</button>
        ) : (
          <button className="btn small" disabled={!canStart} onClick={() => api.start()}>▶ {t("queue.start")}</button>
        )}
        <button className="icon-btn" title={t("queue.backToSearch")} onClick={onClose}>✕</button>
      </div>
      {running && <div className="bar"><i style={{ width: `${pct}%` }} /></div>}
      <div className="card queue-list" ref={listRef}>
        {jobs.length === 0 ? (
          <div className="empty">{t("queue.empty")}</div>
        ) : (
          jobs.map((j) => (
            <div key={j.id} className="queue-slot" data-drag-id={j.id} onMouseDown={(e) => grab(e, String(j.id))}>
              <QueueRow job={j} live={live.get(j.id)} />
            </div>
          ))
        )}
      </div>
    </>
  );
}
