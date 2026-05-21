#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-finance-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/finance-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_FINANCE_CONTROLLERS="${RUN_STAGE2_FINANCE_CONTROLLERS:-1}"
RUN_FINANCE_EXPORT="${RUN_STAGE2_FINANCE_EXPORT:-1}"
DELIVERY_OBSERVER_URL="${FINANCE_EXPORT_DELIVERY_OBSERVER_URL:-}"
AUTH_TOKEN="${MANDOFORGE_STAGE2_GATE_TOKEN:-}"
EXPECTED_FINANCE_SYSTEM_ID="${MANDOFORGE_STAGE2_FINANCE_SYSTEM_ID:-}"

auth_headers=(
)
if [[ -n "$AUTH_TOKEN" ]]; then
  auth_headers+=(-H "authorization: Bearer $AUTH_TOKEN")
else
  auth_headers+=(
    -H "x-mandoforge-subject: $SUBJECT"
    -H "x-mandoforge-roles: $ROLES"
  )
fi

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "finance evidence gate requires $1" >&2
    exit 1
  fi
}

is_production_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  [[ -n "$value" ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

is_finance_system_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  is_production_identity "$value" || return 1
  [[ ! "$value" =~ (^|[./:_-])(feishu|lark|drive|file|artifact)([./:_-]|$) ]] || return 1
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

capture_delivery_observer() {
  local observer_url="$1"
  local target="$EVIDENCE_DIR/finance-export-delivery-observer.json"
  local response_body
  local http_status
  response_body="$(mktemp)"

  http_status="$(curl -sS -o "$response_body" -w "%{http_code}" "$observer_url")"
  if [[ "$http_status" != 2* ]]; then
    rm -f "$response_body"
    return 0
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
}

write_summary() {
  local finance_summary_file="$EVIDENCE_DIR/api-usage-finance-summary.json"
  local operations_file="$EVIDENCE_DIR/api-usage-finance-operations-summary.json"
  local close_evidence_file="$EVIDENCE_DIR/finance-close-evidence.json"
  local reconciliation_evidence_file="$EVIDENCE_DIR/finance-reconciliation-evidence.json"
  local export_metadata_file="$EVIDENCE_DIR/usage-export-csv-evidence.json"
  local export_delivery_file="$EVIDENCE_DIR/finance-export-delivery-evidence.json"
  local export_delivery_observer_file="$EVIDENCE_DIR/finance-export-delivery-observer.json"
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
  local latest_close_controller_age_hours
  local close_controller_evidence_fresh
  local latest_close_controller_closed
  local reconciliation_controller_required
  local reconciliation_controller_configured
  local latest_reconciliation_status
  local latest_reconciliation_age_hours
  local reconciliation_evidence_fresh
  local latest_reconciliation_reconciled
  local close_evidence_status
  local close_run_status
  local reconciliation_evidence_status
  local reconciliation_run_status
  local finance_export_csv_bytes
  local export_delivery_evidence_status
  local export_delivery_status
  local export_delivery_target_configured
  local export_delivery_observer_status
  local export_delivery_mode
  local export_delivery_count
  local export_delivery_file_token
  local export_delivery_file_url
  local export_delivery_file_name
  local export_delivery_system_id
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
  production_blocked="$(jq -r 'if ((.production_close // {}) | has("production_blocked")) then .production_close.production_blocked else true end' "$operations_file")"
  close_controller_required="$(jq -r '.production_close.close_controller_required // false' "$operations_file")"
  close_controller_configured="$(jq -r '.production_close.close_controller_configured // false' "$operations_file")"
  latest_close_controller_status="$(jq -r '.production_close.latest_close_controller_status // "none"' "$operations_file")"
  latest_close_controller_age_hours="$(jq -r '.production_close.latest_close_controller_age_hours // "none"' "$operations_file")"
  close_controller_evidence_fresh="$(jq -r '.production_close.close_controller_evidence_fresh // false' "$operations_file")"
  latest_close_controller_closed="$(jq -r '.production_close.latest_close_controller_closed // false' "$operations_file")"
  reconciliation_controller_required="$(jq -r '.production_close.reconciliation_controller_required // false' "$operations_file")"
  reconciliation_controller_configured="$(jq -r '.production_close.reconciliation_controller_configured // false' "$operations_file")"
  latest_reconciliation_status="$(jq -r '.production_close.latest_reconciliation_status // "none"' "$operations_file")"
  latest_reconciliation_age_hours="$(jq -r '.production_close.latest_reconciliation_age_hours // "none"' "$operations_file")"
  reconciliation_evidence_fresh="$(jq -r '.production_close.reconciliation_evidence_fresh // false' "$operations_file")"
  latest_reconciliation_reconciled="$(jq -r '.production_close.latest_reconciliation_reconciled // false' "$operations_file")"
  close_evidence_status="not_requested"
  close_run_status="not_run"
  if [[ -s "$close_evidence_file" ]]; then
    close_evidence_status="$(jq -r '.status // "unknown"' "$close_evidence_file")"
    close_run_status="$(jq -r '.response.status // "unknown"' "$close_evidence_file")"
  fi
  reconciliation_evidence_status="not_requested"
  reconciliation_run_status="not_run"
  export_delivery_observer_status="not_observed"
  export_delivery_mode="unknown"
  export_delivery_file_token="none"
  export_delivery_file_url="none"
  export_delivery_file_name="none"
  export_delivery_system_id=""
  if [[ -s "$reconciliation_evidence_file" ]]; then
    reconciliation_evidence_status="$(jq -r '.status // "unknown"' "$reconciliation_evidence_file")"
    reconciliation_run_status="$(jq -r '.response.status // "unknown"' "$reconciliation_evidence_file")"
  fi
  finance_export_csv_bytes="0"
  if [[ -s "$export_metadata_file" ]]; then
    finance_export_csv_bytes="$(jq -r '.byte_count // 0' "$export_metadata_file")"
  fi
  export_delivery_evidence_status="missing"
  export_delivery_status="unknown"
  export_delivery_target_configured="false"
  if [[ -s "$export_delivery_file" ]]; then
    export_delivery_evidence_status="$(jq -r '.status // "unknown"' "$export_delivery_file")"
    export_delivery_status="$(jq -r '.response.status // "unknown"' "$export_delivery_file")"
    export_delivery_target_configured="$(jq -r '.response.target_configured // false' "$export_delivery_file")"
  fi
  if [[ -s "$export_delivery_observer_file" ]]; then
    export_delivery_observer_status="$(jq -r '.status // "ok"' "$export_delivery_observer_file")"
    export_delivery_mode="$(jq -r '.export_state.delivery_mode // "unknown"' "$export_delivery_observer_file")"
    export_delivery_count="$(jq -r '.export_state.delivery_count // 0' "$export_delivery_observer_file")"
    export_delivery_file_token="$(jq -r '.export_state.latest_file_token // "none"' "$export_delivery_observer_file")"
    export_delivery_file_url="$(jq -r '.export_state.latest_file_url // "none"' "$export_delivery_observer_file")"
    export_delivery_file_name="$(jq -r '.export_state.latest_file_name // "none"' "$export_delivery_observer_file")"
    export_delivery_system_id="$(jq -r '.export_state.system_id // .export_state.erp_system_id // .export_state.accounting_system_id // .export_state.target_id // ""' "$export_delivery_observer_file")"
  else
    export_delivery_count="0"
  fi
  blocked_count="$(jq -r '[
      .production_close.production_blocked,
      (.production_close.close_controller_required != true),
      (.production_close.close_controller_configured != true),
      (.production_close.latest_close_controller_closed != true),
      (.production_close.close_controller_evidence_fresh != true),
      (.production_close.reconciliation_controller_required != true),
      (.production_close.reconciliation_controller_configured != true),
      (.production_close.latest_reconciliation_reconciled != true),
      (.production_close.reconciliation_evidence_fresh != true),
      (.production_close.export_target_configured != true),
      (.production_close.export_recent != true),
      (.production_close.failed_delivery_evidence == true)
    ] | map(select(. == true)) | length' "$operations_file")"
  if [[ "$RUN_FINANCE_CONTROLLERS" != "1" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$RUN_FINANCE_EXPORT" != "1" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$close_evidence_status" != "captured" || "$close_run_status" != "completed" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$reconciliation_evidence_status" != "captured" || "$reconciliation_run_status" != "reconciled" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$finance_export_csv_bytes" == "0" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$export_delivery_evidence_status" != "captured" || "$export_delivery_status" != "delivered" || "$export_delivery_target_configured" != "true" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$export_delivery_observer_status" != "ok" || "$export_delivery_count" == "0" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  case "$export_delivery_mode" in
    accounting*|erp*|netsuite|quickbooks|xero|sap|oracle_erp)
      ;;
    lark_drive|accept_only|unknown|none)
      blocked_count="$((blocked_count + 1))"
      ;;
    *)
      blocked_count="$((blocked_count + 1))"
      ;;
  esac
  if [[ -z "$export_delivery_system_id" ]]; then
    blocked_count="$((blocked_count + 1))"
  elif ! is_finance_system_identity "$export_delivery_system_id"; then
    blocked_count="$((blocked_count + 1))"
  elif [[ -n "$EXPECTED_FINANCE_SYSTEM_ID" && "$export_delivery_system_id" != "$EXPECTED_FINANCE_SYSTEM_ID" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi

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
    echo "latest_close_controller_age_hours=$latest_close_controller_age_hours"
    echo "close_controller_evidence_fresh=$close_controller_evidence_fresh"
    echo "latest_close_controller_closed=$latest_close_controller_closed"
    echo "reconciliation_controller_required=$reconciliation_controller_required"
    echo "reconciliation_controller_configured=$reconciliation_controller_configured"
    echo "latest_reconciliation_status=$latest_reconciliation_status"
    echo "latest_reconciliation_age_hours=$latest_reconciliation_age_hours"
    echo "reconciliation_evidence_fresh=$reconciliation_evidence_fresh"
    echo "latest_reconciliation_reconciled=$latest_reconciliation_reconciled"
    echo "finance_close_evidence_status=$close_evidence_status"
    echo "finance_close_run_status=$close_run_status"
    echo "finance_reconciliation_evidence_status=$reconciliation_evidence_status"
    echo "finance_reconciliation_run_status=$reconciliation_run_status"
    echo "finance_controllers=$RUN_FINANCE_CONTROLLERS"
    echo "finance_export=$RUN_FINANCE_EXPORT"
    echo "finance_export_csv_bytes=$finance_export_csv_bytes"
    echo "finance_export_delivery_evidence_status=$export_delivery_evidence_status"
    echo "finance_export_delivery_status=$export_delivery_status"
    echo "finance_export_delivery_target_configured=$export_delivery_target_configured"
    echo "finance_export_delivery_observer_status=$export_delivery_observer_status"
    echo "finance_export_delivery_mode=$export_delivery_mode"
    echo "finance_export_delivery_count=$export_delivery_count"
    echo "finance_export_delivery_file_token=$export_delivery_file_token"
    echo "finance_export_delivery_file_url=$export_delivery_file_url"
    echo "finance_export_delivery_file_name=$export_delivery_file_name"
    echo "finance_export_delivery_system_id=$export_delivery_system_id"
    echo "expected_finance_system_id=${EXPECTED_FINANCE_SYSTEM_ID:-<unset>}"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo
    echo "finance_attention_items:"
    jq -r '.attention_items[]? | "- \(.message // .kind // "attention")"' "$finance_summary_file"
    echo
    echo "operations_attention_items:"
    jq -r '.attention_items[]? | "- \(.message // .kind // "attention")"' "$operations_file"
    echo
    echo "production_close_blocking_reasons:"
    jq -r '.production_close.blocking_reasons[]? | "- \(.)"' "$operations_file"
    if [[ "$RUN_FINANCE_CONTROLLERS" != "1" ]]; then
      echo "- finance close/reconciliation controller evidence capture is disabled"
    fi
    if [[ "$RUN_FINANCE_EXPORT" != "1" ]]; then
      echo "- finance export evidence capture is disabled"
    fi
    if [[ "$close_evidence_status" != "captured" || "$close_run_status" != "completed" ]]; then
      echo "- finance close evidence is not completed: evidence=$close_evidence_status status=$close_run_status"
    fi
    if [[ "$latest_close_controller_closed" != "true" || "$close_controller_evidence_fresh" != "true" ]]; then
      echo "- finance close controller evidence is not closed and fresh"
    fi
    if [[ "$reconciliation_evidence_status" != "captured" || "$reconciliation_run_status" != "reconciled" ]]; then
      echo "- finance reconciliation evidence is not reconciled: evidence=$reconciliation_evidence_status status=$reconciliation_run_status"
    fi
    if [[ "$latest_reconciliation_reconciled" != "true" || "$reconciliation_evidence_fresh" != "true" ]]; then
      echo "- accounting reconciliation controller evidence is not reconciled and fresh"
    fi
    if [[ "$finance_export_csv_bytes" == "0" ]]; then
      echo "- finance export CSV evidence is empty or missing"
    fi
    if [[ "$export_delivery_evidence_status" != "captured" || "$export_delivery_status" != "delivered" || "$export_delivery_target_configured" != "true" ]]; then
      echo "- finance export delivery evidence is not delivered to a configured target"
    fi
    if [[ "$export_delivery_observer_status" != "ok" || "$export_delivery_count" == "0" ]]; then
      echo "- finance export delivery observer did not confirm delivery"
    fi
    case "$export_delivery_mode" in
      accounting*|erp*|netsuite|quickbooks|xero|sap|oracle_erp)
        ;;
      lark_drive|accept_only|unknown|none)
        echo "- finance export delivery mode is not an accounting/ERP target: $export_delivery_mode"
        ;;
      *)
        echo "- finance export delivery mode is not an accounting/ERP target: $export_delivery_mode"
        ;;
    esac
    if [[ -z "$export_delivery_system_id" ]]; then
      echo "- finance export delivery observer did not report an ERP/accounting system id"
    elif ! is_finance_system_identity "$export_delivery_system_id"; then
      echo "- finance export delivery system id is not a true ERP/accounting system identity: $export_delivery_system_id"
    elif [[ -n "$EXPECTED_FINANCE_SYSTEM_ID" && "$export_delivery_system_id" != "$EXPECTED_FINANCE_SYSTEM_ID" ]]; then
      echo "- finance export delivery system id does not match expected target: expected=$EXPECTED_FINANCE_SYSTEM_ID actual=$export_delivery_system_id"
    fi
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "finance evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

capture_finance_close_evidence() {
  local close_response
  local close_evidence_file="$EVIDENCE_DIR/finance-close-evidence.json"

  close_response="$(fetch_json POST /api/usage/finance-operations/run)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$close_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$close_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$close_evidence_file"
}

capture_finance_reconciliation_evidence() {
  local reconciliation_response
  local reconciliation_evidence_file="$EVIDENCE_DIR/finance-reconciliation-evidence.json"

  reconciliation_response="$(fetch_json POST /api/usage/finance-operations/reconcile)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$reconciliation_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$reconciliation_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$reconciliation_evidence_file"
}

capture_finance_export_delivery_evidence() {
  local delivery_response
  local delivery_evidence_file="$EVIDENCE_DIR/finance-export-delivery-evidence.json"

  delivery_response="$(fetch_json POST /api/usage/export/deliver)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$delivery_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$delivery_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$delivery_evidence_file"
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/usage/finance-summary >/dev/null
fetch_json GET /api/usage/finance-operations/summary >/dev/null

if [[ -z "$DELIVERY_OBSERVER_URL" && -n "${MANDOFORGE_USAGE_EXPORT_WEBHOOK_URL:-}" ]]; then
  DELIVERY_OBSERVER_URL="${MANDOFORGE_USAGE_EXPORT_WEBHOOK_URL%/finance/export}/healthz"
  DELIVERY_OBSERVER_URL="${DELIVERY_OBSERVER_URL/host.docker.internal/172.17.0.1}"
fi

if [[ "$RUN_FINANCE_CONTROLLERS" == "1" ]]; then
  capture_finance_close_evidence
  capture_finance_reconciliation_evidence
else
  echo "skipping finance close/reconciliation controllers; set RUN_STAGE2_FINANCE_CONTROLLERS=1 to include accounting evidence" >&2
fi

if [[ "$RUN_FINANCE_EXPORT" == "1" ]]; then
  fetch_file GET /api/usage/export.csv "$EVIDENCE_DIR/api-usage-export.csv" "$EVIDENCE_DIR/usage-export-csv-evidence.json"
  capture_finance_export_delivery_evidence
else
  echo "skipping finance export capture; set RUN_STAGE2_FINANCE_EXPORT=1 to include CSV and delivery evidence" >&2
fi

if [[ -n "$DELIVERY_OBSERVER_URL" ]]; then
  capture_delivery_observer "$DELIVERY_OBSERVER_URL"
fi

fetch_json GET /api/usage/finance-operations/summary >/dev/null
write_summary
