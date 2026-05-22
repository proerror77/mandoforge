#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${WHISKEY_REMOTE_HOST:-wishky-2-1}"
REMOTE_ROOT="${WHISKEY_REMOTE_ROOT:-/opt/mandoforge-adoption}"
COMPOSE_PROJECT="${WHISKEY_COMPOSE_PROJECT:-mandoforge-adoption}"
LOCAL_COMPOSE="${WHISKEY_COMPOSE_FILE:-deploy/whiskey/docker-compose.adoption.yml}"
LOCAL_CODEX_CONTROLLER="${WHISKEY_CODEX_CONTROLLER_FILE:-deploy/whiskey/codex-app-server-controller.mjs}"
LOCAL_TENANT_CONTROLLER="${WHISKEY_TENANT_CONTROLLER_FILE:-deploy/whiskey/tenant-routing-controller.mjs}"
LOCAL_WORKER_CONTROLLER="${WHISKEY_WORKER_LOAD_CONTROLLER_FILE:-deploy/whiskey/worker-load-controller.mjs}"
LOCAL_MCP_CONTROLLER="${WHISKEY_MCP_CONTROLLER_FILE:-deploy/whiskey/mcp-pilot-controller.mjs}"
LOCAL_EVAL_RELEASE_CONTROLLER="${WHISKEY_EVAL_RELEASE_CONTROLLER_FILE:-deploy/whiskey/eval-release-controller.mjs}"
LOCAL_OBSERVABILITY_CONTROLLER="${WHISKEY_OBSERVABILITY_CONTROLLER_FILE:-deploy/whiskey/observability-controller.mjs}"
LOCAL_OTEL_COLLECTOR_CONFIG="${WHISKEY_OTEL_COLLECTOR_CONFIG_FILE:-deploy/whiskey/otel-collector-config.yaml}"
LOCAL_PROVIDER_CONTROLLER="${WHISKEY_PROVIDER_CONTROLLER_FILE:-deploy/whiskey/provider-rollout-controller.mjs}"
LOCAL_APPROVAL_NOTIFICATION_CONTROLLER="${WHISKEY_APPROVAL_NOTIFICATION_CONTROLLER_FILE:-deploy/whiskey/approval-notification-controller.mjs}"
LOCAL_VAULT_KMS_CONTROLLER="${WHISKEY_VAULT_KMS_CONTROLLER_FILE:-deploy/whiskey/vault-kms-controller.mjs}"
LOCAL_FINANCE_CONTROLLER="${WHISKEY_FINANCE_CONTROLLER_FILE:-deploy/whiskey/finance-controller.mjs}"
LOCAL_WEB_DIR="${WHISKEY_WEB_DIR:-web}"
REMOTE_COMPOSE="$REMOTE_ROOT/docker-compose.yml"
REMOTE_CODEX_CONTROLLER="$REMOTE_ROOT/codex-app-server-controller.mjs"
REMOTE_TENANT_CONTROLLER="$REMOTE_ROOT/tenant-routing-controller.mjs"
REMOTE_WORKER_CONTROLLER="$REMOTE_ROOT/worker-load-controller.mjs"
REMOTE_MCP_CONTROLLER="$REMOTE_ROOT/mcp-pilot-controller.mjs"
REMOTE_EVAL_RELEASE_CONTROLLER="$REMOTE_ROOT/eval-release-controller.mjs"
REMOTE_OBSERVABILITY_CONTROLLER="$REMOTE_ROOT/observability-controller.mjs"
REMOTE_OTEL_COLLECTOR_CONFIG="$REMOTE_ROOT/otel-collector-config.yaml"
REMOTE_PROVIDER_CONTROLLER="$REMOTE_ROOT/provider-rollout-controller.mjs"
REMOTE_APPROVAL_NOTIFICATION_CONTROLLER="$REMOTE_ROOT/approval-notification-controller.mjs"
REMOTE_VAULT_KMS_CONTROLLER="$REMOTE_ROOT/vault-kms-controller.mjs"
REMOTE_FINANCE_CONTROLLER="$REMOTE_ROOT/finance-controller.mjs"
REMOTE_ENV="$REMOTE_ROOT/whiskey.env"
IMAGE_TAG="${MANDOFORGE_IMAGE_TAG:-latest}"
PULL_IMAGE="${WHISKEY_PULL_IMAGE:-1}"
CODEX_WS_PORT="${WHISKEY_CODEX_APP_SERVER_WS_PORT:-18788}"
CODEX_CONTROLLER_PORT="${WHISKEY_CODEX_APP_SERVER_CONTROLLER_PORT:-18789}"
CODEX_CONTROLLER_TOKEN="${WHISKEY_CODEX_APP_SERVER_CONTROLLER_TOKEN:-whiskey-codex-controller-token}"
TENANT_CONTROLLER_PORT="${WHISKEY_TENANT_ROUTING_CONTROLLER_PORT:-18790}"
TENANT_CONTROLLER_TOKEN="${WHISKEY_TENANT_ROUTING_CONTROLLER_TOKEN:-whiskey-tenant-routing-controller-token}"
WORKER_CONTROLLER_PORT="${WHISKEY_WORKER_LOAD_CONTROLLER_PORT:-18791}"
WORKER_CONTROLLER_TOKEN="${WHISKEY_WORKER_LOAD_CONTROLLER_TOKEN:-whiskey-worker-load-controller-token}"
MCP_CONTROLLER_PORT="${WHISKEY_MCP_CONTROLLER_PORT:-18792}"
MCP_CONTROLLER_TOKEN="${WHISKEY_MCP_CONTROLLER_TOKEN:-whiskey-mcp-controller-token}"
MCP_SERVER_NAME="${WHISKEY_MCP_SERVER_NAME:-whiskey-docs}"
MCP_UPSTREAM_MODE="${WHISKEY_MCP_UPSTREAM_MODE:-lark_chat_messages}"
MCP_GITHUB_REPO_OWNER="${WHISKEY_MCP_GITHUB_REPO_OWNER:-proerror77}"
MCP_GITHUB_REPO_NAME="${WHISKEY_MCP_GITHUB_REPO_NAME:-Goodchance}"
MCP_GITHUB_REPO_REF="${WHISKEY_MCP_GITHUB_REPO_REF:-main}"
MCP_GITHUB_REPO_LIMIT="${WHISKEY_MCP_GITHUB_REPO_LIMIT:-5}"
MCP_LARK_AS="${WHISKEY_MCP_LARK_AS:-user}"
MCP_LARK_USER_OPEN_ID="${WHISKEY_MCP_LARK_USER_OPEN_ID:-}"
MCP_LARK_CHAT_ID="${WHISKEY_MCP_LARK_CHAT_ID:-}"
MCP_LARK_MESSAGE_LIMIT="${WHISKEY_MCP_LARK_MESSAGE_LIMIT:-10}"
MCP_LARK_DOCS_PAGE_SIZE="${WHISKEY_MCP_LARK_DOCS_PAGE_SIZE:-10}"
EVAL_RELEASE_CONTROLLER_PORT="${WHISKEY_EVAL_RELEASE_CONTROLLER_PORT:-18793}"
EVAL_RELEASE_CONTROLLER_TOKEN="${WHISKEY_EVAL_RELEASE_CONTROLLER_TOKEN:-whiskey-eval-release-controller-token}"
EVAL_RELEASE_ENVIRONMENT="${WHISKEY_EVAL_RELEASE_ENVIRONMENT:-whiskey-eval-release}"
OBSERVABILITY_CONTROLLER_PORT="${WHISKEY_OBSERVABILITY_CONTROLLER_PORT:-18794}"
OBSERVABILITY_CONTROLLER_TOKEN="${WHISKEY_OBSERVABILITY_CONTROLLER_TOKEN:-whiskey-observability-controller-token}"
OBSERVABILITY_SERVICE_NAME="${WHISKEY_OBSERVABILITY_SERVICE_NAME:-mandoforge-api}"
PROVIDER_CONTROLLER_PORT="${WHISKEY_PROVIDER_CONTROLLER_PORT:-18795}"
PROVIDER_CONTROLLER_TOKEN="${WHISKEY_PROVIDER_CONTROLLER_TOKEN:-whiskey-provider-controller-token}"
PROVIDER_ROLLOUT_ENVIRONMENT="${WHISKEY_PROVIDER_ROLLOUT_ENVIRONMENT:-production}"
PROVIDER_REAL_MODE="${WHISKEY_PROVIDER_REAL_MODE:-deepseek_if_available}"
APPROVAL_NOTIFICATION_CONTROLLER_PORT="${WHISKEY_APPROVAL_NOTIFICATION_CONTROLLER_PORT:-18796}"
APPROVAL_NOTIFICATION_CONTROLLER_TOKEN="${WHISKEY_APPROVAL_NOTIFICATION_CONTROLLER_TOKEN:-whiskey-approval-notification-controller-token}"
APPROVAL_NOTIFICATION_DELIVERY_MODE="${WHISKEY_APPROVAL_NOTIFICATION_DELIVERY_MODE:-lark_im}"
APPROVAL_NOTIFICATION_LARK_AS="${WHISKEY_APPROVAL_NOTIFICATION_LARK_AS:-user}"
APPROVAL_NOTIFICATION_LARK_OPEN_ID="${WHISKEY_APPROVAL_NOTIFICATION_LARK_OPEN_ID:-}"
VAULT_KMS_CONTROLLER_PORT="${WHISKEY_VAULT_KMS_CONTROLLER_PORT:-18797}"
VAULT_KMS_CONTROLLER_TOKEN="${WHISKEY_VAULT_KMS_CONTROLLER_TOKEN:-whiskey-vault-kms-controller-token}"
VAULT_KMS_VAULT_TOKEN="${WHISKEY_VAULT_TOKEN:-whiskey-vault-token}"
VAULT_KMS_PROVIDER="${WHISKEY_KMS_PROVIDER:-mock-kms}"
VAULT_KMS_KEY_ID="${WHISKEY_KMS_KEY_ID:-whiskey-kms-key-1}"
VAULT_KMS_ROTATION_POLICY="${WHISKEY_KMS_ROTATION_POLICY:-whiskey-manual-confirmed}"
FINANCE_CONTROLLER_PORT="${WHISKEY_FINANCE_CONTROLLER_PORT:-18798}"
FINANCE_CLOSE_CONTROLLER_TOKEN="${WHISKEY_FINANCE_CLOSE_CONTROLLER_TOKEN:-whiskey-finance-close-controller-token}"
FINANCE_RECONCILIATION_CONTROLLER_TOKEN="${WHISKEY_FINANCE_RECONCILIATION_CONTROLLER_TOKEN:-whiskey-finance-reconciliation-controller-token}"
FINANCE_EXPORT_DELIVERY_MODE="${WHISKEY_FINANCE_EXPORT_DELIVERY_MODE:-lark_drive}"
FINANCE_EXPORT_LARK_AS="${WHISKEY_FINANCE_EXPORT_LARK_AS:-user}"
FINANCE_EXPORT_LARK_FOLDER_TOKEN="${WHISKEY_FINANCE_EXPORT_LARK_FOLDER_TOKEN:-}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "whiskey adoption deploy requires $1" >&2
    exit 1
  fi
}

require_cmd ssh
require_cmd rsync

if [[ ! -f "$LOCAL_COMPOSE" ]]; then
  echo "missing Whiskey compose file: $LOCAL_COMPOSE" >&2
  exit 1
fi
if [[ ! -f "$LOCAL_CODEX_CONTROLLER" ]]; then
  echo "missing Whiskey Codex controller file: $LOCAL_CODEX_CONTROLLER" >&2
  exit 1
fi
if [[ ! -f "$LOCAL_TENANT_CONTROLLER" ]]; then
  echo "missing Whiskey tenant routing controller file: $LOCAL_TENANT_CONTROLLER" >&2
  exit 1
fi
if [[ ! -f "$LOCAL_WORKER_CONTROLLER" ]]; then
  echo "missing Whiskey worker load controller file: $LOCAL_WORKER_CONTROLLER" >&2
  exit 1
fi
if [[ ! -f "$LOCAL_MCP_CONTROLLER" ]]; then
  echo "missing Whiskey MCP controller file: $LOCAL_MCP_CONTROLLER" >&2
  exit 1
fi
if [[ ! -f "$LOCAL_EVAL_RELEASE_CONTROLLER" ]]; then
  echo "missing Whiskey eval/release controller file: $LOCAL_EVAL_RELEASE_CONTROLLER" >&2
  exit 1
fi
if [[ ! -f "$LOCAL_OBSERVABILITY_CONTROLLER" ]]; then
  echo "missing Whiskey observability controller file: $LOCAL_OBSERVABILITY_CONTROLLER" >&2
  exit 1
fi
if [[ ! -f "$LOCAL_OTEL_COLLECTOR_CONFIG" ]]; then
  echo "missing Whiskey OTel collector config file: $LOCAL_OTEL_COLLECTOR_CONFIG" >&2
  exit 1
fi
if [[ ! -f "$LOCAL_PROVIDER_CONTROLLER" ]]; then
  echo "missing Whiskey provider controller file: $LOCAL_PROVIDER_CONTROLLER" >&2
  exit 1
fi
if [[ ! -f "$LOCAL_APPROVAL_NOTIFICATION_CONTROLLER" ]]; then
  echo "missing Whiskey approval notification controller file: $LOCAL_APPROVAL_NOTIFICATION_CONTROLLER" >&2
  exit 1
fi
if [[ ! -f "$LOCAL_VAULT_KMS_CONTROLLER" ]]; then
  echo "missing Whiskey Vault/KMS controller file: $LOCAL_VAULT_KMS_CONTROLLER" >&2
  exit 1
fi
if [[ ! -f "$LOCAL_FINANCE_CONTROLLER" ]]; then
  echo "missing Whiskey finance controller file: $LOCAL_FINANCE_CONTROLLER" >&2
  exit 1
fi
if [[ ! -d "$LOCAL_WEB_DIR" ]]; then
  echo "missing Whiskey web asset directory: $LOCAL_WEB_DIR" >&2
  exit 1
fi

ssh "$REMOTE_HOST" "mkdir -p '$REMOTE_ROOT/evidence' '$REMOTE_ROOT/archives' && chown -R 1000:1000 '$REMOTE_ROOT/evidence' && chmod 0750 '$REMOTE_ROOT/evidence'"
rsync -az "$LOCAL_COMPOSE" "$REMOTE_HOST:$REMOTE_COMPOSE"
rsync -az "$LOCAL_CODEX_CONTROLLER" "$REMOTE_HOST:$REMOTE_CODEX_CONTROLLER"
rsync -az "$LOCAL_TENANT_CONTROLLER" "$REMOTE_HOST:$REMOTE_TENANT_CONTROLLER"
rsync -az "$LOCAL_WORKER_CONTROLLER" "$REMOTE_HOST:$REMOTE_WORKER_CONTROLLER"
rsync -az "$LOCAL_MCP_CONTROLLER" "$REMOTE_HOST:$REMOTE_MCP_CONTROLLER"
rsync -az "$LOCAL_EVAL_RELEASE_CONTROLLER" "$REMOTE_HOST:$REMOTE_EVAL_RELEASE_CONTROLLER"
rsync -az "$LOCAL_OBSERVABILITY_CONTROLLER" "$REMOTE_HOST:$REMOTE_OBSERVABILITY_CONTROLLER"
rsync -az "$LOCAL_OTEL_COLLECTOR_CONFIG" "$REMOTE_HOST:$REMOTE_OTEL_COLLECTOR_CONFIG"
rsync -az "$LOCAL_PROVIDER_CONTROLLER" "$REMOTE_HOST:$REMOTE_PROVIDER_CONTROLLER"
rsync -az "$LOCAL_APPROVAL_NOTIFICATION_CONTROLLER" "$REMOTE_HOST:$REMOTE_APPROVAL_NOTIFICATION_CONTROLLER"
rsync -az "$LOCAL_VAULT_KMS_CONTROLLER" "$REMOTE_HOST:$REMOTE_VAULT_KMS_CONTROLLER"
rsync -az "$LOCAL_FINANCE_CONTROLLER" "$REMOTE_HOST:$REMOTE_FINANCE_CONTROLLER"
rsync -az "$LOCAL_WEB_DIR/" "$REMOTE_HOST:$REMOTE_ROOT/web/"

ssh "$REMOTE_HOST" "if [[ ! -f '$REMOTE_ENV' ]]; then cat > '$REMOTE_ENV' <<'ENV'
MANDOFORGE_IMAGE_TAG=$IMAGE_TAG
MANDOFORGE_API_HOST_PORT=18787
MANDOFORGE_POSTGRES_HOST_PORT=15432
MANDOFORGE_SCHEDULER_TOKEN=whiskey-stage2-scheduler-token
ENV
else
  if grep -q '^MANDOFORGE_IMAGE_TAG=' '$REMOTE_ENV'; then
    sed -i 's/^MANDOFORGE_IMAGE_TAG=.*/MANDOFORGE_IMAGE_TAG=$IMAGE_TAG/' '$REMOTE_ENV'
  else
    printf '\nMANDOFORGE_IMAGE_TAG=%s\n' '$IMAGE_TAG' >> '$REMOTE_ENV'
  fi
fi
ensure_env() {
  local key=\"\$1\"
  local value=\"\$2\"
  if grep -q \"^\${key}=\" '$REMOTE_ENV'; then
    if grep -q \"^\${key}=$\" '$REMOTE_ENV'; then
      sed -i \"s#^\${key}=.*#\${key}=\${value}#\" '$REMOTE_ENV'
    fi
  else
    printf '%s=%s\n' \"\$key\" \"\$value\" >> '$REMOTE_ENV'
  fi
}
set_env() {
  local key=\"\$1\"
  local value=\"\$2\"
  if grep -q \"^\${key}=\" '$REMOTE_ENV'; then
    sed -i \"s#^\${key}=.*#\${key}=\${value}#\" '$REMOTE_ENV'
  else
    printf '%s=%s\n' \"\$key\" \"\$value\" >> '$REMOTE_ENV'
  fi
}
ensure_env MANDOFORGE_CODEX_APP_SERVER_URL ws://host.docker.internal:$CODEX_WS_PORT
ensure_env MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_URL http://host.docker.internal:$CODEX_CONTROLLER_PORT/deployment/validate
ensure_env MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_TOKEN $CODEX_CONTROLLER_TOKEN
ensure_env MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_URL http://host.docker.internal:$CODEX_CONTROLLER_PORT/ops/validate
ensure_env MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_TOKEN $CODEX_CONTROLLER_TOKEN
ensure_env MANDOFORGE_SECRET_PROVIDER vault
ensure_env MANDOFORGE_VAULT_ADDR http://host.docker.internal:$VAULT_KMS_CONTROLLER_PORT
ensure_env MANDOFORGE_VAULT_MOUNT kv
ensure_env MANDOFORGE_VAULT_TOKEN $VAULT_KMS_VAULT_TOKEN
ensure_env MANDOFORGE_KMS_PROVIDER $VAULT_KMS_PROVIDER
ensure_env MANDOFORGE_KMS_KEY_ID $VAULT_KMS_KEY_ID
ensure_env MANDOFORGE_KMS_ROTATION_POLICY $VAULT_KMS_ROTATION_POLICY
ensure_env MANDOFORGE_KMS_VALIDATION_MODE external
ensure_env MANDOFORGE_KMS_ENDPOINT http://host.docker.internal:$VAULT_KMS_CONTROLLER_PORT/kms/rotate
ensure_env MANDOFORGE_KMS_TOKEN $VAULT_KMS_CONTROLLER_TOKEN
ensure_env MANDOFORGE_KMS_RECOVERY_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_KMS_RECOVERY_CONTROLLER_URL http://host.docker.internal:$VAULT_KMS_CONTROLLER_PORT/kms/recovery/validate
ensure_env MANDOFORGE_KMS_RECOVERY_CONTROLLER_TOKEN $VAULT_KMS_CONTROLLER_TOKEN
ensure_env MANDOFORGE_TENANT_ROUTING_MODE tenant_routed
ensure_env MANDOFORGE_TENANT_ROUTING_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_TENANT_ROUTING_CONTROLLER_URL http://host.docker.internal:$TENANT_CONTROLLER_PORT/tenant/routing/validate
ensure_env MANDOFORGE_TENANT_ROUTING_CONTROLLER_TOKEN $TENANT_CONTROLLER_TOKEN
ensure_env MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_URL http://host.docker.internal:$WORKER_CONTROLLER_PORT/worker/load/validate
ensure_env MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_TOKEN $WORKER_CONTROLLER_TOKEN
ensure_env MANDOFORGE_MCP_GATEWAY_URL http://host.docker.internal:$MCP_CONTROLLER_PORT
ensure_env MANDOFORGE_MCP_ALLOWED_SERVERS $MCP_SERVER_NAME
ensure_env MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_URL http://host.docker.internal:$MCP_CONTROLLER_PORT/mcp/deployment/validate
ensure_env MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_TOKEN $MCP_CONTROLLER_TOKEN
ensure_env MANDOFORGE_MCP_ROLLOUT_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_MCP_ROLLOUT_CONTROLLER_URL http://host.docker.internal:$MCP_CONTROLLER_PORT/mcp/rollout/approve
ensure_env MANDOFORGE_MCP_ROLLOUT_CONTROLLER_TOKEN $MCP_CONTROLLER_TOKEN
ensure_env MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_URL http://host.docker.internal:$MCP_CONTROLLER_PORT/mcp/rollback/validate
ensure_env MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_TOKEN $MCP_CONTROLLER_TOKEN
ensure_env MANDOFORGE_AGENT_RELEASE_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_AGENT_RELEASE_CONTROLLER_URL http://host.docker.internal:$EVAL_RELEASE_CONTROLLER_PORT/agents/releases/rollout/apply
ensure_env MANDOFORGE_AGENT_RELEASE_CONTROLLER_TOKEN $EVAL_RELEASE_CONTROLLER_TOKEN
ensure_env MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_URL http://host.docker.internal:$EVAL_RELEASE_CONTROLLER_PORT/agents/releases/deployment/validate
ensure_env MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_TOKEN $EVAL_RELEASE_CONTROLLER_TOKEN
ensure_env MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_URL http://host.docker.internal:$EVAL_RELEASE_CONTROLLER_PORT/agents/releases/orchestration/validate
ensure_env MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_TOKEN $EVAL_RELEASE_CONTROLLER_TOKEN
ensure_env MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_URL http://host.docker.internal:$EVAL_RELEASE_CONTROLLER_PORT/agents/releases/rollout/rollback
ensure_env MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_TOKEN $EVAL_RELEASE_CONTROLLER_TOKEN
ensure_env MANDOFORGE_SERVICE_NAME $OBSERVABILITY_SERVICE_NAME
set_env MANDOFORGE_OTEL_EXPORTER_OTLP_ENDPOINT http://otel-collector:4318
set_env MANDOFORGE_OTEL_COLLECTOR_HEALTH_ENDPOINT http://otel-collector:13133/healthz
ensure_env MANDOFORGE_OTEL_SAMPLE_RATIO 1.0
ensure_env MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_URL http://host.docker.internal:$OBSERVABILITY_CONTROLLER_PORT/observability/collector/deployment/validate
ensure_env MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_TOKEN $OBSERVABILITY_CONTROLLER_TOKEN
ensure_env MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_URL http://host.docker.internal:$OBSERVABILITY_CONTROLLER_PORT/observability/collector/cluster/validate
ensure_env MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_TOKEN $OBSERVABILITY_CONTROLLER_TOKEN
ensure_env MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_URL http://host.docker.internal:$OBSERVABILITY_CONTROLLER_PORT/observability/remediation/run
ensure_env MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_TOKEN $OBSERVABILITY_CONTROLLER_TOKEN
ensure_env MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_URL http://host.docker.internal:$PROVIDER_CONTROLLER_PORT/provider/deployment/validate
ensure_env MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_TOKEN $PROVIDER_CONTROLLER_TOKEN
ensure_env MANDOFORGE_PROVIDER_ROLLOUT_CONTROLLER_URL http://host.docker.internal:$PROVIDER_CONTROLLER_PORT/provider/rollout/apply
ensure_env MANDOFORGE_PROVIDER_ROLLOUT_TOKEN $PROVIDER_CONTROLLER_TOKEN
ensure_env MANDOFORGE_PROVIDER_ROLLOUT_ROLLBACK_CONTROLLER_URL http://host.docker.internal:$PROVIDER_CONTROLLER_PORT/provider/rollout/rollback
ensure_env MANDOFORGE_PROVIDER_ROLLOUT_ROLLBACK_TOKEN $PROVIDER_CONTROLLER_TOKEN
if [[ '$PROVIDER_REAL_MODE' == 'deepseek_if_available' ]]; then
  if [[ -f /root/.hermes/.env ]]; then
    set -a
    source /root/.hermes/.env
    set +a
  fi
  if [[ -n "\${DEEPSEEK_API_KEY:-}" ]]; then
    set_env DEEPSEEK_API_KEY "\$DEEPSEEK_API_KEY"
    set_env MANDOFORGE_PROVIDER_BASE_URL https://api.deepseek.com
    set_env MANDOFORGE_PROVIDER_API_KEY "\$DEEPSEEK_API_KEY"
    set_env MANDOFORGE_PROVIDER_MODEL deepseek-v4-flash
  fi
fi
ensure_env MANDOFORGE_APPROVAL_WEBHOOK_URL http://host.docker.internal:$APPROVAL_NOTIFICATION_CONTROLLER_PORT/approval/webhook
ensure_env MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_URL http://host.docker.internal:$APPROVAL_NOTIFICATION_CONTROLLER_PORT/approval-notification/deployment/validate
ensure_env MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_TOKEN $APPROVAL_NOTIFICATION_CONTROLLER_TOKEN
ensure_env MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_URL http://host.docker.internal:$APPROVAL_NOTIFICATION_CONTROLLER_PORT/approval-notification/ops/validate
ensure_env MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_TOKEN $APPROVAL_NOTIFICATION_CONTROLLER_TOKEN
ensure_env MANDOFORGE_USAGE_EXPORT_SCHEDULE true
ensure_env MANDOFORGE_USAGE_EXPORT_WEBHOOK_URL http://host.docker.internal:$FINANCE_CONTROLLER_PORT/finance/export
ensure_env MANDOFORGE_FINANCE_CLOSE_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_FINANCE_CLOSE_CONTROLLER_URL http://host.docker.internal:$FINANCE_CONTROLLER_PORT/finance/close
ensure_env MANDOFORGE_FINANCE_CLOSE_CONTROLLER_TOKEN $FINANCE_CLOSE_CONTROLLER_TOKEN
ensure_env MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_REQUIRED true
ensure_env MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_URL http://host.docker.internal:$FINANCE_CONTROLLER_PORT/finance/reconcile
ensure_env MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_TOKEN $FINANCE_RECONCILIATION_CONTROLLER_TOKEN"

ssh "$REMOTE_HOST" "set -euo pipefail
docker_gateway_ip=\$(ip -4 addr show docker0 2>/dev/null | awk '/inet /{print \$2}' | cut -d/ -f1 | head -1)
if [[ -z \"\$docker_gateway_ip\" ]]; then
  echo 'docker0 gateway IP is required for container-to-host Codex App Server wiring' >&2
  exit 1
fi
if ! ss -ltn | awk '{print \$4}' | grep -q \"\$docker_gateway_ip:$CODEX_WS_PORT$\"; then
  command -v codex >/dev/null 2>&1 || { echo 'codex CLI is required for Whiskey Codex App Server WS target' >&2; exit 1; }
  nohup codex app-server --listen ws://\$docker_gateway_ip:$CODEX_WS_PORT > '$REMOTE_ROOT/codex-app-server-ws.log' 2>&1 &
  echo \$! > '$REMOTE_ROOT/codex-app-server-ws.pid'
  sleep 2
fi
ss -ltn | awk '{print \$4}' | grep -q \"\$docker_gateway_ip:$CODEX_WS_PORT$\" || { cat '$REMOTE_ROOT/codex-app-server-ws.log' >&2; exit 1; }
if ! ss -ltn | awk '{print \$4}' | grep -q \"\$docker_gateway_ip:$CODEX_CONTROLLER_PORT$\"; then
  command -v node >/dev/null 2>&1 || { echo 'node is required for Whiskey Codex App Server controller' >&2; exit 1; }
  nohup env CODEX_APP_SERVER_WS_URL=ws://\$docker_gateway_ip:$CODEX_WS_PORT CODEX_CONTROLLER_HOST=\$docker_gateway_ip CODEX_CONTROLLER_PORT=$CODEX_CONTROLLER_PORT CODEX_CONTROLLER_TOKEN='$CODEX_CONTROLLER_TOKEN' node '$REMOTE_CODEX_CONTROLLER' > '$REMOTE_ROOT/codex-app-server-controller.log' 2>&1 &
  echo \$! > '$REMOTE_ROOT/codex-app-server-controller.pid'
  sleep 2
fi
ss -ltn | awk '{print \$4}' | grep -q \"\$docker_gateway_ip:$CODEX_CONTROLLER_PORT$\" || { cat '$REMOTE_ROOT/codex-app-server-controller.log' >&2; exit 1; }
if [[ -f '$REMOTE_ROOT/tenant-routing-controller.pid' ]]; then
  kill \$(cat '$REMOTE_ROOT/tenant-routing-controller.pid') >/dev/null 2>&1 || true
  rm -f '$REMOTE_ROOT/tenant-routing-controller.pid'
  sleep 1
fi
command -v node >/dev/null 2>&1 || { echo 'node is required for Whiskey tenant routing controller' >&2; exit 1; }
nohup env TENANT_ROUTING_CONTROLLER_API_URL=http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787} TENANT_ROUTING_CONTROLLER_HOST=\$docker_gateway_ip TENANT_ROUTING_CONTROLLER_PORT=$TENANT_CONTROLLER_PORT TENANT_ROUTING_CONTROLLER_TOKEN='$TENANT_CONTROLLER_TOKEN' node '$REMOTE_TENANT_CONTROLLER' > '$REMOTE_ROOT/tenant-routing-controller.log' 2>&1 &
echo \$! > '$REMOTE_ROOT/tenant-routing-controller.pid'
sleep 2
ss -ltn | awk '{print \$4}' | grep -q \"\$docker_gateway_ip:$TENANT_CONTROLLER_PORT$\" || { cat '$REMOTE_ROOT/tenant-routing-controller.log' >&2; exit 1; }
if [[ -f '$REMOTE_ROOT/worker-load-controller.pid' ]]; then
  kill \$(cat '$REMOTE_ROOT/worker-load-controller.pid') >/dev/null 2>&1 || true
  rm -f '$REMOTE_ROOT/worker-load-controller.pid'
  sleep 1
fi
command -v node >/dev/null 2>&1 || { echo 'node is required for Whiskey worker load controller' >&2; exit 1; }
nohup env WORKER_LOAD_CONTROLLER_API_URL=http://127.0.0.1:\${MANDOFORGE_API_HOST_PORT:-18787} WORKER_LOAD_CONTROLLER_HOST=\$docker_gateway_ip WORKER_LOAD_CONTROLLER_PORT=$WORKER_CONTROLLER_PORT WORKER_LOAD_CONTROLLER_TOKEN='$WORKER_CONTROLLER_TOKEN' WORKER_LOAD_CONTROLLER_COMPOSE_FILE='$REMOTE_COMPOSE' WORKER_LOAD_CONTROLLER_COMPOSE_PROJECT='$COMPOSE_PROJECT' node '$REMOTE_WORKER_CONTROLLER' > '$REMOTE_ROOT/worker-load-controller.log' 2>&1 &
echo \$! > '$REMOTE_ROOT/worker-load-controller.pid'
sleep 2
ss -ltn | awk '{print \$4}' | grep -q \"\$docker_gateway_ip:$WORKER_CONTROLLER_PORT$\" || { cat '$REMOTE_ROOT/worker-load-controller.log' >&2; exit 1; }
if [[ -f '$REMOTE_ROOT/mcp-pilot-controller.pid' ]]; then
  kill \$(cat '$REMOTE_ROOT/mcp-pilot-controller.pid') >/dev/null 2>&1 || true
  rm -f '$REMOTE_ROOT/mcp-pilot-controller.pid'
  sleep 1
fi
command -v node >/dev/null 2>&1 || { echo 'node is required for Whiskey MCP pilot controller' >&2; exit 1; }
github_token=""
if command -v gh >/dev/null 2>&1; then
  github_token="$(gh auth token 2>/dev/null || true)"
fi
lark_open_id="$MCP_LARK_USER_OPEN_ID"
if [[ '$MCP_UPSTREAM_MODE' == 'lark_chat_messages' ]]; then
  command -v lark-cli >/dev/null 2>&1 || { echo 'lark-cli is required for Whiskey Lark MCP source' >&2; exit 1; }
  command -v jq >/dev/null 2>&1 || { echo 'jq is required for Whiskey Lark MCP source autodiscovery' >&2; exit 1; }
  if [[ -z "\$lark_open_id" && -z '$MCP_LARK_CHAT_ID' ]]; then
    lark_open_id=\"\$(lark-cli auth status 2>/dev/null | jq -r '.userOpenId // empty' || true)\"
  fi
  if [[ -z "\$lark_open_id" && -z '$MCP_LARK_CHAT_ID' ]]; then
    lark_open_id=\"\$(lark-cli contact +get-user --format json 2>/dev/null | jq -r '.data.user.open_id // empty' || true)\"
  fi
  [[ -n "\$lark_open_id" || -n '$MCP_LARK_CHAT_ID' ]] || { echo 'could not resolve Whiskey Lark user/chat target for MCP source' >&2; exit 1; }
fi
if [[ '$MCP_UPSTREAM_MODE' == 'lark_docs_search' ]]; then
  command -v lark-cli >/dev/null 2>&1 || { echo 'lark-cli is required for Whiskey Lark docs MCP source' >&2; exit 1; }
fi
nohup env MCP_PILOT_CONTROLLER_HOST=\$docker_gateway_ip MCP_PILOT_CONTROLLER_PORT=$MCP_CONTROLLER_PORT MCP_PILOT_CONTROLLER_TOKEN='$MCP_CONTROLLER_TOKEN' MCP_PILOT_SERVER_NAME='$MCP_SERVER_NAME' MCP_PILOT_UPSTREAM_MODE='$MCP_UPSTREAM_MODE' MCP_PILOT_GITHUB_API_URL=https://api.github.com/search/repositories MCP_PILOT_GITHUB_REPO_API_URL=https://api.github.com/repos MCP_PILOT_GITHUB_REPO_OWNER='$MCP_GITHUB_REPO_OWNER' MCP_PILOT_GITHUB_REPO_NAME='$MCP_GITHUB_REPO_NAME' MCP_PILOT_GITHUB_REPO_REF='$MCP_GITHUB_REPO_REF' MCP_PILOT_GITHUB_REPO_LIMIT='$MCP_GITHUB_REPO_LIMIT' MCP_PILOT_GITHUB_TOKEN=\"\$github_token\" MCP_PILOT_LARK_AS='$MCP_LARK_AS' MCP_PILOT_LARK_USER_OPEN_ID=\"\$lark_open_id\" MCP_PILOT_LARK_CHAT_ID='$MCP_LARK_CHAT_ID' MCP_PILOT_LARK_MESSAGE_LIMIT='$MCP_LARK_MESSAGE_LIMIT' MCP_PILOT_LARK_DOCS_PAGE_SIZE='$MCP_LARK_DOCS_PAGE_SIZE' node '$REMOTE_MCP_CONTROLLER' > '$REMOTE_ROOT/mcp-pilot-controller.log' 2>&1 &
echo \$! > '$REMOTE_ROOT/mcp-pilot-controller.pid'
sleep 2
ss -ltn | awk '{print \$4}' | grep -q \"\$docker_gateway_ip:$MCP_CONTROLLER_PORT$\" || { cat '$REMOTE_ROOT/mcp-pilot-controller.log' >&2; exit 1; }
if [[ -f '$REMOTE_ROOT/eval-release-controller.pid' ]]; then
  kill \$(cat '$REMOTE_ROOT/eval-release-controller.pid') >/dev/null 2>&1 || true
  rm -f '$REMOTE_ROOT/eval-release-controller.pid'
  sleep 1
fi
command -v node >/dev/null 2>&1 || { echo 'node is required for Whiskey eval/release controller' >&2; exit 1; }
nohup env EVAL_RELEASE_CONTROLLER_HOST=\$docker_gateway_ip EVAL_RELEASE_CONTROLLER_PORT=$EVAL_RELEASE_CONTROLLER_PORT EVAL_RELEASE_CONTROLLER_TOKEN='$EVAL_RELEASE_CONTROLLER_TOKEN' EVAL_RELEASE_CONTROLLER_ENVIRONMENT='$EVAL_RELEASE_ENVIRONMENT' node '$REMOTE_EVAL_RELEASE_CONTROLLER' > '$REMOTE_ROOT/eval-release-controller.log' 2>&1 &
echo \$! > '$REMOTE_ROOT/eval-release-controller.pid'
sleep 2
ss -ltn | awk '{print \$4}' | grep -q \"\$docker_gateway_ip:$EVAL_RELEASE_CONTROLLER_PORT$\" || { cat '$REMOTE_ROOT/eval-release-controller.log' >&2; exit 1; }
if [[ -f '$REMOTE_ROOT/observability-controller.pid' ]]; then
  kill \$(cat '$REMOTE_ROOT/observability-controller.pid') >/dev/null 2>&1 || true
  rm -f '$REMOTE_ROOT/observability-controller.pid'
  sleep 1
fi
command -v node >/dev/null 2>&1 || { echo 'node is required for Whiskey observability controller' >&2; exit 1; }
nohup env OBSERVABILITY_CONTROLLER_HOST=\$docker_gateway_ip OBSERVABILITY_CONTROLLER_PORT=$OBSERVABILITY_CONTROLLER_PORT OBSERVABILITY_CONTROLLER_TOKEN='$OBSERVABILITY_CONTROLLER_TOKEN' OBSERVABILITY_CONTROLLER_SERVICE_NAME='$OBSERVABILITY_SERVICE_NAME' node '$REMOTE_OBSERVABILITY_CONTROLLER' > '$REMOTE_ROOT/observability-controller.log' 2>&1 &
echo \$! > '$REMOTE_ROOT/observability-controller.pid'
sleep 2
ss -ltn | awk '{print \$4}' | grep -q \"\$docker_gateway_ip:$OBSERVABILITY_CONTROLLER_PORT$\" || { cat '$REMOTE_ROOT/observability-controller.log' >&2; exit 1; }
if [[ -f '$REMOTE_ROOT/provider-rollout-controller.pid' ]]; then
  kill \$(cat '$REMOTE_ROOT/provider-rollout-controller.pid') >/dev/null 2>&1 || true
  rm -f '$REMOTE_ROOT/provider-rollout-controller.pid'
  sleep 1
fi
command -v node >/dev/null 2>&1 || { echo 'node is required for Whiskey provider rollout controller' >&2; exit 1; }
nohup env PROVIDER_ROLLOUT_CONTROLLER_HOST=\$docker_gateway_ip PROVIDER_ROLLOUT_CONTROLLER_PORT=$PROVIDER_CONTROLLER_PORT PROVIDER_ROLLOUT_CONTROLLER_TOKEN='$PROVIDER_CONTROLLER_TOKEN' PROVIDER_ROLLOUT_CONTROLLER_ENVIRONMENT='$PROVIDER_ROLLOUT_ENVIRONMENT' node '$REMOTE_PROVIDER_CONTROLLER' > '$REMOTE_ROOT/provider-rollout-controller.log' 2>&1 &
echo \$! > '$REMOTE_ROOT/provider-rollout-controller.pid'
sleep 2
ss -ltn | awk '{print \$4}' | grep -q \"\$docker_gateway_ip:$PROVIDER_CONTROLLER_PORT$\" || { cat '$REMOTE_ROOT/provider-rollout-controller.log' >&2; exit 1; }
if [[ -f '$REMOTE_ROOT/approval-notification-controller.pid' ]]; then
  kill \$(cat '$REMOTE_ROOT/approval-notification-controller.pid') >/dev/null 2>&1 || true
  rm -f '$REMOTE_ROOT/approval-notification-controller.pid'
  sleep 1
fi
command -v node >/dev/null 2>&1 || { echo 'node is required for Whiskey approval notification controller' >&2; exit 1; }
lark_open_id="$APPROVAL_NOTIFICATION_LARK_OPEN_ID"
if [[ '$APPROVAL_NOTIFICATION_DELIVERY_MODE' == 'lark_im' ]]; then
  command -v lark-cli >/dev/null 2>&1 || { echo 'lark-cli is required for Whiskey Lark approval notification delivery' >&2; exit 1; }
  command -v jq >/dev/null 2>&1 || { echo 'jq is required for Whiskey Lark approval notification delivery autodiscovery' >&2; exit 1; }
  if [[ -z "\$lark_open_id" ]]; then
    lark_open_id=\"\$(lark-cli auth status 2>/dev/null | jq -r '.userOpenId // empty' || true)\"
  fi
  if [[ -z "\$lark_open_id" ]]; then
    lark_open_id=\"\$(lark-cli contact +get-user --format json 2>/dev/null | jq -r '.data.user.open_id // empty' || true)\"
  fi
  [[ -n "\$lark_open_id" ]] || { echo 'could not resolve Whiskey Lark open_id for approval notification delivery' >&2; exit 1; }
fi
nohup env APPROVAL_NOTIFICATION_CONTROLLER_HOST=\$docker_gateway_ip APPROVAL_NOTIFICATION_CONTROLLER_PORT=$APPROVAL_NOTIFICATION_CONTROLLER_PORT APPROVAL_NOTIFICATION_CONTROLLER_TOKEN='$APPROVAL_NOTIFICATION_CONTROLLER_TOKEN' APPROVAL_NOTIFICATION_DELIVERY_MODE='$APPROVAL_NOTIFICATION_DELIVERY_MODE' APPROVAL_NOTIFICATION_LARK_AS='$APPROVAL_NOTIFICATION_LARK_AS' APPROVAL_NOTIFICATION_LARK_OPEN_ID="\$lark_open_id" node '$REMOTE_APPROVAL_NOTIFICATION_CONTROLLER' > '$REMOTE_ROOT/approval-notification-controller.log' 2>&1 &
echo \$! > '$REMOTE_ROOT/approval-notification-controller.pid'
sleep 2
ss -ltn | awk '{print \$4}' | grep -q \"\$docker_gateway_ip:$APPROVAL_NOTIFICATION_CONTROLLER_PORT$\" || { cat '$REMOTE_ROOT/approval-notification-controller.log' >&2; exit 1; }
if [[ -f '$REMOTE_ROOT/vault-kms-controller.pid' ]]; then
  kill \$(cat '$REMOTE_ROOT/vault-kms-controller.pid') >/dev/null 2>&1 || true
  rm -f '$REMOTE_ROOT/vault-kms-controller.pid'
  sleep 1
fi
command -v node >/dev/null 2>&1 || { echo 'node is required for Whiskey Vault/KMS controller' >&2; exit 1; }
nohup env VAULT_KMS_CONTROLLER_HOST=\$docker_gateway_ip VAULT_KMS_CONTROLLER_PORT=$VAULT_KMS_CONTROLLER_PORT VAULT_KMS_CONTROLLER_TOKEN='$VAULT_KMS_CONTROLLER_TOKEN' VAULT_KMS_CONTROLLER_VAULT_TOKEN='$VAULT_KMS_VAULT_TOKEN' VAULT_KMS_CONTROLLER_PROVIDER='$VAULT_KMS_PROVIDER' VAULT_KMS_CONTROLLER_KEY_ID='$VAULT_KMS_KEY_ID' VAULT_KMS_CONTROLLER_ROTATION_POLICY='$VAULT_KMS_ROTATION_POLICY' node '$REMOTE_VAULT_KMS_CONTROLLER' > '$REMOTE_ROOT/vault-kms-controller.log' 2>&1 &
echo \$! > '$REMOTE_ROOT/vault-kms-controller.pid'
sleep 2
ss -ltn | awk '{print \$4}' | grep -q \"\$docker_gateway_ip:$VAULT_KMS_CONTROLLER_PORT$\" || { cat '$REMOTE_ROOT/vault-kms-controller.log' >&2; exit 1; }
if [[ -f '$REMOTE_ROOT/finance-controller.pid' ]]; then
  kill \$(cat '$REMOTE_ROOT/finance-controller.pid') >/dev/null 2>&1 || true
  rm -f '$REMOTE_ROOT/finance-controller.pid'
  sleep 1
fi
command -v node >/dev/null 2>&1 || { echo 'node is required for Whiskey finance controller' >&2; exit 1; }
if [[ '$FINANCE_EXPORT_DELIVERY_MODE' == 'lark_drive' ]]; then
  command -v lark-cli >/dev/null 2>&1 || { echo 'lark-cli is required for Whiskey Lark finance export delivery' >&2; exit 1; }
fi
nohup env FINANCE_CONTROLLER_HOST=\$docker_gateway_ip FINANCE_CONTROLLER_PORT=$FINANCE_CONTROLLER_PORT FINANCE_CLOSE_CONTROLLER_TOKEN='$FINANCE_CLOSE_CONTROLLER_TOKEN' FINANCE_RECONCILIATION_CONTROLLER_TOKEN='$FINANCE_RECONCILIATION_CONTROLLER_TOKEN' FINANCE_EXPORT_DELIVERY_MODE='$FINANCE_EXPORT_DELIVERY_MODE' FINANCE_EXPORT_LARK_AS='$FINANCE_EXPORT_LARK_AS' FINANCE_EXPORT_LARK_FOLDER_TOKEN='$FINANCE_EXPORT_LARK_FOLDER_TOKEN' node '$REMOTE_FINANCE_CONTROLLER' > '$REMOTE_ROOT/finance-controller.log' 2>&1 &
echo \$! > '$REMOTE_ROOT/finance-controller.pid'
sleep 2
ss -ltn | awk '{print \$4}' | grep -q \"\$docker_gateway_ip:$FINANCE_CONTROLLER_PORT$\" || { cat '$REMOTE_ROOT/finance-controller.log' >&2; exit 1; }"

remote_cmd="cd '$REMOTE_ROOT' && set -a && source '$REMOTE_ENV' && set +a"
if [[ "$PULL_IMAGE" == "1" ]]; then
  remote_cmd="$remote_cmd && docker compose -p '$COMPOSE_PROJECT' -f '$REMOTE_COMPOSE' pull"
fi
remote_cmd="$remote_cmd && docker compose -p '$COMPOSE_PROJECT' -f '$REMOTE_COMPOSE' up -d postgres otel-collector && docker volume create '${COMPOSE_PROJECT}_workspace-data' >/dev/null && docker run --rm -u 0 -v '${COMPOSE_PROJECT}_workspace-data:/data' debian:trixie-slim sh -c 'chown -R 1000:1000 /data' && docker compose -p '$COMPOSE_PROJECT' -f '$REMOTE_COMPOSE' up -d --force-recreate api worker && docker compose -p '$COMPOSE_PROJECT' -f '$REMOTE_COMPOSE' ps"

ssh "$REMOTE_HOST" "bash -lc $(printf '%q' "$remote_cmd")"

echo "Whiskey MandoForge pilot is deployed on $REMOTE_HOST at http://127.0.0.1:18787"
