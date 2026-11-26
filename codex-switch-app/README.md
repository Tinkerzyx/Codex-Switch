# Codex Switch App

This directory contains the Tauri 2 app with vanilla HTML, CSS and JavaScript. Windows is the existing runtime implementation; macOS currently has a build/shell configuration, with runtime porting work tracked in [ARCHITECTURE.md](../docs/ARCHITECTURE.md).

## Requirements

- Node.js from `../.node-version` and Rust from `../rust-toolchain.toml`
- Windows: C++ Build Tools, Windows SDK, WebView2 Runtime, MSVC Rust
- macOS: Xcode Command Line Tools

## Development

```powershell
npm ci
npm run check
npm test
```

## Build

```powershell
npm run build:debug
```

Outputs:

```text
src-tauri/target/debug/bundle/nsis/*.exe
src-tauri/target/debug/bundle/macos/Codex Switch.app
src-tauri/target/debug/bundle/dmg/*.dmg
```

## Notes

On Windows, `npm run dev` starts the app. For the current macOS shell, follow the isolated startup instructions in [DEVELOPMENT.md](../docs/DEVELOPMENT.md). The npm scripts add the standard rustup bin directory to their child process PATH.

`npm run build -- -- --locked` builds native release packages. A packaged macOS shell is not evidence that account switching is supported. See [the workflow](../docs/DEVELOPMENT.md) for two-machine synchronization and native acceptance.

The app manages local Codex login snapshots from `.codex/auth.json` and `.codex/config.toml`.

Do not commit or share `codex_switch_data/`, `auth.json`, or `config.toml`.
