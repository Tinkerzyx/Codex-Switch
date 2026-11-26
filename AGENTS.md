# Codex Switch development

- Read `docs/DEVELOPMENT.md`, `docs/ARCHITECTURE.md`, and the latest baseline in `docs/BASELINE.md` before implementing a feature.
- Product work targets Windows and macOS from one repository and one feature branch. Keep frontend behavior, IPC names/payloads, and profile formats shared.
- Current source: vanilla HTML/CSS/JavaScript in `codex-switch-app/src/`; Tauri 2 + Rust in `codex-switch-app/src-tauri/`. Do not assume React or Vite is installed.
- The macOS configuration is a build/shell baseline. Windows-only runtime behavior still requires explicit porting; a successful build does not establish feature parity.
- When porting system operations, put adapters in `src-tauri/src/platform/windows.rs` and `macos.rs` behind `#[cfg]`; keep profile/usage business logic shared. These modules are a planned destination, not existing implementation.
- Use `npm ci`, the committed lockfiles, `.node-version`, and `rust-toolchain.toml`. Run `npm run check`, `npm test`, and a native debug package build from `codex-switch-app/` for relevant changes.
- Tests use synthetic fixtures and isolated directories. Never read real auth tokens into tool output or commit account data. Do not test process-kill or account-switch operations against the Codex instance hosting the active development session.
- macOS startup currently discovers its data directory from the working directory. Follow the isolated smoke instructions in `docs/DEVELOPMENT.md`; do not treat a normal Finder launch as validated.
- Use `docs/templates/FEATURE.md` for scope and acceptance evidence, and `.github/pull_request_template.md` for changes. Record the same commit and separate Windows/macOS outcomes. Use `pending` for untested behavior.
- Keep changes on a feature branch; merge readiness requires both platform checks and the agreed native acceptance cases. No release tagging, installer publication, or signing setup is part of ordinary local validation.
