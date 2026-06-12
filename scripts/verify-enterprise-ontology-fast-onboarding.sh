#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_ONTOLOGY_ONBOARDING_GATE_SUBJECT:-ontology-onboarding-gate}"
ROLES="${MANDOFORGE_ONTOLOGY_ONBOARDING_GATE_ROLES:-admin}"
AUTH_TOKEN="${MANDOFORGE_ONTOLOGY_ONBOARDING_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-${MANDOFORGE_WORKER_TOKEN:-}}}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/enterprise-ontology-fast-onboarding}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "enterprise ontology fast-onboarding gate requires $1" >&2
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

run_file="$EVIDENCE_DIR/demo-run.json"
curl -sS -X POST "${headers[@]}" "$BASE_URL/api/ontology/onboarding/demo-runs" \
  | tee "$run_file" >/dev/null
run_id="$(jq -r '.id // empty' "$run_file")"
if [[ -z "$run_id" ]]; then
  echo "demo onboarding run did not return id" >&2
  exit 1
fi

jq -e '.dataset_count == 8 and .proposal_count >= 24' "$run_file" >/dev/null

mapfile -t proposal_ids < <(
  jq -r '
    .proposals[]
    | select(.proposal_type == "object" or .proposal_type == "relation" or .proposal_type == "action")
    | .id
  ' "$run_file"
)

if [[ "${#proposal_ids[@]}" -lt 1 ]]; then
  echo "demo onboarding run did not return approvable proposals" >&2
  exit 1
fi

for proposal_id in "${proposal_ids[@]}"; do
  curl -sS -X POST "${headers[@]}" \
    -H "content-type: application/json" \
    -d '{"decision":"approve","reason":"gate-approved demo proposal"}' \
    "$BASE_URL/api/ontology/onboarding/proposals/$proposal_id/review" \
    >"$EVIDENCE_DIR/review-$proposal_id.json"
done

materialized_file="$EVIDENCE_DIR/materialized.json"
curl -sS -X POST "${headers[@]}" "$BASE_URL/api/ontology/onboarding/runs/$run_id/materialize" \
  | tee "$materialized_file" >/dev/null
jq -e '.semantic_object_count >= 1 and .semantic_link_count >= 1 and .tool_spec_count >= 1' \
  "$materialized_file" >/dev/null

tool_specs_file="$EVIDENCE_DIR/tool-specs.json"
curl -sS "${headers[@]}" "$BASE_URL/api/ontology/onboarding/runs/$run_id/tool-specs" \
  | tee "$tool_specs_file" >/dev/null
jq -e 'any(.tool_specs[]?; .name == "commerce.refund_order")' "$tool_specs_file" >/dev/null

summary_file="$EVIDENCE_DIR/summary.txt"
{
  echo "run_id=$run_id"
  echo "dataset_count=$(jq -r '.dataset_count' "$run_file")"
  echo "proposal_count=$(jq -r '.proposal_count' "$run_file")"
  echo "semantic_object_count=$(jq -r '.semantic_object_count' "$materialized_file")"
  echo "semantic_link_count=$(jq -r '.semantic_link_count' "$materialized_file")"
  echo "tool_spec_count=$(jq -r '.tool_spec_count' "$materialized_file")"
  echo "run_file=$run_file"
  echo "materialized_file=$materialized_file"
  echo "tool_specs_file=$tool_specs_file"
} >"$summary_file"

cat "$summary_file"
echo "enterprise ontology fast-onboarding gate ok"
