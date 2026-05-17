#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_WORKFLOW_PACK_GATE_SUBJECT:-workflow-pack-evidence-gate}"
ROLES="${MANDOFORGE_WORKFLOW_PACK_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/workflow-pack-evidence}"
MANIFEST_PATH="${WORKFLOW_PACK_MANIFEST_PATH:-packs/ai-governance/package.yaml}"
UPDATE_MANIFEST_PATH="${WORKFLOW_PACK_UPDATE_MANIFEST_PATH:-}"
UPDATE_VERSION="${WORKFLOW_PACK_UPDATE_VERSION:-0.1.1}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "workflow pack evidence gate requires $1" >&2
    exit 1
  fi
}

slugify() {
  printf '%s' "$1" | sed -E 's#^/##; s#[/:]+#-#g; s#[^A-Za-z0-9._-]+#-#g'
}

write_request() {
  local target="$1"
  local payload="$2"
  printf '%s' "$payload" >"$target"
}

fetch_json() {
  local method="$1"
  local path="$2"
  local payload
  local expected_prefix="${4:-2}"
  local label
  label="$(slugify "$path")"
  local target="$EVIDENCE_DIR/$label.json"
  local request_target="$EVIDENCE_DIR/$label.request.json"
  local response_body
  local response_json
  local http_status
  response_body="$(mktemp)"
  response_json="$(mktemp)"
  if [[ $# -ge 3 ]]; then
    payload="$3"
  else
    payload="{}"
  fi
  write_request "$request_target" "$payload"

  if [[ "$method" == "GET" ]]; then
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" \
      -H "x-mandoforge-subject: $SUBJECT" \
      -H "x-mandoforge-roles: $ROLES" \
      "$BASE_URL$path")"
  else
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" -X "$method" \
      -H "x-mandoforge-subject: $SUBJECT" \
      -H "x-mandoforge-roles: $ROLES" \
      -H "content-type: application/json" \
      -d "$payload" \
      "$BASE_URL$path")"
  fi

  if [[ "$http_status" != "$expected_prefix"* ]]; then
    echo "workflow pack evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,80p' "$response_body" >&2
    rm -f "$response_body" "$response_json"
    exit 1
  fi

  if ! jq . "$response_body" >"$response_json" 2>/dev/null; then
    jq -n --rawfile raw "$response_body" '{raw: $raw}' >"$response_json"
  fi

  jq -n \
    --arg method "$method" \
    --arg path "$path" \
    --arg request_file "$request_target" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --argjson http_status "$http_status" \
    --slurpfile response "$response_json" \
    '{
      method: $method,
      path: $path,
      request_file: $request_file,
      generated_at: $generated_at,
      http_status: $http_status,
      response: ($response[0] // {})
    }' >"$target"
  rm -f "$response_body" "$response_json"
  printf '%s\n' "$target"
}

require_cmd curl
require_cmd jq
require_cmd awk
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null

if [[ -z "$UPDATE_MANIFEST_PATH" ]]; then
  update_fixture_path="$(dirname "$MANIFEST_PATH")/package-v${UPDATE_VERSION}.yaml"
  if [[ -f "$update_fixture_path" ]]; then
    UPDATE_MANIFEST_PATH="$update_fixture_path"
  else
    source_dir="$(cd "$(dirname "$MANIFEST_PATH")" && pwd -P)"
    update_package_dir="$EVIDENCE_DIR/workflow-pack-update-package"
    rm -rf "$update_package_dir"
    cp -R "$source_dir" "$update_package_dir"
    awk -v version="$UPDATE_VERSION" '
      !updated && $0 ~ /^version: / {
        print "version: " version
        updated = 1
        next
      }
      { print }
    ' "$update_package_dir/package.yaml" >"$update_package_dir/package.yaml.tmp"
    mv "$update_package_dir/package.yaml.tmp" "$update_package_dir/package.yaml"
    UPDATE_MANIFEST_PATH="$update_package_dir/package.yaml"
  fi
fi

manifest_payload="$(jq -nc --arg manifest_path "$MANIFEST_PATH" '{manifest_path: $manifest_path}')"
update_manifest_payload="$(jq -nc --arg manifest_path "$UPDATE_MANIFEST_PATH" --arg reason "Whiskey WorkflowPack version update proof" '{manifest_path: $manifest_path, reason: $reason}')"
validate_file="$(fetch_json POST /api/workflow-packs/validate "$manifest_payload")"
install_file="$(fetch_json POST /api/workflow-packs/install "$manifest_payload")"
installation_id="$(jq -r '.response.id // empty' "$install_file")"
if [[ -z "$installation_id" ]]; then
  echo "workflow pack install evidence did not return an installation id" >&2
  exit 1
fi

blocked_onboarding_payload="$(jq -nc \
  --arg company_content "$(cat "$(dirname "$MANIFEST_PATH")/profiles/company.md")" \
  '{
    profiles: [
      {
        id: "company",
        content: $company_content
      }
    ],
    connectors: [
      {
        id: "knowledge-base",
        available_permissions: ["document.search"],
        provenance_attested: false,
        tenant_id: "",
        workspace_id: "workspace-demo",
        treats_results_as_data: false
      }
    ],
    reason: "Whiskey WorkflowPack onboarding blocked proof"
  }')"
blocked_onboarding_file="$(fetch_json POST "/api/workflow-packs/installations/$installation_id/onboarding/assess" "$blocked_onboarding_payload")"

fetch_json POST "/api/workflow-packs/installations/$installation_id/release" \
  '{"eval_gate_status":"passed","release_gate_status":"passed","gate_evidence":{"expected_failure":"release_before_stage"}}' \
  4 >/dev/null

stage_file="$(fetch_json POST "/api/workflow-packs/installations/$installation_id/stage" \
  '{"reason":"Whiskey WorkflowPack adoption evidence"}')"

fetch_json POST "/api/workflow-packs/installations/$installation_id/release" \
  '{"eval_gate_status":"pending","release_gate_status":"passed","gate_evidence":{"expected_failure":"eval_gate_not_passed"}}' \
  4 >/dev/null

release_file="$(fetch_json POST "/api/workflow-packs/installations/$installation_id/release" \
  '{"eval_gate_status":"passed","release_gate_status":"passed","gate_evidence":{"source":"workflow-pack-evidence-gate","eval_archive":"whiskey-ai-governance-regression","policy_gate":"approval-policy"},"reason":"Whiskey WorkflowPack adoption release proof"}')"
get_file="$(fetch_json GET "/api/workflow-packs/installations/$installation_id")"
list_file="$(fetch_json GET /api/workflow-packs/installations)"
released_get_file="$EVIDENCE_DIR/api-workflow-packs-installations-$installation_id-before-archive.json"
released_list_file="$EVIDENCE_DIR/api-workflow-packs-installations-before-archive.json"
cp "$get_file" "$released_get_file"
cp "$list_file" "$released_list_file"
rollback_file="$(fetch_json POST "/api/workflow-packs/installations/$installation_id/rollback" \
  '{"reason":"Whiskey WorkflowPack adoption rollback proof","gate_evidence":{"source":"workflow-pack-evidence-gate","rollback_archive":"whiskey-ai-governance-rollback"}}')"
rolled_back_get_file="$(fetch_json GET "/api/workflow-packs/installations/$installation_id")"
rolled_back_list_file="$(fetch_json GET /api/workflow-packs/installations)"
rolled_back_get_snapshot_file="$EVIDENCE_DIR/api-workflow-packs-installations-$installation_id-after-rollback.json"
rolled_back_list_snapshot_file="$EVIDENCE_DIR/api-workflow-packs-installations-after-rollback.json"
cp "$rolled_back_get_file" "$rolled_back_get_snapshot_file"
cp "$rolled_back_list_file" "$rolled_back_list_snapshot_file"
update_file="$(fetch_json POST "/api/workflow-packs/installations/$installation_id/update" "$update_manifest_payload")"
updated_installation_id="$(jq -r '.response.id // empty' "$update_file")"
old_after_update_file="$(fetch_json GET "/api/workflow-packs/installations/$installation_id")"
list_after_update_file="$(fetch_json GET /api/workflow-packs/installations)"
old_after_update_snapshot_file="$EVIDENCE_DIR/api-workflow-packs-installations-$installation_id-after-update.json"
list_after_update_snapshot_file="$EVIDENCE_DIR/api-workflow-packs-installations-after-update.json"
cp "$old_after_update_file" "$old_after_update_snapshot_file"
cp "$list_after_update_file" "$list_after_update_snapshot_file"
ready_onboarding_payload="$(jq -nc '{
  profiles: [
    {
      id: "company",
      content: "# Company Profile\nAcme Financial runs regulated AI adoption reviews with named approval owners and evidence retention."
    },
    {
      id: "department",
      content: "# Department Profile\nSecurity and risk review AI vendors weekly, with named owners and quarterly governance checkpoints."
    },
    {
      id: "approval-matrix",
      content: "approvals:\n  high:\n    required_role: approver\n    escalation_role: admin\n  medium:\n    required_role: operator\n  low:\n    required_role: operator\n  external_ai:\n    required_role: approver\n"
    },
    {
      id: "connector-map",
      content: "connectors:\n  knowledge-base:\n    scope: tenant\n    required_permissions:\n      - document.search\n      - document.read\n    source_system: confluence\n"
    },
    {
      id: "risk-policy",
      content: "risk_policy:\n  high:\n    approval_required: true\n    external_write_allowed: false\n  medium:\n    approval_required: true\n    external_write_allowed: false\n  low:\n    approval_required: false\n    external_write_allowed: false\n"
    },
    {
      id: "output-style",
      content: "# Output Style\nUse executive summary first, then evidence table, then draft recommendation."
    }
  ],
  connectors: [
    {
      id: "knowledge-base",
      available_permissions: ["document.search", "document.read"],
      provenance_attested: true,
      tenant_id: "tenant-demo",
      workspace_id: "workspace-demo",
      treats_results_as_data: true
    }
  ],
  reason: "Whiskey WorkflowPack onboarding readiness proof"
}')"
onboarding_file="$(fetch_json POST "/api/workflow-packs/installations/$updated_installation_id/onboarding/assess" "$ready_onboarding_payload")"
archive_file="$(fetch_json POST "/api/workflow-packs/installations/$installation_id/archive" \
  '{"reason":"Whiskey WorkflowPack adoption archive proof"}')"
archived_get_file="$(fetch_json GET "/api/workflow-packs/installations/$installation_id" "{}" 4)"
list_after_archive_file="$(fetch_json GET /api/workflow-packs/installations)"

validation_pack_id="$(jq -r '.response.pack_id // empty' "$validate_file")"
validated_file_count="$(jq -r '.response.validated_file_count // 0' "$validate_file")"
install_status="$(jq -r '.response.status // "unknown"' "$install_file")"
stage_status="$(jq -r '.response.status // "unknown"' "$stage_file")"
release_status="$(jq -r '.response.status // "unknown"' "$release_file")"
rollback_status="$(jq -r '.response.status // "unknown"' "$rollback_file")"
update_status="$(jq -r '.response.status // "unknown"' "$update_file")"
update_version="$(jq -r '.response.version // "unknown"' "$update_file")"
update_source_id="$(jq -r '.response.gate_evidence.version_update.source_installation_id // "unknown"' "$update_file")"
old_after_update_status="$(jq -r '.response.status // "unknown"' "$old_after_update_snapshot_file")"
old_after_update_released_at="$(jq -r '.response.released_at // "missing"' "$old_after_update_snapshot_file")"
blocked_onboarding_status="$(jq -r '.response.status // "unknown"' "$blocked_onboarding_file")"
onboarding_status="$(jq -r '.response.status // "unknown"' "$onboarding_file")"
onboarding_workflow="$(jq -r '.response.onboarding_workflow // "unknown"' "$onboarding_file")"
onboarding_eval="$(jq -r '.response.onboarding_eval // "unknown"' "$onboarding_file")"
required_profile_count="$(jq -r '.response.required_profile_count // 0' "$onboarding_file")"
profile_schema_count="$(jq -r '.response.profile_schema_count // 0' "$onboarding_file")"
provided_profile_count="$(jq -r '.response.provided_profile_count // 0' "$onboarding_file")"
placeholder_profile_count="$(jq -r '.response.placeholder_profile_count // 0' "$onboarding_file")"
connector_requirement_count="$(jq -r '.response.connector_requirement_count // 0' "$onboarding_file")"
ready_connector_count="$(jq -r '.response.ready_connector_count // 0' "$onboarding_file")"
onboarding_blocker_count="$(jq -r '[.response.blockers[]?] | length' "$onboarding_file")"
archive_status="$(jq -r '.response.status // "unknown"' "$archive_file")"
eval_gate_status="$(jq -r '.response.eval_gate_status // "unknown"' "$release_file")"
release_gate_status="$(jq -r '.response.release_gate_status // "unknown"' "$release_file")"
released_get_status="$(jq -r '.response.status // "unknown"' "$released_get_file")"
released_list_count="$(jq -r --arg id "$installation_id" '[.response[]? | select(.id == $id and .status == "released")] | length' "$released_list_file")"
rolled_back_get_status="$(jq -r '.response.status // "unknown"' "$rolled_back_get_snapshot_file")"
rolled_back_list_count="$(jq -r --arg id "$installation_id" '[.response[]? | select(.id == $id and .status == "rolled_back")] | length' "$rolled_back_list_snapshot_file")"
updated_list_count="$(jq -r --arg id "$updated_installation_id" '[.response[]? | select(.id == $id and .status == "installed")] | length' "$list_after_update_snapshot_file")"
old_after_update_list_count="$(jq -r --arg id "$installation_id" '[.response[]? | select(.id == $id and .status == "rolled_back")] | length' "$list_after_update_snapshot_file")"
archived_get_status="$(jq -r '.http_status // 0' "$archived_get_file")"
active_after_archive_count="$(jq -r --arg id "$installation_id" '[.response[]? | select(.id == $id)] | length' "$list_after_archive_file")"
updated_active_after_archive_count="$(jq -r --arg id "$updated_installation_id" '[.response[]? | select(.id == $id and .status == "installed")] | length' "$list_after_archive_file")"

if [[ "$validation_pack_id" != "ai-governance" ]]; then
  echo "workflow pack validation returned unexpected pack_id=$validation_pack_id" >&2
  exit 1
fi
if [[ "$validated_file_count" -lt 1 ]]; then
  echo "workflow pack validation did not validate referenced files" >&2
  exit 1
fi
if [[ "$install_status" != "installed" || "$stage_status" != "staged" || "$release_status" != "released" ]]; then
  echo "workflow pack lifecycle did not reach released state" >&2
  exit 1
fi
if [[ "$eval_gate_status" != "passed" || "$release_gate_status" != "passed" ]]; then
  echo "workflow pack release gates did not pass" >&2
  exit 1
fi
if [[ "$released_get_status" != "released" || "$released_list_count" != "1" ]]; then
  echo "workflow pack released installation was not retrievable" >&2
  exit 1
fi
if [[ "$rollback_status" != "rolled_back" ]]; then
  echo "workflow pack rollback did not reach rolled_back state" >&2
  exit 1
fi
if [[ "$rolled_back_get_status" != "rolled_back" || "$rolled_back_list_count" != "1" ]]; then
  echo "workflow pack rolled back installation was not retrievable" >&2
  exit 1
fi
if [[ -z "$updated_installation_id" || "$update_status" != "installed" || "$update_version" != "$UPDATE_VERSION" ]]; then
  echo "workflow pack update did not create an installed new version" >&2
  exit 1
fi
if [[ "$blocked_onboarding_status" != "blocked" ]]; then
  echo "workflow pack blocked onboarding assessment did not fail closed" >&2
  exit 1
fi
if [[ "$onboarding_status" != "ready" || "$onboarding_workflow" != "profile-onboarding" || "$onboarding_eval" != "profile-onboarding-regression" ]]; then
  echo "workflow pack onboarding readiness assessment did not reach ready state" >&2
  exit 1
fi
if [[ "$required_profile_count" != "6" || "$profile_schema_count" != "6" || "$provided_profile_count" != "6" || "$placeholder_profile_count" != "0" ]]; then
  echo "workflow pack onboarding profile coverage did not match the contract" >&2
  exit 1
fi
if [[ "$connector_requirement_count" != "1" || "$ready_connector_count" != "1" || "$onboarding_blocker_count" != "0" ]]; then
  echo "workflow pack onboarding connector readiness did not match the contract" >&2
  exit 1
fi
if [[ "$update_source_id" != "$installation_id" ]]; then
  echo "workflow pack update did not record the source installation id" >&2
  exit 1
fi
if [[ "$old_after_update_status" != "rolled_back" || "$old_after_update_released_at" == "missing" ]]; then
  echo "workflow pack update mutated the source installation unexpectedly" >&2
  exit 1
fi
if [[ "$updated_list_count" != "1" || "$old_after_update_list_count" != "1" ]]; then
  echo "workflow pack update did not preserve source and new version in active reads" >&2
  exit 1
fi
if [[ "$archive_status" != "archived" ]]; then
  echo "workflow pack archive did not reach archived state" >&2
  exit 1
fi
if [[ "$archived_get_status" != "404" || "$active_after_archive_count" != "0" || "$updated_active_after_archive_count" != "1" ]]; then
  echo "workflow pack archive did not remove only the source installation from active reads" >&2
  exit 1
fi

{
  echo "workflow_pack_status=version_created_after_rollback_and_archive"
  echo "pack_id=$validation_pack_id"
  echo "manifest_path=$MANIFEST_PATH"
  echo "update_manifest_path=$UPDATE_MANIFEST_PATH"
  echo "installation_id=$installation_id"
  echo "updated_installation_id=$updated_installation_id"
  echo "validated_file_count=$validated_file_count"
  echo "install_status=$install_status"
  echo "stage_status=$stage_status"
  echo "release_status=$release_status"
  echo "rollback_status=$rollback_status"
  echo "update_status=$update_status"
  echo "update_version=$update_version"
  echo "update_source_id=$update_source_id"
  echo "blocked_onboarding_status=$blocked_onboarding_status"
  echo "onboarding_status=$onboarding_status"
  echo "onboarding_workflow=$onboarding_workflow"
  echo "onboarding_eval=$onboarding_eval"
  echo "required_profile_count=$required_profile_count"
  echo "profile_schema_count=$profile_schema_count"
  echo "provided_profile_count=$provided_profile_count"
  echo "placeholder_profile_count=$placeholder_profile_count"
  echo "connector_requirement_count=$connector_requirement_count"
  echo "ready_connector_count=$ready_connector_count"
  echo "onboarding_blocker_count=$onboarding_blocker_count"
  echo "old_after_update_status=$old_after_update_status"
  echo "updated_list_count=$updated_list_count"
  echo "old_after_update_list_count=$old_after_update_list_count"
  echo "archive_status=$archive_status"
  echo "eval_gate_status=$eval_gate_status"
  echo "release_gate_status=$release_gate_status"
  echo "rolled_back_get_status=$rolled_back_get_status"
  echo "rolled_back_list_count=$rolled_back_list_count"
  echo "archived_get_status=$archived_get_status"
  echo "active_after_archive_count=$active_after_archive_count"
  echo "updated_active_after_archive_count=$updated_active_after_archive_count"
  echo "evidence_dir=$EVIDENCE_DIR"
} >"$EVIDENCE_DIR/summary.txt"

cat "$EVIDENCE_DIR/summary.txt"
