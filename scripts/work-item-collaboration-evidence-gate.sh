#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/work-item-collaboration-evidence}"
SUBJECT="${MANDOFORGE_WORK_ITEM_VERIFY_SUBJECT:-admin-1}"
ROLES="${MANDOFORGE_WORK_ITEM_VERIFY_ROLES:-admin}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)-$$}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "work item collaboration evidence gate requires $1" >&2
    exit 1
  fi
}

api_request() {
  local method="$1"
  local path="$2"
  local payload=""
  if [[ $# -ge 3 ]]; then
    payload="$3"
  fi
  local response_file
  local status
  response_file="$(mktemp)"
  if [[ "$method" == "POST" ]]; then
    status="$(
      curl -sS -o "$response_file" -w "%{http_code}" -X POST "$BASE_URL$path" \
        "${auth_headers[@]}" \
        -H 'content-type: application/json' \
        -d "$payload"
    )"
  else
    status="$(
      curl -sS -o "$response_file" -w "%{http_code}" "$BASE_URL$path" \
        "${auth_headers[@]}"
    )"
  fi
  if [[ "$status" != 2* ]]; then
    echo "work item collaboration evidence API request failed: $method $path returned HTTP $status" >&2
    sed -n '1,80p' "$response_file" >&2
    rm -f "$response_file"
    exit 1
  fi
  cat "$response_file"
  rm -f "$response_file"
}

api_get() {
  local path="$1"
  api_request GET "$path"
}

api_post() {
  local path="$1"
  local payload="{}"
  if [[ $# -ge 2 ]]; then
    payload="$2"
  fi
  api_request POST "$path" "$payload"
}

require_cmd jq
require_cmd curl

mkdir -p "$EVIDENCE_DIR"
curl -fsS "$BASE_URL/healthz" >/dev/null

created_file="$EVIDENCE_DIR/work-item-created.json"
list_file="$EVIDENCE_DIR/work-items.json"
audit_file="$EVIDENCE_DIR/audit-logs.json"
summary_file="$EVIDENCE_DIR/summary.txt"

api_post /api/work-items "$(
  jq -nc --arg run_id "$RUN_ID" '{
    title: ("Agent OS collaboration intake " + $run_id),
    description: "Evidence that external work can enter the Agent OS as a tracked WorkItem.",
    source: "manual",
    source_url: ("mandoforge://work-items/" + $run_id),
    priority: "high",
    metadata: {
      gate: "work-item-collaboration-evidence",
      layer: "collaboration",
      runtime_evidence_required: true
    }
  }'
)" >"$created_file"

work_item_id="$(jq -r '.id' "$created_file")"
api_get /api/work-items >"$list_file"
api_get /api/audit-logs >"$audit_file"

jq -e --arg id "$work_item_id" --arg run_id "$RUN_ID" '
  .id == $id
  and .title == ("Agent OS collaboration intake " + $run_id)
  and .status == "open"
  and .priority == "high"
  and .metadata.runtime_evidence_required == true
' "$created_file" >/dev/null

jq -e --arg id "$work_item_id" '
  any(.[]; .id == $id and .metadata.layer == "collaboration")
' "$list_file" >/dev/null

jq -e --arg id "$work_item_id" --arg subject "$SUBJECT" '
  any(.[]; .action == "work_item.created"
    and .resource_type == "work_item"
    and .resource_id == $id
    and .details.subject == $subject
    and .details.source == "manual")
' "$audit_file" >/dev/null

{
  echo "work_item_collaboration_status=validated"
  echo "work_item_id=$work_item_id"
  echo "created_file=$created_file"
  echo "list_file=$list_file"
  echo "audit_file=$audit_file"
} | tee "$summary_file"

echo "work item collaboration evidence gate ok"
