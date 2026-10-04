import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type FileEntry = { name: string; size: number; format: string; original: boolean };
export type Item = { id: string; files: FileEntry[] };
export type Analyzed = { input: string; item: Item | null; error: string | null };
export type JobStatus =
  | { kind: "Queued" }
  | { kind: "Downloading" }
  | { kind: "Retrying"; attempt: number; wait_s: number }
  | { kind: "Paused" }
  | { kind: "Done" }
  | { kind: "Failed"; reason: string };
export type Job = { id: number; item_id: string; name: string; size: number; dest: string; status: JobStatus };
export type Theme = "system" | "light" | "dark";
export type Settings = { out_dir: string; workers: number; default_originals: boolean; default_exts: string; theme: Theme };
export type FullState = { settings: Settings; user: string | null; jobs: Job[]; running: boolean };
export type JobProgress = { id: number; done: number; total: number | null; speed: number };
export type Snapshot = { jobs: JobProgress[]; done: number; total: number; speed: number; eta_s: number | null };
export type NewFile = { item_id: string; name: string; size: number };

export const api = {
  getState: () => invoke<FullState>("get_state"),
  analyze: (text: string) => invoke<Analyzed[]>("analyze_links", { text }),
  enqueue: (files: NewFile[]) => invoke<void>("enqueue", { files }),
  start: () => invoke<void>("start"),
  stop: () => invoke<void>("stop"),
  pauseJob: (id: number) => invoke<void>("pause_job", { id }),
  resumeJob: (id: number) => invoke<void>("resume_job", { id }),
  retryJob: (id: number) => invoke<void>("retry_job", { id }),
  removeJob: (id: number) => invoke<void>("remove_job", { id }),
  clearCompleted: () => invoke<void>("clear_completed"),
  openFolder: (id: number) => invoke<void>("open_folder", { id }),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  pickFolder: () => invoke<string | null>("pick_folder"),
  login: (email: string, password: string) => invoke<string>("login", { email, password }),
  logout: () => invoke<void>("logout"),
};

export const events = {
  onChanged: (cb: (p: { jobs: Job[]; running: boolean }) => void) =>
    listen<{ jobs: Job[]; running: boolean }>("queue-changed", (e) => cb(e.payload)),
  onProgress: (cb: (s: Snapshot) => void) => listen<Snapshot>("queue-progress", (e) => cb(e.payload)),
  onAlert: (cb: (m: string) => void) => listen<string>("queue-alert", (e) => cb(e.payload)),
};
