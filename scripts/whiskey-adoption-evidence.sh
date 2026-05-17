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

    rm -rf /evidence/scheduler /evidence/codex-app-server /evidence/tenant-isolation /evidence/worker /evidence/remote-computer /evidence/workflow-packs /evidence/stage2-production
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

ssh "$REMOTE_HOST" "cd '$REMOTE_ROOT' && set -a && source '$REMOTE_ENV' && set +a && \
  rm -rf '$REMOTE_ROOT/evidence/workflow-packs' && \
  BASE_URL=http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787} EVIDENCE_DIR='$REMOTE_ROOT/evidence/workflow-packs' WORKFLOW_PACK_MANIFEST_PATH=packs/ai-governance/package.yaml scripts/workflow-pack-evidence-gate.sh"

ssh "$REMOTE_HOST" "cd '$REMOTE_ROOT' && set -a && source '$REMOTE_ENV' && set +a && \
  rm -rf '$REMOTE_ROOT/evidence/stage2-production' && \
  BASE_URL=http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787} EVIDENCE_DIR='$REMOTE_ROOT/evidence/stage2-production' ALLOW_BLOCKED=1 RUN_STAGE2_PRODUCTION_VALIDATIONS=$RUN_STRICT_VALIDATIONS RUN_STAGE2_MCP_DUE_RUN=1 RUN_STAGE2_MCP_ROLLBACK=1 MANDOFORGE_SCHEDULER_TOKEN=\"\${MANDOFORGE_SCHEDULER_TOKEN:-}\" scripts/stage2-production-evidence-gate.sh"

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
