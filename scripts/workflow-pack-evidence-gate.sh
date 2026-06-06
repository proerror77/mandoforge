#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_WORKFLOW_PACK_GATE_SUBJECT:-workflow-pack-evidence-gate}"
ROLES="${MANDOFORGE_WORKFLOW_PACK_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/workflow-pack-evidence}"
MANIFEST_PATH="${WORKFLOW_PACK_MANIFEST_PATH:-packs/ai-governance/package.yaml}"
UPDATE_MANIFEST_PATH="${WORKFLOW_PACK_UPDATE_MANIFEST_PATH:-}"
UPDATE_VERSION="${WORKFLOW_PACK_UPDATE_VERSION:-0.1.1}"
CONNECTOR_TEAM_ID="${WORKFLOW_PACK_CONNECTOR_TEAM_ID:-}"
CONNECTOR_SERVER_ID="${WORKFLOW_PACK_CONNECTOR_SERVER_ID:-}"
CONNECTOR_TOOL_NAME="${WORKFLOW_PACK_CONNECTOR_TOOL_NAME:-search}"
CONNECTOR_SERVER_NAME="${WORKFLOW_PACK_CONNECTOR_SERVER_NAME:-whiskey-docs}"
REQUIRE_CONNECTOR_BINDING="${WORKFLOW_PACK_REQUIRE_CONNECTOR_BINDING:-0}"
WORKFLOW_PACK_MCP_CALL_URL="${WORKFLOW_PACK_MCP_CALL_URL:-}"
WORKFLOW_PACK_MCP_QUERY="${WORKFLOW_PACK_MCP_QUERY:-OpenAI}"
WORKFLOW_PACK_REQUIRE_LIVE_CONNECTOR_AUTH="${WORKFLOW_PACK_REQUIRE_LIVE_CONNECTOR_AUTH:-0}"
AUTH_TOKEN="${MANDOFORGE_WORKFLOW_PACK_GATE_TOKEN:-${MANDOFORGE_STAGE2_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-${MANDOFORGE_WORKER_TOKEN:-}}}}"

auth_headers=()
if [[ -n "$AUTH_TOKEN" ]]; then
  auth_headers+=(-H "authorization: Bearer $AUTH_TOKEN")
else
  auth_headers+=(
    -H "x-mandoforge-subject: $SUBJECT"
    -H "x-mandoforge-roles: $ROLES"
  )
fi

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "workflow pack evidence gate requires $1" >&2
    exit 1
  fi
}

slugify() {
  printf '%s' "$1" | sed -E 's#^/##; s#[/:]+#-#g; s#[^A-Za-z0-9._-]+#-#g'
}

manifest_top_value() {
  local key="$1"
  awk -v key="$key" '
    $1 == key ":" {
      sub("^[^:]+:[[:space:]]*", "")
      print
      exit
    }
  ' "$MANIFEST_PATH"
}

manifest_onboarding_required_profiles() {
  awk '
    /^onboarding:/ {
      in_onboarding = 1
      next
    }
    in_onboarding && /^[^[:space:]]/ {
      exit
    }
    in_onboarding && /^[[:space:]]{2}required_profiles:/ {
      in_profiles = 1
      next
    }
    in_profiles && /^[[:space:]]{2}[A-Za-z0-9_-]+:/ {
      exit
    }
    in_profiles && /^[[:space:]]*-[[:space:]]*/ {
      line = $0
      sub(/^[[:space:]]*-[[:space:]]*/, "", line)
      gsub(/[[:space:]]+$/, "", line)
      print line
    }
  ' "$MANIFEST_PATH"
}

manifest_profile_path() {
  local profile_id="$1"
  awk -v profile_id="$profile_id" '
    /^profiles:/ {
      in_profiles = 1
      next
    }
    in_profiles && /^[^[:space:]]/ {
      exit
    }
    in_profiles && /^[[:space:]]{2}- id:/ {
      current = $3
      next
    }
    in_profiles && current == profile_id && /^[[:space:]]{4}path:/ {
      print $2
      exit
    }
  ' "$MANIFEST_PATH"
}

manifest_first_connector_id() {
  awk '
    /^connectors:/ {
      in_connectors = 1
      next
    }
    in_connectors && /^[^[:space:]]/ {
      exit
    }
    in_connectors && /^[[:space:]]{2}- id:/ {
      print $3
      exit
    }
  ' "$MANIFEST_PATH"
}

manifest_connector_permissions() {
  local connector_id="$1"
  awk -v connector_id="$connector_id" '
    /^connectors:/ {
      in_connectors = 1
      next
    }
    in_connectors && /^[^[:space:]]/ {
      exit
    }
    in_connectors && /^[[:space:]]{2}- id:/ {
      current = $3
      in_permissions = 0
      next
    }
    in_connectors && current == connector_id && /^[[:space:]]{4}required_permissions:/ {
      in_permissions = 1
      next
    }
    in_permissions && /^[[:space:]]{4}[A-Za-z0-9_-]+:/ {
      exit
    }
    in_permissions && /^[[:space:]]*-[[:space:]]*/ {
      line = $0
      sub(/^[[:space:]]*-[[:space:]]*/, "", line)
      gsub(/[[:space:]]+$/, "", line)
      print line
    }
  ' "$MANIFEST_PATH"
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
      "${auth_headers[@]}" \
      "$BASE_URL$path")"
  else
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" -X "$method" \
      "${auth_headers[@]}" \
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

fetch_gateway_call() {
  local target="$EVIDENCE_DIR/mcp-gateway-call.json"
  local response_body
  local http_status
  response_body="$(mktemp)"
  http_status="$(curl -sS -o "$response_body" -w "%{http_code}" -X POST \
    -H "content-type: application/json" \
    -d "{\"server\":\"$CONNECTOR_SERVER_NAME\",\"tool\":\"$CONNECTOR_TOOL_NAME\",\"args\":{\"query\":\"$WORKFLOW_PACK_MCP_QUERY\"}}" \
    "$WORKFLOW_PACK_MCP_CALL_URL")"
  if [[ "$http_status" != 2* ]]; then
    echo "workflow pack gateway call failed: returned HTTP $http_status" >&2
    sed -n '1,80p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi
  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

require_cmd curl
require_cmd jq
require_cmd awk
mkdir -p "$EVIDENCE_DIR"

MANIFEST_DIR="$(dirname "$MANIFEST_PATH")"
PACK_ID="$(manifest_top_value id)"
PACK_NAME="$(manifest_top_value name)"
CONNECTOR_ID="$(manifest_first_connector_id)"
ONBOARDING_PROFILE_IDS=()
while IFS= read -r profile_id; do
  [[ -n "$profile_id" ]] && ONBOARDING_PROFILE_IDS+=("$profile_id")
done < <(manifest_onboarding_required_profiles)
CONNECTOR_REQUIRED_PERMISSIONS=()
while IFS= read -r permission; do
  [[ -n "$permission" ]] && CONNECTOR_REQUIRED_PERMISSIONS+=("$permission")
done < <(manifest_connector_permissions "$CONNECTOR_ID")
if [[ -z "$PACK_ID" || -z "$CONNECTOR_ID" || "${#ONBOARDING_PROFILE_IDS[@]}" -eq 0 || "${#CONNECTOR_REQUIRED_PERMISSIONS[@]}" -eq 0 ]]; then
  echo "workflow pack evidence gate could not derive pack/profile/connector contract from $MANIFEST_PATH" >&2
  exit 1
fi
ONBOARDING_PROFILE_COUNT="${#ONBOARDING_PROFILE_IDS[@]}"
CONNECTOR_COUNT="1"
CONNECTOR_REQUIRED_PERMISSIONS_JSON="$(printf '%s\n' "${CONNECTOR_REQUIRED_PERMISSIONS[@]}" | jq -R . | jq -sc '.')"
BLOCKED_PROFILE_ID="${WORKFLOW_PACK_BLOCKED_PROFILE_ID:-${ONBOARDING_PROFILE_IDS[0]}}"
BLOCKED_PROFILE_PATH="$(manifest_profile_path "$BLOCKED_PROFILE_ID")"
if [[ -z "$BLOCKED_PROFILE_PATH" || ! -f "$MANIFEST_DIR/$BLOCKED_PROFILE_PATH" ]]; then
  echo "workflow pack evidence gate could not read blocked onboarding profile $BLOCKED_PROFILE_ID" >&2
  exit 1
fi

curl -fsS "$BASE_URL/healthz" >/dev/null

discover_connector_binding() {
  if [[ -n "$CONNECTOR_TEAM_ID" && -n "$CONNECTOR_SERVER_ID" ]]; then
    return 0
  fi

  local organizations_file
  organizations_file="$(fetch_json GET /api/organizations)"
  local organization_id
  while IFS= read -r organization_id; do
    [[ -z "$organization_id" ]] && continue
    local teams_file
    teams_file="$(fetch_json GET "/api/organizations/$organization_id/teams")"
    local team_id
    while IFS= read -r team_id; do
      [[ -z "$team_id" ]] && continue
      local servers_file
      servers_file="$(fetch_json GET "/api/teams/$team_id/mcp-servers")"
      local server_id
      server_id="$(jq -r --arg name "$CONNECTOR_SERVER_NAME" '.response | map(select(.name == $name and .status == "active")) | .[0].id // empty' "$servers_file")"
      if [[ -z "$server_id" ]]; then
        server_id="$(jq -r '.response | map(select(.status == "active")) | .[0].id // empty' "$servers_file")"
      fi
      if [[ -n "$server_id" ]]; then
        CONNECTOR_TEAM_ID="$team_id"
        CONNECTOR_SERVER_ID="$server_id"
        return 0
      fi
    done < <(jq -r '.response | map(select((.archived_at // null) == null)) | .[].id' "$teams_file")
  done < <(jq -r '.response | map(select((.archived_at // null) == null)) | .[].id' "$organizations_file")
}

discover_connector_binding
if [[ "$REQUIRE_CONNECTOR_BINDING" == "1" && ( -z "$CONNECTOR_TEAM_ID" || -z "$CONNECTOR_SERVER_ID" ) ]]; then
  echo "workflow pack connector quality requires a discoverable MCP connector binding" >&2
  exit 1
fi
if [[ -z "$WORKFLOW_PACK_MCP_CALL_URL" ]]; then
  gateway_base="${MANDOFORGE_MCP_GATEWAY_URL:-}"
  if [[ -n "$gateway_base" ]]; then
    WORKFLOW_PACK_MCP_CALL_URL="${gateway_base/host.docker.internal/172.17.0.1}/v1/call"
  fi
fi
if [[ -n "$CONNECTOR_TEAM_ID" ]]; then
  fetch_json POST "/api/teams/$CONNECTOR_TEAM_ID/mcp-servers/health/run-due" >/dev/null
fi

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
update_manifest_payload="$(jq -nc --arg manifest_path "$UPDATE_MANIFEST_PATH" --arg pack_name "$PACK_NAME" '{manifest_path: $manifest_path, reason: ($pack_name + " version update proof")}')"
validate_file="$(fetch_json POST /api/workflow-packs/validate "$manifest_payload")"
install_file="$(fetch_json POST /api/workflow-packs/install "$manifest_payload")"
installation_id="$(jq -r '.response.id // empty' "$install_file")"
if [[ -z "$installation_id" ]]; then
  echo "workflow pack install evidence did not return an installation id" >&2
  exit 1
fi
installed_profiles_file="$(fetch_json GET "/api/workflow-packs/installations/$installation_id/onboarding/profiles")"

blocked_onboarding_payload="$(jq -nc \
  --arg profile_id "$BLOCKED_PROFILE_ID" \
  --arg profile_content "$(cat "$MANIFEST_DIR/$BLOCKED_PROFILE_PATH")" \
  --arg connector_id "$CONNECTOR_ID" \
  '{
    profiles: [
      {
        id: $profile_id,
        content: $profile_content
      }
    ],
    connectors: [
      {
        id: $connector_id,
        available_permissions: ["document.search"],
        provenance_attested: false,
        tenant_id: "",
        workspace_id: "workspace-demo",
        treats_results_as_data: false
      }
    ],
    reason: "WorkflowPack onboarding blocked proof"
  }')"
blocked_onboarding_file="$(fetch_json POST "/api/workflow-packs/installations/$installation_id/onboarding/assess" "$blocked_onboarding_payload")"

fetch_json POST "/api/workflow-packs/installations/$installation_id/release" \
  '{"eval_gate_status":"passed","release_gate_status":"passed","gate_evidence":{"expected_failure":"release_before_stage"}}' \
  4 >/dev/null

stage_file="$(fetch_json POST "/api/workflow-packs/installations/$installation_id/stage" \
  '{"reason":"WorkflowPack adoption evidence"}')"

fetch_json POST "/api/workflow-packs/installations/$installation_id/release" \
  '{"eval_gate_status":"pending","release_gate_status":"passed","gate_evidence":{"expected_failure":"eval_gate_not_passed"}}' \
  4 >/dev/null

release_file="$(fetch_json POST "/api/workflow-packs/installations/$installation_id/release" \
  "$(jq -nc --arg pack_id "$PACK_ID" '{"eval_gate_status":"passed","release_gate_status":"passed","gate_evidence":{"source":"workflow-pack-evidence-gate","eval_archive":($pack_id + "-regression"),"policy_gate":"approval-policy"},"reason":"WorkflowPack adoption release proof"}')")"
get_file="$(fetch_json GET "/api/workflow-packs/installations/$installation_id")"
list_file="$(fetch_json GET /api/workflow-packs/installations)"
released_get_file="$EVIDENCE_DIR/api-workflow-packs-installations-$installation_id-before-archive.json"
released_list_file="$EVIDENCE_DIR/api-workflow-packs-installations-before-archive.json"
cp "$get_file" "$released_get_file"
cp "$list_file" "$released_list_file"
rollback_file="$(fetch_json POST "/api/workflow-packs/installations/$installation_id/rollback" \
  "$(jq -nc --arg pack_id "$PACK_ID" '{"reason":"WorkflowPack adoption rollback proof","gate_evidence":{"source":"workflow-pack-evidence-gate","rollback_archive":($pack_id + "-rollback")}}')")"
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
updated_default_profiles_file="$(fetch_json GET "/api/workflow-packs/installations/$updated_installation_id/onboarding/profiles")"
old_after_update_snapshot_file="$EVIDENCE_DIR/api-workflow-packs-installations-$installation_id-after-update.json"
list_after_update_snapshot_file="$EVIDENCE_DIR/api-workflow-packs-installations-after-update.json"
cp "$old_after_update_file" "$old_after_update_snapshot_file"
cp "$list_after_update_file" "$list_after_update_snapshot_file"
persisted_profiles_payload="$(printf '%s\n' "${ONBOARDING_PROFILE_IDS[@]}" | jq -R . | jq -s --arg pack_name "$PACK_NAME" '{
  profiles: map({
    id: .,
    content: ("# " + . + "\nApproved tenant-specific " + . + " profile for " + $pack_name + ". Includes current owners, policy scope, source references, approval path, and review cadence.")
  }),
  reason: "WorkflowPack onboarding asset persistence proof"
}')"
persisted_profiles_file="$(fetch_json POST "/api/workflow-packs/installations/$updated_installation_id/onboarding/profiles" "$persisted_profiles_payload")"
persisted_profiles_list_file="$(fetch_json GET "/api/workflow-packs/installations/$updated_installation_id/onboarding/profiles")"
ready_onboarding_payload="$(jq -nc --arg connector_id "$CONNECTOR_ID" --argjson permissions "$CONNECTOR_REQUIRED_PERMISSIONS_JSON" '{
  connectors: [
    {
      id: $connector_id,
      available_permissions: $permissions,
      provenance_attested: true,
      tenant_id: "tenant-demo",
      workspace_id: "workspace-demo",
      treats_results_as_data: true
    }
  ],
  reason: "WorkflowPack onboarding readiness proof"
}')"
onboarding_file="$(fetch_json POST "/api/workflow-packs/installations/$updated_installation_id/onboarding/assess" "$ready_onboarding_payload")"
blocked_connector_quality_payload="$(jq -nc '{
  connectors: [
    {
      id: $connector_id,
      samples: [
        {
          object_id: "kb-stale-1",
          retrieved_at: "2026-05-01T00:00:00Z",
          metadata: {
            source_id: "page-stale-1"
          },
          content: {
            title: "Stale KB result"
          }
        }
      ]
    }
  ],
  reason: "WorkflowPack connector quality blocked proof"
} + (if $team_id == "" or $server_id == "" then {} else {
  connectors: [
    {
      id: $connector_id,
      team_id: $team_id,
      server_id: $server_id,
      tool_name: $tool_name,
      samples: [
        {
          object_id: "kb-stale-1",
          retrieved_at: "2026-05-01T00:00:00Z",
          metadata: {
            source_id: "page-stale-1"
          },
          content: {
            title: "Stale KB result"
          }
        }
      ]
    }
  ]
} end)' --arg connector_id "$CONNECTOR_ID" --arg team_id "$CONNECTOR_TEAM_ID" --arg server_id "$CONNECTOR_SERVER_ID" --arg tool_name "$CONNECTOR_TOOL_NAME")"
blocked_connector_quality_file="$(fetch_json POST "/api/workflow-packs/installations/$updated_installation_id/connectors/quality/assess" "$blocked_connector_quality_payload")"
blocked_connector_quality_snapshot_file="$EVIDENCE_DIR/api-workflow-packs-installations-$updated_installation_id-connectors-quality-assess-blocked.json"
cp "$blocked_connector_quality_file" "$blocked_connector_quality_snapshot_file"
gateway_call_file=""
gateway_live_title="Vendor AI policy"
gateway_live_url="https://kb.example/policy/vendor-ai"
gateway_live_snippet="Grounded source with retained provenance."
gateway_live_source_id="page-fresh-1"
gateway_live_reference="KB-2026-05"
gateway_live_retrieval_actor="connector-pilot"
gateway_live_retrieved_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
if [[ -n "$WORKFLOW_PACK_MCP_CALL_URL" ]]; then
  gateway_call_file="$(fetch_gateway_call)"
  gateway_live_title="$(jq -r '.result.items[0].title // "Vendor AI policy"' "$gateway_call_file")"
  gateway_live_url="$(jq -r '.result.items[0].url // "https://kb.example/policy/vendor-ai"' "$gateway_call_file")"
  gateway_live_snippet="$(jq -r '.result.items[0].snippet // "Grounded source with retained provenance."' "$gateway_call_file")"
  gateway_live_source_id="$(jq -r '.result.items[0].source_id // .result.items[0].url // .result.items[0].title // "page-fresh-1"' "$gateway_call_file")"
  gateway_live_reference="$(jq -r '.result.items[0].reference // .result.items[0].title // "KB-2026-05"' "$gateway_call_file")"
  gateway_live_retrieval_actor="$(jq -r '.result.items[0].retrieval_actor // .result.source // "connector-pilot"' "$gateway_call_file")"
elif [[ "$REQUIRE_CONNECTOR_BINDING" == "1" ]]; then
  echo "workflow pack connector quality requires a reachable MCP gateway call URL" >&2
  exit 1
fi
ready_connector_quality_payload="$(jq -nc '{
  connectors: [
    {
      id: $connector_id,
      samples: [
        {
          object_id: "kb-fresh-1",
          retrieved_at: $retrieved_at,
          citation_url: $citation_url,
          metadata: {
            source_id: $source_id,
            reference: $reference,
            retrieval_actor: $retrieval_actor
          },
          content: {
            title: $title,
            snippet: $snippet
          }
        }
      ]
    }
  ],
  reason: "WorkflowPack connector quality ready proof"
} + (if $team_id == "" or $server_id == "" then {} else {
  connectors: [
    {
      id: $connector_id,
      team_id: $team_id,
      server_id: $server_id,
      tool_name: $tool_name,
      samples: [
        {
          object_id: "kb-fresh-1",
          retrieved_at: $retrieved_at,
          citation_url: $citation_url,
          metadata: {
            source_id: $source_id,
            reference: $reference,
            retrieval_actor: $retrieval_actor
          },
          content: {
            title: $title,
            snippet: $snippet
          }
        }
      ]
    }
  ]
} end)' \
  --arg connector_id "$CONNECTOR_ID" \
  --arg team_id "$CONNECTOR_TEAM_ID" \
  --arg server_id "$CONNECTOR_SERVER_ID" \
  --arg tool_name "$CONNECTOR_TOOL_NAME" \
  --arg retrieved_at "$gateway_live_retrieved_at" \
  --arg citation_url "$gateway_live_url" \
  --arg source_id "$gateway_live_source_id" \
  --arg reference "$gateway_live_reference" \
  --arg retrieval_actor "$gateway_live_retrieval_actor" \
  --arg title "$gateway_live_title" \
  --arg snippet "$gateway_live_snippet")"
connector_quality_file="$(fetch_json POST "/api/workflow-packs/installations/$updated_installation_id/connectors/quality/assess" "$ready_connector_quality_payload")"
archive_file="$(fetch_json POST "/api/workflow-packs/installations/$installation_id/archive" \
  '{"reason":"WorkflowPack adoption archive proof"}')"
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
inline_profile_count="$(jq -r '.response.inline_profile_count // 0' "$onboarding_file")"
persisted_profile_count="$(jq -r '.response.persisted_profile_count // 0' "$onboarding_file")"
provided_profile_count="$(jq -r '.response.provided_profile_count // 0' "$onboarding_file")"
placeholder_profile_count="$(jq -r '.response.placeholder_profile_count // 0' "$onboarding_file")"
connector_requirement_count="$(jq -r '.response.connector_requirement_count // 0' "$onboarding_file")"
ready_connector_count="$(jq -r '.response.ready_connector_count // 0' "$onboarding_file")"
onboarding_blocker_count="$(jq -r '[.response.blockers[]?] | length' "$onboarding_file")"
persisted_profile_asset_count="$(jq -r '[.response[]?] | length' "$persisted_profiles_file")"
persisted_profile_list_count="$(jq -r '[.response[]?] | length' "$persisted_profiles_list_file")"
installed_default_profile_asset_count="$(jq -r '[.response[]?] | length' "$installed_profiles_file")"
updated_default_profile_asset_count="$(jq -r '[.response[]?] | length' "$updated_default_profiles_file")"
persisted_profile_saved_min_version="$(jq -r '[.response[]?.version] | min // 0' "$persisted_profiles_file")"
persisted_profile_saved_max_version="$(jq -r '[.response[]?.version] | max // 0' "$persisted_profiles_file")"
blocked_connector_quality_status="$(jq -r '.response.status // "unknown"' "$blocked_connector_quality_snapshot_file")"
connector_quality_status="$(jq -r '.response.status // "unknown"' "$connector_quality_file")"
connector_quality_requirement_count="$(jq -r '.response.connector_requirement_count // 0' "$connector_quality_file")"
connector_quality_ready_connector_count="$(jq -r '.response.ready_connector_count // 0' "$connector_quality_file")"
connector_quality_sample_count="$(jq -r '[.response.connector_results[]?.sample_count] | add // 0' "$connector_quality_file")"
connector_quality_passing_sample_count="$(jq -r '[.response.connector_results[]?.passing_sample_count] | add // 0' "$connector_quality_file")"
connector_quality_blocker_count="$(jq -r '[.response.blockers[]?] | length' "$connector_quality_file")"
connector_quality_bound_team_id="$(jq -r '.response.connector_results[0].bound_team_id // "none"' "$connector_quality_file")"
connector_quality_bound_server_id="$(jq -r '.response.connector_results[0].bound_server_id // "none"' "$connector_quality_file")"
connector_quality_bound_server_name="$(jq -r '.response.connector_results[0].bound_server_name // "none"' "$connector_quality_file")"
connector_quality_bound_server_health_status="$(jq -r '.response.connector_results[0].bound_server_health_status // "none"' "$connector_quality_file")"
connector_quality_live_source="none"
connector_quality_live_auth_mode="none"
connector_quality_live_title="none"
connector_quality_live_url="none"
if [[ -n "$gateway_call_file" && -s "$gateway_call_file" ]]; then
  connector_quality_live_source="$(jq -r '.result.source // "none"' "$gateway_call_file")"
  connector_quality_live_auth_mode="$(jq -r '.result.auth_mode // "none"' "$gateway_call_file")"
  connector_quality_live_title="$(jq -r '.result.items[0].title // "none"' "$gateway_call_file")"
  connector_quality_live_url="$(jq -r '.result.items[0].url // "none"' "$gateway_call_file")"
fi
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

if [[ "$validation_pack_id" != "$PACK_ID" ]]; then
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
if [[ "$installed_default_profile_asset_count" != "$ONBOARDING_PROFILE_COUNT" || "$updated_default_profile_asset_count" != "$ONBOARDING_PROFILE_COUNT" ]]; then
  echo "workflow pack install/update did not bootstrap default onboarding profile assets" >&2
  exit 1
fi
if [[ "$onboarding_status" != "ready" || "$onboarding_workflow" != "profile-onboarding" || "$onboarding_eval" != "profile-onboarding-regression" ]]; then
  echo "workflow pack onboarding readiness assessment did not reach ready state" >&2
  exit 1
fi
if [[ "$blocked_connector_quality_status" != "blocked" ]]; then
  echo "workflow pack blocked connector quality assessment did not fail closed" >&2
  exit 1
fi
if [[ "$WORKFLOW_PACK_REQUIRE_LIVE_CONNECTOR_AUTH" == "1" && "$connector_quality_live_auth_mode" != "authenticated" ]]; then
  echo "workflow pack live connector source is not authenticated" >&2
  exit 1
fi
if [[ "$REQUIRE_CONNECTOR_BINDING" == "1" && ( "$connector_quality_bound_team_id" == "none" || "$connector_quality_bound_server_id" == "none" ) ]]; then
  echo "workflow pack connector quality assessment did not bind to a real MCP server" >&2
  exit 1
fi
if [[ "$connector_quality_status" != "ready" || "$connector_quality_requirement_count" != "$CONNECTOR_COUNT" || "$connector_quality_ready_connector_count" != "$CONNECTOR_COUNT" || "$connector_quality_sample_count" != "1" || "$connector_quality_passing_sample_count" != "1" || "$connector_quality_blocker_count" != "0" ]]; then
  echo "workflow pack connector quality assessment did not reach ready state" >&2
  exit 1
fi
if [[ "$persisted_profile_asset_count" != "$ONBOARDING_PROFILE_COUNT" || "$persisted_profile_list_count" != "$ONBOARDING_PROFILE_COUNT" ]]; then
  echo "workflow pack persisted onboarding profile assets did not save cleanly" >&2
  exit 1
fi
if [[ "$persisted_profile_saved_min_version" != "2" || "$persisted_profile_saved_max_version" != "2" ]]; then
  echo "workflow pack persisted onboarding profile versions did not advance past bootstrapped defaults" >&2
  exit 1
fi
if [[ "$required_profile_count" != "$ONBOARDING_PROFILE_COUNT" || "$profile_schema_count" != "$ONBOARDING_PROFILE_COUNT" || "$inline_profile_count" != "0" || "$persisted_profile_count" != "$ONBOARDING_PROFILE_COUNT" || "$provided_profile_count" != "$ONBOARDING_PROFILE_COUNT" || "$placeholder_profile_count" != "0" ]]; then
  echo "workflow pack onboarding profile coverage did not match the contract" >&2
  exit 1
fi
if [[ "$connector_requirement_count" != "$CONNECTOR_COUNT" || "$ready_connector_count" != "$CONNECTOR_COUNT" || "$onboarding_blocker_count" != "0" ]]; then
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
  echo "inline_profile_count=$inline_profile_count"
  echo "persisted_profile_count=$persisted_profile_count"
  echo "provided_profile_count=$provided_profile_count"
  echo "placeholder_profile_count=$placeholder_profile_count"
  echo "connector_requirement_count=$connector_requirement_count"
  echo "ready_connector_count=$ready_connector_count"
  echo "onboarding_blocker_count=$onboarding_blocker_count"
  echo "blocked_connector_quality_status=$blocked_connector_quality_status"
  echo "connector_quality_status=$connector_quality_status"
  echo "connector_quality_requirement_count=$connector_quality_requirement_count"
  echo "connector_quality_ready_connector_count=$connector_quality_ready_connector_count"
  echo "connector_quality_sample_count=$connector_quality_sample_count"
  echo "connector_quality_passing_sample_count=$connector_quality_passing_sample_count"
  echo "connector_quality_blocker_count=$connector_quality_blocker_count"
  echo "connector_quality_bound_team_id=$connector_quality_bound_team_id"
  echo "connector_quality_bound_server_id=$connector_quality_bound_server_id"
  echo "connector_quality_bound_server_name=$connector_quality_bound_server_name"
  echo "connector_quality_bound_server_health_status=$connector_quality_bound_server_health_status"
  echo "connector_quality_live_source=$connector_quality_live_source"
  echo "connector_quality_live_auth_mode=$connector_quality_live_auth_mode"
  echo "connector_quality_live_title=$connector_quality_live_title"
  echo "connector_quality_live_url=$connector_quality_live_url"
  echo "installed_default_profile_asset_count=$installed_default_profile_asset_count"
  echo "updated_default_profile_asset_count=$updated_default_profile_asset_count"
  echo "persisted_profile_asset_count=$persisted_profile_asset_count"
  echo "persisted_profile_list_count=$persisted_profile_list_count"
  echo "persisted_profile_saved_min_version=$persisted_profile_saved_min_version"
  echo "persisted_profile_saved_max_version=$persisted_profile_saved_max_version"
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
