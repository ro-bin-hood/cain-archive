import { useCallback, useLayoutEffect, useRef, type MouseEvent } from "react";

/** Pixels the mouse must travel before a press becomes a drag (below that it stays a click). */
const THRESHOLD = 4;
const EASE = "transform 160ms ease";

type Opts = {
  /** The reorderable siblings of `id`, in display order; each carries `data-drag-id`. */
  rows: (id: string) => HTMLElement[];
  /** Moves `id` before `before` (null = to the end). */
  onMove: (id: string, before: string | null) => void;
  /** Changes when the displayed order changes: the dropped rows then settle into the new layout. */
  orderKey: string;
};

const clear = (els: HTMLElement[], animate: boolean) => {
  for (const el of els) {
    el.style.transition = animate ? EASE : "";
    el.style.transform = "";
    delete el.dataset.dragging;
  }
  if (animate) setTimeout(() => els.forEach((el) => { if (!el.dataset.dragging) el.style.transition = ""; }), 200);
};

/**
 * Mouse-based drag to reorder (HTML5 drag & drop doesn't work in the WebView while file dropping
 * is enabled). A row can be grabbed anywhere except on its controls: it follows the pointer
 * while the others slide out of its way, and on drop it stays in its new slot until the new
 * order arrives.
 */
export function useDragReorder({ rows, onMove, orderKey }: Opts) {
  const opts = useRef({ rows, onMove });
  opts.current = { rows, onMove };
  // Rows left translated after a drop, waiting for the new order.
  const settling = useRef<{ els: HTMLElement[]; timer: number } | null>(null);

  useLayoutEffect(() => {
    const s = settling.current;
    if (!s) return;
    settling.current = null;
    clearTimeout(s.timer);
    clear(s.els, false);
  }, [orderKey]);

  return useCallback((e: MouseEvent, id: string) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button,input,select,textarea,a,.menu")) return;
    const startY = e.clientY;
    let drag: { els: HTMLElement[]; from: number; tops: number[]; mids: number[]; slots: number[]; min: number; max: number; to: number } | null = null;

    const begin = () => {
      const els = opts.current.rows(id);
      const from = els.findIndex((el) => el.dataset.dragId === id);
      if (from < 0) return false;
      const rects = els.map((el) => el.getBoundingClientRect());
      // Space each row takes, gap included.
      const slots = rects.map((r, i) => i < rects.length - 1 ? rects[i + 1].top - r.top : i > 0 ? r.bottom - rects[i - 1].bottom : r.height);
      drag = {
        els, from, to: from, slots,
        tops: rects.map((r) => r.top),
        mids: rects.map((r) => r.top + r.height / 2),
        min: rects[0].top - rects[from].top,
        max: rects[rects.length - 1].bottom - rects[from].bottom,
      };
      els.forEach((el, i) => { el.style.transition = i === from ? "" : EASE; });
      els[from].dataset.dragging = "";
      document.body.classList.add("dragging-row");
      return true;
    };

    const move = (ev: globalThis.MouseEvent) => {
      if (!drag && (Math.abs(ev.clientY - startY) < THRESHOLD || !begin())) return;
      const d = drag!;
      const dy = Math.max(d.min, Math.min(d.max, ev.clientY - startY));
      const center = d.mids[d.from] + dy;
      // Slot among the other rows: how many of them stay above the dragged one.
      d.to = d.mids.filter((m, i) => i !== d.from && m < center).length;
      d.els.forEach((el, i) => {
        if (i === d.from) el.style.transform = `translateY(${dy}px)`;
        else if (i < d.from && i >= d.to) el.style.transform = `translateY(${d.slots[d.from]}px)`;
        else if (i > d.from && i <= d.to) el.style.transform = `translateY(${-d.slots[d.from]}px)`;
        else el.style.transform = "";
      });
    };

    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      if (!drag) return;
      const d = drag as NonNullable<typeof drag>;
      document.body.classList.remove("dragging-row");
      // The click that ends a drag must not also select the row.
      const swallow = (ev: Event) => ev.stopPropagation();
      window.addEventListener("click", swallow, true);
      setTimeout(() => window.removeEventListener("click", swallow, true), 0);

      const dragged = d.els[d.from];
      if (d.to === d.from) { clear(d.els, true); return; }
      // Glide into the free slot, then hold there until the new order is rendered.
      const passed = d.to > d.from ? d.slots.slice(d.from + 1, d.to + 1) : d.slots.slice(d.to, d.from);
      const offset = passed.reduce((a, b) => a + b, 0) * (d.to > d.from ? 1 : -1);
      dragged.style.transition = EASE;
      dragged.style.transform = `translateY(${offset}px)`;
      delete dragged.dataset.dragging;
      const others = d.els.filter((_, i) => i !== d.from);
      const before = others[d.to]?.dataset.dragId ?? null;
      const prev = settling.current;
      if (prev) { clearTimeout(prev.timer); clear(prev.els, false); }
      // If the order never changes (the move failed), slide everything back.
      settling.current = { els: d.els, timer: window.setTimeout(() => { settling.current = null; clear(d.els, true); }, 1500) };
      opts.current.onMove(id, before);
    };

    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  }, []);
}
