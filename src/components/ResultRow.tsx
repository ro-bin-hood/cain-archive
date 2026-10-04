import { memo, type ReactNode } from "react";
import type { Hit } from "../api";
import { human } from "../util";

/** Il nome con le parole cercate evidenziate (senza distinguere maiuscole e minuscole). */
function highlight(name: string, words: string[]): ReactNode {
  if (!words.length) return name;
  const lower = name.toLowerCase();
  const marks = new Array<boolean>(name.length).fill(false);
  for (const w of words) {
    for (let i = lower.indexOf(w); i >= 0; i = lower.indexOf(w, i + 1)) marks.fill(true, i, i + w.length);
  }
  const out: ReactNode[] = [];
  let start = 0;
  for (let i = 1; i <= name.length; i++) {
    if (i === name.length || marks[i] !== marks[start]) {
      const part = name.slice(start, i);
      out.push(marks[start] ? <mark key={start}>{part}</mark> : part);
      start = i;
    }
  }
  return out;
}

type Props = { hit: Hit; tag: string; checked: boolean; words: string[]; onToggle: (h: Hit) => void };

function Row({ hit, tag, checked, words, onToggle }: Props) {
  return (
    <label className="item-row" style={{ cursor: "pointer" }}>
      <input type="checkbox" className="check" checked={checked} onChange={() => onToggle(hit)} />
      <span className={`grow name ${hit.original ? "" : "muted"}`} title={hit.name}>{highlight(hit.name, words)}</span>
      <span className="src" title={hit.item_id}>{tag}</span>
      <span className="muted small size">{hit.size ? human(hit.size) : "?"}</span>
    </label>
  );
}

export const ResultRow = memo(Row);
