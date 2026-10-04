# Changelog

All notable changes to Cain Archive are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html). The version lives in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`; keep the three in sync when bumping.

## [Unreleased]

## [0.1.0] - 2026-10-04

First public release.

### Added

- Download whole archive.org items or single files from a link or identifier, with 1–8 parallel downloads, speed, progress and estimated time.
- Resumable downloads: files are written to `name.part`, renamed only after the size checks out, and continued via HTTP Range.
- Persistent queue that survives closing the app; reorder it by dragging files, pause, stop and restart it, and hide or show it.
- Automatic retries (up to 3, with increasing waits) that honor `Retry-After`; retries reset when a download makes progress.
- Collections of saved sources, with file lists kept locally for instant search.
- One level of subcollections: sidebar tree, parent search that includes children, `# --- Name ---` sections in import/export.
- Combined search across a collection, a source or everything: every word must match, extension filter chips, sorting by name or size, results marked as downloaded or queued.
- Result selection that persists across searches.
- Multi-select of sources with a context menu: remove, copy or move several at once; select all sources.
- Reorder collections by dragging.
- Import and export collections as text/CSV files, including drag & drop of list files.
- Quick open: paste an archive.org link to browse an item without saving it, then star it to keep it.
- Readable source tags that strip the common prefix of a family of identifiers.
- Login for items that require an archive.org account; the password is never stored and the session is encrypted for the current Windows user (DPAPI).
- Light, dark or automatic theme; Italian or English interface and engine messages.
- Windows-safe file names: forbidden characters, reserved names and path traversal are handled, collisions get ` (2)`, ` (3)`…
- Resizable sidebar.
- New logo: vector emblem with the temple, Cain with his staff and the log.

### Changed

- Rewritten as a Tauri desktop app and renamed from IA Downloader to Cain Archive; the legacy Python version has been removed.

### Security

- Tightened security settings and linting ahead of publishing.

[Unreleased]: https://github.com/ro-bin-hood/cain-archive/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ro-bin-hood/cain-archive/releases/tag/v0.1.0
