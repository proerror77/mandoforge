#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_ONTOLOGY_RELEASE_GATE_SUBJECT:-ontology-release-loop-gate}"
ROLES="${MANDOFORGE_ONTOLOGY_RELEASE_GATE_ROLES:-admin}"
AUTH_TOKEN="${MANDOFORGE_ONTOLOGY_RELEASE_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-${MANDOFORGE_WORKER_TOKEN:-}}}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/ontology-release-loop}"
RELEASE_CLASS="${MANDOFORGE_ONTOLOGY_RELEASE_CLASS:-customer_grade}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ontology release loop gate requires $1" >&2
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

json_headers=(-H "content-type: application/json")

fetch_json() {
  local method="$1"
  local path="$2"
  local body="$3"
  local outfile="$4"
  local status
  if [[ -n "$body" ]]; then
    status="$(curl -sS -X "$method" -o "$outfile" -w "%{http_code}" "${headers[@]}" "${json_headers[@]}" -d "$body" "$BASE_URL$path")"
  else
    status="$(curl -sS -X "$method" -o "$outfile" -w "%{http_code}" "${headers[@]}" "$BASE_URL$path")"
  fi
  if [[ "$status" != 2* ]]; then
    echo "ontology release loop gate request failed: $method $path HTTP $status" >&2
    head -c 600 "$outfile" >&2 || true
    echo >&2
    exit 1
  fi
}

approve_release_proposals() {
  local run_file="$1"
  local index="$2"
  local proposal_ids
  proposal_ids="$(jq -r '.proposals[]? | select(.proposal_type == "object" or .proposal_type == "action") | .id' "$run_file")"
  if [[ -z "$proposal_ids" ]]; then
    echo "demo run has no object/action proposals to approve" >&2
    exit 1
  fi
  while IFS= read -r proposal_id; do
    [[ -z "$proposal_id" ]] && continue
    fetch_json \
      POST \
      "/api/ontology/onboarding/proposals/$proposal_id/review" \
      '{"decision":"approve","reason":"release loop gate fixture"}' \
      "$EVIDENCE_DIR/review-$index-$proposal_id.json"
  done <<<"$proposal_ids"
}

create_promoted_release() {
  local index="$1"
  local version="$2"
  local run_file="$EVIDENCE_DIR/run-$index.json"
  local materialized_file="$EVIDENCE_DIR/materialized-$index.json"
  local candidate_file="$EVIDENCE_DIR/candidate-$index.json"
  local gated_file="$EVIDENCE_DIR/gated-$index.json"
  local promoted_file="$EVIDENCE_DIR/promoted-$index.json"
  local run_id
  local release_id

  fetch_json POST /api/ontology/onboarding/demo-runs "" "$run_file"
  run_id="$(jq -r '.id' "$run_file")"
  approve_release_proposals "$run_file" "$index"
  fetch_json POST "/api/ontology/onboarding/runs/$run_id/materialize" "" "$materialized_file"
  fetch_json \
    POST \
    "/api/ontology/onboarding/runs/$run_id/release-candidate" \
    "$(jq -n --arg version "$version" --arg release_class "$RELEASE_CLASS" '{version: $version, release_class: $release_class}')" \
    "$candidate_file"
  release_id="$(jq -r '.id' "$candidate_file")"
  fetch_json POST "/api/ontology/releases/$release_id/gate" "" "$gated_file"
  jq -e '.gate_result.status == "passed"' "$gated_file" >/dev/null || {
    echo "ontology release gate did not pass for $version" >&2
    jq '.gate_result' "$gated_file" >&2
    exit 1
  }
  fetch_json POST "/api/ontology/releases/$release_id/promote" "" "$promoted_file"
  jq -r '.id' "$promoted_file"
}

stamp="$(date -u +%Y%m%d%H%M%S)"
first_id="$(create_promoted_release 1 "commerce-gate-$stamp-001")"
second_id="$(create_promoted_release 2 "commerce-gate-$stamp-002")"

fetch_json POST "/api/ontology/releases/$second_id/rollback" "" "$EVIDENCE_DIR/rollback.json"
rollback_active_id="$(jq -r '.id' "$EVIDENCE_DIR/rollback.json")"
if [[ "$rollback_active_id" != "$first_id" ]]; then
  echo "rollback did not restore first active release: expected $first_id got $rollback_active_id" >&2
  exit 1
fi

fetch_json GET /api/ontology/releases "" "$EVIDENCE_DIR/releases.json"
fetch_json GET /api/ontology/engine-readiness "" "$EVIDENCE_DIR/readiness.json"

jq -e '
  any(.checks[]?; .id == "domain-ontology-lifecycle" and .status == "ready")
  and any(.checks[]?; .id == "approved-release-materialization" and .status == "ready")
  and any(.checks[]?; .id == "migration-policy" and .status == "ready")
  and any(.checks[]?; .id == "conflict-trust-runtime-gates" and .status == "pilot_ready")
' "$EVIDENCE_DIR/readiness.json" >/dev/null || {
  echo "ontology release readiness readback is missing expected release-backed checks" >&2
  jq '.checks[]? | {id, status, current_evidence_class, blockers}' "$EVIDENCE_DIR/readiness.json" >&2
  exit 1
}

summary_file="$EVIDENCE_DIR/summary.txt"
{
  echo "first_release_id=$first_id"
  echo "second_release_id=$second_id"
  echo "rollback_active_release_id=$rollback_active_id"
  echo "readiness_status=$(jq -r '.status' "$EVIDENCE_DIR/readiness.json")"
  echo "ready_check_count=$(jq -r '.ready_check_count' "$EVIDENCE_DIR/readiness.json")"
  echo "pilot_ready_check_count=$(jq -r '.pilot_ready_check_count' "$EVIDENCE_DIR/readiness.json")"
  echo "blocked_check_count=$(jq -r '.blocked_check_count' "$EVIDENCE_DIR/readiness.json")"
  echo "evidence_dir=$EVIDENCE_DIR"
} >"$summary_file"

cat "$summary_file"
echo "release_backed_checks:"
jq -r '.checks[]? | select(.id == "domain-ontology-lifecycle" or .id == "approved-release-materialization" or .id == "migration-policy" or .id == "conflict-trust-runtime-gates") | "- \(.id)=\(.status) evidence=\(.current_evidence_class)"' "$EVIDENCE_DIR/readiness.json"
echo "ontology release loop gate ok"
