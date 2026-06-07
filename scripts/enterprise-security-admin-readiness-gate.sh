#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_ENTERPRISE_SECURITY_GATE_SUBJECT:-enterprise-security-admin-readiness-gate}"
ROLES="${MANDOFORGE_ENTERPRISE_SECURITY_GATE_ROLES:-admin}"
AUTH_TOKEN="${MANDOFORGE_ENTERPRISE_SECURITY_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-${MANDOFORGE_WORKER_TOKEN:-}}}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/enterprise-security-admin}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "enterprise security/admin readiness gate requires $1" >&2
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

readiness_file="$EVIDENCE_DIR/api-enterprise-security-admin-readiness.json"
http_status="$(curl -sS -o "$readiness_file" -w "%{http_code}" "${headers[@]}" "$BASE_URL/api/enterprise-security/admin-readiness")"
if [[ "$http_status" != 2* ]]; then
  echo "enterprise security/admin readiness gate could not fetch $BASE_URL/api/enterprise-security/admin-readiness: HTTP $http_status" >&2
  head -c 400 "$readiness_file" >&2 || true
  echo >&2
  exit 1
fi

status="$(jq -r '.status // "unknown"' "$readiness_file")"
completion_blocked="$(jq -r 'if has("completion_blocked") then .completion_blocked else true end' "$readiness_file")"
required_evidence_class="$(jq -r '.required_evidence_class // "unknown"' "$readiness_file")"
check_count="$(jq -r '.check_count // 0' "$readiness_file")"
ready_check_count="$(jq -r '.ready_check_count // 0' "$readiness_file")"
blocked_check_count="$(jq -r '.blocked_check_count // 0' "$readiness_file")"

summary_file="$EVIDENCE_DIR/readiness-summary.txt"
{
  echo "enterprise_security_admin_status=$status"
  echo "completion_blocked=$completion_blocked"
  echo "required_evidence_class=$required_evidence_class"
  echo "check_count=$check_count"
  echo "ready_check_count=$ready_check_count"
  echo "blocked_check_count=$blocked_check_count"
  echo "readiness_file=$readiness_file"
  echo
  echo "blocked_checks:"
  jq -r '.checks[]? | select(.status != "ready") | "- \(.id): \(.current_evidence_class) -> \(.blockers | join("; "))"' "$readiness_file"
} >"$summary_file"

cat "$summary_file"

if [[ "$required_evidence_class" != "customer_grade" ]]; then
  echo "enterprise security/admin readiness must require customer_grade evidence" >&2
  exit 1
fi

if [[ "$completion_blocked" == "true" && "$ALLOW_BLOCKED" != "1" ]]; then
  echo "Enterprise security/admin readiness gate failed closed; set ALLOW_BLOCKED=1 for inventory runs." >&2
  exit 1
fi

if [[ "$completion_blocked" != "true" && "$status" != "ready" ]]; then
  echo "enterprise security/admin readiness reports unblocked but status is not ready" >&2
  exit 1
fi

echo "enterprise security/admin readiness gate ok"
