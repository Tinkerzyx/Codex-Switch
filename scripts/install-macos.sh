#!/bin/bash
set -euo pipefail

source_app="${1:?Usage: install-macos.sh /path/to/Codex\ Switch.app [installation-directory]}"
install_parent="${2:-/Applications}"
[[ "$source_app" = /* && -d "$source_app/Contents/MacOS" ]] || { echo 'Provide an absolute path to the built app.' >&2; exit 1; }
[[ "$install_parent" = /Applications || "$install_parent" = "$HOME/Applications" ]] || { echo 'Install into /Applications or ~/Applications.' >&2; exit 1; }
[[ ! -L "$install_parent" ]] || { echo 'Installation directory must not be a symlink.' >&2; exit 1; }
mkdir -p "$install_parent"
target_app="$install_parent/Codex Switch.app"
[[ ! -L "$target_app" ]] || { echo 'Application path must not be a symlink.' >&2; exit 1; }
/usr/bin/codesign --verify --deep --strict "$source_app"
identifier=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$source_app/Contents/Info.plist")
[[ "$identifier" = com.codexswitch.app ]] || { echo 'Unexpected application identifier.' >&2; exit 1; }
staging=$(/usr/bin/mktemp -d "$install_parent/.codex-switch-install.XXXXXX")
/usr/bin/ditto "$source_app" "$staging/Codex Switch.app"
/usr/bin/codesign --verify --deep --strict "$staging/Codex Switch.app"
/usr/bin/osascript -e 'if application id "com.codexswitch.app" is running then tell application id "com.codexswitch.app" to quit' 2>/dev/null || true
backup_app="$install_parent/Codex Switch.previous-$(date +%Y%m%d-%H%M%S).app"
if [[ -d "$target_app" ]]; then /bin/mv "$target_app" "$backup_app"; fi
if ! /bin/mv "$staging/Codex Switch.app" "$target_app"; then
  if [[ -d "$backup_app" ]]; then /bin/mv "$backup_app" "$target_app"; fi
  exit 1
fi
/bin/rmdir "$staging"
/usr/bin/open "$target_app"
echo 'Installed Codex Switch. Profiles remain in ~/Library/Application Support/com.codexswitch.app.'
