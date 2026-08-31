#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-approval-notification-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/approval-notification-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_APPROVAL_DELIVERY="${RUN_STAGE2_APPROVAL_DELIVERY:-0}"
DELIVERY_OBSERVER_URL="${APPROVAL_NOTIFICATION_DELIVERY_OBSERVER_URL:-}"
AUTH_TOKEN="${MANDOFORGE_STAGE2_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-${MANDOFORGE_WORKER_TOKEN:-}}}"

auth_headers=()
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

capture_delivery_observer() {
  local observer_url="$1"
  local target="$EVIDENCE_DIR/approval-notification-delivery-observer.json"
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
  local routing_file="$EVIDENCE_DIR/api-approvals-notification-routing-summary.json"
  local runs_file="$EVIDENCE_DIR/api-approvals-notifications-runs.json"
  local delivery_evidence_file="$EVIDENCE_DIR/approval-notification-delivery-evidence.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local routing_status
  local configured_channels
  local active_policies
  local pending_approvals
  local unroutable_pending
  local production_ops_status
  local ops_controller_fresh
  local ops_controller_age_hours
  local deployment_status
  local deployment_controller_fresh
  local deployment_controller_age_hours
  local delivery_evidence_status
  local delivery_status
  local delivery_target_count
  local delivery_delivered
  local observer_file="$EVIDENCE_DIR/approval-notification-delivery-observer.json"
  local delivery_observer_status
  local delivery_mode
  local delivery_forwarding_status
  local delivery_forwarding_channel
  local delivery_forwarded_message_id
  local delivery_forwarded_chat_id
  local blocked_count

  routing_status="$(jq -r '.status // "unknown"' "$routing_file")"
  configured_channels="$(jq -r '.configured_channel_count // .channel_count // (.configured_channels // [] | length)' "$routing_file")"
  active_policies="$(jq -r '.active_policy_count // .active_channel_policy_count // 0' "$routing_file")"
  pending_approvals="$(jq -r '.pending_approval_count // .pending_count // 0' "$routing_file")"
  unroutable_pending="$(jq -r '.unroutable_pending_approval_count // .unroutable_pending_count // 0' "$routing_file")"
  production_ops_status="$(jq -r '.production_ops.status // "unknown"' "$runs_file")"
  ops_controller_fresh="$(jq -r '.production_ops.controller_evidence_fresh // false' "$runs_file")"
  ops_controller_age_hours="$(jq -r '.production_ops.latest_controller_age_hours // "none"' "$runs_file")"
  deployment_status="$(jq -r '.deployment_readiness.status // "unknown"' "$runs_file")"
  deployment_controller_fresh="$(jq -r '.deployment_readiness.controller_evidence_fresh // false' "$runs_file")"
  deployment_controller_age_hours="$(jq -r '.deployment_readiness.latest_controller_age_hours // "none"' "$runs_file")"
  delivery_evidence_status="not_requested"
  delivery_status="not_run"
  delivery_target_count="0"
  delivery_delivered="false"
  delivery_observer_status="not_observed"
  delivery_mode="unknown"
  delivery_forwarding_status="unknown"
  delivery_forwarding_channel="unknown"
  delivery_forwarded_message_id="none"
  delivery_forwarded_chat_id="none"
  if [[ -s "$delivery_evidence_file" ]]; then
    delivery_evidence_status="$(jq -r '.status // "unknown"' "$delivery_evidence_file")"
    delivery_status="$(jq -r '.response.status // "unknown"' "$delivery_evidence_file")"
    delivery_target_count="$(jq -r '[.response.deliveries[]?.target_count // 0] | add // 0' "$delivery_evidence_file")"
    delivery_delivered="$(jq -r '(.response.delivered_count // 0) > 0' "$delivery_evidence_file")"
  fi
  if [[ -s "$observer_file" ]]; then
    delivery_observer_status="$(jq -r '.status // "unknown"' "$observer_file")"
    delivery_mode="$(jq -r '.delivery.delivery_mode // "unknown"' "$observer_file")"
    delivery_forwarding_status="$(jq -r '.delivery.latest_forwarding_status // "unknown"' "$observer_file")"
    delivery_forwarding_channel="$(jq -r '.delivery.latest_forwarding_channel // "unknown"' "$observer_file")"
    delivery_forwarded_message_id="$(jq -r '.delivery.latest_forwarded_message_id // "none"' "$observer_file")"
    delivery_forwarded_chat_id="$(jq -r '.delivery.latest_forwarded_chat_id // "none"' "$observer_file")"
  fi
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
    echo "production_ops_controller_evidence_fresh=$ops_controller_fresh"
    echo "production_ops_controller_age_hours=$ops_controller_age_hours"
    echo "deployment_readiness_status=$deployment_status"
    echo "deployment_controller_evidence_fresh=$deployment_controller_fresh"
    echo "deployment_controller_age_hours=$deployment_controller_age_hours"
    echo "delivery_evidence_status=$delivery_evidence_status"
    echo "delivery_status=$delivery_status"
    echo "delivery_target_count=$delivery_target_count"
    echo "delivery_delivered=$delivery_delivered"
    echo "delivery_observer_status=$delivery_observer_status"
    echo "delivery_mode=$delivery_mode"
    echo "delivery_forwarding_status=$delivery_forwarding_status"
    echo "delivery_forwarding_channel=$delivery_forwarding_channel"
    echo "delivery_forwarded_message_id=$delivery_forwarded_message_id"
    echo "delivery_forwarded_chat_id=$delivery_forwarded_chat_id"
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

capture_delivery_evidence() {
  local delivery_response
  local delivery_evidence_file="$EVIDENCE_DIR/approval-notification-delivery-evidence.json"

  delivery_response="$(fetch_json POST /api/approvals/notifications/run)"
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
fetch_json GET /api/approvals/notification-routing/summary >/dev/null
fetch_json GET /api/approvals/notifications/runs >/dev/null
fetch_json POST /api/approvals/notifications/deployment/validate >/dev/null
fetch_json POST /api/approvals/notifications/ops/validate >/dev/null

if [[ -z "$DELIVERY_OBSERVER_URL" && -n "${MANDOFORGE_APPROVAL_WEBHOOK_URL:-}" ]]; then
  DELIVERY_OBSERVER_URL="${MANDOFORGE_APPROVAL_WEBHOOK_URL%/approval/webhook}/healthz"
  DELIVERY_OBSERVER_URL="${DELIVERY_OBSERVER_URL/host.docker.internal/172.17.0.1}"
fi

if [[ "$RUN_APPROVAL_DELIVERY" == "1" ]]; then
  capture_delivery_evidence
else
  echo "skipping approval notification delivery; set RUN_STAGE2_APPROVAL_DELIVERY=1 to include delivery evidence" >&2
fi

if [[ -n "$DELIVERY_OBSERVER_URL" ]]; then
  capture_delivery_observer "$DELIVERY_OBSERVER_URL"
fi

fetch_json GET /api/approvals/notification-routing/summary >/dev/null
fetch_json GET /api/approvals/notifications/runs >/dev/null
write_summary
