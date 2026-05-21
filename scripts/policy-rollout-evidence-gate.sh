#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-policy-rollout-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/policy-rollout-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_POLICY_DUE_RUN="${RUN_STAGE2_POLICY_DUE_RUN:-1}"
AUTH_TOKEN="${MANDOFORGE_STAGE2_GATE_TOKEN:-}"

auth_headers=(
)
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
    echo "policy rollout evidence gate requires $1" >&2
    exit 1
  fi
}

is_production_policy_controller_kind() {
  local value="$1"
  case "$value" in
    production_policy_controller|enterprise_policy_controller|external_policy_controller|policy_controller_cluster)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_production_environment() {
  local value="$1"
  case "$value" in
    production|prod)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_production_rollout_scope() {
  local value="$1"
  case "$value" in
    production|global|enterprise|multi_tenant)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_production_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  [[ -n "$value" ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

policy_due_run_scan_detail_count() {
  jq -r '[
    (
      .response.scanned_revisions[]?,
      .response.scanned_policies[]?,
      .response.policy_revisions[]?,
      .response.scanned_items[]?,
      .response.checked_revisions[]?
    )
    | select(
        type == "object"
        and ((.policy_id // .policy // .policy_key // .policy_name // "") | length > 0)
        and ((.revision_id // .revision // .policy_revision_id // .version // "") | length > 0)
        and ((.status // .result // .action // "") | ascii_downcase | IN("scanned", "checked", "skipped", "noop", "activated", "validated", "passed"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .scanned_at // .timestamp // "") | length > 0)
      )
  ] | length' "$1" 2>/dev/null || echo "0"
}

policy_rollout_step_detail_count() {
  jq -r '
    (.response.controller_execution.controller_id // "") as $root_controller_id
    | (.response.controller_execution.policy_store_id // "") as $root_policy_store_id
    | (.response.controller_execution.deployment_id // "") as $root_deployment_id
    | [
    .response.controller_execution.steps[]?
    | select(
        type == "object"
        and ($root_controller_id | length > 0)
        and ($root_policy_store_id | length > 0)
        and ($root_deployment_id | length > 0)
        and ((.controller_id // .policy_controller_id // "") == $root_controller_id)
        and ((.policy_store_id // .store_id // "") == $root_policy_store_id)
        and ((.deployment_id // .policy_deployment_id // "") == $root_deployment_id)
        and ((.name // .step // .kind // .action // "") | length > 0)
        and ((.status // .result // "") | ascii_downcase | IN("passed", "validated", "completed"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .executed_at // .timestamp // "") | length > 0)
      )
  ] | length' "$1" 2>/dev/null || echo "0"
}

slugify() {
  printf '%s' "$1" | sed -E 's#^/##; s#[/:]+#-#g; s#[^A-Za-z0-9._-]+#-#g'
}

fetch_json() {
  local method="$1"
  local path="$2"
  local label
  label="$(slugify "$path")"
  local target="$EVIDENCE_DIR/$label.json"
  local response_body
  local http_status
  response_body="$(mktemp)"

  if [[ "$method" == "GET" ]]; then
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" "${auth_headers[@]}" "$BASE_URL$path")"
  else
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" -X "$method" "${auth_headers[@]}" \
      -H "content-type: application/json" \
      -d '{}' \
      "$BASE_URL$path")"
  fi

  if [[ "$http_status" != 2* ]]; then
    echo "policy rollout evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local readiness_file="$EVIDENCE_DIR/api-policy-rollout-orchestration-readiness.json"
  local validation_evidence_file="$EVIDENCE_DIR/policy-rollout-orchestration-validation-evidence.json"
  local due_run_evidence_file="$EVIDENCE_DIR/policy-rollout-due-run-evidence.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local readiness_status
  local validation_status
  local validation_evidence_status
  local due_run_evidence_status
  local due_run_status
  local due_run_scanned_count
  local due_run_scan_detail_count
  local due_run_checked_at
  local production_blocked
  local rollout_active
  local active_revision_id
  local staged_revision_id
  local due_run_fresh
  local latest_due_run_status
  local controller_required
  local controller_configured
  local latest_controller_status
  local latest_controller_validated
  local controller_fresh
  local controller_age_hours
  local controller_production_target
  local controller_target_kind
  local controller_environment
  local controller_id
  local controller_rollout_scope
  local controller_production_policy_store
  local controller_rollback_supported
  local controller_rollback_evidence_id
  local controller_rollback_audit_evidence
  local controller_policy_store_id
  local controller_deployment_id
  local controller_step_count
  local controller_step_detail_count
  local blocked_count

  readiness_status="$(jq -r '.status // "unknown"' "$readiness_file")"
  validation_status="unknown"
  validation_evidence_status="missing"
  controller_target_kind="unknown"
  controller_environment="unknown"
  controller_id=""
  controller_rollout_scope="unknown"
  controller_production_policy_store="false"
  controller_rollback_supported="false"
  controller_rollback_evidence_id=""
  controller_rollback_audit_evidence=""
  controller_policy_store_id=""
  controller_deployment_id=""
  controller_step_count="0"
  controller_step_detail_count="0"
  if [[ -s "$validation_evidence_file" ]]; then
    validation_evidence_status="$(jq -r '.status // "unknown"' "$validation_evidence_file")"
    validation_status="$(jq -r '.response.status // "unknown"' "$validation_evidence_file")"
    controller_target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$validation_evidence_file")"
    controller_environment="$(jq -r '.response.controller_execution.environment // "unknown"' "$validation_evidence_file")"
    controller_id="$(jq -r '.response.controller_execution.controller_id // ""' "$validation_evidence_file")"
    controller_rollout_scope="$(jq -r '.response.controller_execution.rollout_scope // "unknown"' "$validation_evidence_file")"
    controller_production_policy_store="$(jq -r '.response.controller_execution.production_policy_store // false' "$validation_evidence_file")"
    controller_rollback_supported="$(jq -r '.response.controller_execution.rollback_supported // false' "$validation_evidence_file")"
    controller_rollback_evidence_id="$(jq -r '.response.controller_execution.rollback_plan_id // .response.controller_execution.rollback_procedure_id // .response.controller_execution.rollback_strategy_id // .response.controller_execution.rollback_revision_id // .response.controller_execution.rollback_run_id // ""' "$validation_evidence_file")"
    controller_rollback_audit_evidence="$(jq -r '.response.controller_execution.rollback_audit_id // .response.controller_execution.rollback_trace_id // .response.controller_execution.rollback_run_audit_id // .response.controller_execution.rollback_checked_at // .response.controller_execution.rollback_validated_at // ""' "$validation_evidence_file")"
    controller_policy_store_id="$(jq -r '.response.controller_execution.policy_store_id // ""' "$validation_evidence_file")"
    controller_deployment_id="$(jq -r '.response.controller_execution.deployment_id // ""' "$validation_evidence_file")"
    controller_step_count="$(jq -r 'if ((.response.controller_execution.steps // null) | type) == "array" then (.response.controller_execution.steps | length) else 0 end' "$validation_evidence_file")"
    controller_step_detail_count="$(policy_rollout_step_detail_count "$validation_evidence_file")"
  fi
  due_run_evidence_status="not_requested"
  due_run_status="not_run"
  due_run_scanned_count="0"
  due_run_scan_detail_count="0"
  due_run_checked_at=""
  if [[ -s "$due_run_evidence_file" ]]; then
    due_run_evidence_status="$(jq -r '.status // "unknown"' "$due_run_evidence_file")"
    due_run_status="$(jq -r '.response.status // "unknown"' "$due_run_evidence_file")"
    due_run_scanned_count="$(jq -r '.response.scanned_count // 0' "$due_run_evidence_file")"
    due_run_scan_detail_count="$(policy_due_run_scan_detail_count "$due_run_evidence_file")"
    due_run_checked_at="$(jq -r '.response.checked_at // ""' "$due_run_evidence_file")"
  fi
  production_blocked="$(jq -r 'if has("production_blocked") then .production_blocked else true end' "$readiness_file")"
  rollout_active="$(jq -r '.rollout_active // false' "$readiness_file")"
  active_revision_id="$(jq -r '.active_revision_id // "none"' "$readiness_file")"
  staged_revision_id="$(jq -r '.staged_revision_id // "none"' "$readiness_file")"
  due_run_fresh="$(jq -r '.due_run_fresh // false' "$readiness_file")"
  latest_due_run_status="$(jq -r '.latest_due_run_status // "none"' "$readiness_file")"
  controller_required="$(jq -r '.controller_required // false' "$readiness_file")"
  controller_configured="$(jq -r '.controller_configured // false' "$readiness_file")"
  latest_controller_status="$(jq -r '.latest_controller_status // "none"' "$readiness_file")"
  latest_controller_validated="$(jq -r '.latest_controller_validated // false' "$readiness_file")"
  controller_fresh="$(jq -r '.controller_evidence_fresh // false' "$readiness_file")"
  controller_age_hours="$(jq -r '.latest_controller_age_hours // "none"' "$readiness_file")"
  controller_production_target="$(jq -r '.latest_controller_production_target // false' "$readiness_file")"
  blocked_count="$(jq -r '[
      .production_blocked,
      (.controller_required != true),
      (.controller_configured != true),
      (.latest_controller_validated != true),
      (.latest_controller_production_target != true),
      (.controller_evidence_fresh != true),
      (.due_run_fresh != true),
      (.rollout_active == true),
      ((.active_revision_id // null) == null)
    ] | map(select(. == true)) | length' "$readiness_file")"
  if [[ "$validation_evidence_status" != "captured" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$validation_status" != "validated" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_policy_controller_kind "$controller_target_kind"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_environment "$controller_environment"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ -z "$controller_id" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_identity "$controller_id"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_rollout_scope "$controller_rollout_scope"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$controller_production_policy_store" != "true" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$controller_rollback_supported" != "true" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_identity "$controller_rollback_evidence_id"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ -z "$controller_rollback_audit_evidence" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_identity "$controller_policy_store_id"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_identity "$controller_deployment_id"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ ! "$controller_step_count" =~ ^[0-9]+$ || "$controller_step_count" == "0" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ ! "$controller_step_detail_count" =~ ^[0-9]+$ || ! "$controller_step_count" =~ ^[0-9]+$ || "$controller_step_detail_count" -lt "$controller_step_count" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$RUN_POLICY_DUE_RUN" != "1" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$due_run_evidence_status" != "captured" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$due_run_status" != "activated" && "$due_run_status" != "noop" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ ! "$due_run_scanned_count" =~ ^[0-9]+$ || "$due_run_scanned_count" == "0" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ ! "$due_run_scan_detail_count" =~ ^[0-9]+$ || ! "$due_run_scanned_count" =~ ^[0-9]+$ || "$due_run_scan_detail_count" -lt "$due_run_scanned_count" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ -z "$due_run_checked_at" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi

  {
    echo "policy_rollout_readiness_status=$readiness_status"
    echo "policy_rollout_validation_status=$validation_status"
    echo "policy_rollout_validation_evidence_status=$validation_evidence_status"
    echo "policy_rollout_due_run_evidence_status=$due_run_evidence_status"
    echo "policy_rollout_due_run_status=$due_run_status"
    echo "policy_rollout_due_run_scanned_count=$due_run_scanned_count"
    echo "policy_rollout_due_run_scan_detail_count=$due_run_scan_detail_count"
    echo "policy_rollout_due_run_checked_at=$due_run_checked_at"
    echo "production_blocked=$production_blocked"
    echo "production_blocked_count=$blocked_count"
    echo "rollout_active=$rollout_active"
    echo "active_revision_id=$active_revision_id"
    echo "staged_revision_id=$staged_revision_id"
    echo "due_run_fresh=$due_run_fresh"
    echo "latest_due_run_status=$latest_due_run_status"
    echo "controller_required=$controller_required"
    echo "controller_configured=$controller_configured"
    echo "latest_controller_status=$latest_controller_status"
    echo "latest_controller_validated=$latest_controller_validated"
    echo "controller_evidence_fresh=$controller_fresh"
    echo "controller_age_hours=$controller_age_hours"
    echo "controller_production_target=$controller_production_target"
    echo "controller_target_kind=$controller_target_kind"
    echo "controller_environment=$controller_environment"
    echo "controller_id=$controller_id"
    echo "controller_rollout_scope=$controller_rollout_scope"
    echo "controller_production_policy_store=$controller_production_policy_store"
    echo "controller_rollback_supported=$controller_rollback_supported"
    echo "controller_rollback_evidence_id=$controller_rollback_evidence_id"
    echo "controller_rollback_audit_evidence=$controller_rollback_audit_evidence"
    echo "controller_policy_store_id=$controller_policy_store_id"
    echo "controller_deployment_id=$controller_deployment_id"
    echo "controller_step_count=$controller_step_count"
    echo "controller_step_detail_count=$controller_step_detail_count"
    echo "policy_due_run=$RUN_POLICY_DUE_RUN"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo
    echo "blocking_reasons:"
    jq -r '.blocking_reasons[]? | "- \(.)"' "$readiness_file"
    if [[ "$controller_required" != "true" ]]; then
      echo "- policy rollout orchestration controller is not required by configuration"
    fi
    if [[ "$controller_configured" != "true" ]]; then
      echo "- policy rollout orchestration controller is not configured"
    fi
    if [[ "$latest_controller_validated" != "true" ]]; then
      echo "- policy rollout orchestration controller evidence is not validated"
    fi
    if [[ "$controller_fresh" != "true" ]]; then
      echo "- policy rollout orchestration controller evidence is not fresh"
    fi
    if [[ "$controller_production_target" != "true" ]]; then
      echo "- policy rollout readiness has not recorded a real production policy controller target"
    fi
    if ! is_production_policy_controller_kind "$controller_target_kind"; then
      echo "- policy rollout controller target kind is not production: $controller_target_kind"
    fi
    if ! is_production_environment "$controller_environment"; then
      echo "- policy rollout controller environment is not production: $controller_environment"
    fi
    if [[ -z "$controller_id" ]]; then
      echo "- policy rollout controller did not report a controller_id"
    fi
    if ! is_production_identity "$controller_id"; then
      echo "- policy rollout controller id is pilot/mock/local: ${controller_id:-<empty>}"
    fi
    if ! is_production_rollout_scope "$controller_rollout_scope"; then
      echo "- policy rollout controller scope is not production-grade: $controller_rollout_scope"
    fi
    if [[ "$controller_production_policy_store" != "true" ]]; then
      echo "- policy rollout controller did not confirm a production policy store"
    fi
    if [[ "$controller_rollback_supported" != "true" ]]; then
      echo "- policy rollout controller did not confirm rollback support"
    fi
    if ! is_production_identity "$controller_rollback_evidence_id"; then
      echo "- policy rollout controller rollback evidence id is pilot/mock/local: ${controller_rollback_evidence_id:-<empty>}"
    fi
    if [[ -z "$controller_rollback_audit_evidence" ]]; then
      echo "- policy rollout controller did not report rollback audit or trace evidence"
    fi
    if ! is_production_identity "$controller_policy_store_id"; then
      echo "- policy rollout controller policy_store_id is pilot/mock/local: ${controller_policy_store_id:-<empty>}"
    fi
    if ! is_production_identity "$controller_deployment_id"; then
      echo "- policy rollout controller deployment_id is pilot/mock/local: ${controller_deployment_id:-<empty>}"
    fi
    if [[ ! "$controller_step_count" =~ ^[0-9]+$ || "$controller_step_count" == "0" ]]; then
      echo "- policy rollout controller did not report any audited orchestration steps"
    fi
    if [[ ! "$controller_step_detail_count" =~ ^[0-9]+$ || ! "$controller_step_count" =~ ^[0-9]+$ || "$controller_step_detail_count" -lt "$controller_step_count" ]]; then
      echo "- policy rollout controller did not include audited detail bound to the controller/policy store/deployment for every orchestration step: detail_count=$controller_step_detail_count step_count=$controller_step_count"
    fi
    if [[ "$validation_status" != "validated" ]]; then
      echo "- policy rollout orchestration validation status is not validated: $validation_status"
    fi
    if [[ "$RUN_POLICY_DUE_RUN" != "1" ]]; then
      echo "- policy due-run evidence capture is disabled"
    fi
    if [[ "$due_run_evidence_status" != "captured" ]]; then
      echo "- policy due-run evidence was not captured"
    fi
    if [[ "$due_run_status" != "activated" && "$due_run_status" != "noop" ]]; then
      echo "- policy due-run status is not activated or noop: $due_run_status"
    fi
    if [[ ! "$due_run_scanned_count" =~ ^[0-9]+$ || "$due_run_scanned_count" == "0" ]]; then
      echo "- policy due-run did not scan any policy revisions: scanned_count=$due_run_scanned_count"
    fi
    if [[ ! "$due_run_scan_detail_count" =~ ^[0-9]+$ || ! "$due_run_scanned_count" =~ ^[0-9]+$ || "$due_run_scan_detail_count" -lt "$due_run_scanned_count" ]]; then
      echo "- policy due-run did not report audited scan details for every scanned revision: detail_count=$due_run_scan_detail_count scanned_count=$due_run_scanned_count"
    fi
    if [[ -z "$due_run_checked_at" ]]; then
      echo "- policy due-run did not report checked_at"
    fi
    echo
    echo "validation_issues:"
    if [[ -s "$validation_evidence_file" ]]; then
      jq -r '.response.issues[]? | "- \(.)"' "$validation_evidence_file"
    fi
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Policy rollout evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

capture_policy_due_run_evidence() {
  local due_run_response
  local due_run_evidence_file="$EVIDENCE_DIR/policy-rollout-due-run-evidence.json"

  due_run_response="$(fetch_json POST /api/policy/rollout/run-due)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$due_run_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$due_run_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$due_run_evidence_file"
}

capture_policy_orchestration_validation_evidence() {
  local validation_response
  local validation_evidence_file="$EVIDENCE_DIR/policy-rollout-orchestration-validation-evidence.json"

  validation_response="$(fetch_json POST /api/policy/rollout/orchestration/validate)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$validation_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$validation_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$validation_evidence_file"
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/policy/rollout/orchestration/readiness >/dev/null

if [[ "$RUN_POLICY_DUE_RUN" == "1" ]]; then
  capture_policy_due_run_evidence
else
  echo "skipping policy rollout due-run; set RUN_STAGE2_POLICY_DUE_RUN=1 to include due-run evidence" >&2
fi

capture_policy_orchestration_validation_evidence
fetch_json GET /api/policy/rollout/orchestration/readiness >/dev/null
write_summary
