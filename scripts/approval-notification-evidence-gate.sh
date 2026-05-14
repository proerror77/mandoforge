#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-approval-notification-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/approval-notification-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_APPROVAL_DELIVERY="${RUN_STAGE2_APPROVAL_DELIVERY:-0}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "approval notification evidence gate requires $1" >&2
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
    echo "approval notification evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local routing_file="$EVIDENCE_DIR/api-approvals-notification-routing-summary.json"
  local runs_file="$EVIDENCE_DIR/api-approvals-notifications-runs.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local routing_status
  local configured_channels
  local active_policies
  local pending_approvals
  local unroutable_pending
  local production_ops_status
  local deployment_status
  local blocked_count

  routing_status="$(jq -r '.status // "unknown"' "$routing_file")"
  configured_channels="$(jq -r '.configured_channel_count // (.configured_channels // [] | length)' "$routing_file")"
  active_policies="$(jq -r '.active_policy_count // .active_channel_policy_count // 0' "$routing_file")"
  pending_approvals="$(jq -r '.pending_approval_count // .pending_count // 0' "$routing_file")"
  unroutable_pending="$(jq -r '.unroutable_pending_approval_count // .unroutable_pending_count // 0' "$routing_file")"
  production_ops_status="$(jq -r '.production_ops.status // "unknown"' "$runs_file")"
  deployment_status="$(jq -r '.deployment_readiness.status // "unknown"' "$runs_file")"
  blocked_count="$(jq -r '[
      .production_ops.production_blocked,
      .deployment_readiness.production_blocked
    ] | map(select(. == true)) | length' "$runs_file")"

  {
    echo "approval_notification_routing_status=$routing_status"
    echo "configured_channel_count=$configured_channels"
    echo "active_policy_count=$active_policies"
    echo "pending_approval_count=$pending_approvals"
    echo "unroutable_pending_approval_count=$unroutable_pending"
    echo "production_ops_status=$production_ops_status"
    echo "deployment_readiness_status=$deployment_status"
    echo "production_blocked_count=$blocked_count"
    echo "approval_delivery_run=$RUN_APPROVAL_DELIVERY"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo
    echo "routing_attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.kind // .resource_type // "approval_notifications") - \(.message)"' "$routing_file"
    echo
    echo "delivery_attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.kind // .resource_type // "approval_notifications") - \(.message)"' "$runs_file"
    echo
    echo "production_ops_blocking_reasons:"
    jq -r '.production_ops.blocking_reasons[]? | "- \(.)"' "$runs_file"
    echo
    echo "deployment_blocking_reasons:"
    jq -r '.deployment_readiness.blocking_reasons[]? | "- \(.)"' "$runs_file"
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Approval notification evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/approvals/notification-routing/summary >/dev/null
fetch_json GET /api/approvals/notifications/runs >/dev/null
fetch_json POST /api/approvals/notifications/deployment/validate >/dev/null
fetch_json POST /api/approvals/notifications/ops/validate >/dev/null

if [[ "$RUN_APPROVAL_DELIVERY" == "1" ]]; then
  fetch_json POST /api/approvals/notifications/run >/dev/null
else
  echo "skipping approval notification delivery; set RUN_STAGE2_APPROVAL_DELIVERY=1 to include delivery evidence" >&2
fi

fetch_json GET /api/approvals/notification-routing/summary >/dev/null
fetch_json GET /api/approvals/notifications/runs >/dev/null
write_summary
