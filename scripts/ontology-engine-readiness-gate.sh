#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_ONTOLOGY_ENGINE_GATE_SUBJECT:-ontology-engine-readiness-gate}"
ROLES="${MANDOFORGE_ONTOLOGY_ENGINE_GATE_ROLES:-admin}"
AUTH_TOKEN="${MANDOFORGE_ONTOLOGY_ENGINE_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-${MANDOFORGE_WORKER_TOKEN:-}}}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/ontology-engine-readiness}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ontology engine readiness gate requires $1" >&2
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

readiness_file="$EVIDENCE_DIR/api-ontology-engine-readiness.json"
http_status="$(curl -sS -o "$readiness_file" -w "%{http_code}" "${headers[@]}" "$BASE_URL/api/ontology/engine-readiness")"
if [[ "$http_status" != 2* ]]; then
  echo "ontology engine readiness gate could not fetch $BASE_URL/api/ontology/engine-readiness: HTTP $http_status" >&2
  head -c 400 "$readiness_file" >&2 || true
  echo >&2
  exit 1
fi

status="$(jq -r '.status // "unknown"' "$readiness_file")"
required_evidence_class="$(jq -r '.required_evidence_class // "unknown"' "$readiness_file")"
registry_version="$(jq -r '.registry_version // "unknown"' "$readiness_file")"
check_count="$(jq -r '.check_count // 0' "$readiness_file")"
ready_check_count="$(jq -r '.ready_check_count // 0' "$readiness_file")"
pilot_ready_check_count="$(jq -r '.pilot_ready_check_count // 0' "$readiness_file")"
blocked_check_count="$(jq -r '.blocked_check_count // 0' "$readiness_file")"

summary_file="$EVIDENCE_DIR/summary.txt"
{
  echo "ontology_engine_status=$status"
  echo "required_evidence_class=$required_evidence_class"
  echo "registry_version=$registry_version"
  echo "check_count=$check_count"
  echo "ready_check_count=$ready_check_count"
  echo "pilot_ready_check_count=$pilot_ready_check_count"
  echo "blocked_check_count=$blocked_check_count"
  echo "readiness_file=$readiness_file"
} >"$summary_file"

cat "$summary_file"

echo "readiness_checks:"
jq -r '.checks[]? | "- \(.id)=\(.status) evidence=\(.current_evidence_class)"' "$readiness_file"

if [[ "$required_evidence_class" != "customer_grade" ]]; then
  echo "ontology engine readiness must require customer_grade evidence" >&2
  exit 1
fi

if [[ "$registry_version" == "unknown" || "$registry_version" == "" ]]; then
  echo "ontology engine readiness must expose a registry version" >&2
  exit 1
fi

jq -e '
  any(.checks[]?; .id == "core-registry" and .status == "ready")
  and any(.checks[]?; .id == "relation-constraints" and .status == "ready")
  and any(.checks[]?; .id == "builder-review-proposals" and .status == "ready")
  and any(.checks[]?; .id == "context-packet-rendering" and .status == "ready")
  and any(.checks[]?; .id == "domain-ontology-lifecycle")
  and any(.checks[]?; .id == "approved-release-materialization")
  and any(.checks[]?; .id == "migration-policy")
' "$readiness_file" >/dev/null || {
  echo "ontology engine readiness is missing required checks" >&2
  exit 1
}

if [[ "$status" == "blocked" && "$ALLOW_BLOCKED" != "1" ]]; then
  echo "Ontology Engine readiness gate failed closed; set ALLOW_BLOCKED=1 for inventory runs." >&2
  jq -r '.checks[]? | select(.status != "ready") | "- \(.id): \(.status) -> \(.blockers | join("; "))"' "$readiness_file" >&2
  exit 1
fi

if [[ "$status" != "blocked" && "$status" != "ready" ]]; then
  echo "ontology engine readiness status must be blocked or ready" >&2
  exit 1
fi

echo "ontology engine readiness gate ok"
