import { useCallback, useRef, useState, type MouseEvent } from "react";
import { api, type Job, type Snapshot } from "../api";
import { duration, human } from "../util";
import { QueueRow } from "./QueueRow";
import { useT } from "../i18n";

type Props = { jobs: Job[]; running: boolean; progress: Snapshot | null; onClose: () => void };

export function QueueView({ jobs, running, progress, onClose }: Props) {
  const { t } = useT();
  // Mouse-based reordering (HTML5 drag & drop doesn't work in the WebView while file dropping
  // is enabled): `over` is where the dragged file will land.
  const listRef = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<{ id: number; over: number } | null>(null);
  const onGrab = useCallback((e: MouseEvent, id: number) => {
    e.preventDefault();
    const rows = () => [...(listRef.current?.querySelectorAll<HTMLElement>("[data-job]") ?? [])];
    const indexAt = (y: number) => {
      const i = rows().findIndex((r) => { const b = r.getBoundingClientRect(); return y < b.top + b.height / 2; });
      return i < 0 ? rows().length : i;
    };
    let over = indexAt(e.clientY);
    setDrag({ id, over });
    const move = (ev: globalThis.MouseEvent) => { over = indexAt(ev.clientY); setDrag({ id, over }); };
    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      document.body.classList.remove("dragging-row");
      setDrag(null);
      const ids = rows().map((r) => Number(r.dataset.job));
      const from = ids.indexOf(id);
      // Dropped on itself or right after itself: no move.
      if (from < 0 || over === from || over === from + 1) return;
      api.moveJob(id, over < ids.length ? ids[over] : null);
    };
    document.body.classList.add("dragging-row");
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  }, []);
  const live = new Map((progress?.jobs ?? []).map((p) => [p.id, p]));
  const canStart = jobs.some((j) => j.status.kind === "Queued" || j.status.kind === "Paused");
  const hasDone = jobs.some((j) => j.status.kind === "Done");
  const pct = progress && progress.total ? Math.min(100, (progress.done / progress.total) * 100) : 0;
  return (
    <>
      <div className="row">
        <span className="section-title">{t("Coda · {n} file", { n: jobs.length })}</span>
        {running && progress && progress.total > 0 && (
          <span className="muted small">
            · {t("{done} di {total}", { done: human(progress.done), total: human(progress.total) })} · {human(progress.speed)}/s
            {progress.eta_s != null ? ` · ${duration(progress.eta_s)}` : ""}
          </span>
        )}
        <div className="grow" />
        {hasDone && <button className="btn small ghost" onClick={() => api.clearCompleted()}>{t("Pulisci completati")}</button>}
        {running ? (
          <button className="btn small ghost" onClick={() => api.stop()}>■ Stop</button>
        ) : (
          <button className="btn small" disabled={!canStart} onClick={() => api.start()}>▶ {t("Avvia")}</button>
        )}
        <button className="icon-btn" title={t("Torna alla ricerca")} onClick={onClose}>✕</button>
      </div>
      {running && <div className="bar"><i style={{ width: `${pct}%` }} /></div>}
      <div className={`card queue-list ${drag && drag.over >= jobs.length ? "drop-end" : ""}`} ref={listRef}>
        {jobs.length === 0 ? (
          <div className="empty">{t("La coda è vuota: cerca dei file e aggiungili con «Aggiungi alla coda».")}</div>
        ) : (
          jobs.map((j, i) => (
            <div key={j.id} className={drag && drag.over === i ? "drop-before" : drag?.id === j.id ? "dragged" : ""}>
              <QueueRow job={j} live={live.get(j.id)} onGrab={onGrab} />
            </div>
          ))
        )}
      </div>
    </>
  );
}
