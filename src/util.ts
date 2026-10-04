import type { FileEntry } from "./api";

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

export function parseExts(s: string): string[] {
  return s.split(",").map((x) => x.trim().toLowerCase().replace(/^\./, "")).filter(Boolean);
}

export function duration(s: number): string {
  if (s < 60) return `${s} s`;
  if (s < 3600) return `~${Math.round(s / 60)} min`;
  return `~${(s / 3600).toFixed(1)} h`;
}

/** Nessuna estensione attiva = tutte le estensioni. */
export function preselect(files: FileEntry[], originalsOnly: boolean, exts: string[]): Set<string> {
  return new Set(
    files
      .filter((f) => (!originalsOnly || f.original) && (exts.length === 0 || exts.includes(ext(f.name))))
      .map((f) => f.name),
  );
}
