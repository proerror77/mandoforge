#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-observability-collector-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/observability-collector-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_REMEDIATION="${RUN_STAGE2_OBSERVABILITY_REMEDIATION:-0}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "observability collector evidence gate requires $1" >&2
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
    echo "observability collector evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local readiness_file="$EVIDENCE_DIR/api-observability-collector-readiness.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local status
  local production_ops_status
  local deployment_status
  local cluster_status
  local remediation_status
  local blocked_count

  status="$(jq -r '.status // "unknown"' "$readiness_file")"
  production_ops_status="$(jq -r '.production_ops.status // "unknown"' "$readiness_file")"
  deployment_status="$(jq -r '.deployment_readiness.status // "unknown"' "$readiness_file")"
  cluster_status="$(jq -r '.cluster_rollout.status // "unknown"' "$readiness_file")"
  remediation_status="$(jq -r '.remediation_supervision.status // "unknown"' "$readiness_file")"
  blocked_count="$(jq -r '[
      .production_ops.production_blocked,
      .deployment_readiness.production_blocked,
      .cluster_rollout.production_blocked,
      .remediation_supervision.production_blocked
    ] | map(select(. == true)) | length' "$readiness_file")"

  {
    echo "observability_collector_status=$status"
    echo "production_ops_status=$production_ops_status"
    echo "deployment_readiness_status=$deployment_status"
    echo "cluster_rollout_status=$cluster_status"
    echo "remediation_supervision_status=$remediation_status"
    echo "production_blocked_count=$blocked_count"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo "remediation_run=$RUN_REMEDIATION"
    echo
    echo "attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.kind) - \(.message)"' "$readiness_file"
    echo
    echo "deployment_blocking_reasons:"
    jq -r '.deployment_readiness.blocking_reasons[]? | "- \(.)"' "$readiness_file"
    echo
    echo "cluster_blocking_reasons:"
    jq -r '.cluster_rollout.blocking_reasons[]? | "- \(.)"' "$readiness_file"
    echo
    echo "remediation_blocking_reasons:"
    jq -r '.remediation_supervision.blocking_reasons[]? | "- \(.)"' "$readiness_file"
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Observability collector evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/observability >/dev/null
fetch_json GET /api/observability/collector-readiness >/dev/null
fetch_json POST /api/observability/collector/deployment/validate >/dev/null
fetch_json POST /api/observability/collector/cluster/validate >/dev/null

if [[ "$RUN_REMEDIATION" == "1" ]]; then
  fetch_json POST /api/observability/remediation/run >/dev/null
else
  echo "skipping observability remediation run; set RUN_STAGE2_OBSERVABILITY_REMEDIATION=1 to include remediation evidence" >&2
fi

fetch_json GET /api/observability/collector-readiness >/dev/null
write_summary
