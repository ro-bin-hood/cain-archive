import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** An engine error: a code (translated as `errors.<code>`) plus its parameters. */
export type AppError = { code: string; [param: string]: string | number };
export type FileEntry = { name: string; size: number; format: string; original: boolean };
export type JobStatus =
  | { kind: "Queued" }
  | { kind: "Downloading" }
  | { kind: "Retrying"; attempt: number; wait_s: number }
  | { kind: "Paused" }
  | { kind: "Done" }
  | { kind: "Failed"; reason: AppError };
export type Job = { id: number; item_id: string; name: string; size: number; dest: string; status: JobStatus };
export type Theme = "system" | "light" | "dark";
/** "system" or a language code with a file in src/locales. */
export type Language = string;
export type Settings = { out_dir: string; workers: number; default_originals: boolean; default_exts: string; theme: Theme; language: Language };
export type FullState = { settings: Settings; user: string | null; jobs: Job[]; running: boolean };
export type JobProgress = { id: number; done: number; total: number | null; speed: number };
export type Snapshot = { jobs: JobProgress[]; done: number; total: number; speed: number; eta_s: number | null };
export type SourceMeta = { item_id: string; title: string | null; file_count: number; total_size: number; updated_at: number; error: AppError | null };
export type SourceView = SourceMeta & { included: boolean };
export type CollectionView = { id: string; name: string; sources: SourceView[]; parent: string | null };
export type LibraryState = { collections: CollectionView[]; unsaved: SourceMeta[] };
export type Scope = { kind: "all" } | { kind: "collection"; id: string } | { kind: "source"; item_id: string } | { kind: "unsaved" };
export type Hit = { item_id: string; name: string; size: number; original: boolean; local: "downloaded" | "queued" | null };
export type SearchResult = { results: Hit[]; total: number; extensions: { ext: string; count: number }[] };
export type Sort = "name" | "size_desc" | "size_asc";
export type Filters = { originals_only: boolean; exts: string[]; sort: Sort };
export type ParsedList = { name: string | null; inputs: string[]; invalid: string[]; sections: { name: string; inputs: string[] }[] };
export type NewFile = { item_id: string; name: string; size: number };

export const api = {
  getState: () => invoke<FullState>("get_state"),
  enqueue: (files: NewFile[]) => invoke<void>("enqueue", { files }),
  start: () => invoke<void>("start"),
  stop: () => invoke<void>("stop"),
  pauseJob: (id: number) => invoke<void>("pause_job", { id }),
  resumeJob: (id: number) => invoke<void>("resume_job", { id }),
  retryJob: (id: number) => invoke<void>("retry_job", { id }),
  removeJob: (id: number) => invoke<void>("remove_job", { id }),
  moveJob: (id: number, before: number | null) => invoke<void>("move_job", { id, before }),
  clearCompleted: () => invoke<void>("clear_completed"),
  openFolder: (id: number) => invoke<void>("open_folder", { id }),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  pickFolder: () => invoke<string | null>("pick_folder"),
  login: (email: string, password: string) => invoke<string>("login", { email, password }),
  logout: () => invoke<void>("logout"),
  libraryState: () => invoke<LibraryState>("library_state"),
  takeLibraryWarning: () => invoke<AppError | null>("take_library_warning"),
  createCollection: (name: string) => invoke<LibraryState>("create_collection", { name }),
  renameCollection: (id: string, name: string) => invoke<LibraryState>("rename_collection", { id, name }),
  deleteCollection: (id: string) => invoke<LibraryState>("delete_collection", { id }),
  addSource: (collectionId: string, input: string) => invoke<SourceMeta>("add_source", { collectionId, input }),
  removeSource: (collectionId: string, itemId: string) => invoke<LibraryState>("remove_source", { collectionId, itemId }),
  setIncluded: (collectionId: string, itemId: string, included: boolean) => invoke<LibraryState>("set_included", { collectionId, itemId, included }),
  refreshSource: (itemId: string) => invoke<SourceMeta>("refresh_source", { itemId }),
  openUnsaved: (input: string) => invoke<SourceMeta>("open_unsaved", { input }),
  closeUnsaved: (itemId: string) => invoke<LibraryState>("close_unsaved", { itemId }),
  saveUnsaved: (itemId: string, collectionId: string) => invoke<LibraryState>("save_unsaved", { itemId, collectionId }),
  readImportFile: (path: string) => invoke<ParsedList>("read_import_file", { path }),
  pickImportFile: (filterName: string) => invoke<ParsedList | null>("pick_import_file", { filterName }),
  exportCollection: (id: string, filterName: string) => invoke<string | null>("export_collection", { id, filterName }),
  exportCollections: (ids: string[]) => invoke<string | null>("export_collections", { ids }),
  createSubcollection: (parent: string, name: string) => invoke<LibraryState>("create_subcollection", { parent, name }),
  moveCollection: (id: string, before: string | null) => invoke<LibraryState>("move_collection", { id, before }),
  removeSources: (collectionId: string, itemIds: string[]) => invoke<LibraryState>("remove_sources", { collectionId, itemIds }),
  copySources: (from: string | null, to: string, itemIds: string[]) => invoke<LibraryState>("copy_sources", { from, to, itemIds }),
  moveSources: (from: string, to: string, itemIds: string[]) => invoke<LibraryState>("move_sources", { from, to, itemIds }),
  search: (scope: Scope, query: string, filters: Filters) => invoke<SearchResult>("search", { scope, query, filters }),
};

export const events = {
  onChanged: (cb: (p: { jobs: Job[]; running: boolean }) => void) =>
    listen<{ jobs: Job[]; running: boolean }>("queue-changed", (e) => cb(e.payload)),
  onProgress: (cb: (s: Snapshot) => void) => listen<Snapshot>("queue-progress", (e) => cb(e.payload)),
  onAlert: (cb: (e: AppError) => void) => listen<AppError>("queue-alert", (e) => cb(e.payload)),
};
