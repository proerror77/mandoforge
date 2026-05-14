#!/usr/bin/env bash
set -euo pipefail

if ! command -v kubectl >/dev/null 2>&1; then
  echo "kubectl is required to verify Stage 2 evidence manifests" >&2
  exit 1
fi

manifests=(
  deploy/stage2-evidence/stage2-controller-env-secret.example.yaml
  deploy/stage2-evidence/stage2-evidence-gate-job.yaml
  deploy/stage2-evidence/stage2-production-evidence-pvc.example.yaml
  deploy/stage2-evidence/stage2-production-evidence-gate-job.example.yaml
  deploy/stage2-evidence/stage2-completion-audit-job.example.yaml
  deploy/stage2-evidence/observability-collector-evidence-job.example.yaml
  deploy/stage2-evidence/remote-computer-evidence-job.example.yaml
  deploy/stage2-evidence/provider-governance-evidence-job.example.yaml
  deploy/stage2-evidence/tenant-isolation-evidence-job.example.yaml
  deploy/stage2-evidence/vault-evidence-job.example.yaml
  deploy/stage2-evidence/approval-notification-evidence-job.example.yaml
  deploy/stage2-evidence/worker-evidence-job.example.yaml
  deploy/stage2-evidence/scheduler-evidence-job.example.yaml
  deploy/stage2-evidence/policy-rollout-evidence-job.example.yaml
  deploy/stage2-evidence/codex-app-server-evidence-job.example.yaml
  deploy/stage2-evidence/mcp-gateway-evidence-job.example.yaml
  deploy/stage2-evidence/eval-release-evidence-job.example.yaml
  deploy/stage2-evidence/finance-evidence-job.example.yaml
)
archive_script="scripts/archive-stage2-production-evidence.sh"
observability_script="scripts/observability-collector-evidence-gate.sh"
remote_computer_script="scripts/remote-computer-evidence-gate.sh"
provider_script="scripts/provider-governance-evidence-gate.sh"
tenant_script="scripts/tenant-isolation-evidence-gate.sh"
vault_script="scripts/vault-evidence-gate.sh"
approval_notification_script="scripts/approval-notification-evidence-gate.sh"
worker_script="scripts/worker-evidence-gate.sh"
scheduler_script="scripts/scheduler-evidence-gate.sh"
policy_rollout_script="scripts/policy-rollout-evidence-gate.sh"
codex_app_server_script="scripts/codex-app-server-evidence-gate.sh"
mcp_gateway_script="scripts/mcp-gateway-evidence-gate.sh"
eval_release_script="scripts/eval-release-evidence-gate.sh"
finance_script="scripts/finance-evidence-gate.sh"
completion_audit_script="scripts/stage2-completion-audit-gate.sh"
worker_isolated_pool_manifests=(
  deploy/k8s/worker-isolated-pool.yaml
  deploy/k8s/worker-isolated-pool-networkpolicy.yaml
  deploy/k8s/worker-isolated-pool-keda.yaml
)

for manifest in "${manifests[@]}"; do
  if [[ ! -f "$manifest" ]]; then
    echo "missing Stage 2 evidence manifest: $manifest" >&2
    exit 1
  fi
done

for manifest in "${worker_isolated_pool_manifests[@]}"; do
  if [[ ! -f "$manifest" ]]; then
    echo "missing isolated worker-pool manifest: $manifest" >&2
    exit 1
  fi
done

if [[ ! -x "$archive_script" ]]; then
  echo "missing executable Stage 2 production evidence archive script: $archive_script" >&2
  exit 1
fi

if [[ ! -x "$observability_script" ]]; then
  echo "missing executable observability collector evidence script: $observability_script" >&2
  exit 1
fi

if [[ ! -x "$remote_computer_script" ]]; then
  echo "missing executable Remote Computer evidence script: $remote_computer_script" >&2
  exit 1
fi

if [[ ! -x "$provider_script" ]]; then
  echo "missing provider governance evidence script: $provider_script" >&2
  exit 1
fi

if [[ ! -x "$tenant_script" ]]; then
  echo "missing tenant isolation evidence script: $tenant_script" >&2
  exit 1
fi

if [[ ! -x "$vault_script" ]]; then
  echo "missing Vault evidence script: $vault_script" >&2
  exit 1
fi

if [[ ! -x "$approval_notification_script" ]]; then
  echo "missing approval notification evidence script: $approval_notification_script" >&2
  exit 1
fi

if [[ ! -x "$worker_script" ]]; then
  echo "missing worker evidence script: $worker_script" >&2
  exit 1
fi

if [[ ! -x "$scheduler_script" ]]; then
  echo "missing scheduler evidence script: $scheduler_script" >&2
  exit 1
fi

if [[ ! -x "$policy_rollout_script" ]]; then
  echo "missing policy rollout evidence script: $policy_rollout_script" >&2
  exit 1
fi

if [[ ! -x "$codex_app_server_script" ]]; then
  echo "missing Codex App Server evidence script: $codex_app_server_script" >&2
  exit 1
fi

if [[ ! -x "$mcp_gateway_script" ]]; then
  echo "missing MCP Gateway evidence script: $mcp_gateway_script" >&2
  exit 1
fi

if [[ ! -x "$eval_release_script" ]]; then
  echo "missing eval/release evidence script: $eval_release_script" >&2
  exit 1
fi

if [[ ! -x "$finance_script" ]]; then
  echo "missing finance evidence script: $finance_script" >&2
  exit 1
fi

if [[ ! -x "$completion_audit_script" ]]; then
  echo "missing Stage 2 completion audit script: $completion_audit_script" >&2
  exit 1
fi

kubectl kustomize deploy/stage2-evidence >/tmp/mandoforge-stage2-evidence-kustomize.out
kubectl kustomize deploy/stage2-production-evidence --load-restrictor LoadRestrictionsNone \
  >/tmp/mandoforge-stage2-production-evidence-kustomize.out
kubectl kustomize deploy/k8s >/tmp/mandoforge-deploy-kustomize.out

if [[ ! -s /tmp/mandoforge-stage2-evidence-kustomize.out ]]; then
  echo "Stage 2 evidence kustomize render produced no output" >&2
  exit 1
fi

if [[ ! -s /tmp/mandoforge-stage2-production-evidence-kustomize.out ]]; then
  echo "Stage 2 production evidence kustomize render produced no output" >&2
  exit 1
fi

if [[ ! -s /tmp/mandoforge-deploy-kustomize.out ]]; then
  echo "deploy/k8s kustomize render produced no output" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-worker-isolated" /tmp/mandoforge-deploy-kustomize.out; then
  echo "deploy/k8s render is missing isolated worker-pool Deployment/NetworkPolicy" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-worker-isolated-queue-depth" /tmp/mandoforge-deploy-kustomize.out; then
  echo "deploy/k8s render is missing isolated worker-pool KEDA ScaledObject" >&2
  exit 1
fi

if ! grep -q "kind: Job" /tmp/mandoforge-stage2-evidence-kustomize.out; then
  echo "Stage 2 evidence kustomize render is missing a Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-stage2-production-evidence-gate" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the strict production Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-stage2-completion-audit" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the completion audit Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-observability-collector-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the observability collector evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-remote-computer-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the Remote Computer evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-provider-governance-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the provider governance evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-tenant-isolation-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the tenant isolation evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-vault-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the Vault evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-approval-notification-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the approval notification evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-worker-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the worker evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-scheduler-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the scheduler evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-policy-rollout-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the policy rollout evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-codex-app-server-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the Codex App Server evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-mcp-gateway-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the MCP Gateway evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-eval-release-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the eval/release evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-finance-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the finance evidence Job" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the persistent evidence PVC mount" >&2
  exit 1
fi

if ! grep -q "kind: Secret" deploy/stage2-evidence/stage2-controller-env-secret.example.yaml; then
  echo "Stage 2 controller env example is not a Kubernetes Secret" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-stage2-controller-env" deploy/stage2-evidence/stage2-controller-env-secret.example.yaml; then
  echo "Stage 2 controller env Secret example has the wrong name" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-stage2-production-evidence-gate" deploy/stage2-evidence/stage2-production-evidence-gate-job.example.yaml; then
  echo "Stage 2 production evidence Job example has the wrong name" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-stage2-production-evidence" deploy/stage2-evidence/stage2-production-evidence-pvc.example.yaml; then
  echo "Stage 2 production evidence PVC example has the wrong name" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-stage2-controller-env" deploy/stage2-evidence/stage2-production-evidence-gate-job.example.yaml; then
  echo "Stage 2 production evidence Job example does not consume the controller env Secret" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/stage2-production-evidence-gate-job.example.yaml; then
  echo "Stage 2 production evidence Job example does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "stage2-completion-audit-gate.sh" deploy/stage2-evidence/stage2-completion-audit-job.example.yaml; then
  echo "Stage 2 completion audit Job does not run the completion audit gate" >&2
  exit 1
fi

if ! grep -q "SOURCE_EVIDENCE_DIR" deploy/stage2-evidence/stage2-completion-audit-job.example.yaml; then
  echo "Stage 2 completion audit Job does not point at the shared evidence directory" >&2
  exit 1
fi

if ! grep -q "AUDIT_DIR" deploy/stage2-evidence/stage2-completion-audit-job.example.yaml; then
  echo "Stage 2 completion audit Job does not configure an output audit directory" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/stage2-completion-audit-job.example.yaml; then
  echo "Stage 2 completion audit Job does not persist the checklist to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "observability-collector-evidence-gate.sh" deploy/stage2-evidence/observability-collector-evidence-job.example.yaml; then
  echo "Observability collector evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/observability-collector-evidence-job.example.yaml; then
  echo "Observability collector evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/observability/collector/cluster/validate" "$observability_script"; then
  echo "Observability collector evidence script must validate cluster rollout" >&2
  exit 1
fi

if ! grep -q "observability-collector-deployment-evidence.json" "$observability_script"; then
  echo "Observability collector evidence script must write explicit deployment evidence metadata" >&2
  exit 1
fi

if ! grep -q "observability-collector-deployment-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit observability collector deployment evidence metadata" >&2
  exit 1
fi

if ! grep -q "observability-collector-cluster-rollout-evidence.json" "$observability_script"; then
  echo "Observability collector evidence script must write explicit cluster rollout evidence metadata" >&2
  exit 1
fi

if ! grep -q "observability-collector-cluster-rollout-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit observability collector cluster rollout evidence metadata" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_OBSERVABILITY_REMEDIATION" "$observability_script"; then
  echo "Observability collector evidence script must support optional remediation evidence capture" >&2
  exit 1
fi

if ! grep -q "observability-collector-remediation-evidence.json" "$observability_script"; then
  echo "Observability collector evidence script must write explicit remediation evidence metadata" >&2
  exit 1
fi

if ! grep -q "observability-collector-remediation-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit observability collector remediation evidence metadata" >&2
  exit 1
fi

if ! grep -q "remote-computer-evidence-gate.sh" deploy/stage2-evidence/remote-computer-evidence-job.example.yaml; then
  echo "Remote Computer evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/remote-computer-evidence-job.example.yaml; then
  echo "Remote Computer evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/remote-computers/state-sync/validate" "$remote_computer_script"; then
  echo "Remote Computer evidence script must validate production state sync" >&2
  exit 1
fi

if ! grep -q "remote-computer-state-sync-evidence.json" "$remote_computer_script"; then
  echo "Remote Computer evidence script must write explicit state-sync evidence metadata" >&2
  exit 1
fi

if ! grep -q "remote-computer-state-sync-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit Remote Computer state-sync evidence metadata" >&2
  exit 1
fi

if ! grep -q "/api/remote-computers/sidecars/recovery/run" "$remote_computer_script"; then
  echo "Remote Computer evidence script must capture sidecar recovery evidence" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_REMOTE_SIDECAR_RECOVERY" "$remote_computer_script"; then
  echo "Remote Computer evidence script must support optional sidecar recovery evidence capture" >&2
  exit 1
fi

if ! grep -q "remote-computer-sidecar-recovery-evidence.json" "$remote_computer_script"; then
  echo "Remote Computer evidence script must write explicit sidecar recovery evidence metadata" >&2
  exit 1
fi

if ! grep -q "remote-computer-sidecar-recovery-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit Remote Computer sidecar recovery evidence metadata" >&2
  exit 1
fi

if ! grep -q "provider-governance-evidence-gate.sh" deploy/stage2-evidence/provider-governance-evidence-job.example.yaml; then
  echo "Provider governance evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/provider-governance-evidence-job.example.yaml; then
  echo "Provider governance evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/providers/deployment/validate" "$provider_script"; then
  echo "Provider governance evidence script must validate provider deployment" >&2
  exit 1
fi

if ! grep -q "/api/providers/production-rollout/run" "$provider_script"; then
  echo "Provider governance evidence script must capture provider rollout evidence" >&2
  exit 1
fi

if ! grep -q "provider-production-rollout-evidence.json" "$provider_script"; then
  echo "Provider governance evidence script must write explicit provider rollout evidence metadata" >&2
  exit 1
fi

if ! grep -q "provider-production-rollout-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit provider rollout evidence metadata" >&2
  exit 1
fi

if ! grep -q "provider-production-rollback-evidence.json" "$provider_script"; then
  echo "Provider governance evidence script must write explicit provider rollback evidence metadata" >&2
  exit 1
fi

if ! grep -q "provider-production-rollback-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit provider rollback evidence metadata" >&2
  exit 1
fi

if ! grep -q "tenant-isolation-evidence-gate.sh" deploy/stage2-evidence/tenant-isolation-evidence-job.example.yaml; then
  echo "Tenant isolation evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/tenant-isolation-evidence-job.example.yaml; then
  echo "Tenant isolation evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/tenant-isolation/routing/validate" "$tenant_script"; then
  echo "Tenant isolation evidence script must validate tenant production routing" >&2
  exit 1
fi

if ! grep -q "vault-evidence-gate.sh" deploy/stage2-evidence/vault-evidence-job.example.yaml; then
  echo "Vault evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/vault-evidence-job.example.yaml; then
  echo "Vault evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/vault/kms/recovery/validate" "$vault_script"; then
  echo "Vault evidence script must validate KMS recovery readiness" >&2
  exit 1
fi

if ! grep -q "vault-kms-recovery-evidence.json" "$vault_script"; then
  echo "Vault evidence script must write explicit KMS recovery evidence metadata" >&2
  exit 1
fi

if ! grep -q "vault-kms-recovery-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit KMS recovery evidence metadata" >&2
  exit 1
fi

if ! grep -q "/api/vault/kms/rotation/run" "$vault_script"; then
  echo "Vault evidence script must capture KMS rotation evidence" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_SECRET_LIFECYCLE" "$vault_script"; then
  echo "Vault evidence script must support optional secret lifecycle evidence capture" >&2
  exit 1
fi

if ! grep -q "vault-kms-rotation-evidence.json" "$vault_script"; then
  echo "Vault evidence script must write explicit KMS rotation evidence metadata" >&2
  exit 1
fi

if ! grep -q "vault-kms-rotation-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit KMS rotation evidence metadata" >&2
  exit 1
fi

if ! grep -q "approval-notification-evidence-gate.sh" deploy/stage2-evidence/approval-notification-evidence-job.example.yaml; then
  echo "Approval notification evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/approval-notification-evidence-job.example.yaml; then
  echo "Approval notification evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/approvals/notifications/deployment/validate" "$approval_notification_script"; then
  echo "Approval notification evidence script must validate deployment readiness" >&2
  exit 1
fi

if ! grep -q "/api/approvals/notifications/ops/validate" "$approval_notification_script"; then
  echo "Approval notification evidence script must validate production ops readiness" >&2
  exit 1
fi

if ! grep -q "/api/approvals/notifications/run" "$approval_notification_script"; then
  echo "Approval notification evidence script must capture delivery evidence" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_APPROVAL_DELIVERY" "$approval_notification_script"; then
  echo "Approval notification evidence script must support optional delivery evidence capture" >&2
  exit 1
fi

if ! grep -q "approval-notification-delivery-evidence.json" "$approval_notification_script"; then
  echo "Approval notification evidence script must write explicit delivery evidence metadata" >&2
  exit 1
fi

if ! grep -q "approval-notification-delivery-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit approval notification delivery evidence metadata" >&2
  exit 1
fi

if ! grep -q "worker-evidence-gate.sh" deploy/stage2-evidence/worker-evidence-job.example.yaml; then
  echo "Worker evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/worker-evidence-job.example.yaml; then
  echo "Worker evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/execution-jobs/worker-readiness" "$worker_script"; then
  echo "Worker evidence script must collect worker readiness" >&2
  exit 1
fi

if ! grep -q "/api/execution-jobs/worker-load-validation/run" "$worker_script"; then
  echo "Worker evidence script must validate worker load readiness" >&2
  exit 1
fi

if ! grep -q "worker-load-validation-evidence.json" "$worker_script"; then
  echo "Worker evidence script must write explicit load-validation evidence metadata" >&2
  exit 1
fi

if ! grep -q "worker-load-validation-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit worker load-validation evidence metadata" >&2
  exit 1
fi

if ! grep -q "scheduler-evidence-gate.sh" deploy/stage2-evidence/scheduler-evidence-job.example.yaml; then
  echo "Scheduler evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/scheduler-evidence-job.example.yaml; then
  echo "Scheduler evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/scheduler/summary" "$scheduler_script"; then
  echo "Scheduler evidence script must collect orchestration summary" >&2
  exit 1
fi

if ! grep -q "/api/scheduler/due-plan" "$scheduler_script"; then
  echo "Scheduler evidence script must collect due-plan evidence" >&2
  exit 1
fi

if ! grep -q "/api/scheduler/run-due" "$scheduler_script"; then
  echo "Scheduler evidence script must capture run-due evidence" >&2
  exit 1
fi

if ! grep -q "x-mandoforge-scheduler-token" "$scheduler_script"; then
  echo "Scheduler evidence script must support shared-token authentication" >&2
  exit 1
fi

if ! grep -q "/api/scheduler/due-plan" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must collect scheduler due-plan evidence" >&2
  exit 1
fi

if ! grep -q "/api/scheduler/run-due" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must capture scheduler run-due evidence when validations run" >&2
  exit 1
fi

if ! grep -q "x-mandoforge-scheduler-token" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must support scheduler shared-token authentication" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_UI_ACTIONBOOK" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must support optional static UI Actionbook evidence" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_UI_STATIC_ASSETS" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must support optional browserless static UI asset evidence" >&2
  exit 1
fi

if ! grep -q "/api/organizations" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must auto-discover a team for team-scoped MCP evidence" >&2
  exit 1
fi

if ! grep -q "team-discovery.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must persist team discovery evidence" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_MCP_ROLLBACK" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must support optional MCP rollback evidence capture" >&2
  exit 1
fi

if ! grep -q "mcp-rollback-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must persist MCP rollback evidence metadata" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_EVAL_RELEASE_ROLLBACK" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must support optional eval/release rollback evidence capture" >&2
  exit 1
fi

if ! grep -q "eval-release-rollback-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must persist eval/release rollback evidence metadata" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_FINANCE_EXPORT" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must support optional finance export evidence capture" >&2
  exit 1
fi

if ! grep -q "usage-export-csv-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must persist finance export CSV metadata" >&2
  exit 1
fi

if ! grep -q "local-script-" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit gate must map local script evidence artifacts" >&2
  exit 1
fi

if ! grep -q "team-discovery.json" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit gate must reuse team discovery evidence for team-scoped endpoints" >&2
  exit 1
fi

if ! grep -q "required_evidence_artifacts_for_requirement" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit gate must map requirements to explicit evidence metadata artifacts" >&2
  exit 1
fi

if ! grep -q "missing_required_evidence_artifact_count" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit gate must report missing explicit evidence metadata artifacts" >&2
  exit 1
fi

if ! grep -q "policy-rollout-evidence-gate.sh" deploy/stage2-evidence/policy-rollout-evidence-job.example.yaml; then
  echo "Policy rollout evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/policy-rollout-evidence-job.example.yaml; then
  echo "Policy rollout evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/policy/rollout/orchestration/readiness" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must collect orchestration readiness" >&2
  exit 1
fi

if ! grep -q "/api/policy/rollout/orchestration/validate" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must validate rollout orchestration" >&2
  exit 1
fi

if ! grep -q "/api/policy/rollout/run-due" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must capture due-run evidence" >&2
  exit 1
fi

if ! grep -q "policy-rollout-orchestration-validation-evidence.json" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must write explicit orchestration validation evidence metadata" >&2
  exit 1
fi

if ! grep -q "policy-rollout-orchestration-validation-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit policy rollout orchestration validation evidence metadata" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_POLICY_DUE_RUN" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must support optional due-run evidence capture" >&2
  exit 1
fi

if ! grep -q "policy-rollout-due-run-evidence.json" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must write explicit due-run evidence metadata" >&2
  exit 1
fi

if ! grep -q "policy-rollout-due-run-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit policy rollout due-run evidence metadata" >&2
  exit 1
fi

if ! grep -q "codex-app-server-evidence-gate.sh" deploy/stage2-evidence/codex-app-server-evidence-job.example.yaml; then
  echo "Codex App Server evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/codex-app-server-evidence-job.example.yaml; then
  echo "Codex App Server evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/codex-app-server/control-plane/summary" "$codex_app_server_script"; then
  echo "Codex App Server evidence script must collect control-plane summary" >&2
  exit 1
fi

if ! grep -q "/api/codex-app-server/deployment/validate" "$codex_app_server_script"; then
  echo "Codex App Server evidence script must validate deployment readiness" >&2
  exit 1
fi

if ! grep -q "/api/codex-app-server/ops/validate" "$codex_app_server_script"; then
  echo "Codex App Server evidence script must validate ops readiness" >&2
  exit 1
fi

if ! grep -q "/api/codex-app-server/runs/poll-stale" "$codex_app_server_script"; then
  echo "Codex App Server evidence script must capture stale-poll evidence" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_CODEX_STALE_POLL" "$codex_app_server_script"; then
  echo "Codex App Server evidence script must support optional stale-poll evidence capture" >&2
  exit 1
fi

if ! grep -q "codex-app-server-stale-poll-evidence.json" "$codex_app_server_script"; then
  echo "Codex App Server evidence script must write explicit stale-poll evidence metadata" >&2
  exit 1
fi

if ! grep -q "codex-app-server-stale-poll-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit Codex App Server stale-poll evidence metadata" >&2
  exit 1
fi

if ! grep -q "mcp-gateway-evidence-gate.sh" deploy/stage2-evidence/mcp-gateway-evidence-job.example.yaml; then
  echo "MCP Gateway evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/mcp-gateway-evidence-job.example.yaml; then
  echo "MCP Gateway evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_MCP_ROLLBACK" deploy/stage2-evidence/mcp-gateway-evidence-job.example.yaml; then
  echo "MCP Gateway evidence Job does not explicitly enable rollback evidence capture" >&2
  exit 1
fi

if ! grep -q '/api/teams/\$TEAM_ID/mcp-servers/rollouts/summary' "$mcp_gateway_script"; then
  echo "MCP Gateway evidence script must collect rollout summary" >&2
  exit 1
fi

if ! grep -q '/api/teams/\$TEAM_ID/mcp-servers/deployment/validate' "$mcp_gateway_script"; then
  echo "MCP Gateway evidence script must validate connector deployment readiness" >&2
  exit 1
fi

if ! grep -q '/api/teams/\$TEAM_ID/mcp-servers/rollouts/run-due' "$mcp_gateway_script"; then
  echo "MCP Gateway evidence script must capture due-run supervision evidence" >&2
  exit 1
fi

if ! grep -q "mcp-deployment-validation-evidence.json" "$mcp_gateway_script"; then
  echo "MCP Gateway evidence script must write explicit deployment validation evidence metadata" >&2
  exit 1
fi

if ! grep -q "mcp-deployment-validation-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit MCP deployment validation evidence metadata" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_MCP_DUE_RUN" "$mcp_gateway_script"; then
  echo "MCP Gateway evidence script must support optional due-run evidence capture" >&2
  exit 1
fi

if ! grep -q "mcp-rollout-due-run-evidence.json" "$mcp_gateway_script"; then
  echo "MCP Gateway evidence script must write explicit due-run evidence metadata" >&2
  exit 1
fi

if ! grep -q "mcp-rollout-due-run-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit MCP due-run evidence metadata" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_MCP_ROLLBACK" "$mcp_gateway_script"; then
  echo "MCP Gateway evidence script must support optional rollback evidence capture" >&2
  exit 1
fi

if ! grep -q "mcp-rollback-evidence.json" "$mcp_gateway_script"; then
  echo "MCP Gateway evidence script must persist rollback evidence metadata" >&2
  exit 1
fi

if ! grep -q "/api/organizations" "$mcp_gateway_script"; then
  echo "MCP Gateway evidence script must auto-discover a team when MANDOFORGE_STAGE2_TEAM_ID is absent" >&2
  exit 1
fi

if ! grep -q "team-discovery.json" "$mcp_gateway_script"; then
  echo "MCP Gateway evidence script must persist team discovery evidence" >&2
  exit 1
fi

if ! grep -q "eval-release-evidence-gate.sh" deploy/stage2-evidence/eval-release-evidence-job.example.yaml; then
  echo "eval/release evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/eval-release-evidence-job.example.yaml; then
  echo "eval/release evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_EVAL_RELEASE_ROLLBACK" deploy/stage2-evidence/eval-release-evidence-job.example.yaml; then
  echo "eval/release evidence Job does not explicitly enable rollback evidence capture" >&2
  exit 1
fi

if ! grep -q "/api/agents/releases/automation-runs" "$eval_release_script"; then
  echo "eval/release evidence script must collect release automation history" >&2
  exit 1
fi

if ! grep -q "/api/agents/releases/deployment/validate" "$eval_release_script"; then
  echo "eval/release evidence script must validate release deployment readiness" >&2
  exit 1
fi

if ! grep -q "/api/agents/releases/orchestration/validate" "$eval_release_script"; then
  echo "eval/release evidence script must validate release orchestration readiness" >&2
  exit 1
fi

if ! grep -q "/api/eval/suites/stage2-regression" "$eval_release_script"; then
  echo "eval/release evidence script must capture Stage 2 regression suite evidence" >&2
  exit 1
fi

if ! grep -q "eval-release-deployment-validation-evidence.json" "$eval_release_script"; then
  echo "eval/release evidence script must write explicit deployment validation evidence metadata" >&2
  exit 1
fi

if ! grep -q "eval-release-deployment-validation-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit eval/release deployment validation evidence metadata" >&2
  exit 1
fi

if ! grep -q "eval-release-orchestration-validation-evidence.json" "$eval_release_script"; then
  echo "eval/release evidence script must write explicit orchestration validation evidence metadata" >&2
  exit 1
fi

if ! grep -q "eval-release-orchestration-validation-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit eval/release orchestration validation evidence metadata" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_EVAL_RELEASE_AUTOMATION" "$eval_release_script"; then
  echo "eval/release evidence script must support optional automation evidence capture" >&2
  exit 1
fi

if ! grep -q "eval-release-stage2-regression-evidence.json" "$eval_release_script"; then
  echo "eval/release evidence script must write explicit Stage 2 regression evidence metadata" >&2
  exit 1
fi

if ! grep -q "eval-release-stage2-regression-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit eval/release regression evidence metadata" >&2
  exit 1
fi

if ! grep -q "eval-release-due-run-evidence.json" "$eval_release_script"; then
  echo "eval/release evidence script must write explicit release due-run evidence metadata" >&2
  exit 1
fi

if ! grep -q "eval-release-due-run-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit eval/release due-run evidence metadata" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_EVAL_RELEASE_ROLLBACK" "$eval_release_script"; then
  echo "eval/release evidence script must support optional rollback evidence capture" >&2
  exit 1
fi

if ! grep -q "eval-release-rollback-evidence.json" "$eval_release_script"; then
  echo "eval/release evidence script must persist rollback evidence metadata" >&2
  exit 1
fi

if ! grep -q "finance-evidence-gate.sh" deploy/stage2-evidence/finance-evidence-job.example.yaml; then
  echo "finance evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/finance-evidence-job.example.yaml; then
  echo "finance evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_FINANCE_EXPORT" deploy/stage2-evidence/finance-evidence-job.example.yaml; then
  echo "finance evidence Job does not explicitly enable export evidence capture" >&2
  exit 1
fi

if ! grep -q "/api/usage/finance-operations/summary" "$finance_script"; then
  echo "finance evidence script must collect finance operations summary" >&2
  exit 1
fi

if ! grep -q "/api/usage/finance-operations/run" "$finance_script"; then
  echo "finance evidence script must capture finance close evidence" >&2
  exit 1
fi

if ! grep -q "/api/usage/finance-operations/reconcile" "$finance_script"; then
  echo "finance evidence script must capture accounting reconciliation evidence" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_FINANCE_EXPORT" "$finance_script"; then
  echo "finance evidence script must support optional finance export evidence capture" >&2
  exit 1
fi

if ! grep -q "/api/usage/export.csv" "$finance_script"; then
  echo "finance evidence script must capture CSV export evidence" >&2
  exit 1
fi

if ! grep -q "/api/usage/export/deliver" "$finance_script"; then
  echo "finance evidence script must capture export delivery evidence" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_FINANCE_CONTROLLERS" "$finance_script"; then
  echo "finance evidence script must support optional finance controller evidence capture" >&2
  exit 1
fi

if ! grep -q "finance-close-evidence.json" "$finance_script"; then
  echo "finance evidence script must write explicit close evidence metadata" >&2
  exit 1
fi

if ! grep -q "finance-close-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit finance close evidence metadata" >&2
  exit 1
fi

if ! grep -q "finance-reconciliation-evidence.json" "$finance_script"; then
  echo "finance evidence script must write explicit reconciliation evidence metadata" >&2
  exit 1
fi

if ! grep -q "finance-reconciliation-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit finance reconciliation evidence metadata" >&2
  exit 1
fi

if ! grep -q "finance-export-delivery-evidence.json" "$finance_script"; then
  echo "finance evidence script must write explicit export delivery evidence metadata" >&2
  exit 1
fi

if ! grep -q "finance-export-delivery-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit finance export delivery evidence metadata" >&2
  exit 1
fi

if ! grep -q "MANDOFORGE_STAGE2_EVIDENCE_PVC:-mandoforge-stage2-production-evidence" "$archive_script"; then
  echo "Stage 2 production evidence archive script does not default to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "readOnly: true" "$archive_script"; then
  echo "Stage 2 production evidence archive script must mount the evidence PVC read-only" >&2
  exit 1
fi

if ! grep -q "archive_sha256" "$archive_script"; then
  echo "Stage 2 production evidence archive script must write checksum metadata" >&2
  exit 1
fi

if ! grep -q "manifest_file=" "$archive_script"; then
  echo "Stage 2 production evidence archive script must write a release manifest" >&2
  exit 1
fi

if ! grep -q "COPY deploy ./deploy" Dockerfile; then
  echo "Runtime image must package deploy metadata for in-cluster completion audits" >&2
  exit 1
fi

if ! grep -q "missing_evidence_script_count" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must report missing evidence scripts" >&2
  exit 1
fi

if ! grep -q "missing_evidence_job_manifest_count" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must report missing evidence Job manifests" >&2
  exit 1
fi

if ! grep -q "missing_required_flag_count" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must report missing required controller flags" >&2
  exit 1
fi

echo "stage2 evidence k8s manifests ok"
