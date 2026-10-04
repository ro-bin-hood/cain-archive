import { api, type Job, type Snapshot } from "../api";
import { duration, human } from "../util";
import { QueueRow } from "./QueueRow";

type Props = { jobs: Job[]; running: boolean; progress: Snapshot | null };

export function QueueView({ jobs, running, progress }: Props) {
  const live = new Map((progress?.jobs ?? []).map((p) => [p.id, p]));
  const canStart = jobs.some((j) => j.status.kind === "Queued" || j.status.kind === "Paused");
  const hasDone = jobs.some((j) => j.status.kind === "Done");
  const pct = progress && progress.total ? Math.min(100, (progress.done / progress.total) * 100) : 0;
  return (
    <>
      <div className="row">
        <span className="section-title">Coda · {jobs.length} file</span>
        {running && progress && progress.total > 0 && (
          <span className="muted small">
            · {human(progress.done)} di {human(progress.total)} · {human(progress.speed)}/s
            {progress.eta_s != null ? ` · ${duration(progress.eta_s)}` : ""}
          </span>
        )}
        <div className="grow" />
        {hasDone && <button className="btn small ghost" onClick={() => api.clearCompleted()}>Pulisci completati</button>}
        {running ? (
          <button className="btn small ghost" onClick={() => api.stop()}>■ Stop</button>
        ) : (
          <button className="btn small" disabled={!canStart} onClick={() => api.start()}>▶ Avvia</button>
        )}
      </div>
      {running && <div className="bar"><i style={{ width: `${pct}%` }} /></div>}
      <div className="card">
        {jobs.length === 0 ? (
          <div className="empty">La coda è vuota. Incolla un link e premi Analizza.</div>
        ) : (
          jobs.map((j) => <QueueRow key={j.id} job={j} live={live.get(j.id)} />)
        )}
      </div>
    </>
  );
}
