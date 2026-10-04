import type { Job, Snapshot } from "../api";
import { duration, human } from "../util";
import { useT } from "../i18n";

type Props = { jobs: Job[]; running: boolean; progress: Snapshot | null; onOpen: () => void };

/** Always-visible bottom strip: overall progress; a click opens the queue. */
export function QueueStrip({ jobs, running, progress, onOpen }: Props) {
  const { t } = useT();
  const active = jobs.filter((j) => j.status.kind === "Downloading" || j.status.kind === "Retrying").length;
  const pct = progress && progress.total ? Math.min(100, (progress.done / progress.total) * 100) : 0;
  let text: string;
  if (running && active > 0 && progress) {
    text = `⬇ ${t("{n} in corso", { n: active })} · ${pct.toFixed(0)}% · ${human(progress.speed)}/s${progress.eta_s != null ? ` · ${duration(progress.eta_s)}` : ""}`;
  } else {
    text = jobs.length ? t("Coda: {n} file", { n: jobs.length }) : t("Coda vuota");
  }
  return (
    <footer className="strip" onClick={onOpen} title={t("Apri o chiudi la coda")}>
      <span className={running ? "" : "muted"}>{text}</span>
      <div className="grow" />
      {running && <div className="bar" style={{ width: 180 }}><i style={{ width: `${pct}%` }} /></div>}
    </footer>
  );
}
