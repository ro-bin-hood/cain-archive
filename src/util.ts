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

export function formatDate(unix: number): string {
  return unix ? new Date(unix * 1000).toLocaleDateString("it-IT", { day: "numeric", month: "short", year: "numeric" }) : "mai";
}

export function hitKey(h: { item_id: string; name: string }): string {
  return `${h.item_id}/${h.name}`;
}

/** Sigle per le sorgenti di un ambito: si toglie il prefisso comune (fino a un "_" o "-") e resta
 *  la parte che le distingue, es. microsoft_xbox360_a_part1 → a_part1. Con una sola sorgente: l'identificatore intero. */
export function tagLabels(ids: string[]): Map<string, string> {
  const uniq = [...new Set(ids)];
  let prefix = "";
  if (uniq.length > 1) {
    prefix = uniq.reduce((p, id) => {
      let i = 0;
      while (i < p.length && i < id.length && p[i] === id[i]) i++;
      return p.slice(0, i);
    });
    const cut = Math.max(prefix.lastIndexOf("_"), prefix.lastIndexOf("-"));
    prefix = cut >= 0 ? prefix.slice(0, cut + 1) : "";
  }
  return new Map(uniq.map((id) => {
    const rest = id.slice(prefix.length);
    return [id, uniq.length > 1 && rest ? rest : id];
  }));
}
