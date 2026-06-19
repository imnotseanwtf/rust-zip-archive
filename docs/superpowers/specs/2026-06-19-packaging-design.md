# Installable App Packaging — Design Spec

**Date:** 2026-06-19
**Status:** Approved (pending spec review)

## Goal

Ship `rza` as an installable desktop application on Windows, macOS, and Linux —
with an app icon, a launcher entry (Start menu / Applications / app grid), and
file associations so opening an archive launches the `rza-gui` window. The `rza`
CLI is included alongside the app.

This is independent of the 7z/rar format plans (B/C); it packages whatever the
GUI currently supports.

## Tooling decision

Use **`cargo-packager`** (the actively-maintained packager from the Tauri
ecosystem). It produces, from a single `[package.metadata.packager]` config:

- Windows: `.msi` (WiX) and/or NSIS `.exe` installer
- macOS: `.app` bundle inside a `.dmg`
- Linux: `.deb` and AppImage

…with icons, desktop entries, and file associations. Rejected alternatives:
`cargo-dist` (weak on `.app`/`.dmg`, icons, file associations — targets CLI
tools); hand-rolled per-OS scripts (WiX + create-dmg + cargo-deb/AppImage —
triple the scripting and maintenance).

## 1. The application = `rza-gui`

The bundled application is the `rza-gui` binary (built with `--features gui`).
The `rza` CLI binary is shipped alongside it (as an extra binary/resource) so
command-line use keeps working after install.

## 2. App identity & icon

- Display name: **"rza — Archive Utility"**; bundle identifier: `com.imnotseanwtf.rza`.
- Add `assets/icon.png` (a simple, legible archive-box icon, 512×512, with
  transparency). cargo-packager derives platform icons (`.ico`, `.icns`) from
  the PNG set. The icon drives the launcher entry on every OS.

## 3. File associations + "open on launch"

cargo-packager declares the handled extensions: `zip, tar, gz, tgz, bz2, xz,
zst`. For a double-click / "Open with rza" to actually do something, the GUI
must open the passed file:

- **`rza-gui` accepts an optional file-path argument.** On startup, if
  `std::env::args().nth(1)` is an existing file, the app opens it in Browse mode
  (calls the existing `open_archive`). If absent, the app starts empty as today.
- **Platform reach (v1):** Windows and Linux pass the path via argv when a file
  is opened with the app → works. macOS delivers "open document" via an Apple
  event (not argv), so macOS double-click-to-open is a **follow-up**; in v1 the
  association registers and `open -a "rza — Archive Utility" file.zip` / drag-
  onto-dock-icon paths still route through argv where the OS provides it.

## 4. CI / distribution

- New workflow `.github/workflows/package.yml`, triggered on `v*` tags:
  installs `cargo-packager`, runs it on `ubuntu-latest`, `macos-latest`,
  `windows-latest` (installing the Linux GUI system libs on Ubuntu), and uploads
  the resulting installers to the GitHub Release.
- The existing `release.yml` (raw `.tar.gz`/`.zip` binaries) is **kept** —
  installers are additive, for users who prefer a plain binary.

## 5. Signing (documented, not blocking)

Builds are **unsigned** in v1:
- macOS: Gatekeeper shows "unidentified developer"; users right-click → Open
  once. The app then runs normally.
- Windows: SmartScreen warns on first run of an unsigned installer; "More info →
  Run anyway".

README documents both and the steps to proceed. Code signing / notarization is a
paid follow-up (Apple Developer ID ~$99/yr; Windows code-signing cert ~$100+/yr).

## 6. Architecture / files

- `Cargo.toml`: add `[package.metadata.packager]` (product name, identifier,
  icons, the `rza`+`rza-gui` binaries, file associations, per-format
  description). No new runtime crate dependencies.
- `assets/icon.png`: the source icon (plus any sizes cargo-packager wants).
- `src/bin/rza-gui.rs`: parse the optional path arg and open it on first frame.
- `.github/workflows/package.yml`: the packaging pipeline.
- `README.md`: an "Install the app" section (download links + the unsigned-app
  steps for macOS/Windows).

## 7. Error handling

- If `rza-gui` is launched with a path that isn't a readable archive, it shows
  the existing error banner (via `open_archive`'s `Err` → status) rather than
  failing to start.
- A path argument that doesn't exist is ignored (app starts empty), so a stray
  argument never blocks launch.

## 8. Testing

- **argv open-on-launch:** unit-test the "should we open this arg?" decision
  (existing file path → Some; missing path / no arg → None) as a small pure
  function so it's testable without a window. A headless startup smoke test on
  Linux confirms the app still launches with and without an argument.
- **Packaging:** verified by CI building the installers on each OS runner. Only
  the Linux package (`.deb`/AppImage) can be built/inspected on the dev machine.

## 9. Scope guard (YAGNI)

**In:** 3-OS installers, app icon + launcher entry, file associations with argv
open-on-launch (Windows/Linux), unsigned builds, kept raw-binary release.
**Out:** code signing/notarization, macOS Apple-event document open, auto-update,
shell-extension right-click "Extract here" menus (only the "open with"
association), and store channels (winget/Homebrew/Flatpak).
