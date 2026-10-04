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
  // La punteggiatura finale ("…/details/foo." in una frase) non fa parte del link.
  return (text.match(/https?:\/\/(?:www\.)?archive\.org\/\S+/g) ?? []).map((l) => l.replace(/[.,;:!?)\]}>"']+$/, ""));
}

/** Data breve nella lingua dell'interfaccia; "" se manca (il chiamante mostra "mai"). */
export function formatDate(unix: number, locale: string): string {
  return unix ? new Date(unix * 1000).toLocaleDateString(locale, { day: "numeric", month: "short", year: "numeric" }) : "";
}

export function hitKey(h: { item_id: string; name: string }): string {
  return `${h.item_id}/${h.name}`;
}

/** Sigle per le sorgenti di un ambito. Le sorgenti della stessa "famiglia" (stessa prima parola
 *  dell'identificatore) perdono il prefisso che hanno in comune, fino a un "_" o "-":
 *  microsoft_xbox360_a_part1 → a_part1 anche accanto a nasa, che resta nasa. */
export function tagLabels(ids: string[]): Map<string, string> {
  const uniq = [...new Set(ids)];
  const family = (id: string) => id.split(/[_-]/)[0];
  const groups = new Map<string, string[]>();
  for (const id of uniq) groups.set(family(id), [...(groups.get(family(id)) ?? []), id]);
  const out = new Map<string, string>();
  for (const members of groups.values()) {
    let prefix = "";
    if (members.length > 1) {
      prefix = members.reduce((p, id) => {
        let i = 0;
        while (i < p.length && i < id.length && p[i] === id[i]) i++;
        return p.slice(0, i);
      });
      const cut = Math.max(prefix.lastIndexOf("_"), prefix.lastIndexOf("-"));
      prefix = cut >= 0 ? prefix.slice(0, cut + 1) : "";
    }
    for (const id of members) out.set(id, id.slice(prefix.length) || id);
  }
  return out;
}
