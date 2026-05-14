#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-finance-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/finance-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_FINANCE_CONTROLLERS="${RUN_STAGE2_FINANCE_CONTROLLERS:-0}"
RUN_FINANCE_EXPORT="${RUN_STAGE2_FINANCE_EXPORT:-0}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "finance evidence gate requires $1" >&2
    exit 1
  fi
}

slugify() {
  printf '%s' "$1" | sed -E 's#^/##; s#[/:]+#-#g; s#[^A-Za-z0-9._-]+#-#g'
}

fetch_json() {
  local method="$1"
  local path="$2"
  local label
  label="$(slugify "$path")"
  local target="$EVIDENCE_DIR/$label.json"
  local response_body
  local http_status
  response_body="$(mktemp)"

  if [[ "$method" == "GET" ]]; then
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" "${auth_headers[@]}" "$BASE_URL$path")"
  else
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" -X "$method" "${auth_headers[@]}" \
      -H "content-type: application/json" \
      -d '{}' \
      "$BASE_URL$path")"
  fi

  if [[ "$http_status" != 2* ]]; then
    echo "finance evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

fetch_file() {
  local method="$1"
  local path="$2"
  local target="$3"
  local metadata="$4"
  local response_body
  local http_status
  response_body="$(mktemp)"

  if [[ "$method" == "GET" ]]; then
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" "${auth_headers[@]}" "$BASE_URL$path")"
  else
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" -X "$method" "${auth_headers[@]}" "$BASE_URL$path")"
  fi

  if [[ "$http_status" != 2* ]]; then
    echo "finance evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  cp "$response_body" "$target"
  local byte_count
  byte_count="$(wc -c <"$target" | tr -d ' ')"
  jq -n \
    --arg path "$path" \
    --arg target "$target" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --argjson http_status "$http_status" \
    --argjson byte_count "$byte_count" \
    '{
      path: $path,
      target: $target,
      generated_at: $generated_at,
      http_status: $http_status,
      byte_count: $byte_count
    }' >"$metadata"
  rm -f "$response_body"
}

write_summary() {
  local finance_summary_file="$EVIDENCE_DIR/api-usage-finance-summary.json"
  local operations_file="$EVIDENCE_DIR/api-usage-finance-operations-summary.json"
  local export_metadata_file="$EVIDENCE_DIR/usage-export-csv-evidence.json"
  local export_delivery_file="$EVIDENCE_DIR/api-usage-export-deliver.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local current_cost_cents
  local budget_pressure_status
  local critical_budget_count
  local alert_count
  local operations_status
  local readiness_score
  local rollup_status
  local export_status
  local alert_delivery_status
  local production_close_status
  local production_blocked
  local close_controller_required
  local close_controller_configured
  local latest_close_controller_status
  local reconciliation_controller_required
  local reconciliation_controller_configured
  local latest_reconciliation_status
  local blocked_count

  current_cost_cents="$(jq -r '.current_cost_cents // 0' "$finance_summary_file")"
  budget_pressure_status="$(jq -r '.budget_pressure_status // "unknown"' "$finance_summary_file")"
  critical_budget_count="$(jq -r '.critical_budget_count // 0' "$finance_summary_file")"
  alert_count="$(jq -r '.alert_count // 0' "$finance_summary_file")"
  operations_status="$(jq -r '.status // "unknown"' "$operations_file")"
  readiness_score="$(jq -r '.readiness_score // 0' "$operations_file")"
  rollup_status="$(jq -r '.rollup_status // "unknown"' "$operations_file")"
  export_status="$(jq -r '.export_status // "unknown"' "$operations_file")"
  alert_delivery_status="$(jq -r '.alert_delivery_status // "unknown"' "$operations_file")"
  production_close_status="$(jq -r '.production_close.status // "unknown"' "$operations_file")"
  production_blocked="$(jq -r '.production_close.production_blocked // true' "$operations_file")"
  close_controller_required="$(jq -r '.production_close.close_controller_required // false' "$operations_file")"
  close_controller_configured="$(jq -r '.production_close.close_controller_configured // false' "$operations_file")"
  latest_close_controller_status="$(jq -r '.production_close.latest_close_controller_status // "none"' "$operations_file")"
  reconciliation_controller_required="$(jq -r '.production_close.reconciliation_controller_required // false' "$operations_file")"
  reconciliation_controller_configured="$(jq -r '.production_close.reconciliation_controller_configured // false' "$operations_file")"
  latest_reconciliation_status="$(jq -r '.production_close.latest_reconciliation_status // "none"' "$operations_file")"
  blocked_count="$(jq -r 'if .production_close.production_blocked == true then 1 else 0 end' "$operations_file")"

  {
    echo "current_cost_cents=$current_cost_cents"
    echo "budget_pressure_status=$budget_pressure_status"
    echo "critical_budget_count=$critical_budget_count"
    echo "alert_count=$alert_count"
    echo "operations_status=$operations_status"
    echo "readiness_score=$readiness_score"
    echo "rollup_status=$rollup_status"
    echo "export_status=$export_status"
    echo "alert_delivery_status=$alert_delivery_status"
    echo "production_close_status=$production_close_status"
    echo "production_blocked=$production_blocked"
    echo "production_blocked_count=$blocked_count"
    echo "close_controller_required=$close_controller_required"
    echo "close_controller_configured=$close_controller_configured"
    echo "latest_close_controller_status=$latest_close_controller_status"
    echo "reconciliation_controller_required=$reconciliation_controller_required"
    echo "reconciliation_controller_configured=$reconciliation_controller_configured"
    echo "latest_reconciliation_status=$latest_reconciliation_status"
    echo "finance_controllers=$RUN_FINANCE_CONTROLLERS"
    echo "finance_export=$RUN_FINANCE_EXPORT"
    if [[ -s "$export_metadata_file" ]]; then
      echo "finance_export_csv_bytes=$(jq -r '.byte_count // 0' "$export_metadata_file")"
    fi
    if [[ -s "$export_delivery_file" ]]; then
      echo "finance_export_delivery_status=$(jq -r '.status // "unknown"' "$export_delivery_file")"
      echo "finance_export_delivery_target_configured=$(jq -r '.target_configured // false' "$export_delivery_file")"
    fi
    echo "evidence_dir=$EVIDENCE_DIR"
    echo
    echo "finance_attention_items:"
    jq -r '.attention_items[]? | "- \(.message // .kind // \"attention\")"' "$finance_summary_file"
    echo
    echo "operations_attention_items:"
    jq -r '.attention_items[]? | "- \(.message // .kind // \"attention\")"' "$operations_file"
    echo
    echo "production_close_blocking_reasons:"
    jq -r '.production_close.blocking_reasons[]? | "- \(.)"' "$operations_file"
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "finance evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/usage/finance-summary >/dev/null
fetch_json GET /api/usage/finance-operations/summary >/dev/null

if [[ "$RUN_FINANCE_CONTROLLERS" == "1" ]]; then
  fetch_json POST /api/usage/finance-operations/run >/dev/null
  fetch_json POST /api/usage/finance-operations/reconcile >/dev/null
else
  echo "skipping finance close/reconciliation controllers; set RUN_STAGE2_FINANCE_CONTROLLERS=1 to include accounting evidence" >&2
fi

if [[ "$RUN_FINANCE_EXPORT" == "1" ]]; then
  fetch_file GET /api/usage/export.csv "$EVIDENCE_DIR/api-usage-export.csv" "$EVIDENCE_DIR/usage-export-csv-evidence.json"
  fetch_json POST /api/usage/export/deliver >/dev/null
else
  echo "skipping finance export capture; set RUN_STAGE2_FINANCE_EXPORT=1 to include CSV and delivery evidence" >&2
fi

fetch_json GET /api/usage/finance-operations/summary >/dev/null
write_summary
