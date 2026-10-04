export function human(n: number): string {
  const u = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
  return `${n.toFixed(i === 0 ? 0 : 1)} ${u[i]}`;
}

export function ext(name: string): string {
  const base = name.split("/").pop() ?? name;
  const d = base.lastIndexOf(".");
  return d > 0 ? base.slice(d + 1).toLowerCase() : "";
}

export function duration(s: number): string {
  if (s < 60) return `${s} s`;
  if (s < 3600) return `~${Math.round(s / 60)} min`;
  return `~${(s / 3600).toFixed(1)} h`;
}

/** Gli URL archive.org contenuti nel testo, nell'ordine in cui compaiono. */
export function extractLinks(text: string): string[] {
  return text.match(/https?:\/\/(?:www\.)?archive\.org\/\S+/g) ?? [];
}

/** Sigla breve di una sorgente: l'ultimo pezzo dell'identificatore se corto, altrimenti l'inizio. */
export function shortId(id: string): string {
  const last = id.split(/[_-]/).pop() ?? id;
  return last.length <= 6 ? last.toUpperCase() : id.length <= 12 ? id : id.slice(0, 11) + "…";
}

export function formatDate(unix: number): string {
  return unix ? new Date(unix * 1000).toLocaleDateString("it-IT", { day: "numeric", month: "short", year: "numeric" }) : "mai";
}

export function hitKey(h: { item_id: string; name: string }): string {
  return `${h.item_id}/${h.name}`;
}
