#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${WHISKEY_REMOTE_HOST:-wishky-2-1}"
REMOTE_ROOT="${WHISKEY_REMOTE_ROOT:-/opt/mandoforge-adoption}"
COMPOSE_PROJECT="${WHISKEY_COMPOSE_PROJECT:-mandoforge-adoption}"
REMOTE_COMPOSE="$REMOTE_ROOT/docker-compose.yml"
REMOTE_ENV="$REMOTE_ROOT/whiskey.env"
LOCAL_SYNC_DIR="${WHISKEY_LOCAL_SYNC_DIR:-.mandoforge/remote-adoption/whiskey}"
RUN_STRICT_VALIDATIONS="${RUN_STAGE2_PRODUCTION_VALIDATIONS:-0}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "whiskey adoption evidence requires $1" >&2
    exit 1
  fi
}

require_cmd ssh
require_cmd rsync

sha256_value() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
  else
    echo "whiskey adoption evidence requires sha256sum or shasum" >&2
    exit 1
  fi
}

seed_eval_release_evidence() {
  local evidence_dir="$1"
  local reason="$2"
  local remote_script

  remote_script="$(cat <<REMOTE
set -euo pipefail
cd '$REMOTE_ROOT'
set -a
source '$REMOTE_ENV'
set +a

base_url="http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787}"
evidence_dir='$evidence_dir'
reason='$reason'
mkdir -p "\$evidence_dir"
curl -fsS "\$base_url/healthz" >/dev/null

auth_headers=(
  -H "x-mandoforge-subject: whiskey-adoption-admin"
  -H "x-mandoforge-roles: admin"
)

bootstrap_file="\$(mktemp)"
agents_file="\$(mktemp)"
agent_file="\$(mktemp)"
run_file="\$(mktemp)"
release_file="\$(mktemp)"
release_body="\$(mktemp)"

cleanup() {
  rm -f "\$bootstrap_file" "\$agents_file" "\$agent_file" "\$run_file" "\$release_file" "\$release_body"
}
trap cleanup EXIT

curl -fsS "\${auth_headers[@]}" \
  -H "content-type: application/json" \
  -d '{"name":"Whiskey Eval Release Regression"}' \
  "\$base_url/api/eval/suites/stage2-regression" >"\$bootstrap_file"
dataset_id="\$(jq -r '.dataset.id' "\$bootstrap_file")"
if [[ -z "\$dataset_id" || "\$dataset_id" == "null" ]]; then
  echo "Whiskey eval/release seed could not determine dataset id" >&2
  cat "\$bootstrap_file" >&2
  exit 1
fi

curl -fsS "\${auth_headers[@]}" "\$base_url/api/agents" >"\$agents_file"
agent_id="\$(jq -r '.[0].id // empty' "\$agents_file")"
if [[ -z "\$agent_id" ]]; then
  curl -fsS -X POST "\${auth_headers[@]}" \
    -H "content-type: application/json" \
    -d '{"name":"Whiskey Eval Release Pilot","kind":"assistant","provider":"openai","model":"gpt-5.4-mini","system_prompt":"Whiskey eval release adoption pilot","tools":[]}' \
    "\$base_url/api/agents" >"\$agent_file"
  agent_id="\$(jq -r '.id' "\$agent_file")"
fi
if [[ -z "\$agent_id" || "\$agent_id" == "null" ]]; then
  echo "Whiskey eval/release seed could not determine agent id" >&2
  cat "\$agents_file" >&2
  cat "\$agent_file" >&2 || true
  exit 1
fi

curl -fsS -X POST "\${auth_headers[@]}" \
  -H "content-type: application/json" \
  -d "{\\"agent_id\\":\\"\$agent_id\\"}" \
  "\$base_url/api/eval/datasets/\$dataset_id/runs" >"\$run_file"
eval_run_id="\$(jq -r '.id' "\$run_file")"
if [[ -z "\$eval_run_id" || "\$eval_run_id" == "null" ]]; then
  echo "Whiskey eval/release seed could not determine eval run id" >&2
  cat "\$run_file" >&2
  exit 1
fi

activate_after="\$(date -u -d '1 minute ago' +%Y-%m-%dT%H:%M:%SZ)"
expires_at="\$(date -u -d '10 minutes' +%Y-%m-%dT%H:%M:%SZ)"
jq -n \
  --arg eval_run_id "\$eval_run_id" \
  --arg environment "whiskey-eval-release" \
  --arg approver_subject "system" \
  --arg activate_after "\$activate_after" \
  --arg expires_at "\$expires_at" \
  --arg reason "\$reason" \
  '{
    eval_run_id: \$eval_run_id,
    environment: \$environment,
    min_score: 1.0,
    approver_subject: \$approver_subject,
    auto_approve: true,
    activate_after: \$activate_after,
    expires_at: \$expires_at,
    reason: \$reason
  }' >"\$release_body"

curl -fsS -X POST "\${auth_headers[@]}" \
  -H "content-type: application/json" \
  -d @"\$release_body" \
  "\$base_url/api/agents/\$agent_id/release-requests" >"\$release_file"

jq -n \
  --arg status "seeded" \
  --arg agent_id "\$agent_id" \
  --arg dataset_id "\$dataset_id" \
  --arg eval_run_id "\$eval_run_id" \
  --arg generated_at "\$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --slurpfile bootstrap "\$bootstrap_file" \
  --slurpfile agents "\$agents_file" \
  --slurpfile created_agent "\$agent_file" \
  --slurpfile run "\$run_file" \
  --slurpfile release "\$release_file" \
  '{
    status: \$status,
    agent_id: \$agent_id,
    dataset_id: \$dataset_id,
    eval_run_id: \$eval_run_id,
    generated_at: \$generated_at,
    bootstrap: (\$bootstrap[0] // {}),
    agents: (\$agents[0] // []),
    created_agent: (\$created_agent[0] // null),
    run: (\$run[0] // {}),
    release: (\$release[0] // {})
  }' >"\$evidence_dir/whiskey-eval-release-seed.json"
REMOTE
)"

  ssh "$REMOTE_HOST" "bash -lc $(printf '%q' "$remote_script")"
}

seed_observability_remediation_evidence() {
  local evidence_dir="$1"
  local reason="$2"
  local remote_script

  remote_script="$(cat <<REMOTE
set -euo pipefail
cd '$REMOTE_ROOT'
set -a
source '$REMOTE_ENV'
set +a

base_url="http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787}"
evidence_dir='$evidence_dir'
reason='$reason'
mkdir -p "\$evidence_dir"
curl -fsS "\$base_url/healthz" >/dev/null

auth_headers=(
  -H "x-mandoforge-subject: whiskey-adoption-admin"
  -H "x-mandoforge-roles: admin"
)

agents_file="\$(mktemp)"
session_file="\$(mktemp)"
run_file="\$(mktemp)"
session_body="\$(mktemp)"

cleanup() {
  rm -f "\$agents_file" "\$session_file" "\$run_file" "\$session_body"
}
trap cleanup EXIT

curl -fsS "\${auth_headers[@]}" "\$base_url/api/agents" >"\$agents_file"
agent_id="\$(jq -r 'map(select(.name == "Generic Orchestrator Agent")) | .[0].id // .[0].id // empty' "\$agents_file")"
if [[ -z "\$agent_id" || "\$agent_id" == "null" ]]; then
  echo "Whiskey observability seed could not determine agent id" >&2
  cat "\$agents_file" >&2
  exit 1
fi

jq -n \
  --arg agent_id "\$agent_id" \
  --arg title "Whiskey observability remediation seed" \
  --arg message "Run the diagnostics flow until shell approval is requested for Whiskey observability remediation evidence." \
  '{
    agent_id: \$agent_id,
    title: \$title,
    message: \$message
  }' >"\$session_body"

curl -fsS -X POST "\${auth_headers[@]}" \
  -H "content-type: application/json" \
  -d @"\$session_body" \
  "\$base_url/api/sessions" >"\$session_file"
session_id="\$(jq -r '.id' "\$session_file")"
if [[ -z "\$session_id" || "\$session_id" == "null" ]]; then
  echo "Whiskey observability seed could not determine session id" >&2
  cat "\$session_file" >&2
  exit 1
fi

curl -fsS -X POST "\${auth_headers[@]}" \
  "\$base_url/api/sessions/\$session_id/run" >"\$run_file"

jq -n \
  --arg status "seeded" \
  --arg agent_id "\$agent_id" \
  --arg session_id "\$session_id" \
  --arg generated_at "\$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg reason "\$reason" \
  --slurpfile agents "\$agents_file" \
  --slurpfile session "\$session_file" \
  --slurpfile run "\$run_file" \
  '{
    status: \$status,
    agent_id: \$agent_id,
    session_id: \$session_id,
    reason: \$reason,
    generated_at: \$generated_at,
    agents: (\$agents[0] // []),
    session: (\$session[0] // {}),
    run: (\$run[0] // {})
  }' >"\$evidence_dir/whiskey-observability-remediation-seed.json"
REMOTE
)"

  ssh "$REMOTE_HOST" "bash -lc $(printf '%q' "$remote_script")"
}

seed_provider_rollout_evidence() {
  local evidence_dir="$1"
  local reason="$2"
  local remote_script

  remote_script="$(cat <<REMOTE
set -euo pipefail
cd '$REMOTE_ROOT'
set -a
source '$REMOTE_ENV'
set +a

base_url="http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787}"
evidence_dir='$evidence_dir'
reason='$reason'
mkdir -p "\$evidence_dir"
curl -fsS "\$base_url/healthz" >/dev/null

auth_headers=(
  -H "x-mandoforge-subject: whiskey-adoption-admin"
  -H "x-mandoforge-roles: admin"
)

providers_before="\$(mktemp)"
provider_file="\$(mktemp)"
providers_after="\$(mktemp)"
provider_body="\$(mktemp)"

cleanup() {
  rm -f "\$providers_before" "\$provider_file" "\$providers_after" "\$provider_body"
}
trap cleanup EXIT

curl -fsS "\${auth_headers[@]}" "\$base_url/api/providers" >"\$providers_before"

jq -n \
  --arg reason "\$reason" \
  '{
    provider_type: "mock",
    name: "whiskey-mock-provider",
    default_model: "gpt-5.4-mini",
    config: {
      source: "whiskey-provider-rollout-evidence",
      reason: \$reason,
      budget: {
        daily_request_limit: 1000
      }
    }
  }' >"\$provider_body"

curl -fsS -X POST "\${auth_headers[@]}" \
  -H "content-type: application/json" \
  -d @"\$provider_body" \
  "\$base_url/api/providers" >"\$provider_file"

curl -fsS "\${auth_headers[@]}" "\$base_url/api/providers" >"\$providers_after"

jq -n \
  --arg status "seeded" \
  --arg generated_at "\$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg reason "\$reason" \
  --slurpfile providers_before "\$providers_before" \
  --slurpfile provider "\$provider_file" \
  --slurpfile providers_after "\$providers_after" \
  '{
    status: \$status,
    generated_at: \$generated_at,
    reason: \$reason,
    providers_before: (\$providers_before[0] // []),
    provider: (\$provider[0] // {}),
    providers_after: (\$providers_after[0] // [])
  }' >"\$evidence_dir/whiskey-provider-rollout-seed.json"
REMOTE
)"

  ssh "$REMOTE_HOST" "bash -lc $(printf '%q' "$remote_script")"
}

seed_approval_notification_evidence() {
  local evidence_dir="$1"
  local reason="$2"
  local remote_script

  remote_script="$(cat <<REMOTE
set -euo pipefail
cd '$REMOTE_ROOT'
set -a
source '$REMOTE_ENV'
set +a

base_url="http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787}"
evidence_dir='$evidence_dir'
reason='$reason'
mkdir -p "\$evidence_dir"
curl -fsS "\$base_url/healthz" >/dev/null

auth_headers=(
  -H "x-mandoforge-subject: whiskey-adoption-admin"
  -H "x-mandoforge-roles: admin"
)

policies_file="\$(mktemp)"
policy_file="\$(mktemp)"
agents_file="\$(mktemp)"
session_file="\$(mktemp)"
approval_file="\$(mktemp)"
approvals_before="\$(mktemp)"
rejected_file="\$(mktemp)"
session_body="\$(mktemp)"
approval_body="\$(mktemp)"
policy_body="\$(mktemp)"

cleanup() {
  rm -f "\$policies_file" "\$policy_file" "\$agents_file" "\$session_file" "\$approval_file" "\$approvals_before" "\$rejected_file" "\$session_body" "\$approval_body" "\$policy_body"
}
trap cleanup EXIT

curl -fsS "\${auth_headers[@]}" "\$base_url/api/approvals" >"\$approvals_before"
jq -n '{rejected: []}' >"\$rejected_file"
mapfile -t unroutable_approval_ids < <(jq -r '
  .[]
  | select(.status == "pending")
  | select(
      ((.evidence.approver_subject // .evidence.delegated_approver // .evidence.args.approver_subject // .evidence.args.delegated_approver // "") | tostring | length) == 0
      and
      ((.evidence.approver_group_id // .evidence.delegated_approver_group_id // .evidence.args.approver_group_id // .evidence.args.delegated_approver_group_id // "") | tostring | length) == 0
    )
  | .id
' "\$approvals_before")
for approval_id in "\${unroutable_approval_ids[@]}"; do
  rejected_response="\$(mktemp)"
  curl -fsS -X POST "\${auth_headers[@]}" "\$base_url/api/approvals/\$approval_id/reject" >"\$rejected_response"
  jq --slurpfile rejected "\$rejected_response" '.rejected += [\$rejected[0]]' "\$rejected_file" >"\$rejected_file.tmp"
  mv "\$rejected_file.tmp" "\$rejected_file"
  rm -f "\$rejected_response"
done

curl -fsS "\${auth_headers[@]}" "\$base_url/api/approvals/notification-channel-policies" >"\$policies_file"
policy_id="\$(jq -r 'map(select(.name == "whiskey-approval-webhook")) | .[0].id // empty' "\$policies_file")"
if [[ -z "\$policy_id" ]]; then
  jq -n \
    '{
      name: "whiskey-approval-webhook",
      channel: "webhook",
      risk_filter: "all",
      max_attempts: 2,
      backoff_seconds: 0
    }' >"\$policy_body"
  curl -fsS -X POST "\${auth_headers[@]}" \
    -H "content-type: application/json" \
    -d @"\$policy_body" \
    "\$base_url/api/approvals/notification-channel-policies" >"\$policy_file"
else
  jq -n --arg id "\$policy_id" '{id: \$id, reused: true}' >"\$policy_file"
fi

curl -fsS "\${auth_headers[@]}" "\$base_url/api/agents" >"\$agents_file"
agent_id="\$(jq -r 'map(select(.name == "Generic Orchestrator Agent")) | .[0].id // .[0].id // empty' "\$agents_file")"
if [[ -z "\$agent_id" || "\$agent_id" == "null" ]]; then
  echo "Whiskey approval notification seed could not determine agent id" >&2
  cat "\$agents_file" >&2
  exit 1
fi

jq -n \
  --arg agent_id "\$agent_id" \
  --arg title "Whiskey approval notification seed" \
  --arg message "Create pending approval notification evidence for Whiskey." \
  '{
    agent_id: \$agent_id,
    title: \$title,
    message: \$message
  }' >"\$session_body"

curl -fsS -X POST "\${auth_headers[@]}" \
  -H "content-type: application/json" \
  -d @"\$session_body" \
  "\$base_url/api/sessions" >"\$session_file"
session_id="\$(jq -r '.id' "\$session_file")"
if [[ -z "\$session_id" || "\$session_id" == "null" ]]; then
  echo "Whiskey approval notification seed could not determine session id" >&2
  cat "\$session_file" >&2
  exit 1
fi

jq -n \
  --arg session_id "\$session_id" \
  --arg reason "\$reason" \
  '{
    session_id: \$session_id,
    args: {
      action: "whiskey.approval_notification_review",
      risk_level: "medium",
      reason: \$reason,
      approver_subject: "whiskey-approver"
    }
  }' >"\$approval_body"

curl -fsS -X POST "\${auth_headers[@]}" \
  -H "content-type: application/json" \
  -d @"\$approval_body" \
  "\$base_url/api/tools/approval.request/execute" >"\$approval_file"

jq -n \
  --arg status "seeded" \
  --arg generated_at "\$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg reason "\$reason" \
  --slurpfile approvals_before "\$approvals_before" \
  --slurpfile rejected "\$rejected_file" \
  --slurpfile policies "\$policies_file" \
  --slurpfile policy "\$policy_file" \
  --slurpfile agents "\$agents_file" \
  --slurpfile session "\$session_file" \
  --slurpfile approval "\$approval_file" \
  '{
    status: \$status,
    generated_at: \$generated_at,
    reason: \$reason,
    approvals_before: (\$approvals_before[0] // []),
    rejected_unroutable: (\$rejected[0].rejected // []),
    policies_before: (\$policies[0] // []),
    policy: (\$policy[0] // {}),
    agents: (\$agents[0] // []),
    session: (\$session[0] // {}),
    approval: (\$approval[0] // {})
  }' >"\$evidence_dir/whiskey-approval-notification-seed.json"
REMOTE
)"

  ssh "$REMOTE_HOST" "bash -lc $(printf '%q' "$remote_script")"
}

seed_vault_kms_evidence() {
  local evidence_dir="$1"
  local reason="$2"
  local remote_script

  remote_script="$(cat <<REMOTE
set -euo pipefail
cd '$REMOTE_ROOT'
set -a
source '$REMOTE_ENV'
set +a

base_url="http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787}"
evidence_dir='$evidence_dir'
reason='$reason'
mkdir -p "\$evidence_dir"
curl -fsS "\$base_url/healthz" >/dev/null

auth_headers=(
  -H "x-mandoforge-subject: whiskey-adoption-admin"
  -H "x-mandoforge-roles: admin"
)

secrets_before="\$(mktemp)"
secret_file="\$(mktemp)"
secrets_after="\$(mktemp)"
secret_body="\$(mktemp)"

cleanup() {
  rm -f "\$secrets_before" "\$secret_file" "\$secrets_after" "\$secret_body"
}
trap cleanup EXIT

curl -fsS "\${auth_headers[@]}" "\$base_url/api/vault/secrets" >"\$secrets_before"
secret_id="\$(jq -r 'map(select(.name == "whiskey-kms-provider-secret" and .path == "providers/whiskey" and .key == "api_key")) | .[0].id // empty' "\$secrets_before")"
if [[ -z "\$secret_id" ]]; then
  jq -n \
    '{
      name: "whiskey-kms-provider-secret",
      path: "providers/whiskey",
      key: "api_key",
      scope_type: "tenant",
      scope_id: null
    }' >"\$secret_body"
  curl -fsS -X POST "\${auth_headers[@]}" \
    -H "content-type: application/json" \
    -d @"\$secret_body" \
    "\$base_url/api/vault/secrets" >"\$secret_file"
else
  jq -n --arg id "\$secret_id" '{id: \$id, reused: true}' >"\$secret_file"
fi

curl -fsS "\${auth_headers[@]}" "\$base_url/api/vault/secrets" >"\$secrets_after"

jq -n \
  --arg status "seeded" \
  --arg generated_at "\$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg reason "\$reason" \
  --slurpfile secrets_before "\$secrets_before" \
  --slurpfile secret "\$secret_file" \
  --slurpfile secrets_after "\$secrets_after" \
  '{
    status: \$status,
    generated_at: \$generated_at,
    reason: \$reason,
    secrets_before: (\$secrets_before[0] // []),
    secret: (\$secret[0] // {}),
    secrets_after: (\$secrets_after[0] // [])
  }' >"\$evidence_dir/whiskey-vault-kms-seed.json"
REMOTE
)"

  ssh "$REMOTE_HOST" "bash -lc $(printf '%q' "$remote_script")"
}

mkdir -p "$LOCAL_SYNC_DIR"

ssh "$REMOTE_HOST" "cd '$REMOTE_ROOT' && test -f '$REMOTE_COMPOSE' && test -f '$REMOTE_ENV'"
ssh "$REMOTE_HOST" "mkdir -p '$REMOTE_ROOT/evidence' '$REMOTE_ROOT/archives' '$REMOTE_ROOT/scripts' '$REMOTE_ROOT/deploy/stage2-evidence' '$REMOTE_ROOT/deploy/stage2-production-evidence' && chown -R 1000:1000 '$REMOTE_ROOT/evidence' && chmod 0750 '$REMOTE_ROOT/evidence'"
rsync -az scripts/ "$REMOTE_HOST:$REMOTE_ROOT/scripts/"
rsync -az deploy/stage2-evidence/ "$REMOTE_HOST:$REMOTE_ROOT/deploy/stage2-evidence/"
rsync -az deploy/stage2-production-evidence/ "$REMOTE_HOST:$REMOTE_ROOT/deploy/stage2-production-evidence/"

ssh "$REMOTE_HOST" "cd '$REMOTE_ROOT' && set -a && source '$REMOTE_ENV' && set +a && \
  docker compose -p '$COMPOSE_PROJECT' -f '$REMOTE_COMPOSE' exec -T api bash -lc '
    set -euo pipefail
    mkdir -p /evidence
    curl -fsS http://127.0.0.1:8787/healthz >/dev/null

    org_id=\$(curl -fsS -H \"x-mandoforge-subject: whiskey-adoption-admin\" -H \"x-mandoforge-roles: admin\" http://127.0.0.1:8787/api/organizations | jq -r \"map(select(.slug == \\\"whiskey-adoption\\\")) | .[0].id // empty\")
    if [[ -z \"\$org_id\" ]]; then
      org_json=\$(curl -fsS -X POST -H \"x-mandoforge-subject: whiskey-adoption-admin\" -H \"x-mandoforge-roles: admin\" -H \"content-type: application/json\" -d \"{\\\"name\\\":\\\"Whiskey Adoption Org\\\",\\\"slug\\\":\\\"whiskey-adoption\\\"}\" http://127.0.0.1:8787/api/organizations)
      org_id=\$(printf \"%s\" \"\$org_json\" | jq -r .id)
    else
      org_json=\$(curl -fsS -H \"x-mandoforge-subject: whiskey-adoption-admin\" -H \"x-mandoforge-roles: admin\" http://127.0.0.1:8787/api/organizations | jq \"map(select(.id == \\\"\$org_id\\\")) | .[0]\")
    fi

    team_id=\$(curl -fsS -H \"x-mandoforge-subject: whiskey-adoption-admin\" -H \"x-mandoforge-roles: admin\" http://127.0.0.1:8787/api/organizations/\$org_id/teams | jq -r \"map(select(.slug == \\\"whiskey-pilot\\\")) | .[0].id // empty\")
    if [[ -z \"\$team_id\" ]]; then
      team_json=\$(curl -fsS -X POST -H \"x-mandoforge-subject: whiskey-adoption-admin\" -H \"x-mandoforge-roles: admin\" -H \"content-type: application/json\" -d \"{\\\"name\\\":\\\"Whiskey Pilot Team\\\",\\\"slug\\\":\\\"whiskey-pilot\\\"}\" http://127.0.0.1:8787/api/organizations/\$org_id/teams)
    else
      team_json=\$(curl -fsS -H \"x-mandoforge-subject: whiskey-adoption-admin\" -H \"x-mandoforge-roles: admin\" http://127.0.0.1:8787/api/organizations/\$org_id/teams | jq \"map(select(.id == \\\"\$team_id\\\")) | .[0]\")
    fi
    printf \"%s\\n%s\\n\" \"\$org_json\" \"\$team_json\" | jq -s \"{organization: .[0], team: .[1]}\" > /evidence/pilot-scope.json
    mcp_server_id=\$(curl -fsS -H \"x-mandoforge-subject: whiskey-adoption-admin\" -H \"x-mandoforge-roles: admin\" http://127.0.0.1:8787/api/teams/\$team_id/mcp-servers | jq -r \"map(select(.name == \\\"whiskey-docs\\\")) | .[0].id // empty\")
    if [[ -z \"\$mcp_server_id\" ]]; then
      mcp_server_json=\$(curl -fsS -X POST -H \"x-mandoforge-subject: whiskey-adoption-admin\" -H \"x-mandoforge-roles: admin\" -H \"content-type: application/json\" -d \"{\\\"name\\\":\\\"whiskey-docs\\\",\\\"transport\\\":\\\"http\\\",\\\"tool_allowlist\\\":[\\\"search\\\"],\\\"config\\\":{\\\"source\\\":\\\"whiskey-pilot\\\",\\\"health_check\\\":{\\\"interval_seconds\\\":1}}}\" http://127.0.0.1:8787/api/teams/\$team_id/mcp-servers)
      mcp_server_id=\$(printf \"%s\" \"\$mcp_server_json\" | jq -r .id)
    else
      mcp_server_json=\$(curl -fsS -X PATCH -H \"x-mandoforge-subject: whiskey-adoption-admin\" -H \"x-mandoforge-roles: admin\" -H \"content-type: application/json\" -d \"{\\\"transport\\\":\\\"http\\\",\\\"tool_allowlist\\\":[\\\"search\\\"],\\\"config\\\":{\\\"source\\\":\\\"whiskey-pilot\\\",\\\"health_check\\\":{\\\"interval_seconds\\\":1}}}\" http://127.0.0.1:8787/api/teams/\$team_id/mcp-servers/\$mcp_server_id)
      curl -fsS -X PATCH -H \"x-mandoforge-subject: whiskey-adoption-admin\" -H \"x-mandoforge-roles: admin\" -H \"content-type: application/json\" -d \"{\\\"status\\\":\\\"active\\\"}\" http://127.0.0.1:8787/api/teams/\$team_id/mcp-servers/\$mcp_server_id/status >/dev/null
    fi
    mcp_pending_rollout_id=\$(printf \"%s\" \"\$mcp_server_json\" | jq -r \".config.pending_rollout.id // empty\")
    if [[ -z \"\$mcp_pending_rollout_id\" ]]; then
      rollout_stamp=\$(date -u +%Y%m%dT%H%M%SZ)
      activate_after=\$(date -u -d \"1 minute ago\" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -v-1M +%Y-%m-%dT%H:%M:%SZ)
      curl -fsS -X POST -H \"x-mandoforge-subject: whiskey-adoption-admin\" -H \"x-mandoforge-roles: admin\" -H \"content-type: application/json\" -d \"{\\\"config\\\":{\\\"source\\\":\\\"whiskey-pilot-\$rollout_stamp\\\",\\\"health_check\\\":{\\\"interval_seconds\\\":1}},\\\"tool_allowlist\\\":[\\\"search\\\"],\\\"status\\\":\\\"active\\\",\\\"activate_after\\\":\\\"\$activate_after\\\",\\\"reason\\\":\\\"Whiskey MCP adoption evidence\\\"}\" http://127.0.0.1:8787/api/teams/\$team_id/mcp-servers/\$mcp_server_id/rollouts >/dev/null
    fi

    rm -rf /evidence/scheduler /evidence/codex-app-server /evidence/tenant-isolation /evidence/worker /evidence/remote-computer /evidence/eval-release /evidence/observability-collector /evidence/provider-governance /evidence/approval-notifications /evidence/vault-kms /evidence/workflow-packs /evidence/stage2-production
    BASE_URL=http://127.0.0.1:8787 EVIDENCE_DIR=/evidence/scheduler ALLOW_BLOCKED=1 MANDOFORGE_SCHEDULER_TOKEN=\"\${MANDOFORGE_SCHEDULER_TOKEN:-}\" /app/scripts/scheduler-evidence-gate.sh
    mcp_server_json=\$(curl -fsS -H \"x-mandoforge-subject: whiskey-adoption-admin\" -H \"x-mandoforge-roles: admin\" http://127.0.0.1:8787/api/teams/\$team_id/mcp-servers | jq \"map(select(.id == \\\"\$mcp_server_id\\\")) | .[0]\")
    mcp_pending_rollout_id=\$(printf \"%s\" \"\$mcp_server_json\" | jq -r \".config.pending_rollout.id // empty\")
    if [[ -z \"\$mcp_pending_rollout_id\" ]]; then
      rollout_stamp=\$(date -u +%Y%m%dT%H%M%SZ)
      activate_after=\$(date -u -d \"1 minute ago\" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -v-1M +%Y-%m-%dT%H:%M:%SZ)
      curl -fsS -X POST -H \"x-mandoforge-subject: whiskey-adoption-admin\" -H \"x-mandoforge-roles: admin\" -H \"content-type: application/json\" -d \"{\\\"config\\\":{\\\"source\\\":\\\"whiskey-stage2-\$rollout_stamp\\\",\\\"health_check\\\":{\\\"interval_seconds\\\":1}},\\\"tool_allowlist\\\":[\\\"search\\\"],\\\"status\\\":\\\"active\\\",\\\"activate_after\\\":\\\"\$activate_after\\\",\\\"reason\\\":\\\"Whiskey MCP strict evidence\\\"}\" http://127.0.0.1:8787/api/teams/\$team_id/mcp-servers/\$mcp_server_id/rollouts >/dev/null
    fi
    BASE_URL=http://127.0.0.1:8787 EVIDENCE_DIR=/evidence/codex-app-server ALLOW_BLOCKED=1 RUN_STAGE2_CODEX_STALE_POLL=1 /app/scripts/codex-app-server-evidence-gate.sh
    BASE_URL=http://127.0.0.1:8787 EVIDENCE_DIR=/evidence/tenant-isolation ALLOW_BLOCKED=1 /app/scripts/tenant-isolation-evidence-gate.sh
    BASE_URL=http://127.0.0.1:8787 EVIDENCE_DIR=/evidence/worker ALLOW_BLOCKED=1 /app/scripts/worker-evidence-gate.sh
    BASE_URL=http://127.0.0.1:8787 EVIDENCE_DIR=/evidence/remote-computer ALLOW_BLOCKED=1 /app/scripts/remote-computer-evidence-gate.sh
  '"

seed_eval_release_evidence "$REMOTE_ROOT/evidence/eval-release" "Whiskey focused eval/release adoption evidence"

ssh "$REMOTE_HOST" "cd '$REMOTE_ROOT' && set -a && source '$REMOTE_ENV' && set +a && \
  BASE_URL=http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787} EVIDENCE_DIR='$REMOTE_ROOT/evidence/eval-release' ALLOW_BLOCKED=1 RUN_STAGE2_EVAL_RELEASE_AUTOMATION=1 RUN_STAGE2_EVAL_RELEASE_ROLLBACK=1 MANDOFORGE_EVAL_RELEASE_ROLLBACK_ENVIRONMENT=whiskey-eval-release scripts/eval-release-evidence-gate.sh"

seed_observability_remediation_evidence "$REMOTE_ROOT/evidence/observability-collector" "Whiskey focused observability remediation evidence"

ssh "$REMOTE_HOST" "cd '$REMOTE_ROOT' && set -a && source '$REMOTE_ENV' && set +a && \
  BASE_URL=http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787} EVIDENCE_DIR='$REMOTE_ROOT/evidence/observability-collector' ALLOW_BLOCKED=1 RUN_STAGE2_OBSERVABILITY_REMEDIATION=1 scripts/observability-collector-evidence-gate.sh"

seed_provider_rollout_evidence "$REMOTE_ROOT/evidence/provider-governance" "Whiskey focused provider rollout adoption evidence"

ssh "$REMOTE_HOST" "cd '$REMOTE_ROOT' && set -a && source '$REMOTE_ENV' && set +a && \
  BASE_URL=http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787} EVIDENCE_DIR='$REMOTE_ROOT/evidence/provider-governance' ALLOW_BLOCKED=1 RUN_STAGE2_PROVIDER_ROLLOUT=1 scripts/provider-governance-evidence-gate.sh"

seed_approval_notification_evidence "$REMOTE_ROOT/evidence/approval-notifications" "Whiskey focused approval notification delivery evidence"

ssh "$REMOTE_HOST" "cd '$REMOTE_ROOT' && set -a && source '$REMOTE_ENV' && set +a && \
  BASE_URL=http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787} EVIDENCE_DIR='$REMOTE_ROOT/evidence/approval-notifications' ALLOW_BLOCKED=1 RUN_STAGE2_APPROVAL_DELIVERY=1 scripts/approval-notification-evidence-gate.sh"

seed_vault_kms_evidence "$REMOTE_ROOT/evidence/vault-kms" "Whiskey focused Vault/KMS lifecycle evidence"

ssh "$REMOTE_HOST" "cd '$REMOTE_ROOT' && set -a && source '$REMOTE_ENV' && set +a && \
  BASE_URL=http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787} EVIDENCE_DIR='$REMOTE_ROOT/evidence/vault-kms' ALLOW_BLOCKED=1 RUN_STAGE2_SECRET_LIFECYCLE=1 scripts/vault-evidence-gate.sh"

ssh "$REMOTE_HOST" "cd '$REMOTE_ROOT' && set -a && source '$REMOTE_ENV' && set +a && \
  rm -rf '$REMOTE_ROOT/evidence/workflow-packs' && \
  BASE_URL=http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787} EVIDENCE_DIR='$REMOTE_ROOT/evidence/workflow-packs' WORKFLOW_PACK_MANIFEST_PATH=packs/ai-governance/package.yaml scripts/workflow-pack-evidence-gate.sh"

ssh "$REMOTE_HOST" "cd '$REMOTE_ROOT' && set -a && source '$REMOTE_ENV' && set +a && \
  rm -rf '$REMOTE_ROOT/evidence/stage2-production'"

seed_eval_release_evidence "$REMOTE_ROOT/evidence/stage2-production" "Whiskey strict eval/release adoption evidence"
seed_observability_remediation_evidence "$REMOTE_ROOT/evidence/stage2-production" "Whiskey strict observability remediation evidence"
seed_provider_rollout_evidence "$REMOTE_ROOT/evidence/stage2-production" "Whiskey strict provider rollout adoption evidence"
seed_approval_notification_evidence "$REMOTE_ROOT/evidence/stage2-production" "Whiskey strict approval notification delivery evidence"
seed_vault_kms_evidence "$REMOTE_ROOT/evidence/stage2-production" "Whiskey strict Vault/KMS lifecycle evidence"

ssh "$REMOTE_HOST" "cd '$REMOTE_ROOT' && set -a && source '$REMOTE_ENV' && set +a && \
  BASE_URL=http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787} EVIDENCE_DIR='$REMOTE_ROOT/evidence/stage2-production' ALLOW_BLOCKED=1 RUN_STAGE2_PRODUCTION_VALIDATIONS=$RUN_STRICT_VALIDATIONS RUN_STAGE2_MCP_DUE_RUN=1 RUN_STAGE2_MCP_ROLLBACK=1 RUN_STAGE2_EVAL_RELEASE_AUTOMATION=1 RUN_STAGE2_EVAL_RELEASE_ROLLBACK=1 RUN_STAGE2_OBSERVABILITY_REMEDIATION=1 RUN_STAGE2_PROVIDER_ROLLOUT=1 RUN_STAGE2_APPROVAL_DELIVERY=1 RUN_STAGE2_CODEX_STALE_POLL=1 RUN_STAGE2_SECRET_LIFECYCLE=1 MANDOFORGE_EVAL_RELEASE_ROLLBACK_ENVIRONMENT=whiskey-eval-release MANDOFORGE_SCHEDULER_TOKEN=\"\${MANDOFORGE_SCHEDULER_TOKEN:-}\" scripts/stage2-production-evidence-gate.sh"

archive_paths="$(ssh "$REMOTE_HOST" "set -euo pipefail
  mkdir -p '$REMOTE_ROOT/archives'
  stamp=\$(date -u +%Y%m%dT%H%M%SZ)
  stage_archive='$REMOTE_ROOT/archives/stage2-production-whiskey-'\$stamp'.tar.gz'
  all_archive='$REMOTE_ROOT/archives/mandoforge-whiskey-pilot-'\$stamp'.tar.gz'
  tar czf \"\$stage_archive\" -C '$REMOTE_ROOT/evidence/stage2-production' .
  tar czf \"\$all_archive\" -C '$REMOTE_ROOT/evidence' .
  sha256sum \"\$stage_archive\" > \"\$stage_archive.sha256\"
  sha256sum \"\$all_archive\" > \"\$all_archive.sha256\"
  {
    echo created_at=\$(date -u +%Y-%m-%dT%H:%M:%SZ)
    echo host=\$(hostname)
    echo base_url=http://127.0.0.1:18787
    echo compose_project='$COMPOSE_PROJECT'
    echo archive_path=\$stage_archive
    echo archive_sha256=\$(sha256sum \"\$stage_archive\" | awk '{print \$1}')
    echo note=blocked inventory archive from Whiskey production-like pilot
  } > \"\$stage_archive.manifest.txt\"
  {
    echo created_at=\$(date -u +%Y-%m-%dT%H:%M:%SZ)
    echo host=\$(hostname)
    echo base_url=http://127.0.0.1:18787
    echo compose_project='$COMPOSE_PROJECT'
    echo archive_path=\$all_archive
    echo archive_sha256=\$(sha256sum \"\$all_archive\" | awk '{print \$1}')
    echo note=all Whiskey pilot evidence
  } > \"\$all_archive.manifest.txt\"
  printf '%s\n%s\n' \"\$stage_archive\" \"\$all_archive\"")"

while IFS= read -r archive; do
  [[ -z "$archive" ]] && continue
  rsync -az "$REMOTE_HOST:$archive"* "$LOCAL_SYNC_DIR/"
done <<<"$archive_paths"

stage_copy="$(printf '%s\n' "$archive_paths" | head -1)"
stage_name="$(basename "$stage_copy")"
local_stage="$LOCAL_SYNC_DIR/$stage_name"
local_sha="$(sha256_value "$local_stage")"
{
  echo "created_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "host=$REMOTE_HOST"
  echo "base_url=http://127.0.0.1:18787"
  echo "compose_project=$COMPOSE_PROJECT"
  echo "archive_path=$local_stage"
  echo "archive_sha256=$local_sha"
  echo "note=local copy of Whiskey production-like pilot archive"
} >"$local_stage.manifest.txt"
printf '%s  %s\n' "$local_sha" "$local_stage" >"$local_stage.sha256"

ALLOW_BLOCKED=1 ./scripts/verify-stage2-evidence-archive.sh "$local_stage"
echo "Whiskey evidence synced to $LOCAL_SYNC_DIR"
