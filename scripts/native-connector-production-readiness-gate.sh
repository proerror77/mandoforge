#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_NATIVE_CONNECTOR_GATE_SUBJECT:-native-connector-production-gate}"
ROLES="${MANDOFORGE_NATIVE_CONNECTOR_GATE_ROLES:-admin}"
AUTH_TOKEN="${MANDOFORGE_NATIVE_CONNECTOR_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-}}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/native-connector-production-readiness}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

required_connectors=(
  "tmall-top"
  "taobao-open-platform"
  "xiaohongshu-shop"
  "xianyu-goofish"
  "tiktok-shop-open-api"
  "amazon-selling-partner-api"
  "github-connector"
  "lark-mcp"
  "feishu-mcp"
  "lark-native"
  "feishu-native"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "native connector production readiness gate requires $1" >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq

mkdir -p "$EVIDENCE_DIR"

headers=()
if [[ -n "$AUTH_TOKEN" ]]; then
  headers+=(-H "authorization: Bearer $AUTH_TOKEN")
else
  headers+=(
    -H "x-mandoforge-subject: $SUBJECT"
    -H "x-mandoforge-roles: $ROLES"
  )
fi

readiness_file="$EVIDENCE_DIR/api-native-connector-production-readiness.json"
http_status="$(curl -sS -o "$readiness_file" -w "%{http_code}" "${headers[@]}" "$BASE_URL/api/native-connectors/production-readiness")"
if [[ "$http_status" != 2* ]]; then
  echo "native connector production readiness gate could not fetch $BASE_URL/api/native-connectors/production-readiness: HTTP $http_status" >&2
  head -c 400 "$readiness_file" >&2 || true
  echo >&2
  exit 1
fi

status="$(jq -r '.status // "unknown"' "$readiness_file")"
required_evidence_class="$(jq -r '.required_evidence_class // "unknown"' "$readiness_file")"
connector_count="$(jq -r '.connector_count // 0' "$readiness_file")"
ready_connector_count="$(jq -r '.ready_connector_count // 0' "$readiness_file")"
blocked_connector_count="$(jq -r '.blocked_connector_count // 0' "$readiness_file")"
live_enabled="$(jq -r '.live_enabled // false' "$readiness_file")"
missing_connectors=()
for connector_id in "${required_connectors[@]}"; do
  if ! jq -e --arg connector_id "$connector_id" \
    'any(.connectors[]?; .connector_id == $connector_id)' \
    "$readiness_file" >/dev/null; then
    missing_connectors+=("$connector_id")
  fi
done

summary_file="$EVIDENCE_DIR/summary.txt"
{
  echo "native_connector_production_status=$status"
  echo "required_evidence_class=$required_evidence_class"
  echo "live_enabled=$live_enabled"
  echo "connector_count=$connector_count"
  echo "required_connector_count=${#required_connectors[@]}"
  echo "missing_connector_count=${#missing_connectors[@]}"
  echo "ready_connector_count=$ready_connector_count"
  echo "blocked_connector_count=$blocked_connector_count"
  echo "readiness_file=$readiness_file"
  if [[ "${#missing_connectors[@]}" -gt 0 ]]; then
    printf 'missing_connectors=%s\n' "$(IFS=,; echo "${missing_connectors[*]}")"
  fi
} >"$summary_file"

cat "$summary_file"

if [[ "$required_evidence_class" != "customer_grade" ]]; then
  echo "native connector production readiness must require customer_grade evidence" >&2
  exit 1
fi

if [[ "$connector_count" -lt "${#required_connectors[@]}" ]]; then
  echo "native connector production readiness must include all required ecommerce, SWE, Lark, and Feishu connectors" >&2
  exit 1
fi

if [[ "${#missing_connectors[@]}" -gt 0 ]]; then
  printf 'native connector production readiness is missing required connector ids: %s\n' "$(IFS=,; echo "${missing_connectors[*]}")" >&2
  exit 1
fi

jq -e '
  all(.connectors[]?;
    any(.checks[]?; .id == "approval-commit-boundary" and .status == "ready")
    and any(.checks[]?; .id == "secret-redaction" and .status == "ready")
    and any(.checks[]?; .id == "idempotency-reconciliation")
    and any(.checks[]?; .id == "sandbox-live-separation")
    and any(.checks[]?; .id == "archived-deployment-evidence")
  )
' "$readiness_file" >/dev/null || {
  echo "native connector production readiness is missing required per-connector checks" >&2
  exit 1
}

if [[ "$status" == "blocked" && "$ALLOW_BLOCKED" != "1" ]]; then
  echo "Native connector production readiness gate failed closed; set ALLOW_BLOCKED=1 for inventory runs." >&2
  jq -r '.connectors[]? | select(.status != "ready") | "- \(.connector_id): \(.status) -> \(.blockers | .[0:3] | join("; "))"' "$readiness_file" >&2
  exit 1
fi

if [[ "$status" != "blocked" && "$status" != "ready" ]]; then
  echo "native connector production readiness status must be blocked or ready" >&2
  exit 1
fi

echo "native connector production readiness gate ok"
