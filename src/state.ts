import type { Job, Scope, Snapshot } from "./api";

export type QueueState = { jobs: Job[]; running: boolean; progress: Snapshot | null };
export type QueueAction = { type: "changed"; jobs: Job[]; running: boolean } | { type: "progress"; snap: Snapshot };

export const initialQueue: QueueState = { jobs: [], running: false, progress: null };

export function queueReducer(s: QueueState, a: QueueAction): QueueState {
  switch (a.type) {
    case "changed": return { ...s, jobs: a.jobs, running: a.running };
    case "progress": return { ...s, progress: a.snap };
  }
}

export type View = { kind: "search"; scope: Scope } | { kind: "queue" };

export function scopeKey(s: Scope): string {
  return s.kind === "collection" ? `c:${s.id}` : s.kind === "source" ? `s:${s.item_id}` : s.kind;
}
