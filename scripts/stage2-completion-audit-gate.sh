#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-stage2-completion-audit-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
SOURCE_EVIDENCE_DIR="${SOURCE_EVIDENCE_DIR:-${STAGE2_EVIDENCE_DIR:-.mandoforge/stage2-production-evidence}}"
AUDIT_DIR="${AUDIT_DIR:-.mandoforge/stage2-completion-audit}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
TEAM_ID="${MANDOFORGE_STAGE2_TEAM_ID:-}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "stage2 completion audit gate requires $1" >&2
    exit 1
  fi
}

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

slugify() {
  printf '%s' "$1" | sed -E 's#^/##; s#[/:]+#-#g; s#[^A-Za-z0-9._-]+#-#g'
}

resolve_endpoint() {
  local endpoint="$1"
  if [[ "$endpoint" == ./* ]]; then
    return 1
  fi
  if [[ "$endpoint" == *"{team_id}"* ]]; then
    if [[ -z "$TEAM_ID" ]]; then
      return 1
    fi
    endpoint="${endpoint//\{team_id\}/$TEAM_ID}"
  fi
  printf '%s\n' "$endpoint"
}

local_script_artifact_path() {
  local endpoint="$1"
  printf '%s/local-script-%s.json\n' "$SOURCE_EVIDENCE_DIR" "$(slugify "$endpoint")"
}

json_array_from_file() {
  local path="$1"
  jq -R -s 'split("\n") | map(select(length > 0))' "$path"
}

require_cmd curl
require_cmd jq
require_cmd base64

mkdir -p "$AUDIT_DIR"
tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

readiness_file="$AUDIT_DIR/api-stage2-readiness.json"
response_body="$(mktemp)"
http_status="$(curl -sS -o "$response_body" -w "%{http_code}" "${auth_headers[@]}" "$BASE_URL/api/stage2/readiness")"
if [[ "$http_status" != 2* ]]; then
  echo "stage2 completion audit gate could not fetch /api/stage2/readiness: HTTP $http_status" >&2
  sed -n '1,40p' "$response_body" >&2
  rm -f "$response_body"
  exit 1
fi
tee "$readiness_file" <"$response_body" >/dev/null
rm -f "$response_body"

status="$(jq -r '.status // "unknown"' "$readiness_file")"
objective="$(jq -r '.objective // ""' "$readiness_file")"
completion_blocked="$(jq -r '.completion_blocked // true' "$readiness_file")"
open_gap_count="$(jq -r '.open_gap_count // 0' "$readiness_file")"
requirement_count="$(jq -r '.evidence_requirements | length' "$readiness_file")"

requirements_jsonl="$tmp_dir/requirements.jsonl"
: >"$requirements_jsonl"

total_missing_readiness=0
total_missing_validation=0
total_unresolved=0

while IFS= read -r encoded; do
  req_json="$(printf '%s' "$encoded" | base64 -d)"
  req_id="$(jq -r '.id' <<<"$req_json")"
  req_title="$(jq -r '.title' <<<"$req_json")"
  req_gap="$(jq -r '.gap' <<<"$req_json")"
  production_target="$(jq -r '.production_target' <<<"$req_json")"

  readiness_declared="$tmp_dir/$req_id.readiness-declared"
  validation_declared="$tmp_dir/$req_id.validation-declared"
  readiness_artifacts="$tmp_dir/$req_id.readiness-artifacts"
  validation_artifacts="$tmp_dir/$req_id.validation-artifacts"
  missing_readiness="$tmp_dir/$req_id.missing-readiness"
  missing_validation="$tmp_dir/$req_id.missing-validation"
  unresolved_endpoints="$tmp_dir/$req_id.unresolved"
  required_evidence="$tmp_dir/$req_id.required-evidence"

  : >"$readiness_declared"
  : >"$validation_declared"
  : >"$readiness_artifacts"
  : >"$validation_artifacts"
  : >"$missing_readiness"
  : >"$missing_validation"
  : >"$unresolved_endpoints"

  jq -r '.required_evidence[]?' <<<"$req_json" >"$required_evidence"

  while IFS= read -r endpoint; do
    [[ -z "$endpoint" ]] && continue
    if [[ "$endpoint" == ./* ]]; then
      echo "$endpoint" >>"$readiness_declared"
      artifact="$(local_script_artifact_path "$endpoint")"
      if [[ -s "$artifact" ]]; then
        echo "$artifact" >>"$readiness_artifacts"
      else
        echo "$endpoint" >>"$missing_readiness"
      fi
      continue
    fi
    if ! resolved="$(resolve_endpoint "$endpoint")"; then
      echo "$endpoint" >>"$unresolved_endpoints"
      echo "$endpoint" >>"$missing_readiness"
      continue
    fi
    echo "$resolved" >>"$readiness_declared"
    artifact="$SOURCE_EVIDENCE_DIR/$(slugify "$resolved").json"
    if [[ -s "$artifact" ]]; then
      echo "$artifact" >>"$readiness_artifacts"
    else
      echo "$resolved" >>"$missing_readiness"
    fi
  done < <(jq -r '.readiness_endpoints[]?' <<<"$req_json")

  while IFS= read -r endpoint; do
    [[ -z "$endpoint" ]] && continue
    if [[ "$endpoint" == ./* ]]; then
      echo "$endpoint" >>"$validation_declared"
      artifact="$(local_script_artifact_path "$endpoint")"
      if [[ -s "$artifact" ]]; then
        echo "$artifact" >>"$validation_artifacts"
      else
        echo "$endpoint" >>"$missing_validation"
      fi
      continue
    fi
    if ! resolved="$(resolve_endpoint "$endpoint")"; then
      echo "$endpoint" >>"$unresolved_endpoints"
      echo "$endpoint" >>"$missing_validation"
      continue
    fi
    echo "$resolved" >>"$validation_declared"
    artifact="$SOURCE_EVIDENCE_DIR/$(slugify "$resolved").json"
    if [[ -s "$artifact" ]]; then
      echo "$artifact" >>"$validation_artifacts"
    else
      echo "$resolved" >>"$missing_validation"
    fi
  done < <(jq -r '.validation_endpoints[]?' <<<"$req_json")

  missing_readiness_count="$(grep -c . "$missing_readiness" || true)"
  missing_validation_count="$(grep -c . "$missing_validation" || true)"
  unresolved_count="$(grep -c . "$unresolved_endpoints" || true)"
  readiness_artifact_count="$(grep -c . "$readiness_artifacts" || true)"
  validation_artifact_count="$(grep -c . "$validation_artifacts" || true)"

  total_missing_readiness=$((total_missing_readiness + missing_readiness_count))
  total_missing_validation=$((total_missing_validation + missing_validation_count))
  total_unresolved=$((total_unresolved + unresolved_count))

  req_status="blocked"
  if [[ "$completion_blocked" != "true" && "$missing_readiness_count" == "0" && "$missing_validation_count" == "0" ]]; then
    req_status="ready"
  fi

  jq -n \
    --arg id "$req_id" \
    --arg title "$req_title" \
    --arg gap "$req_gap" \
    --arg production_target "$production_target" \
    --arg status "$req_status" \
    --argjson readiness_endpoints "$(json_array_from_file "$readiness_declared")" \
    --argjson validation_endpoints "$(json_array_from_file "$validation_declared")" \
    --argjson required_evidence "$(json_array_from_file "$required_evidence")" \
    --argjson readiness_artifacts "$(json_array_from_file "$readiness_artifacts")" \
    --argjson validation_artifacts "$(json_array_from_file "$validation_artifacts")" \
    --argjson missing_readiness_endpoints "$(json_array_from_file "$missing_readiness")" \
    --argjson missing_validation_endpoints "$(json_array_from_file "$missing_validation")" \
    --argjson unresolved_endpoints "$(json_array_from_file "$unresolved_endpoints")" \
    --argjson readiness_artifact_count "$readiness_artifact_count" \
    --argjson validation_artifact_count "$validation_artifact_count" \
    --argjson missing_readiness_count "$missing_readiness_count" \
    --argjson missing_validation_count "$missing_validation_count" \
    '{
      id: $id,
      title: $title,
      gap: $gap,
      production_target: $production_target,
      status: $status,
      readiness_endpoints: $readiness_endpoints,
      validation_endpoints: $validation_endpoints,
      required_evidence: $required_evidence,
      readiness_artifacts: $readiness_artifacts,
      validation_artifacts: $validation_artifacts,
      missing_readiness_endpoints: $missing_readiness_endpoints,
      missing_validation_endpoints: $missing_validation_endpoints,
      unresolved_endpoints: $unresolved_endpoints,
      readiness_artifact_count: $readiness_artifact_count,
      validation_artifact_count: $validation_artifact_count,
      missing_readiness_count: $missing_readiness_count,
      missing_validation_count: $missing_validation_count
    }' >>"$requirements_jsonl"
done < <(jq -r '.evidence_requirements[]? | @base64' "$readiness_file")

checklist_json="$AUDIT_DIR/checklist.json"
jq -s \
  --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  --arg base_url "$BASE_URL" \
  --arg source_evidence_dir "$SOURCE_EVIDENCE_DIR" \
  --arg audit_dir "$AUDIT_DIR" \
  --arg status "$status" \
  --arg objective "$objective" \
  --arg completion_blocked "$completion_blocked" \
  --argjson open_gap_count "$open_gap_count" \
  --argjson requirement_count "$requirement_count" \
  --argjson missing_readiness_endpoint_count "$total_missing_readiness" \
  --argjson missing_validation_endpoint_count "$total_missing_validation" \
  --argjson unresolved_endpoint_count "$total_unresolved" \
  '{
    generated_at: $generated_at,
    base_url: $base_url,
    source_evidence_dir: $source_evidence_dir,
    audit_dir: $audit_dir,
    stage2_status: $status,
    objective: $objective,
    completion_blocked: ($completion_blocked == "true"),
    open_gap_count: $open_gap_count,
    evidence_requirement_count: $requirement_count,
    missing_readiness_endpoint_count: $missing_readiness_endpoint_count,
    missing_validation_endpoint_count: $missing_validation_endpoint_count,
    unresolved_endpoint_count: $unresolved_endpoint_count,
    requirements: .
  }' "$requirements_jsonl" >"$checklist_json"

checklist_md="$AUDIT_DIR/checklist.md"
{
  echo "# Stage 2 Completion Evidence Checklist"
  echo
  echo "- generated_at: $(jq -r '.generated_at' "$checklist_json")"
  echo "- base_url: $BASE_URL"
  echo "- source_evidence_dir: $SOURCE_EVIDENCE_DIR"
  echo "- stage2_status: $status"
  echo "- completion_blocked: $completion_blocked"
  echo "- open_gap_count: $open_gap_count"
  echo "- evidence_requirement_count: $requirement_count"
  echo "- missing_readiness_endpoint_count: $total_missing_readiness"
  echo "- missing_validation_endpoint_count: $total_missing_validation"
  echo "- unresolved_endpoint_count: $total_unresolved"
  echo
  echo "## Requirements"
  jq -r '
    .requirements[]
    | "### " + .id + "\n"
      + "- title: " + .title + "\n"
      + "- status: " + .status + "\n"
      + "- production_target: " + .production_target + "\n"
      + "- missing_readiness_count: " + (.missing_readiness_count | tostring) + "\n"
      + "- missing_validation_count: " + (.missing_validation_count | tostring) + "\n"
      + "- readiness_artifacts: " + (.readiness_artifact_count | tostring) + "\n"
      + "- validation_artifacts: " + (.validation_artifact_count | tostring) + "\n"
      + "- gap: " + .gap + "\n"
      + "- required_evidence:\n"
      + ((.required_evidence // []) | map("  - " + .) | join("\n")) + "\n"
      + "- missing_readiness_endpoints:\n"
      + (if (.missing_readiness_endpoints | length) == 0 then "  - <none>" else ((.missing_readiness_endpoints // []) | map("  - " + .) | join("\n")) end) + "\n"
      + "- missing_validation_endpoints:\n"
      + (if (.missing_validation_endpoints | length) == 0 then "  - <none>" else ((.missing_validation_endpoints // []) | map("  - " + .) | join("\n")) end) + "\n"
  ' "$checklist_json"
} >"$checklist_md"

cat "$checklist_md"

if [[ "$completion_blocked" == "true" && "$ALLOW_BLOCKED" != "1" ]]; then
  echo "Stage 2 completion audit gate failed closed because readiness reports completion_blocked=true." >&2
  exit 1
fi

if [[ "$total_missing_readiness" != "0" || "$total_missing_validation" != "0" || "$total_unresolved" != "0" ]]; then
  if [[ "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Stage 2 completion audit gate failed closed because required evidence artifacts are missing." >&2
    exit 1
  fi
fi
