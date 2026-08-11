# Codex Switch App

This directory contains the Tauri desktop app for Codex Switch.

## Requirements

- Windows
- Node.js
- Rust toolchain
- WebView2 Runtime

## Development

```powershell
npm install
npm run tauri dev
```

## Build

```powershell
npm run tauri build
```

Outputs:

```text
src-tauri/target/release/codex-switch-app.exe
src-tauri/target/release/bundle/nsis/Codex Switch_0.1.0_x64-setup.exe
```

## Notes

The app manages local Codex login snapshots from `.codex/auth.json` and `.codex/config.toml`.

Do not commit or share `codex_switch_data/`, `auth.json`, or `config.toml`.
