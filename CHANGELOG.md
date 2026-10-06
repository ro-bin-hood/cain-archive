# Changelog

All notable changes to Cain Archive are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html). The version lives in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`; keep the three in sync when bumping.

## [Unreleased]

## [0.2.1] - 2026-10-06

### Fixed

- When search results are grouped by source, the heading of the group being scrolled stays flush with the top of the list. Rows no longer show through a gap above it or hide under it.

## [0.2.0] - 2026-10-05

### Added

- Select several collections with Ctrl+click or Shift+click and export them in one go: each one becomes its own list file in the folder you choose. A subcollection selected together with its parent is already inside the parent's file.
- "Enable all sources" and "Disable all sources" in the collection menu, covering subcollections too.
- Search results from several sources are grouped under a heading per source, in the sidebar's order.

### Changed

- Collections and queued files can be dragged from anywhere on the row, not only by the handle. The row follows the pointer while the others slide out of its way, and settles into place on drop.
- Right-clicking a collection opens its menu. The browser's own context menu (Back, Refresh, Print…) no longer appears, except in text fields.
- Deleting a collection asks for confirmation in a dialog instead of a second click on the menu item.
- Esc also closes the collection and source menus.
- Adding a failed or paused file again from search puts it back in the queue instead of doing nothing.

### Fixed

- Saving the library, queue and settings flushes to disk before replacing the old file, so a power cut can't leave an empty file. A corrupt library is set aside as `library.bak`, `library.2.bak`… without overwriting an older backup.
- The file lists of removed sources are deleted only after the library is saved.
- Copying sources keeps their "included in search" setting from the collection they are copied from.
- Exported lists whose source titles are web addresses import back correctly.
- A download removed just as it was starting no longer keeps the queue "running" forever.
- The search buttons no longer flicker while files download, and they don't stay disabled after a failed search.

### Security

- File names are made safe in more cases: invisible text-direction characters are removed (they could disguise `.exe` files), more Windows device names are caught, and an item can't place files outside its own folder.
- Exported list files go through the same file-name rules.
- "Open folder" only ever opens folders.
- Only `.txt` and `.csv` list files on this computer can be imported, up to 16 MB.
- Login credentials are never sent over a plain-HTTP redirect, and archive.org responses are size-limited.
- Removed an unused permission that let the interface open web links.

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
- Login for items that require an archive.org account; the password is never stored and the session is encrypted for the current user (DPAPI on Windows, the system keyring on Linux).
- Light, dark or automatic theme; English interface, with Italian included. A language is added by dropping a JSON file into `src/locales/`; missing texts fall back to English.
- Windows-safe file names: forbidden characters, reserved names and path traversal are handled, collisions get ` (2)`, ` (3)`…
- Resizable sidebar.
- Linux builds: `.deb` package and AppImage, alongside the Windows installer and portable zip.
- New logo: vector emblem with the temple, Cain with his staff and the log.

### Changed

- Rewritten as a Tauri desktop app and renamed from IA Downloader to Cain Archive; the legacy Python version has been removed.
- The engine reports errors as codes with parameters, translated by the interface: messages follow the current language, including those saved with sources and queued files.

### Security

- Tightened security settings and linting ahead of publishing.

[Unreleased]: https://github.com/ro-bin-hood/cain-archive/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/ro-bin-hood/cain-archive/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/ro-bin-hood/cain-archive/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/ro-bin-hood/cain-archive/releases/tag/v0.1.0
