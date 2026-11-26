# Codex Switch App

This directory contains the shared Tauri 2 desktop app for Windows and macOS. Windows is the existing supported platform; macOS is a beta port requiring real-account acceptance testing.

## Requirements

- Node.js 24 (CI pins 24.18.0)
- Rust (CI pins 1.97.1)
- Windows: MSVC build tools and WebView2 Runtime
- macOS: macOS 14+, Xcode Command Line Tools; build natively for Apple Silicon or Intel

## Development

```powershell
npm ci
npm test
npm run dev
```

## Build

```powershell
npm run version:check
npm run build
```

Outputs:

```text
src-tauri/target/release/codex-switch-app.exe
src-tauri/target/release/bundle/nsis/Codex Switch_<version>_x64-setup.exe
src-tauri/target/release/bundle/macos/Codex Switch.app
src-tauri/target/release/bundle/dmg/*.dmg
```

## Notes

Local `npm run build` allocates the next version and synchronizes npm, Cargo, Tauri, and lock metadata. Numbering carries at nine: `1.7.9 -> 1.8.0`, `1.9.9 -> 2.0.0`. All platform artifacts in one release use the same version and commit. CI uses `npm run build -- --ci`, checks the assigned version, and does not install on the user's computer.

On Windows the local build then silently uninstalls/reinstalls the app, backs up profile data outside the installation directory, verifies retained files by SHA-256, checks the installed executable version, and starts the app. The profile data location remains `%LOCALAPPDATA%\Codex Switch`.

On macOS, use `bash ../scripts/install-macos.sh '/absolute/path/to/Codex Switch.app'` to replace the app while retaining the previous app for recovery. Profiles stay in `~/Documents/CodexSwitch`. Existing data in `~/Library/Application Support/com.codexswitch.app` remains there; this verification build uses the requested Documents store without merging or overwriting the two stores. Override either platform's data directory with `CODEX_SWITCH_DATA_DIR`.

Before running Rust tests, set `CODEX_SWITCH_DATA_DIR` to a disposable directory (CI uses `.test-output/runtime`), then run `cargo test --locked --lib --manifest-path src-tauri/Cargo.toml`.

## Structure and shared behavior

- `src-tauri/src/core`: authentication validation and usage parsing, without OS dependencies.
- `src-tauri/src/services`: common HTTP and payment requests, shared with CloudBase.
- `src-tauri/src/storage`: data schema and private credential-file creation.
- `src-tauri/src/platform`: OS paths, application discovery and lifecycle, CLI, proxies, web sessions, and floating windows.
- `src-tauri/src/platform/windows`: Windows-only WebView2 screenshot capture and WinForms floating bar.
- `src-tauri/src/platform/macos.rs`: Mac app paths, exact executable-path process matching, CLI search, and static HTTP/HTTPS system proxies.
- `src-tauri/src/commands.rs` and `src/bridge/platform.js`: frontend platform capability discovery.
- `src-tauri/src/lib.rs`: existing orchestration, extracted incrementally while retaining regression coverage.
- `cloudbase/` at the repository root: one shared server; deploy independently and maintain compatibility with existing clients.

Keep both platforms on `main` and use short-lived `codex/...` branches. Shared changes must pass Windows, Apple Silicon, and Intel CI. `tauri.conf.json` contains common configuration; `tauri.windows.conf.json` and `tauri.macos.conf.json` contain platform overrides.

## Data compatibility

Codex files use `CODEX_HOME` when set, otherwise the user's `~/.codex`. Valid JSON credentials and optional valid TOML config replace minimum file-size thresholds. Content hashes detect same-size credential changes while waiting for a stable login. Metadata without `schemaVersion` remains compatible as schema 1; newer unknown schemas are rejected. Windows path matching keeps case-insensitive behavior; macOS preserves case and separators.

File-based profiles support `cli_auth_credentials_store = "file"` (including the default when absent). Explicit `keyring`, `auto`, and `ephemeral` modes are detected and rejected before switching. The app does not change the credential-store setting. Native Keychain integration is a later compatibility milestone.

Do not copy a whole Windows `config.toml` to a Mac: paths, MCP commands, and installed tools are machine-specific. CloudBase installation identity remains device-local. Shared code and releases do not synchronize credentials, web sessions, or purchases between computers.

Never commit credentials, profile data, `.env` files, or signing keys. The embedded CloudBase config contains public routing identifiers only; server credentials stay in the server environment.

## macOS beta acceptance

CI builds both architectures natively. A real Mac with the intended Codex/ChatGPT version still needs to pass:

- Save A, log in to B, switch back to A, verify the actual signed-in account, and restart.
- Cancel or interrupt login/switch, then verify recovery and both profiles.
- Verify usage, five-hour requests, local project preservation, tray and floating bar.
- Exercise sleep/wake, multiple monitors, Retina, case-sensitive volumes, and paths with spaces/Chinese characters.
- Verify CLI discovery when launched from Finder. Use `CODEX_SWITCH_CODEX_CLI` for custom installations and `CODEX_SWITCH_APP_PATH` for a nonstandard target app location.
- Verify login-session isolation. Mac uses per-session WKWebView data-store identifiers (macOS 14+).
- Verify upgrades retain existing profile files.

Shared payment requests and direct QR extraction work on both platforms. Native WebView2 QR screenshots remain Windows-only; Mac uses the browser-payment fallback when a page requires native capture. System proxy discovery supports static HTTP/HTTPS proxies; use proxy environment variables for other arrangements. Mac web-session storage cleanup and native Keychain integration need further runtime verification.

## GitHub builds and releases

`Desktop checks` runs tests and produces preview artifacts on pushes and PRs. Mac app archives use tar.gz to retain executable permissions and symlinks. `Desktop release` builds all three installers and creates one **draft** GitHub Release only after all succeed. A release tag must match the source version. Do not rebuild an already published version; allocate the next version for a new release batch.

Tagged Mac releases require repository secrets `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_API_ISSUER`, `APPLE_API_KEY`, and `APPLE_API_KEY_CONTENT`. Manual workflow runs can build ad-hoc signed previews without Apple credentials. Stable distribution needs Apple signing and notarization.

In-app automatic updating is a later milestone: configure updater signing keys and a signed update feed before enabling download and installation. CI artifacts are verification builds, not stable releases.
