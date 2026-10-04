import { api, type Job, type Snapshot } from "../api";
import { duration, human } from "../util";
import { QueueRow } from "./QueueRow";
import { useT } from "../i18n";

type Props = { jobs: Job[]; running: boolean; progress: Snapshot | null };

export function QueueView({ jobs, running, progress }: Props) {
  const { t } = useT();
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
      </div>
      {running && <div className="bar"><i style={{ width: `${pct}%` }} /></div>}
      <div className="card">
        {jobs.length === 0 ? (
          <div className="empty">{t("La coda è vuota: cerca dei file e aggiungili con «Aggiungi alla coda».")}</div>
        ) : (
          jobs.map((j) => <QueueRow key={j.id} job={j} live={live.get(j.id)} />)
        )}
      </div>
    </>
  );
}
