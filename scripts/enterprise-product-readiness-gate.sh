#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_ENTERPRISE_GATE_SUBJECT:-enterprise-product-readiness-gate}"
ROLES="${MANDOFORGE_ENTERPRISE_GATE_ROLES:-admin}"
AUTH_TOKEN="${MANDOFORGE_ENTERPRISE_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-${MANDOFORGE_WORKER_TOKEN:-}}}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/enterprise-product-completion}"
ENTERPRISE_EVIDENCE_DIR="${ENTERPRISE_PRODUCT_EVIDENCE_DIR:-${SOURCE_EVIDENCE_DIR:-${STAGE2_EVIDENCE_DIR:-${MANDOFORGE_ENTERPRISE_PRODUCT_EVIDENCE_DIR:-}}}}"
COMPLETION_CHECKLIST_PATH="${ENTERPRISE_PRODUCT_COMPLETION_CHECKLIST:-${ENTERPRISE_EVIDENCE_DIR:+$ENTERPRISE_EVIDENCE_DIR/enterprise-product-completion-contract-gate/checklist.json}}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

required_lanes=(
  "production-deployment-safety"
  "runtime-production"
  "remote-computer-multinode"
  "live-connector-production"
  "ontology-engine"
  "workflowpack-enterprise-lifecycle"
  "enterprise-security-admin"
  "observability-ops"
  "product-surfaces"
)

required_lane_sources=(
  "production-deployment-safety-gate"
  "runtime-production-readiness-gate"
  "remote-computer-production-state-gate"
  "live-connector-production-semantics-gate"
  "ontology-engine-production-gate"
  "workflowpack-enterprise-lifecycle-gate"
  "enterprise-security-production-controls-gate"
  "observability-ops-production-gate"
  "product-surfaces-production-gate"
)

required_result_lanes=(
  "production-deployment-safety"
  "runtime-production"
  "remote-computer-multinode"
  "live-connector-production"
  "ontology-engine"
  "ontology-release-workflow-trigger"
  "workflowpack-enterprise-lifecycle"
  "enterprise-security-admin"
  "observability-ops"
  "product-surfaces"
)

required_result_sources=(
  "production-deployment-safety-gate"
  "runtime-production-readiness-gate"
  "remote-computer-production-state-gate"
  "live-connector-production-semantics-gate"
  "ontology-engine-production-gate"
  "ontology-release-workflow-trigger-gate"
  "workflowpack-enterprise-lifecycle-gate"
  "enterprise-security-production-controls-gate"
  "observability-ops-production-gate"
  "product-surfaces-production-gate"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "enterprise product readiness gate requires $1" >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq

summary_path_safe() {
  local summary_path="$1"
  local segment
  local -a segments
  [[ -n "$summary_path" ]] || return 1
  [[ "$summary_path" != /* ]] || return 1
  IFS='/' read -r -a segments <<<"$summary_path"
  for segment in "${segments[@]}"; do
    [[ -n "$segment" && "$segment" != "." && "$segment" != ".." ]] || return 1
  done
}

checklist_summary_path_ready() {
  local summary_path="$1"
  local expected_source="$2"
  local root="${ENTERPRISE_EVIDENCE_DIR:-$(dirname "$(dirname "$COMPLETION_CHECKLIST_PATH")")}"
  summary_path_safe "$summary_path" || return 1
  [[ -n "$root" && -s "$root/$summary_path" ]] || return 1
  jq -e --arg expected_source "$expected_source" '
    (.source // "") == $expected_source
    and (.required_evidence_class // "") == "customer_grade"
    and ((.status // "") | IN("ready", "validated", "completed", "passed"))
    and ((.blocked_count // 0) == 0)
  ' "$root/$summary_path" >/dev/null
}

completion_checklist_ready() {
  local checklist="$1"
  local i
  local required_lane_count="${#required_lanes[@]}"
  local required_result_count="${#required_result_lanes[@]}"
  local summary_path
  [[ -n "$checklist" && -s "$checklist" ]] || return 1
  jq -e \
    --argjson required_lane_count "$required_lane_count" \
    --argjson required_result_count "$required_result_count" \
    '
    (.source // "") == "enterprise-product-completion-contract-gate"
    and (.enterprise_product_status // "") == "enterprise_product_complete"
    and (.completion_blocked == false)
    and (.required_evidence_class // "") == "customer_grade"
    and (.archive_metadata_ready == true)
    and ((.support_owner // "") | length > 0)
    and ((.evidence_archive.immutable // false) == true)
    and ((.evidence_archive.uri // "") | test("^(s3|gs|az|https)://"))
    and ((.evidence_archive.uri // "") | test("(^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost|sandbox-only)([./:_-]|$)") | not)
    and ((.evidence_archive.digest // "") | test("^(sha256:)?[A-Fa-f0-9]{64}$"))
    and ((.evidence_archive.retention_policy // "") | length > 0)
    and ((.required_lanes // []) | length == $required_lane_count)
    and ((.ready_lanes // []) | length == $required_lane_count)
    and ((.lane_results // []) | length == $required_result_count)
    and ((.blocked_lanes // []) | length == 0)
  ' "$checklist" >/dev/null || return 1
  for i in "${!required_lanes[@]}"; do
    jq -e \
      --arg lane "${required_lanes[$i]}" \
      --arg expected_source "${required_lane_sources[$i]}" \
      '
      ((.required_lanes // []) | index($lane) != null)
      and ((.ready_lanes // []) | index($lane) != null)
      and any(.lane_results[]?;
        (.lane // "") == $lane
        and (.expected_source // "") == $expected_source
        and (.status // "") == "ready"
        and ((.summary_path // "") | length > 0)
        and ((.issue // null) == null)
      )
    ' "$checklist" >/dev/null || return 1
  done
  for i in "${!required_result_lanes[@]}"; do
    summary_path="$(jq -r \
      --arg lane "${required_result_lanes[$i]}" \
      --arg expected_source "${required_result_sources[$i]}" \
      '
        first(.lane_results[]?
          | select(
            (.lane // "") == $lane
            and (.expected_source // "") == $expected_source
            and (.status // "") == "ready"
            and ((.issue // null) == null)
          )
          | .summary_path // "")
      ' "$checklist")"
    checklist_summary_path_ready "$summary_path" "${required_result_sources[$i]}" || return 1
  done
}

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

readiness_file="$EVIDENCE_DIR/api-enterprise-product-readiness.json"
http_status="$(curl -sS -o "$readiness_file" -w "%{http_code}" "${headers[@]}" "$BASE_URL/api/enterprise-product/readiness")"
if [[ "$http_status" != 2* ]]; then
  echo "enterprise product readiness gate could not fetch $BASE_URL/api/enterprise-product/readiness: HTTP $http_status" >&2
  head -c 400 "$readiness_file" >&2 || true
  echo >&2
  exit 1
fi

status="$(jq -r '.status // "unknown"' "$readiness_file")"
completion_blocked="$(jq -r 'if has("completion_blocked") then .completion_blocked else true end' "$readiness_file")"
required_evidence_class="$(jq -r '.required_evidence_class // "unknown"' "$readiness_file")"
lane_count="$(jq -r '.lane_count // 0' "$readiness_file")"
ready_lane_count="$(jq -r '.ready_lane_count // 0' "$readiness_file")"
pilot_ready_lane_count="$(jq -r '.pilot_ready_lane_count // 0' "$readiness_file")"
blocked_lane_count="$(jq -r '.blocked_lane_count // 0' "$readiness_file")"
api_archive_uri="$(jq -r '.evidence_archive.uri // ""' "$readiness_file")"
api_archive_digest="$(jq -r '.evidence_archive.digest // ""' "$readiness_file")"
api_support_owner="$(jq -r '.evidence_archive.support_owner // ""' "$readiness_file")"
missing_lanes=()
for lane_id in "${required_lanes[@]}"; do
  if ! jq -e --arg lane_id "$lane_id" 'any(.lanes[]?; .id == $lane_id)' "$readiness_file" >/dev/null; then
    missing_lanes+=("$lane_id")
  fi
done

summary_file="$EVIDENCE_DIR/readiness-summary.txt"
{
  echo "enterprise_product_status=$status"
  echo "completion_blocked=$completion_blocked"
  echo "required_evidence_class=$required_evidence_class"
  echo "lane_count=$lane_count"
  echo "required_lane_count=${#required_lanes[@]}"
  echo "missing_lane_count=${#missing_lanes[@]}"
  echo "ready_lane_count=$ready_lane_count"
  echo "pilot_ready_lane_count=$pilot_ready_lane_count"
  echo "blocked_lane_count=$blocked_lane_count"
  echo "readiness_file=$readiness_file"
  echo "evidence_archive_uri=$api_archive_uri"
  echo "evidence_archive_digest=$api_archive_digest"
  echo "support_owner=$api_support_owner"
  if [[ -n "$COMPLETION_CHECKLIST_PATH" ]]; then
    echo "completion_checklist_file=$COMPLETION_CHECKLIST_PATH"
  fi
  if [[ "${#missing_lanes[@]}" -gt 0 ]]; then
    printf 'missing_lanes=%s\n' "$(IFS=,; echo "${missing_lanes[*]}")"
  fi
} >"$summary_file"

cat "$summary_file"

if [[ "$required_evidence_class" != "customer_grade" ]]; then
  echo "enterprise product readiness must require customer_grade evidence" >&2
  exit 1
fi

if [[ "$lane_count" != "${#required_lanes[@]}" ]]; then
  echo "enterprise product readiness must report exactly every required enterprise lane" >&2
  exit 1
fi

if [[ "${#missing_lanes[@]}" -gt 0 ]]; then
  printf 'enterprise product readiness is missing required lane ids: %s\n' "$(IFS=,; echo "${missing_lanes[*]}")" >&2
  exit 1
fi

if [[ "$completion_blocked" == "true" && "$ALLOW_BLOCKED" != "1" ]]; then
  echo "Enterprise product readiness gate failed closed; set ALLOW_BLOCKED=1 for inventory runs." >&2
  jq -r '.lanes[]? | select(.status != "ready") | "- \(.id): \(.status) -> \(.blockers | join("; "))"' "$readiness_file" >&2
  exit 1
fi

if [[ "$completion_blocked" != "true" && "$status" != "enterprise_product_complete" ]]; then
  echo "enterprise product readiness reports unblocked but status is not enterprise_product_complete" >&2
  exit 1
fi

if [[ "$completion_blocked" != "true" ]] && ! jq -e '
  (.evidence_archive.immutable == true)
  and ((.evidence_archive.support_owner // "") | length > 0)
  and ((.evidence_archive.uri // "") | test("^(s3|gs|az|https)://"))
  and ((.evidence_archive.uri // "") | test("(^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost|sandbox-only)([./:_-]|$)") | not)
  and ((.evidence_archive.digest // "") | test("^(sha256:)?[A-Fa-f0-9]{64}$"))
  and ((.evidence_archive.retention_policy // "") | length > 0)
' "$readiness_file" >/dev/null; then
  echo "enterprise product readiness reports complete without customer-grade archive metadata in API readback" >&2
  exit 1
fi

if [[ "$completion_blocked" != "true" ]] && ! completion_checklist_ready "$COMPLETION_CHECKLIST_PATH"; then
  echo "enterprise product readiness reports complete without a matching customer-grade completion checklist" >&2
  exit 1
fi

echo "enterprise product readiness gate ok"
