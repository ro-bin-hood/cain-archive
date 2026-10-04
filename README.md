# Cain Archive

*Stay awhile and download…*

An unofficial desktop downloader for the [Internet Archive](https://archive.org).

Paste one or more item links, pick the files you want, and let it download them in parallel. Downloads resume where they stopped, even after closing the app.

> **Unofficial project.** Cain Archive is not affiliated with, endorsed by, or connected to the Internet Archive. "Internet Archive" is a trademark of its respective owner. Please respect the Internet Archive's [terms of use](https://archive.org/about/terms.php) and the rights attached to each item.

## Features

- **File picker.** Analyze an item and see all its files with type and size. Filter chips ("originals only", extensions) preselect files; you can tick or untick any of them.
- **Parallel downloads** (1–8) with speed, progress and estimated time.
- **Resume.** Files download to `name.part` and are renamed only after the size checks out. Interrupted downloads continue via HTTP Range.
- **Persistent queue.** Close the app mid-download, reopen it, press ▶ and it picks up from where it was.
- **Retries.** Up to 3 attempts per file with increasing waits, honoring `Retry-After` when the server is busy.
- **Login** for items that need an archive.org account. Your password is never stored; the session is kept encrypted for your Windows user only (DPAPI).
- **Light, dark or automatic theme.**
- **Windows-safe file names.** Forbidden characters, reserved names (`CON`, `aux.h`…) and path traversal are handled. Names that would collide get ` (2)`, ` (3)`…

Accepted inputs, one per line:
- `https://archive.org/details/<identifier>`: the whole item
- `https://archive.org/download/<identifier>/<file>`: a single file
- `<identifier>`

Files are saved to `<destination>/<identifier>/<path>`. The default destination is `~/Downloads/archive`; change it from ⚙.

## Install

Download the installer (`Cain Archive_x.y.z_x64-setup.exe`) from the [Releases](../../releases) page.

The installer is not code-signed, so Windows SmartScreen may warn you: choose *More info → Run anyway*. The app needs WebView2, which is preinstalled on Windows 11.

## Development

The UI is React + TypeScript in `src/`. The engine is Rust in `src-tauri/`: archive.org client, downloads, queue and storage.

Requirements:
- Node.js 20+ and Rust (stable)
- Windows: Visual Studio Build Tools with "Desktop development with C++" and the Windows 11 SDK
- If `cargo` can't find the linker or `kernel32.lib`, run the commands from a *Developer PowerShell for VS* (or after `vcvars64.bat`)

```bash
npm install
npm run tauri dev      # run in development
npm run tauri build    # portable .exe + NSIS installer in src-tauri/target/release/
cd src-tauri && cargo test
```

Settings and queue live in `%APPDATA%\com.cainarchive.app\`. The login session is in `session.bin` in the same folder, encrypted with DPAPI.

## Name

A nod to Deckard Cain, the last of the Horadrim and keeper of forgotten lore in *Diablo II*. This project is not affiliated with Blizzard Entertainment.

## License

[MIT](LICENSE) © 2026 SignorAutoma
