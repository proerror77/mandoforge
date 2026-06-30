#!/usr/bin/env bash
set -euo pipefail

CONTRACT_PATH="${CONTRACT_PATH:-docs/enterprise-product-completion-contract.md}"
AUDIT_DIR="${AUDIT_DIR:-${EVIDENCE_DIR:-.mandoforge/enterprise-product-completion}}"
ENTERPRISE_EVIDENCE_DIR="${ENTERPRISE_PRODUCT_EVIDENCE_DIR:-${SOURCE_EVIDENCE_DIR:-${STAGE2_EVIDENCE_DIR:-.mandoforge/stage2-production-evidence}}}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
SUPPORT_OWNER="${MANDOFORGE_STAGE2_SUPPORT_OWNER:-}"
EVIDENCE_ARCHIVE_URI="${MANDOFORGE_STAGE2_EVIDENCE_ARCHIVE_URI:-}"
EVIDENCE_ARCHIVE_DIGEST="${MANDOFORGE_STAGE2_EVIDENCE_ARCHIVE_DIGEST:-}"
EVIDENCE_ARCHIVE_RETENTION_POLICY="${MANDOFORGE_STAGE2_EVIDENCE_RETENTION_POLICY:-}"

if ! command -v jq >/dev/null 2>&1; then
  echo "enterprise product completion contract gate requires jq" >&2
  exit 1
fi

required_lanes=(
  production-deployment-safety
  runtime-production
  remote-computer-multinode
  live-connector-production
  ontology-engine
  workflowpack-enterprise-lifecycle
  enterprise-security-admin
  observability-ops
  product-surfaces
)

required_files=(
  docs/architecture.md
  docs/runtime-truth-audit.md
  docs/ecommerce-platform-closed-loop.md
  docs/workflow-pack-manifest-contract.md
  docs/drafts/context-os-memory-architecture.md
  docs/agent-remote-computer-plan.md
  docs/whiskey-adoption-completion-audit.md
  scripts/production-launch-preflight.sh
  scripts/enterprise-product-completion-contract-gate.sh
  scripts/production-deployment-safety-gate.sh
  scripts/agent-os-core-evidence-gate.sh
  scripts/runtime-production-readiness-gate.sh
  scripts/managed-session-runtime-evidence-gate.sh
  scripts/managed-workflow-runtime-evidence-gate.sh
  scripts/worker-evidence-gate.sh
  scripts/worker-remote-computer-evidence-gate.sh
  scripts/remote-computer-evidence-gate.sh
  scripts/remote-computer-production-state-gate.sh
  scripts/whiskey-remote-computer-k3s-verify.sh
  scripts/enterprise-security-admin-readiness-gate.sh
  scripts/enterprise-security-production-controls-gate.sh
  scripts/approval-notification-evidence-gate.sh
  scripts/native-connector-production-readiness-gate.sh
  scripts/live-connector-production-semantics-gate.sh
  scripts/verify-ecommerce-platform-closed-loop.sh
  scripts/ontology-engine-readiness-gate.sh
  scripts/ontology-engine-production-gate.sh
  scripts/ontology-release-workflow-trigger-gate.sh
  scripts/verify-ecommerce-tmall-context-os.sh
  scripts/verify-workflow-pack-manifest.sh
  scripts/workflow-pack-evidence-gate.sh
  scripts/workflowpack-enterprise-lifecycle-gate.sh
  scripts/provider-governance-evidence-gate.sh
  scripts/tenant-isolation-evidence-gate.sh
  scripts/vault-evidence-gate.sh
  scripts/observability-collector-evidence-gate.sh
  scripts/observability-ops-production-gate.sh
  scripts/finance-evidence-gate.sh
  scripts/product-surfaces-production-gate.sh
  scripts/stage2-production-evidence-preflight.sh
  scripts/stage2-production-evidence-gate.sh
  scripts/verify-stage2-evidence-archive.sh
  scripts/verify-stage2-evidence-k8s-manifests.sh
  scripts/enterprise-product-readiness-gate.sh
  scripts/verify-static-ui-assets.sh
  scripts/verify-static-ui-actionbook.sh
  scripts/verify-ui-api-truth-gate.mjs
)

mkdir -p "$AUDIT_DIR"
lane_results_jsonl="$AUDIT_DIR/lane-results.jsonl"
: >"$lane_results_jsonl"

summary_ready_file() {
  local path="$1"
  local expected_source="$2"
  [[ -s "$path" ]] || return 1
  jq -e --arg expected_source "$expected_source" '
    (.status // "") as $status
    | (.source // "") == $expected_source
      and ((.required_evidence_class // "") == "customer_grade")
      and ($status == "ready" or $status == "validated" or $status == "completed" or $status == "passed")
      and ((.blocked_count // 0) == 0)
  ' "$path" >/dev/null
}

json_string() {
  jq -n --arg value "$1" '$value'
}

looks_production_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  [[ -n "$value" ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

looks_production_archive_uri() {
  local value="$1"
  [[ "$value" =~ ^(s3|gs|az|https):// ]] || return 1
  [[ ! "$value" =~ example\.com ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost|sandbox-only)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

looks_evidence_digest() {
  local value="$1"
  [[ "$value" =~ ^(sha256:)?[A-Fa-f0-9]{64}$ ]] || return 1
}

archive_metadata_ready() {
  looks_production_identity "$SUPPORT_OWNER" &&
    looks_production_archive_uri "$EVIDENCE_ARCHIVE_URI" &&
    looks_evidence_digest "$EVIDENCE_ARCHIVE_DIGEST" &&
    [[ -n "$EVIDENCE_ARCHIVE_RETENTION_POLICY" ]]
}

gate_passed() {
  local lane="$1"
  local expected_source="$2"
  shift 2
  local gate_dir="$AUDIT_DIR/lane-gates/$lane"
  local gate_output
  local issue
  mkdir -p "$gate_dir"
  if ! gate_output="$("$@" 2>&1)"; then
    printf '%s\n' "$gate_output" >"$gate_dir/output.log"
    issue="$(printf '%s' "$gate_output" | tr '\n' ' ' | cut -c1-500)"
    write_lane_result "$lane" "$expected_source" "$gate_dir" "blocked" "$issue"
    return 1
  fi
  printf '%s\n' "$gate_output" >"$gate_dir/output.log"
  if summary_ready_file "$gate_dir/summary.json" "$expected_source"; then
    write_lane_result "$lane" "$expected_source" "$gate_dir" "ready" ""
    return 0
  fi
  write_lane_result "$lane" "$expected_source" "$gate_dir" "blocked" "gate passed but summary.json did not match source, evidence class, status, or blocked_count contract"
  return 1
}

relative_evidence_path() {
  local path="$1"
  case "$path" in
    "$ENTERPRISE_EVIDENCE_DIR"/*)
      printf '%s' "${path#"$ENTERPRISE_EVIDENCE_DIR"/}"
      ;;
    *)
      printf '%s' "$path"
      ;;
  esac
}

ontology_trigger_summary_ready() {
  local path="$ENTERPRISE_EVIDENCE_DIR/ontology-release-workflow-trigger/summary.json"
  summary_ready_file "$path" "ontology-release-workflow-trigger-gate" || return 1
  jq -e '
    ((.target.environment // "") == "production")
    and ((.target.id // .target.deployment_id // .target.cluster_id // "") | length > 0)
    and ((.target.kind // "") | length > 0)
    and ((.support_owner // .workflow_owner // .oncall_owner // "") | length > 0)
    and ((.evidence_archive.immutable // .archive.immutable // false) == true)
    and ((.evidence_archive.uri // .archive.uri // "") | length > 0)
    and ((.evidence_archive.digest // .archive.digest // "") | length > 0)
    and ((.evidence_archive.retention_policy // .archive.retention_policy // "") | length > 0)
    and ((.domain_scope // "") | length > 0)
    and ((.workflow_definition_id // "") | length > 0)
    and ((.workflow_run_id // "") | length > 0)
    and ((.ontology_release_id // "") | length > 0)
    and (.checks.release_promoted == true)
    and (.checks.workflow_trigger_reported == true)
    and (.checks.workflow_run_queued == true)
    and (.checks.audit_log_recorded == true)
    and (.checks.scheduler_drain_exposed == true)
    and (.checks.readiness_reflected == true)
  ' "$path" >/dev/null
}

write_lane_result() {
  local lane="$1"
  local expected_source="$2"
  local gate_dir="$3"
  local result_status="$4"
  local issue="$5"
  jq -n \
    --arg lane "$lane" \
    --arg expected_source "$expected_source" \
    --arg status "$result_status" \
    --arg summary_path "$(relative_evidence_path "$gate_dir/summary.json")" \
    --arg output_log "$(relative_evidence_path "$gate_dir/output.log")" \
    --arg issue "$issue" \
    '{
      lane: $lane,
      expected_source: $expected_source,
      status: $status,
      summary_path: $summary_path,
      output_log: $output_log,
      issue: (if $issue == "" then null else $issue end)
    }' >>"$lane_results_jsonl"
}

write_summary_lane_result() {
  local lane="$1"
  local expected_source="$2"
  local summary_path="$3"
  local result_status="$4"
  local issue="$5"
  jq -n \
    --arg lane "$lane" \
    --arg expected_source "$expected_source" \
    --arg status "$result_status" \
    --arg summary_path "$summary_path" \
    --arg issue "$issue" \
    '{
      lane: $lane,
      expected_source: $expected_source,
      status: $status,
      summary_path: $summary_path,
      output_log: null,
      issue: (if $issue == "" then null else $issue end)
    }' >>"$lane_results_jsonl"
}

lane_ready() {
  case "$1" in
    production-deployment-safety)
      gate_passed "$1" "production-deployment-safety-gate" \
        env EVIDENCE_DIR="$AUDIT_DIR/lane-gates/$1" \
        SOURCE_EVIDENCE_DIR="$ENTERPRISE_EVIDENCE_DIR" \
        PRODUCTION_DEPLOYMENT_SAFETY_EVIDENCE_FILE="$ENTERPRISE_EVIDENCE_DIR/production-deployment-safety/summary.json" \
        ALLOW_BLOCKED=0 \
        scripts/production-deployment-safety-gate.sh
      ;;
    runtime-production)
      gate_passed "$1" "runtime-production-readiness-gate" \
        env EVIDENCE_DIR="$AUDIT_DIR/lane-gates/$1" \
        SOURCE_EVIDENCE_DIR="$ENTERPRISE_EVIDENCE_DIR" \
        RUNTIME_PRODUCTION_RECOVERY_EVIDENCE_FILE="$ENTERPRISE_EVIDENCE_DIR/runtime-production-recovery-evidence.json" \
        ALLOW_BLOCKED=0 \
        scripts/runtime-production-readiness-gate.sh
      ;;
    remote-computer-multinode)
      gate_passed "$1" "remote-computer-production-state-gate" \
        env EVIDENCE_DIR="$AUDIT_DIR/lane-gates/$1" \
        SOURCE_EVIDENCE_DIR="$ENTERPRISE_EVIDENCE_DIR" \
        REMOTE_COMPUTER_EVIDENCE_DIR="$ENTERPRISE_EVIDENCE_DIR/remote-computer" \
        WORKER_REMOTE_COMPUTER_EVIDENCE_DIR="$ENTERPRISE_EVIDENCE_DIR/worker-remote-computer" \
        REMOTE_COMPUTER_SESSION_POD_LIFECYCLE_EVIDENCE_FILE="$ENTERPRISE_EVIDENCE_DIR/remote-computer-session-pod-lifecycle-evidence.json" \
        ALLOW_BLOCKED=0 \
        scripts/remote-computer-production-state-gate.sh
      ;;
    live-connector-production)
      gate_passed "$1" "live-connector-production-semantics-gate" \
        env EVIDENCE_DIR="$AUDIT_DIR/lane-gates/$1" \
        SOURCE_EVIDENCE_DIR="$ENTERPRISE_EVIDENCE_DIR/live-connector-production-semantics" \
        ALLOW_BLOCKED=0 \
        scripts/live-connector-production-semantics-gate.sh
      ;;
    ontology-engine)
      ontology_trigger_summary_path="$ENTERPRISE_EVIDENCE_DIR/ontology-release-workflow-trigger/summary.json"
      gate_passed "$1" "ontology-engine-production-gate" \
        env EVIDENCE_DIR="$AUDIT_DIR/lane-gates/$1" \
        SOURCE_EVIDENCE_DIR="$ENTERPRISE_EVIDENCE_DIR" \
        ONTOLOGY_ENGINE_PRODUCTION_EVIDENCE_FILE="$ENTERPRISE_EVIDENCE_DIR/ontology-engine-production/summary.json" \
        ALLOW_BLOCKED=0 \
        scripts/ontology-engine-production-gate.sh \
        || return 1
      if ontology_trigger_summary_ready; then
        write_summary_lane_result "ontology-release-workflow-trigger" "ontology-release-workflow-trigger-gate" "$ontology_trigger_summary_path" "ready" ""
        return 0
      fi
      write_summary_lane_result "ontology-release-workflow-trigger" "ontology-release-workflow-trigger-gate" "$ontology_trigger_summary_path" "blocked" "ontology trigger summary is missing or incomplete"
      return 1
      ;;
    workflowpack-enterprise-lifecycle)
      gate_passed "$1" "workflowpack-enterprise-lifecycle-gate" \
        env EVIDENCE_DIR="$AUDIT_DIR/lane-gates/$1" \
        SOURCE_EVIDENCE_DIR="$ENTERPRISE_EVIDENCE_DIR" \
        WORKFLOWPACK_ENTERPRISE_LIFECYCLE_EVIDENCE_FILE="$ENTERPRISE_EVIDENCE_DIR/workflowpack-enterprise-lifecycle/summary.json" \
        ALLOW_BLOCKED=0 \
        scripts/workflowpack-enterprise-lifecycle-gate.sh
      ;;
    enterprise-security-admin)
      gate_passed "$1" "enterprise-security-production-controls-gate" \
        env EVIDENCE_DIR="$AUDIT_DIR/lane-gates/$1" \
        SOURCE_EVIDENCE_DIR="$ENTERPRISE_EVIDENCE_DIR/enterprise-security-production-controls" \
        ENTERPRISE_SECURITY_CONTROLS_EVIDENCE_FILE="$ENTERPRISE_EVIDENCE_DIR/enterprise-security-production-controls/summary.json" \
        ALLOW_BLOCKED=0 \
        scripts/enterprise-security-production-controls-gate.sh
      ;;
    observability-ops)
      gate_passed "$1" "observability-ops-production-gate" \
        env EVIDENCE_DIR="$AUDIT_DIR/lane-gates/$1" \
        SOURCE_EVIDENCE_DIR="$ENTERPRISE_EVIDENCE_DIR/observability-ops-production" \
        OBSERVABILITY_OPS_EVIDENCE_FILE="$ENTERPRISE_EVIDENCE_DIR/observability-ops-production/summary.json" \
        ALLOW_BLOCKED=0 \
        scripts/observability-ops-production-gate.sh
      ;;
    product-surfaces)
      gate_passed "$1" "product-surfaces-production-gate" \
        env EVIDENCE_DIR="$AUDIT_DIR/lane-gates/$1" \
        SOURCE_EVIDENCE_DIR="$ENTERPRISE_EVIDENCE_DIR" \
        PRODUCT_SURFACES_EVIDENCE_FILE="$ENTERPRISE_EVIDENCE_DIR/product-surfaces/summary.json" \
        ALLOW_BLOCKED=0 \
        scripts/product-surfaces-production-gate.sh
      ;;
    *)
      return 1
      ;;
  esac
}

missing=()
if [[ ! -s "$CONTRACT_PATH" ]]; then
  missing+=("$CONTRACT_PATH")
fi

for lane in "${required_lanes[@]}"; do
  if [[ -s "$CONTRACT_PATH" ]] && ! grep -q "### $lane" "$CONTRACT_PATH"; then
    missing+=("$CONTRACT_PATH:$lane")
  fi
done

for path in "${required_files[@]}"; do
  if [[ ! -s "$path" ]]; then
    missing+=("$path")
  fi
done

blocked_lanes=()
ready_lanes=()
for lane in "${required_lanes[@]}"; do
  if lane_ready "$lane"; then
    ready_lanes+=("$lane")
  else
    blocked_lanes+=("$lane")
  fi
done

status="enterprise_product_complete"
if [[ "${#missing[@]}" -gt 0 ]]; then
  status="invalid_contract"
elif [[ "${#blocked_lanes[@]}" -gt 0 ]] || ! archive_metadata_ready; then
  status="blocked"
fi

lane_results_json="$AUDIT_DIR/lane-results.json"
jq -s '.' "$lane_results_jsonl" >"$lane_results_json"

checklist_json="$AUDIT_DIR/checklist.json"
{
  printf '{\n'
  printf '  "source": "enterprise-product-completion-contract-gate",\n'
  printf '  "contract_path": "%s",\n' "$CONTRACT_PATH"
  printf '  "evidence_dir": "%s",\n' "$ENTERPRISE_EVIDENCE_DIR"
  printf '  "enterprise_product_status": "%s",\n' "$status"
  printf '  "completion_blocked": %s,\n' "$(if [[ "$status" == "blocked" || "$status" == "invalid_contract" ]]; then echo true; else echo false; fi)"
  printf '  "required_evidence_class": "customer_grade",\n'
  printf '  "support_owner": %s,\n' "$(json_string "$SUPPORT_OWNER")"
  printf '  "archive_metadata_ready": %s,\n' "$(if archive_metadata_ready; then echo true; else echo false; fi)"
  printf '  "evidence_archive": {\n'
  printf '    "uri": %s,\n' "$(json_string "$EVIDENCE_ARCHIVE_URI")"
  printf '    "digest": %s,\n' "$(json_string "$EVIDENCE_ARCHIVE_DIGEST")"
  printf '    "retention_policy": %s,\n' "$(json_string "$EVIDENCE_ARCHIVE_RETENTION_POLICY")"
  printf '    "immutable": %s\n' "$(if archive_metadata_ready; then echo true; else echo false; fi)"
  printf '  },\n'
  printf '  "required_lanes": [\n'
  for i in "${!required_lanes[@]}"; do
    comma=","
    if [[ "$i" -eq $((${#required_lanes[@]} - 1)) ]]; then
      comma=""
    fi
    printf '    "%s"%s\n' "${required_lanes[$i]}" "$comma"
  done
  printf '  ],\n'
  printf '  "ready_lanes": [\n'
  for i in "${!ready_lanes[@]}"; do
    comma=","
    if [[ "$i" -eq $((${#ready_lanes[@]} - 1)) ]]; then
      comma=""
    fi
    printf '    "%s"%s\n' "${ready_lanes[$i]}" "$comma"
  done
  printf '  ],\n'
  printf '  "blocked_lanes": [\n'
  for i in "${!blocked_lanes[@]}"; do
    comma=","
    if [[ "$i" -eq $((${#blocked_lanes[@]} - 1)) ]]; then
      comma=""
    fi
    printf '    "%s"%s\n' "${blocked_lanes[$i]}" "$comma"
  done
  printf '  ],\n'
  printf '  "lane_results": '
  cat "$lane_results_json"
  printf ',\n'
  printf '  "missing_contract_items": [\n'
  for i in "${!missing[@]}"; do
    comma=","
    if [[ "$i" -eq $((${#missing[@]} - 1)) ]]; then
      comma=""
    fi
    printf '    "%s"%s\n' "${missing[$i]}" "$comma"
  done
  printf '  ]\n'
  printf '}\n'
} >"$checklist_json"

readiness_checklist_dir="$ENTERPRISE_EVIDENCE_DIR/enterprise-product-completion-contract-gate"
mkdir -p "$readiness_checklist_dir"
if [[ "$checklist_json" != "$readiness_checklist_dir/checklist.json" ]]; then
  cp "$checklist_json" "$readiness_checklist_dir/checklist.json"
fi
if [[ "$lane_results_json" != "$readiness_checklist_dir/lane-results.json" ]]; then
  cp "$lane_results_json" "$readiness_checklist_dir/lane-results.json"
fi
if [[ "$lane_results_jsonl" != "$readiness_checklist_dir/lane-results.jsonl" ]]; then
  cp "$lane_results_jsonl" "$readiness_checklist_dir/lane-results.jsonl"
fi

summary_file="$AUDIT_DIR/summary.txt"
{
  echo "enterprise_product_status=$status"
  echo "completion_blocked=$(if [[ "$status" == "blocked" || "$status" == "invalid_contract" ]]; then echo true; else echo false; fi)"
  echo "required_evidence_class=customer_grade"
  echo "archive_metadata_ready=$(if archive_metadata_ready; then echo true; else echo false; fi)"
  echo "support_owner=$SUPPORT_OWNER"
  echo "evidence_archive_uri=$EVIDENCE_ARCHIVE_URI"
  echo "evidence_archive_digest=$EVIDENCE_ARCHIVE_DIGEST"
  echo "evidence_archive_retention_policy=$EVIDENCE_ARCHIVE_RETENTION_POLICY"
  echo "evidence_dir=$ENTERPRISE_EVIDENCE_DIR"
  echo "required_lane_count=${#required_lanes[@]}"
  echo "ready_lane_count=${#ready_lanes[@]}"
  echo "blocked_lane_count=${#blocked_lanes[@]}"
  echo "missing_contract_item_count=${#missing[@]}"
  echo "lane_results_jsonl=$lane_results_jsonl"
  echo "lane_results_json=$lane_results_json"
  echo "checklist_json=$checklist_json"
  echo "readiness_checklist_json=$readiness_checklist_dir/checklist.json"
} >"$summary_file"

cat "$summary_file"

if [[ "$status" == "invalid_contract" ]]; then
  echo "enterprise product completion contract is missing required items" >&2
  exit 1
fi

if [[ "$status" == "blocked" && "$ALLOW_BLOCKED" != "1" ]]; then
  echo "Enterprise product completion gate failed closed; set ALLOW_BLOCKED=1 for inventory runs." >&2
  exit 1
fi

echo "enterprise product completion inventory ok"
