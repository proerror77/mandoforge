#!/usr/bin/env bash
set -euo pipefail

DESKTOP_ROOT="${DESKTOP_ROOT:-crates/mandoforge-desktop}"

require_file() {
  if [[ ! -s "$1" ]]; then
    echo "desktop shell contract missing file: $1" >&2
    exit 1
  fi
}

require_text() {
  local file="$1"
  local pattern="$2"
  if ! grep -q "$pattern" "$file"; then
    echo "desktop shell contract failed: $file missing pattern: $pattern" >&2
    exit 1
  fi
}

for command in grep test; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "desktop shell contract requires $command" >&2
    exit 1
  fi
done

required_files=(
  "Cargo.toml"
  "$DESKTOP_ROOT/Cargo.toml"
  "$DESKTOP_ROOT/build.rs"
  "$DESKTOP_ROOT/src/main.rs"
  "$DESKTOP_ROOT/src/lib.rs"
  "$DESKTOP_ROOT/src/commands.rs"
  "$DESKTOP_ROOT/src/tray.rs"
  "$DESKTOP_ROOT/tauri.conf.json"
  "$DESKTOP_ROOT/capabilities/default.json"
  "$DESKTOP_ROOT/icons/icon.png"
  "scripts/verify-desktop-runtime-smoke.sh"
)

for file in "${required_files[@]}"; do
  require_file "$file"
done

require_text "Cargo.toml" "crates/mandoforge-desktop"
require_text "$DESKTOP_ROOT/Cargo.toml" "tauri"
require_text "$DESKTOP_ROOT/Cargo.toml" "tray-icon"
require_text "$DESKTOP_ROOT/tauri.conf.json" "com.mandonothing.mandoforge"
require_text "$DESKTOP_ROOT/tauri.conf.json" "http://127.0.0.1:8787"
require_text "$DESKTOP_ROOT/tauri.conf.json" "frontendDist"
require_text "$DESKTOP_ROOT/capabilities/default.json" "\"windows\": \\[\"main\"\\]"
require_text "$DESKTOP_ROOT/capabilities/default.json" "core:webview:allow-create-webview-window"

for command in \
  get_status \
  get_api_base_url \
  open_browser \
  open_config_dir \
  open_logs_dir \
  get_notification_status; do
  require_text "$DESKTOP_ROOT/src/commands.rs" "fn $command"
  require_text "$DESKTOP_ROOT/src/lib.rs" "commands::$command"
done

for tray_item in \
  show_console \
  open_browser \
  copy_api_url \
  open_logs \
  status \
  quit; do
  require_text "$DESKTOP_ROOT/src/tray.rs" "$tray_item"
done

require_text "$DESKTOP_ROOT/src/lib.rs" "MANDOFORGE_API_BASE_URL"
require_text "$DESKTOP_ROOT/src/lib.rs" "MANDOFORGE_DESKTOP_EMBEDDED_API"
require_text "$DESKTOP_ROOT/src/lib.rs" "MANDOFORGE_DESKTOP_API_COMMAND"
require_text "$DESKTOP_ROOT/src/lib.rs" "MANDOFORGE_DESKTOP_SMOKE_EXIT_AFTER_MS"
require_text "$DESKTOP_ROOT/src/lib.rs" "DesktopMode::ExistingApi"
require_text "$DESKTOP_ROOT/src/lib.rs" "DesktopMode::EmbeddedLocalApi"
require_text "$DESKTOP_ROOT/src/lib.rs" "embedded_api_enabled: matches!"
require_text "$DESKTOP_ROOT/src/lib.rs" "embedded_api_owned"
require_text "$DESKTOP_ROOT/src/lib.rs" "127.0.0.1:0"
require_text "$DESKTOP_ROOT/src/lib.rs" "api_reachable"
require_text "$DESKTOP_ROOT/src/lib.rs" "api_unreachable"
require_text "$DESKTOP_ROOT/src/commands.rs" "native_forwarding_enabled: false"
require_text "$DESKTOP_ROOT/src/commands.rs" "mandoforge.criticalNotificationsMuted"
require_text "scripts/verify-desktop-runtime-smoke.sh" "START_API"
require_text "scripts/verify-desktop-runtime-smoke.sh" "EMBEDDED_API"
require_text "scripts/verify-desktop-runtime-smoke.sh" "cargo run -p mandoforge-desktop"
require_text "scripts/verify-desktop-runtime-smoke.sh" "MANDOFORGE_DESKTOP_EMBEDDED_API=1"

if grep -R -q "cargo run -p mandoforge-api" "$DESKTOP_ROOT/src"; then
  echo "desktop shell contract failed: desktop MVP must not hard-code cargo-run API startup" >&2
  exit 1
fi

if grep -R -q "native_forwarding_enabled: true" "$DESKTOP_ROOT/src"; then
  echo "desktop shell contract failed: native OS notification forwarding is not part of this MVP" >&2
  exit 1
fi

echo "desktop shell contract verification ok"
