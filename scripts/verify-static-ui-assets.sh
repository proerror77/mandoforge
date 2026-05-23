#!/usr/bin/env bash
set -euo pipefail

INDEX_FILE="${INDEX_FILE:-web/index.html}"
ASSET_DIR="${ASSET_DIR:-web/assets}"

require_file() {
  if [[ ! -s "$1" ]]; then
    echo "missing static UI asset: $1" >&2
    exit 1
  fi
}

require_text() {
  local file="$1"
  local pattern="$2"
  if ! grep -q "$pattern" "$file"; then
    echo "static UI asset check failed: $file missing pattern: $pattern" >&2
    exit 1
  fi
}

for command in find grep sed; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "missing required command: $command" >&2
    exit 1
  fi
done

require_file "$INDEX_FILE"
if [[ ! -d "$ASSET_DIR" ]]; then
  echo "missing static UI asset directory: $ASSET_DIR" >&2
  exit 1
fi

asset_refs=()
while IFS= read -r asset_ref; do
  asset_refs+=("$asset_ref")
done < <(grep -Eo '/assets/[^"]+\.(js|css)' "$INDEX_FILE" | sort -u)
if [[ "${#asset_refs[@]}" -eq 0 ]]; then
  echo "static UI asset check failed: $INDEX_FILE does not reference Vite assets" >&2
  exit 1
fi

for asset_ref in "${asset_refs[@]}"; do
  require_file "web${asset_ref}"
done

require_text "$INDEX_FILE" "MandoForge Agent OS Console"
require_text "$INDEX_FILE" "id=\"root\""

js_asset_count="$(find "$ASSET_DIR" -maxdepth 1 -type f -name '*.js' | wc -l | tr -d ' ')"
css_asset_count="$(find "$ASSET_DIR" -maxdepth 1 -type f -name '*.css' | wc -l | tr -d ' ')"
if [[ "$js_asset_count" -lt 1 || "$css_asset_count" -lt 1 ]]; then
  echo "static UI asset check failed: expected at least one JS and one CSS asset" >&2
  exit 1
fi

asset_patterns=(
  "MandoForge Co-Work"
  "Managed agent observability"
  "Agents running"
  "Live log stream"
  "Workflow"
  "Steps"
  "Transitions"
  "Grants"
  "Approvals"
  "Tool calls"
  "Artifacts"
  "Start task"
  "/api/sessions"
  "/api/approvals"
  "/api/tool-calls"
  "/api/workflow-runs"
  "/transitions"
  "/task-grants"
)

for pattern in "${asset_patterns[@]}"; do
  if ! grep -R -q "$pattern" "$ASSET_DIR"; then
    echo "static UI asset check failed: $ASSET_DIR missing pattern: $pattern" >&2
    exit 1
  fi
done

if grep -R -q "window.prompt" "$ASSET_DIR"; then
  echo "static UI must not use window.prompt" >&2
  exit 1
fi

echo "static UI asset verification ok"
