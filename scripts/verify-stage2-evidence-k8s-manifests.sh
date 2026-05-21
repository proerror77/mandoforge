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
  deploy/stage2-evidence/worker-remote-computer-evidence-job.example.yaml
  deploy/stage2-evidence/provider-governance-evidence-job.example.yaml
  deploy/stage2-evidence/tenant-isolation-evidence-job.example.yaml
  deploy/stage2-evidence/vault-evidence-job.example.yaml
  deploy/stage2-evidence/approval-notification-evidence-job.example.yaml
  deploy/stage2-evidence/worker-evidence-job.example.yaml
  deploy/stage2-evidence/scheduler-evidence-job.example.yaml
  deploy/stage2-evidence/policy-rollout-evidence-job.example.yaml
  deploy/stage2-evidence/codex-app-server-evidence-job.example.yaml
  deploy/stage2-evidence/managed-session-runtime-evidence-job.example.yaml
  deploy/stage2-evidence/mcp-gateway-evidence-job.example.yaml
  deploy/stage2-evidence/eval-release-evidence-job.example.yaml
  deploy/stage2-evidence/finance-evidence-job.example.yaml
)
archive_script="scripts/archive-stage2-production-evidence.sh"
observability_script="scripts/observability-collector-evidence-gate.sh"
remote_computer_script="scripts/remote-computer-evidence-gate.sh"
worker_remote_computer_script="scripts/worker-remote-computer-evidence-gate.sh"
provider_script="scripts/provider-governance-evidence-gate.sh"
tenant_script="scripts/tenant-isolation-evidence-gate.sh"
vault_script="scripts/vault-evidence-gate.sh"
approval_notification_script="scripts/approval-notification-evidence-gate.sh"
worker_script="scripts/worker-evidence-gate.sh"
scheduler_script="scripts/scheduler-evidence-gate.sh"
policy_rollout_script="scripts/policy-rollout-evidence-gate.sh"
codex_app_server_script="scripts/codex-app-server-evidence-gate.sh"
managed_session_runtime_script="scripts/managed-session-runtime-evidence-gate.sh"
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

if [[ ! -x "$worker_remote_computer_script" ]]; then
  echo "missing executable worker/Remote Computer evidence script: $worker_remote_computer_script" >&2
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

if [[ ! -x "$managed_session_runtime_script" ]]; then
  echo "missing managed-session runtime evidence script: $managed_session_runtime_script" >&2
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

if ! grep -q "MANDOFORGE_TENANT_ID" /tmp/mandoforge-deploy-kustomize.out; then
  echo "deploy/k8s render is missing configurable runtime tenant id" >&2
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

if ! grep -q "name: mandoforge-worker-remote-computer-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the worker/Remote Computer evidence Job" >&2
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

if ! grep -q "runner_ready" "$remote_computer_script"; then
  echo "Remote Computer evidence script must fail closed when runner readiness is not configured" >&2
  exit 1
fi

if ! grep -q "state_sync_checked_path_count" "$remote_computer_script" || ! grep -q "state_sync_checked_path_detail_count" "$remote_computer_script" || ! grep -q "state_sync_state_claim" "$remote_computer_script" || ! grep -q "persistent_volume_claim" "$remote_computer_script" || ! grep -q "passed.*validated.*completed.*ready.*exists.*mounted.*available.*ok.*healthy.*accessible.*readable.*writable" "$remote_computer_script"; then
  echo "Remote Computer evidence script must require audited state claim and checked state contract path detail evidence" >&2
  exit 1
fi

if ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*validated_at.*timestamp" "$remote_computer_script"; then
  echo "Remote Computer evidence script must require audited checked state path and sidecar Pod details" >&2
  exit 1
fi

if ! grep -q "is_real_cluster_kind" "$remote_computer_script" || ! grep -q "state_sync_node_count" "$remote_computer_script" || ! grep -q "state_sync_cluster_id" "$remote_computer_script"; then
  echo "Remote Computer evidence script must require real multi-node state-sync cluster evidence" >&2
  exit 1
fi

if ! grep -q "MANDOFORGE_STAGE2_PRODUCTION_CLUSTER_ID" "$remote_computer_script" || ! grep -q "expected_production_cluster_id" "$remote_computer_script"; then
  echo "Remote Computer evidence script must bind state-sync and sidecar evidence to the declared production cluster id" >&2
  exit 1
fi

if ! grep -q "is_distributed_state_backend" "$remote_computer_script" || ! grep -q "state_sync_backend" "$remote_computer_script"; then
  echo "Remote Computer evidence script must require distributed state backend evidence" >&2
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

if ! grep -q "sidecar_replacement_pods_healthy" "$remote_computer_script" || ! grep -q "sidecar_checked_pod_count" "$remote_computer_script" || ! grep -q "sidecar_checked_pod_detail_count" "$remote_computer_script"; then
  echo "Remote Computer evidence script must require healthy replacement Pod evidence and checked Pod details" >&2
  exit 1
fi

if ! grep -q "state_sync_cluster_id.*target_cluster_id" "$remote_computer_script" || ! grep -q "sidecar_cluster_id.*target_cluster_id" "$remote_computer_script"; then
  echo "Remote Computer evidence script must bind state path and sidecar Pod details to their cluster ids" >&2
  exit 1
fi

if ! grep -q "sidecar_target_kind" "$remote_computer_script" || ! grep -q "sidecar_node_count" "$remote_computer_script" || ! grep -q "sidecar_replacement_scope" "$remote_computer_script"; then
  echo "Remote Computer evidence script must require real cluster-wide sidecar replacement evidence" >&2
  exit 1
fi

if ! grep -q "remote-computer-sidecar-recovery-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit Remote Computer sidecar recovery evidence metadata" >&2
  exit 1
fi

if ! grep -q "worker-remote-computer-evidence-gate.sh" deploy/stage2-evidence/worker-remote-computer-evidence-job.example.yaml; then
  echo "Worker/Remote Computer evidence Job does not run the combined evidence gate" >&2
  exit 1
fi

if ! grep -q "worker-remote-computer-evidence-gate.sh" crates/mandoforge-api/src/main.rs; then
  echo "Stage 2 readiness must require the combined worker/Remote Computer evidence script" >&2
  exit 1
fi

if ! grep -q "worker-remote-computer-evidence-job.example.yaml" crates/mandoforge-api/src/main.rs; then
  echo "Stage 2 readiness must require the combined worker/Remote Computer evidence Job manifest" >&2
  exit 1
fi

if ! grep -q "worker-remote-computer/summary.json" crates/mandoforge-api/src/main.rs; then
  echo "Stage 2 readiness must require combined worker/Remote Computer summary evidence" >&2
  exit 1
fi

if ! grep -q "worker-remote-computer/summary.json" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must require combined worker/Remote Computer summary evidence" >&2
  exit 1
fi

if ! grep -q "worker-remote-computer/summary.json" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must require combined worker/Remote Computer summary evidence" >&2
  exit 1
fi

if ! grep -q "capture_worker_remote_computer_combined_validation" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must capture combined worker/Remote Computer evidence" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_REMOTE_SIDECAR_RECOVERY" deploy/stage2-evidence/worker-remote-computer-evidence-job.example.yaml; then
  echo "Worker/Remote Computer evidence Job must force sidecar recovery evidence capture" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/worker-remote-computer-evidence-job.example.yaml; then
  echo "Worker/Remote Computer evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "worker-evidence-gate.sh" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must run the worker evidence gate" >&2
  exit 1
fi

if ! grep -q "remote-computer-evidence-gate.sh" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must run the Remote Computer evidence gate" >&2
  exit 1
fi

if ! grep -q "isolated_worker_pool_configured" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must verify isolated worker-pool evidence" >&2
  exit 1
fi

if ! grep -q "load_validated" "$worker_remote_computer_script" || ! grep -q "worker_load_check_detail_count" "$worker_remote_computer_script" || ! grep -q "worker_load_checks" "$worker_remote_computer_script" || ! grep -q "root_worker_pool" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must verify and summarize worker-pool-bound load validation detail evidence" >&2
  exit 1
fi

if ! grep -q "worker_cluster_id.*target_cluster_id" "$worker_remote_computer_script" || ! grep -q "state_sync_cluster_id.*target_cluster_id" "$worker_remote_computer_script" || ! grep -q "sidecar_cluster_id.*target_cluster_id" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must bind load-check, state-path, and sidecar Pod details to the shared cluster ids" >&2
  exit 1
fi

if ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*executed_at.*validated_at.*timestamp" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must require audited worker load-check details" >&2
  exit 1
fi

if ! grep -q "sidecar_recovery_required" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must require sidecar recovery evidence" >&2
  exit 1
fi

if ! grep -q "state_sync_evidence_status" "$worker_remote_computer_script" || ! grep -q "sidecar_recovery_evidence_status" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must require captured state-sync and sidecar evidence wrappers" >&2
  exit 1
fi

if ! grep -q "same_cluster_target" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must require worker and Remote Computer evidence from the same cluster" >&2
  exit 1
fi

if ! grep -q "MANDOFORGE_STAGE2_PRODUCTION_CLUSTER_ID" "$worker_remote_computer_script" || ! grep -q "expected_production_cluster_id" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must bind combined evidence to the declared production cluster id" >&2
  exit 1
fi

if ! grep -q "pilot/mock/local" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must reject pilot/mock/local cluster ids" >&2
  exit 1
fi

if ! grep -q "is_real_cluster_kind" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must distinguish real cluster evidence from local/single-host evidence" >&2
  exit 1
fi

if ! grep -q "is_distributed_state_backend" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must require distributed state backend evidence" >&2
  exit 1
fi

if ! grep -q "juicefs|cephfs|longhorn-rwx" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must allow only supported distributed state backends" >&2
  exit 1
fi

if ! grep -q "state_checked_path_count" "$worker_remote_computer_script" || ! grep -q "state_checked_path_detail_count" "$worker_remote_computer_script" || ! grep -q "sidecar_checked_pod_detail_count" "$worker_remote_computer_script" || ! grep -q "persistent_volume_claim" "$worker_remote_computer_script" || ! grep -q "passed.*validated.*completed.*ready.*exists.*mounted.*available.*ok.*healthy.*accessible.*readable.*writable" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must require checked state path and sidecar Pod details" >&2
  exit 1
fi

if ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*validated_at.*timestamp" "$worker_remote_computer_script"; then
  echo "Worker/Remote Computer evidence script must require audited checked state path and sidecar Pod details" >&2
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

if ! grep -q "tenant-routing-validation-evidence.json" "$tenant_script"; then
  echo "Tenant isolation evidence script must write explicit routing validation evidence metadata" >&2
  exit 1
fi

if ! grep -q "is_multi_tenant_target_kind" "$tenant_script"; then
  echo "Tenant isolation evidence script must distinguish broader multi-tenant deployment targets" >&2
  exit 1
fi

if ! grep -q "routing_tenant_count" "$tenant_script"; then
  echo "Tenant isolation evidence script must require reported tenant count evidence" >&2
  exit 1
fi

if ! grep -q "routing_tenant_sample_count" "$tenant_script" || ! grep -q "routing_unique_tenant_sample_count" "$tenant_script" || ! grep -q "routing_tenant_sample_detail_count" "$tenant_script"; then
  echo "Tenant isolation evidence script must require unique audited tenant sample detail evidence" >&2
  exit 1
fi

if ! grep -q "routing_rls_table_count" "$tenant_script" || ! grep -q "routing_rls_forced_table_count" "$tenant_script" || ! grep -q "routing_forced_rls_table_detail_count" "$tenant_script" || ! grep -q "unique forced-RLS table details" "$tenant_script"; then
  echo "Tenant isolation evidence script must require RLS table counts and unique forced-RLS table detail evidence" >&2
  exit 1
fi

if ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*validated_at.*timestamp" "$tenant_script"; then
  echo "Tenant isolation evidence script must require audited forced-RLS table detail evidence" >&2
  exit 1
fi

if ! grep -q "routing_cross_tenant_negative_tests" "$tenant_script"; then
  echo "Tenant isolation evidence script must require cross-tenant negative-test evidence" >&2
  exit 1
fi

if ! grep -q "routing_cross_tenant_negative_test_count" "$tenant_script" || ! grep -q "routing_cross_tenant_negative_test_detail_count" "$tenant_script" || ! grep -q "sampled tenants" "$tenant_script" || ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*tested_at.*timestamp" "$tenant_script"; then
  echo "Tenant isolation evidence script must require audited cross-tenant negative-test counts and details" >&2
  exit 1
fi

if ! grep -q "tenant_deployment_id.*routing_deployment_id" "$tenant_script"; then
  echo "Tenant isolation evidence script must bind tenant detail evidence to the routing deployment id" >&2
  exit 1
fi

if ! grep -q "routing_deployment_id" "$tenant_script" || ! grep -q "pilot/mock/local" "$tenant_script"; then
  echo "Tenant isolation evidence script must reject pilot/mock/local tenant deployment ids" >&2
  exit 1
fi

if ! grep -q "MANDOFORGE_STAGE2_TENANT_DEPLOYMENT_ID" "$tenant_script" || ! grep -q "expected_tenant_deployment_id" "$tenant_script"; then
  echo "Tenant isolation evidence script must bind routing evidence to the declared tenant deployment id" >&2
  exit 1
fi

if ! grep -q "routing_rls_enforced" "$tenant_script"; then
  echo "Tenant isolation evidence script must require controller-confirmed RLS enforcement" >&2
  exit 1
fi

if ! grep -q "target_kind=.*broader multi-tenant" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must reject tenant routing evidence without broader multi-tenant target identity" >&2
  exit 1
fi

if ! grep -q "deployment_id=.*pilot/mock/local" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must reject pilot/mock/local tenant deployment ids" >&2
  exit 1
fi

if ! grep -q "cross_tenant_negative_tests" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must require tenant cross-tenant negative-test evidence" >&2
  exit 1
fi

if ! grep -q "tenant_sample_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "tenant_sample_detail_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "rls_forced_table_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "deployment_bound_unique_forced_rls_table_detail_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "deployment_bound_sampled_tenant_negative_test_detail_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*tested_at.*timestamp" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must require audited tenant samples, forced-RLS table details, and negative-test details" >&2
  exit 1
fi

if ! grep -q "tenant_deployment_id.*routing_deployment_id" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must bind tenant detail evidence to the routing deployment id" >&2
  exit 1
fi

if ! grep -q "unique_tenant_sample_count" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must require distinct audited tenant samples" >&2
  exit 1
fi

if ! grep -q "tenant-routing-validation-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit tenant routing validation evidence metadata" >&2
  exit 1
fi

if ! grep -q "runtime_tenant_mode.*tenant_routed" "$tenant_script"; then
  echo "Tenant isolation evidence script must fail closed unless runtime tenant mode is tenant_routed" >&2
  exit 1
fi

if ! grep -q "rls.forced" "$tenant_script"; then
  echo "Tenant isolation evidence script must fail closed unless RLS is forced" >&2
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

if ! grep -q "RUN_STAGE2_SECRET_LIFECYCLE.*:-1" "$vault_script"; then
  echo "Vault evidence script must default to KMS rotation evidence capture" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_SECRET_LIFECYCLE" deploy/stage2-evidence/vault-evidence-job.example.yaml; then
  echo "Vault evidence Job must configure secret lifecycle evidence capture" >&2
  exit 1
fi

if ! grep -q 'value: "1"' deploy/stage2-evidence/vault-evidence-job.example.yaml; then
  echo "Vault evidence Job must enable secret lifecycle evidence capture" >&2
  exit 1
fi

if ! grep -q "vault-kms-rotation-evidence.json" "$vault_script"; then
  echo "Vault evidence script must write explicit KMS rotation evidence metadata" >&2
  exit 1
fi

if ! grep -q "external_kms_rotation_confirmed" "$vault_script"; then
  echo "Vault evidence script must require explicit external KMS rotation confirmation action evidence" >&2
  exit 1
fi

if ! grep -q "vault-kms-rotation-evidence.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit KMS rotation evidence metadata" >&2
  exit 1
fi

if ! grep -q "kms_provider.*reserved" "$vault_script"; then
  echo "Vault evidence script must fail closed on reserved KMS provider" >&2
  exit 1
fi

if ! grep -q "rotation_run_status.*validated" "$vault_script"; then
  echo "Vault evidence script must require validated KMS rotation evidence" >&2
  exit 1
fi

if ! grep -q "recovery_controller_validated" "$vault_script"; then
  echo "Vault evidence script must verify validated recovery controller evidence" >&2
  exit 1
fi

if ! grep -q "is_production_kms_provider" "$vault_script"; then
  echo "Vault evidence script must reject mock or pilot KMS providers" >&2
  exit 1
fi

if ! grep -q "rotation_production_backend" "$vault_script"; then
  echo "Vault evidence script must require production rotation backend identity" >&2
  exit 1
fi

if ! grep -q "recovery_controller_production_backend" "$vault_script"; then
  echo "Vault evidence script must require production recovery backend identity" >&2
  exit 1
fi

if ! grep -q "backend_id or key_id is pilot/mock/local" "$vault_script"; then
  echo "Vault evidence script must reject pilot/mock/local backend and key ids" >&2
  exit 1
fi

if ! grep -q "MANDOFORGE_STAGE2_KMS_BACKEND_ID" "$vault_script" || ! grep -q "MANDOFORGE_KMS_KEY_ID" "$vault_script" || ! grep -q "expected_kms_backend_id" "$vault_script" || ! grep -q "expected_kms_key_id" "$vault_script"; then
  echo "Vault evidence script must bind rotation and recovery evidence to the declared KMS backend and key ids" >&2
  exit 1
fi

if ! grep -q "rotation_rotated_count" "$vault_script" || ! grep -q "rotation_catalog_updated_count" "$vault_script" || ! grep -q "rotation_detail_count" "$vault_script" || ! grep -q "root_rotation_id" "$vault_script"; then
  echo "Vault evidence script must require audited KMS rotation id, rotated count, catalog update count, and backend/key/rotation-bound key detail count" >&2
  exit 1
fi

if ! grep -q "rotation_details: Vec<VaultKmsRotationDetail>" crates/mandoforge-api/src/main.rs || ! grep -q "catalog_updated: true" crates/mandoforge-api/src/main.rs || ! grep -q "audit_id: Uuid" crates/mandoforge-api/src/main.rs || ! grep -q "rotated_at: DateTime<Utc>" crates/mandoforge-api/src/main.rs; then
  echo "Vault KMS rotation API must return audited key-level rotation details for production evidence" >&2
  exit 1
fi

if ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*executed_at.*rotated_at.*timestamp" "$vault_script"; then
  echo "Vault evidence script must require KMS rotation key details with audit or timestamp evidence" >&2
  exit 1
fi

if ! grep -q "recovery_id" "$vault_script" || ! grep -q "recovery_target_kind" "$vault_script" || ! grep -q "recovery_step_count" "$vault_script" || ! grep -q "kms_recovery_step_detail_count" "$vault_script" || ! grep -q "kms_backend_id" "$vault_script" || ! grep -q "recovery_run_id" "$vault_script"; then
  echo "Vault evidence script must require audited KMS recovery id, target kind, and recovery step details" >&2
  exit 1
fi

if ! grep -q "bound_recovery_step_detail_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "bound_recovery_step_detail_count" scripts/verify-stage2-evidence-archive.sh; then
  echo "Completion audit and archive verifier must require KMS recovery step details bound to backend, key, and recovery ids" >&2
  exit 1
fi

if ! grep -q "invalid recovery step status count" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must reject failed KMS recovery steps" >&2
  exit 1
fi

if ! grep -q "backend_kind=.*is not production KMS/HSM" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must reject non-production KMS/HSM backend evidence" >&2
  exit 1
fi

if ! grep -q "backend_id or key_id is pilot/mock/local" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must reject pilot/mock/local KMS backend and key ids" >&2
  exit 1
fi

if ! grep -q "rotated_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "catalog_updated_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "rotation_detail_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "root_rotation_id" scripts/stage2-completion-audit-gate.sh || ! grep -q "recovery_target_kind" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must require audited KMS rotation and recovery details bound to backend/key/rotation ids" >&2
  exit 1
fi

if ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*executed_at.*rotated_at.*timestamp" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must require KMS rotation key details with audit or timestamp evidence" >&2
  exit 1
fi

if ! grep -q "KMS rotation evidence_status" scripts/stage2-completion-audit-gate.sh || ! grep -q "KMS recovery evidence_status" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must require captured KMS rotation and recovery evidence" >&2
  exit 1
fi

if ! grep -q "external KMS rotation confirmation action missing" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must require explicit KMS rotation confirmation action evidence" >&2
  exit 1
fi

if ! grep -q "root_rotation_id" scripts/verify-stage2-evidence-archive.sh || ! grep -q "catalog_updated_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "rotation_detail_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "recovery_target_kind" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must require audited KMS rotation and recovery details bound to backend/key/rotation ids" >&2
  exit 1
fi

if ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*executed_at.*rotated_at.*timestamp" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must require KMS rotation key details with audit or timestamp evidence" >&2
  exit 1
fi

if ! grep -q "external KMS rotation confirmation action missing" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must require explicit KMS rotation confirmation action evidence" >&2
  exit 1
fi

if ! grep -q "vault-kms-rotation-evidence.json" scripts/verify-stage2-evidence-archive.sh || ! grep -q "vault-kms-recovery-evidence.json" scripts/verify-stage2-evidence-archive.sh || ! grep -q "evidence_status=%s" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must require captured KMS rotation and recovery evidence" >&2
  exit 1
fi

if ! grep -q "zero KMS catalog update evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject KMS rotation evidence with no catalog updates" >&2
  exit 1
fi

if ! grep -q "missing KMS rotation key detail evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject KMS rotation evidence without key-level detail records" >&2
  exit 1
fi

if ! grep -q "missing KMS rotation audit evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject KMS rotation key details without audit evidence" >&2
  exit 1
fi

if ! grep -q "zero KMS rotated count" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject KMS rotation evidence with no rotated records" >&2
  exit 1
fi

if ! grep -q "missing KMS rotation confirmation action evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject KMS rotation evidence without the confirmation action" >&2
  exit 1
fi

if ! grep -q "non-production KMS backend evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject KMS rotation evidence from non-production backends" >&2
  exit 1
fi

if ! grep -q "missing KMS recovery steps" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject KMS recovery evidence without audited steps" >&2
  exit 1
fi

if ! grep -q "invalid KMS recovery step status evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject failed KMS recovery steps" >&2
  exit 1
fi

if ! grep -q "missing KMS recovery step audit evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject KMS recovery steps without audit details" >&2
  exit 1
fi

if ! grep -q "mismatched KMS recovery step binding evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject KMS recovery steps bound to the wrong backend, key, or recovery id" >&2
  exit 1
fi

if ! grep -q "non-production KMS recovery target evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject non-production KMS recovery target evidence" >&2
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

if ! grep -q "is_real_cluster_kind" "$worker_script" || ! grep -q "load_validation_node_count" "$worker_script" || ! grep -q "load_validation_cluster_id" "$worker_script"; then
  echo "Worker evidence script must require real multi-node cluster load-validation evidence" >&2
  exit 1
fi

if ! grep -q "load_validation_controller_load_validated" "$worker_script" || ! grep -q "load_validation_check_detail_count" "$worker_script" || ! grep -q "load_validation_controller_isolated_worker_pool" "$worker_script" || ! grep -q "root_worker_pool" "$worker_script"; then
  echo "Worker evidence script must require controller-confirmed load detail bound to the isolated worker-pool" >&2
  exit 1
fi

if ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*executed_at.*validated_at.*timestamp" "$worker_script"; then
  echo "Worker evidence script must require audited worker load-check details" >&2
  exit 1
fi

if ! grep -q "worker_cluster_id.*target_cluster_id" "$worker_script"; then
  echo "Worker evidence script must bind load-check detail rows to the controller cluster id" >&2
  exit 1
fi

if ! grep -q "MANDOFORGE_STAGE2_PRODUCTION_CLUSTER_ID" "$worker_script"; then
  echo "Worker evidence script must compare controller evidence with the configured production cluster id" >&2
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

if ! grep -q "/api/scheduler/deployment/validate" "$scheduler_script"; then
  echo "Scheduler evidence script must validate scheduler deployment readiness" >&2
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

if ! grep -q "/api/scheduler/deployment/validate" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must validate scheduler deployment readiness" >&2
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

if ! grep -q "RUN_STAGE2_COMPLETION_AUDIT" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must support archive-ready completion audit capture" >&2
  exit 1
fi

if ! grep -q "STAGE2_EVIDENCE_MAX_AGE_HOURS" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must enforce freshness for endpoint coverage artifacts" >&2
  exit 1
fi

if ! grep -q "validation_stale_endpoint_count" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must report stale validation endpoint coverage" >&2
  exit 1
fi

if ! grep -q "completion-audit/checklist.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must write completion audit checklist evidence" >&2
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

if ! grep -q 'endpoint="${endpoint#./}"' scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit gate must canonicalize ./ local script artifact names" >&2
  exit 1
fi

if ! grep -q 'slugify "${script_path#./}"' scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must write canonical local script artifact names" >&2
  exit 1
fi

if ! grep -q "local_script_artifact_path" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must include local script validations in endpoint coverage" >&2
  exit 1
fi

if ! grep -q "local_validation_endpoint_enabled" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must keep optional local script coverage gated by explicit UI validation flags" >&2
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

if ! grep -q "ui-production-polish)" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit gate must map the UI production-polish requirement id to its evidence artifacts" >&2
  exit 1
fi

if grep -q "ui-production-crud)" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit gate still contains the old UI production CRUD requirement id" >&2
  exit 1
fi

if ! grep -q "missing_required_evidence_artifact_count" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit gate must report missing explicit evidence metadata artifacts" >&2
  exit 1
fi

if ! grep -q "STAGE2_EVIDENCE_MAX_AGE_HOURS" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit gate must enforce freshness for evidence artifacts" >&2
  exit 1
fi

if ! grep -q "stale_required_evidence_artifact_count" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit gate must report stale explicit evidence metadata artifacts" >&2
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

if ! grep -q "RUN_STAGE2_POLICY_DUE_RUN.*:-1" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must default to due-run evidence capture" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_POLICY_DUE_RUN" deploy/stage2-evidence/policy-rollout-evidence-job.example.yaml; then
  echo "Policy rollout evidence Job must configure due-run evidence capture" >&2
  exit 1
fi

if ! grep -q 'value: "1"' deploy/stage2-evidence/policy-rollout-evidence-job.example.yaml; then
  echo "Policy rollout evidence Job must enable due-run evidence capture" >&2
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

if ! grep -q "due_run_scanned_count" "$policy_rollout_script" || ! grep -q "due_run_scan_detail_count" "$policy_rollout_script" || ! grep -q "due_run_checked_at" "$policy_rollout_script" || ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*scanned_at.*timestamp" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must require audited due-run scan count, scan details, and checked_at evidence" >&2
  exit 1
fi

if ! grep -q "controller_required.*true" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must require a production controller target" >&2
  exit 1
fi

if ! grep -q "latest_controller_validated" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must verify validated controller evidence" >&2
  exit 1
fi

if ! grep -q "latest_controller_production_target" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must verify production controller target identity" >&2
  exit 1
fi

if ! grep -q "controller id is pilot/mock/local" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must reject pilot/mock/local controller ids" >&2
  exit 1
fi

if ! grep -q "MANDOFORGE_STAGE2_POLICY_CONTROLLER_ID" "$policy_rollout_script" || ! grep -q "expected_policy_controller_id" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must bind controller evidence to the declared production controller id" >&2
  exit 1
fi

if ! grep -q "production_policy_store" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must require production policy store evidence" >&2
  exit 1
fi

if ! grep -q "controller_rollback_evidence_id" "$policy_rollout_script" || ! grep -q "controller_rollback_audit_evidence" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must require rollback evidence id and audit evidence" >&2
  exit 1
fi

if ! grep -q "controller_policy_store_id" "$policy_rollout_script" || ! grep -q "controller_deployment_id" "$policy_rollout_script" || ! grep -q "controller_step_count" "$policy_rollout_script"; then
  echo "Policy rollout evidence script must require policy store id, deployment id, and audited orchestration steps" >&2
  exit 1
fi

if ! grep -q "policy-rollout-orchestration-validation-evidence.json" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must contract-check policy rollout validation evidence" >&2
  exit 1
fi

if ! grep -q "policy rollout validation evidence_status" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must require captured policy rollout validation evidence" >&2
  exit 1
fi

if ! grep -q "policy-rollout-due-run-evidence.json" scripts/stage2-completion-audit-gate.sh || ! grep -q "scanned_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "scan_detail_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "policy_controller_id" scripts/stage2-completion-audit-gate.sh || ! grep -q "policy_deployment_id" scripts/stage2-completion-audit-gate.sh || ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*scanned_at.*timestamp" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must contract-check policy rollout due-run evidence" >&2
  exit 1
fi

if ! grep -q "scanned_revisions: Vec<PolicyScheduledRolloutScanDetail>" crates/mandoforge-api/src/main.rs || ! grep -q "audit_id: audit_log.id" crates/mandoforge-api/src/main.rs || ! grep -q "latest_policy_rollout_controller_binding" crates/mandoforge-api/src/main.rs; then
  echo "Policy due-run API must return audited per-revision scan details bound to production controller evidence" >&2
  exit 1
fi

if ! grep -q "invalid policy rollout step status count" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must reject failed policy rollout orchestration steps" >&2
  exit 1
fi

if ! grep -q "policy_rollout_step_detail_count" "$policy_rollout_script" || ! grep -q "policy_rollout_step_detail_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "policy_rollout_step_detail_count" scripts/verify-stage2-evidence-archive.sh; then
  echo "Policy rollout evidence gates must require audited orchestration step details" >&2
  exit 1
fi

if ! grep -q "policy_controller_id" "$policy_rollout_script" || ! grep -q "policy_controller_id" scripts/stage2-completion-audit-gate.sh || ! grep -q "policy_controller_id" scripts/verify-stage2-evidence-archive.sh; then
  echo "Policy rollout evidence gates must bind orchestration step details to the controller identity" >&2
  exit 1
fi

if ! grep -q "store_id" "$policy_rollout_script" || ! grep -q "policy_deployment_id" scripts/stage2-completion-audit-gate.sh || ! grep -q "mismatched policy rollout step binding evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Policy rollout evidence gates must bind orchestration step details to the policy store and deployment" >&2
  exit 1
fi

if ! grep -q "policy due-run evidence_status" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must require captured policy due-run evidence" >&2
  exit 1
fi

if ! grep -q "is not production policy controller" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must reject non-production policy controller evidence" >&2
  exit 1
fi

if ! grep -q "policy_store_id" scripts/stage2-completion-audit-gate.sh || ! grep -q "deployment_id" scripts/stage2-completion-audit-gate.sh || ! grep -q "step_count" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must require policy store id, deployment id, and audited orchestration steps" >&2
  exit 1
fi

if ! grep -q "rollback_evidence_id" scripts/stage2-completion-audit-gate.sh || ! grep -q "rollback_audit_evidence" scripts/stage2-completion-audit-gate.sh || ! grep -q "rollback_evidence_id" scripts/verify-stage2-evidence-archive.sh || ! grep -q "rollback_audit_evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Completion audit and archive verifier must require policy rollback evidence id and audit evidence" >&2
  exit 1
fi

if ! grep -q "policy-rollout-due-run-evidence.json" scripts/verify-stage2-evidence-archive.sh || ! grep -q "scanned_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "scan_detail_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "policy_controller_id" scripts/verify-stage2-evidence-archive.sh || ! grep -q "policy_deployment_id" scripts/verify-stage2-evidence-archive.sh || ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*scanned_at.*timestamp" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must contract-check policy rollout due-run evidence" >&2
  exit 1
fi

if ! grep -q "controller_id=.*pilot/mock/local" scripts/stage2-completion-audit-gate.sh; then
  echo "Completion audit gate must reject pilot/mock/local policy controller ids" >&2
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

if ! grep -q "RUN_STAGE2_FINANCE_EXPORT.*:-1" "$finance_script"; then
  echo "finance evidence script must default to finance export evidence capture" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_FINANCE_CONTROLLERS.*:-1" "$finance_script"; then
  echo "finance evidence script must default to finance controller evidence capture" >&2
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

if ! grep -q "finance-export-delivery-observer.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Strict production evidence gate must write explicit finance export delivery observer evidence" >&2
  exit 1
fi

if ! grep -q "finance-export-delivery-observer.json" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must require finance export delivery observer evidence" >&2
  exit 1
fi

if ! grep -q "finance-close-evidence.json" scripts/stage2-completion-audit-gate.sh || ! grep -q "usage_finance_close_controller_executed" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must contract-check finance close controller evidence" >&2
  exit 1
fi

if ! grep -q "finance-reconciliation-evidence.json" scripts/stage2-completion-audit-gate.sh || ! grep -q "reconciliation_id" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must contract-check finance reconciliation evidence" >&2
  exit 1
fi

if ! grep -q "usage-export-csv-evidence.json" scripts/stage2-completion-audit-gate.sh || ! grep -q "finance export CSV byte_count" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must require nonempty finance CSV evidence" >&2
  exit 1
fi

if ! grep -q "finance-export-delivery-evidence.json" scripts/stage2-completion-audit-gate.sh || ! grep -q "finance export delivery byte_count" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must contract-check finance export delivery evidence" >&2
  exit 1
fi

if ! grep -q "true ERP/accounting system identity" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must reject artifact-store finance system ids" >&2
  exit 1
fi

if ! grep -q "FINANCE_EXPORT_DELIVERY_OBSERVER_URL" deploy/stage2-evidence/stage2-production-controllers.env.example || ! grep -q "FINANCE_EXPORT_DELIVERY_OBSERVER_TOKEN" deploy/stage2-evidence/stage2-production-controllers.env.example; then
  echo "Stage 2 controller env template must include finance export delivery observer URL and token" >&2
  exit 1
fi

if ! grep -q "FINANCE_EXPORT_DELIVERY_OBSERVER_URL" deploy/stage2-evidence/stage2-controller-env-secret.example.yaml || ! grep -q "FINANCE_EXPORT_DELIVERY_OBSERVER_TOKEN" deploy/stage2-evidence/stage2-controller-env-secret.example.yaml; then
  echo "Stage 2 controller secret template must include finance export delivery observer URL and token" >&2
  exit 1
fi

if ! grep -q "FINANCE_EXPORT_DELIVERY_OBSERVER_URL" "$finance_script" || ! grep -q "FINANCE_EXPORT_DELIVERY_OBSERVER_TOKEN" "$finance_script"; then
  echo "finance evidence script must support authenticated export delivery observer evidence" >&2
  exit 1
fi

if ! grep -q "finance export delivery observer token is missing" "$finance_script"; then
  echo "finance evidence script must fail closed without an export delivery observer token" >&2
  exit 1
fi

if ! grep -q "FINANCE_EXPORT_DELIVERY_OBSERVER_TOKEN" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must support authenticated finance export delivery observer evidence" >&2
  exit 1
fi

if ! grep -q "finance_export_delivery_mode" "$finance_script"; then
  echo "finance evidence script must report export delivery mode" >&2
  exit 1
fi

if ! grep -q "lark_drive" "$finance_script"; then
  echo "finance evidence script must distinguish Feishu Drive from accounting/ERP targets" >&2
  exit 1
fi

if ! grep -q "latest_reconciliation_reconciled" "$finance_script"; then
  echo "finance evidence script must verify reconciled accounting evidence" >&2
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

if ! grep -q "verify-stage2-evidence-archive.sh" "$archive_script"; then
  echo "Stage 2 production evidence archive script must verify the archive after creation" >&2
  exit 1
fi

if ! grep -q "completion-audit/checklist.json" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must require the completion audit checklist" >&2
  exit 1
fi

if ! grep -q "missing_required_flag_count" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must reject archives with missing required controller flags" >&2
  exit 1
fi

if ! grep -q "stale_required_evidence_artifact_count" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must reject archives with stale required evidence artifacts" >&2
  exit 1
fi

if ! grep -q "verify_semantic_artifacts" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must independently validate production evidence artifact semantics" >&2
  exit 1
fi

if ! grep -q "worker-load-validation-evidence.json" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must inspect worker real-cluster evidence" >&2
  exit 1
fi

if ! grep -q "load_validated" scripts/verify-stage2-evidence-archive.sh || ! grep -q "root_worker_pool" scripts/verify-stage2-evidence-archive.sh || ! grep -q "cluster_bound_summary_worker_load_check_detail_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "cluster_bound_checked_path_detail_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "cluster_bound_sidecar_checked_pod_detail_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "persistent_volume_claim" scripts/verify-stage2-evidence-archive.sh || ! grep -q "passed.*validated.*completed.*ready.*exists.*mounted.*available.*ok.*healthy.*accessible.*readable.*writable" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must require audited worker-pool-bound load, state-contract path detail, and sidecar replacement Pod detail evidence" >&2
  exit 1
fi

if ! grep -q "worker_cluster_id.*target_cluster_id" scripts/verify-stage2-evidence-archive.sh || ! grep -q "state_sync_cluster_id.*target_cluster_id" scripts/verify-stage2-evidence-archive.sh || ! grep -q "sidecar_cluster_id.*target_cluster_id" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must bind worker and Remote Computer detail rows to their cluster ids" >&2
  exit 1
fi

if ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*executed_at.*validated_at.*timestamp" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must require audited worker load-check detail evidence" >&2
  exit 1
fi

if ! grep -q "worker-load-validation-evidence.json evidence_status" scripts/verify-stage2-evidence-archive.sh && ! grep -q "evidence_status=%s" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must require captured worker and Remote Computer evidence wrappers" >&2
  exit 1
fi

if ! grep -q "missing isolated worker pool evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject worker evidence without isolated worker pool proof" >&2
  exit 1
fi

if ! grep -q "missing worker load validation evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject worker evidence without load validation proof" >&2
  exit 1
fi

if ! grep -q "missing worker load check detail evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject worker evidence without load-check details" >&2
  exit 1
fi

if ! grep -q "missing worker load check audit evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject worker load-check details without audit evidence" >&2
  exit 1
fi

if ! grep -q "single-node worker evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject single-node worker evidence" >&2
  exit 1
fi

if ! grep -q "missing Remote Computer state claim evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject Remote Computer state-sync evidence without a state claim" >&2
  exit 1
fi

if ! grep -q "zero Remote Computer state path evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject Remote Computer state-sync evidence without checked state paths" >&2
  exit 1
fi

if ! grep -q "missing Remote Computer checked path detail evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject Remote Computer state-sync evidence without checked path details" >&2
  exit 1
fi

if ! grep -q "mismatched Remote Computer checked path claim evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject Remote Computer checked path details bound to a different state claim" >&2
  exit 1
fi

if ! grep -q "missing Remote Computer checked path status evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject Remote Computer state-sync evidence without checked path statuses" >&2
  exit 1
fi

if ! grep -q "missing Remote Computer checked path audit evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject Remote Computer state-sync evidence without checked path audit details" >&2
  exit 1
fi

if ! grep -q "missing sidecar checked Pod audit evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject sidecar replacement evidence without checked Pod audit details" >&2
  exit 1
fi

if ! grep -q "summary without worker load-check detail evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject combined worker/Remote Computer summaries without worker load-check details" >&2
  exit 1
fi

if ! grep -q "summary detail cluster mismatch" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject combined summaries with detail rows from a different cluster" >&2
  exit 1
fi

if ! grep -q "worker-load-validation-evidence.json" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must inspect worker real-cluster evidence" >&2
  exit 1
fi

if ! grep -q "load_validated" scripts/stage2-completion-audit-gate.sh || ! grep -q "root_worker_pool" scripts/stage2-completion-audit-gate.sh || ! grep -q "cluster_bound_summary_worker_load_check_detail_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "cluster_bound_checked_path_detail_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "cluster_bound_checked_pod_detail_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "persistent_volume_claim" scripts/stage2-completion-audit-gate.sh || ! grep -q "passed.*validated.*completed.*ready.*exists.*mounted.*available.*ok.*healthy.*accessible.*readable.*writable" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must require audited worker-pool-bound load, state-contract path detail, and sidecar replacement Pod detail evidence" >&2
  exit 1
fi

if ! grep -q "worker_cluster_id.*target_cluster_id" scripts/stage2-completion-audit-gate.sh || ! grep -q "state_sync_cluster_id.*target_cluster_id" scripts/stage2-completion-audit-gate.sh || ! grep -q "sidecar_cluster_id.*target_cluster_id" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must bind worker and Remote Computer detail rows to their cluster ids" >&2
  exit 1
fi

if ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*executed_at.*validated_at.*timestamp" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must require audited worker load-check detail evidence" >&2
  exit 1
fi

if ! grep -q "worker evidence_status" scripts/stage2-completion-audit-gate.sh || ! grep -q "state-sync evidence_status" scripts/stage2-completion-audit-gate.sh || ! grep -q "sidecar recovery evidence_status" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must require captured worker and Remote Computer evidence wrappers" >&2
  exit 1
fi

if ! grep -q "production-evidence-run.json" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must require the run identity manifest" >&2
  exit 1
fi

if ! grep -q "tenant-routing-validation-evidence.json" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must require tenant routing validation evidence" >&2
  exit 1
fi

if ! grep -q "tenant routing evidence_status" scripts/stage2-completion-audit-gate.sh || ! grep -q "tenant routing validation_status" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must require captured and validated tenant routing evidence" >&2
  exit 1
fi

if ! grep -q "tenant_sample_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "tenant_sample_detail_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "rls_forced_table_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "deployment_bound_unique_forced_rls_table_detail_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "deployment_bound_sampled_tenant_negative_test_detail_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*checked_at.*tested_at.*timestamp" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive verifier must require audited tenant samples, forced-RLS table details, and negative-test details" >&2
  exit 1
fi

if ! grep -q "tenant_deployment_id.*routing_deployment_id" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive verifier must bind tenant detail evidence to the routing deployment id" >&2
  exit 1
fi

if ! grep -q "unique_tenant_sample_count" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive verifier must require distinct audited tenant samples" >&2
  exit 1
fi

if ! grep -q "incomplete forced-RLS evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject incomplete forced-RLS tenant evidence" >&2
  exit 1
fi

if ! grep -q "missing forced-RLS table detail evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject tenant evidence without forced-RLS table details" >&2
  exit 1
fi

if ! grep -q "duplicate forced-RLS table detail evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject duplicate forced-RLS table details" >&2
  exit 1
fi

if ! grep -q "missing forced-RLS table audit evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject tenant evidence without forced-RLS table audit details" >&2
  exit 1
fi

if ! grep -q "missing cross-tenant negative tests" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject tenant evidence without cross-tenant negative tests" >&2
  exit 1
fi

if ! grep -q "missing cross-tenant negative test detail evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject tenant evidence without cross-tenant negative test details" >&2
  exit 1
fi

if ! grep -q "unsampled cross-tenant negative test evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject cross-tenant negative tests for unsampled tenants" >&2
  exit 1
fi

if ! grep -q "missing cross-tenant negative test audit evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject tenant evidence without cross-tenant negative test audit details" >&2
  exit 1
fi

if ! grep -q "missing tenant context validation evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject tenant evidence without tenant context validation" >&2
  exit 1
fi

if ! grep -q "single-tenant deployment evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject single-tenant deployment evidence" >&2
  exit 1
fi

if ! grep -q "single tenant sample evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject tenant evidence without audited multi-tenant samples" >&2
  exit 1
fi

if ! grep -q "duplicate tenant sample evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject duplicate tenant sample evidence" >&2
  exit 1
fi

if ! grep -q "missing tenant sample audit evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject tenant samples without audit details" >&2
  exit 1
fi

if ! grep -q "evidence_status=%s" scripts/verify-stage2-evidence-archive.sh || ! grep -q "validation_status=%s" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive verifier must require captured and validated tenant routing evidence" >&2
  exit 1
fi

if ! grep -q "policy-rollout-orchestration-validation-evidence.json" scripts/verify-stage2-evidence-archive.sh || ! grep -q "policy-rollout-due-run-evidence.json" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive verifier must inspect policy rollout validation and due-run artifacts" >&2
  exit 1
fi

if ! grep -q "policy rollout without rollback support" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject policy rollout evidence without rollback support" >&2
  exit 1
fi

if ! grep -q "missing policy rollback evidence id" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject policy rollout evidence without rollback evidence id" >&2
  exit 1
fi

if ! grep -q "missing policy rollback audit evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject policy rollout evidence without rollback audit evidence" >&2
  exit 1
fi

if ! grep -q "missing policy rollout steps" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject policy rollout evidence without audited steps" >&2
  exit 1
fi

if ! grep -q "invalid policy rollout step status evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject failed policy rollout orchestration steps" >&2
  exit 1
fi

if ! grep -q "missing policy rollout step audit evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject policy rollout steps without audit details" >&2
  exit 1
fi

if ! grep -q "mismatched policy rollout step binding evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject policy rollout steps bound to the wrong controller, store, or deployment" >&2
  exit 1
fi

if ! grep -q "zero policy due-run scan count" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject policy due-run evidence without scanned revisions" >&2
  exit 1
fi

if ! grep -q "missing policy due-run scan detail evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject policy due-run evidence without scanned revision details" >&2
  exit 1
fi

if ! grep -q "missing policy due-run scan audit evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject policy due-run scanned revisions without audit details" >&2
  exit 1
fi

if ! grep -q "mismatched policy due-run scan binding evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject policy due-run scanned revisions bound to the wrong controller, store, or deployment" >&2
  exit 1
fi

if ! grep -q "missing policy due-run checked_at evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive self-test must reject policy due-run evidence without checked_at" >&2
  exit 1
fi

if ! grep -q "evidence_status=%s" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 evidence archive verifier must require captured policy rollout artifacts" >&2
  exit 1
fi

if ! grep -q "do not share one cluster id" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must reject mixed-cluster worker/Remote Computer evidence" >&2
  exit 1
fi

if ! grep -q "does not match production-evidence-run.json" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must cross-check evidence identities against the run manifest" >&2
  exit 1
fi

if ! grep -q "summary worker cluster id does not match production-evidence-run.json" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must bind combined worker/Remote Computer summary to the run manifest" >&2
  exit 1
fi

if ! grep -q "do not share one cluster id" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must reject mixed-cluster worker/Remote Computer evidence" >&2
  exit 1
fi

if ! grep -q "summary worker evidence cluster id" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must bind combined worker/Remote Computer summary to root evidence artifacts" >&2
  exit 1
fi

if ! grep -q "summary without shared cluster proof" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject combined worker/Remote Computer summaries without shared cluster proof" >&2
  exit 1
fi

if ! grep -q "summary checked path claim mismatch" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject combined worker/Remote Computer summaries with mismatched checked path claims" >&2
  exit 1
fi

if ! grep -q "local Remote Computer state backend evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject local Remote Computer state backend evidence" >&2
  exit 1
fi

if ! grep -q "zero sidecar checked Pod count" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject sidecar replacement evidence without checked Pods" >&2
  exit 1
fi

if ! grep -q "missing sidecar checked Pod detail evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject sidecar replacement evidence without checked Pod details" >&2
  exit 1
fi

if ! grep -q "unhealthy sidecar replacement Pod evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject unhealthy sidecar replacement Pod evidence" >&2
  exit 1
fi

if ! grep -q "pod-scoped sidecar replacement evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject pod-scoped sidecar replacement evidence" >&2
  exit 1
fi

if ! grep -q "production-evidence-run.json" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must write a run identity manifest" >&2
  exit 1
fi

if ! grep -q "production-evidence-run.json" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must require the run identity manifest" >&2
  exit 1
fi

if ! grep -q "managed-session-restart-resume-evidence.json" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must require managed-session restart/resume evidence" >&2
  exit 1
fi

if ! grep -q "managed-session-restart-resume)" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must map managed-session restart/resume to required artifacts" >&2
  exit 1
fi

if ! grep -q "managed-session-restart-resume-evidence.json" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must inspect managed-session restart/resume evidence" >&2
  exit 1
fi

if ! grep -q "RUN_STAGE2_MANAGED_SESSION_RESTART_RESUME" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must know how to run managed-session restart/resume evidence" >&2
  exit 1
fi

if ! grep -q "MANDOFORGE_STAGE2_MANAGED_SESSION_RUNTIME_TARGET_ID" scripts/stage2-production-evidence-gate.sh; then
  echo "Stage 2 production evidence gate must write the managed-session target id into the run manifest" >&2
  exit 1
fi

if ! grep -q "managed-session runtime target id does not match production-evidence-run.json" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must bind managed-session evidence to the run manifest target id" >&2
  exit 1
fi

if ! grep -q "managed-session runtime target id" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must bind managed-session evidence to the declared target id" >&2
  exit 1
fi

if ! grep -q "managed_session_detail_issue" scripts/managed-session-runtime-evidence-gate.sh || ! grep -q "pending_event_seq_start" scripts/managed-session-runtime-evidence-gate.sh || ! grep -q "processed_event_seq_before_restart" scripts/managed-session-runtime-evidence-gate.sh; then
  echo "Managed-session runtime evidence gate must require structured cursor, lineage, lease, and final-message details" >&2
  exit 1
fi

if ! grep -q "managed_session_detail_issue" scripts/stage2-completion-audit-gate.sh || ! grep -q "managed_session_detail_issue" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 audit and archive verifier must reject managed-session evidence without structured restart/resume details" >&2
  exit 1
fi

if ! grep -q "missing managed-session processed cursor evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject managed-session restart/resume evidence without processed cursor preservation" >&2
  exit 1
fi

if ! grep -q "MANDOFORGE_STAGE2_MANAGED_SESSION_RUNTIME_TARGET_ID" scripts/stage2-production-evidence-preflight.sh; then
  echo "Stage 2 preflight must require a managed-session runtime target id" >&2
  exit 1
fi

if ! grep -q "require_production_identity MANDOFORGE_KMS_KEY_ID" scripts/stage2-production-evidence-preflight.sh; then
  echo "Stage 2 preflight must require a production KMS key id" >&2
  exit 1
fi

if ! grep -q "MANAGED_SESSION_RUNTIME_TARGET_ID" scripts/render-stage2-controller-secret.sh; then
  echo "Stage 2 Secret render must reject missing or pilot managed-session runtime target ids" >&2
  exit 1
fi

if ! grep -q "stage2-production-evidence-preflight.sh" scripts/render-stage2-controller-secret.sh; then
  echo "Stage 2 Secret render must run the strict production evidence preflight before rendering" >&2
  exit 1
fi

if ! grep -q "finance ERP system id" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must bind finance evidence to the declared ERP system id" >&2
  exit 1
fi

if ! grep -q "pilot/mock/local identity" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must reject pilot/mock/local target identities" >&2
  exit 1
fi

if ! grep -q "delivery_mode=.* is not accounting/ERP" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must reject non-ERP finance delivery evidence" >&2
  exit 1
fi

if ! grep -q "zero finance delivery count" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject zero-count finance delivery evidence" >&2
  exit 1
fi

if ! grep -q "zero finance export delivery byte evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject zero-byte finance export delivery evidence" >&2
  exit 1
fi

if ! grep -q "unconfigured finance delivery target" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject finance delivery without configured target" >&2
  exit 1
fi

if ! grep -q "finance-close-evidence.json" scripts/verify-stage2-evidence-archive.sh || ! grep -q "usage_finance_close_controller_executed" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must contract-check finance close controller evidence" >&2
  exit 1
fi

if ! grep -q "invalid finance close step status count" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must reject failed finance close controller steps" >&2
  exit 1
fi

if ! grep -q "finance_close_step_detail_count" "$finance_script" || ! grep -q "finance_close_step_detail_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "finance_close_step_detail_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "root_close_id" "$finance_script" || ! grep -q "root_close_id" scripts/stage2-completion-audit-gate.sh || ! grep -q "root_close_id" scripts/verify-stage2-evidence-archive.sh; then
  echo "Finance evidence gates must require audited finance close step details bound to the close id" >&2
  exit 1
fi

if ! grep -q "finance_close_id" "$finance_script" || ! grep -q "finance close_id is not a true ERP/accounting system identity" "$finance_script"; then
  echo "Finance evidence script must require a true ERP/accounting close id" >&2
  exit 1
fi

if ! grep -q "missing finance close controller action evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject finance close evidence without controller action proof" >&2
  exit 1
fi

if ! grep -q "invalid finance close step status evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject failed finance close controller steps" >&2
  exit 1
fi

if ! grep -q "missing finance close step audit evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject finance close steps without audit details" >&2
  exit 1
fi

if ! grep -q "finance-reconciliation-evidence.json" scripts/verify-stage2-evidence-archive.sh || ! grep -q "reconciliation_id" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must contract-check finance reconciliation evidence" >&2
  exit 1
fi

if ! grep -q "finance_reconciliation_check_detail_count" "$finance_script" || ! grep -q "finance_reconciliation_check_detail_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "finance_reconciliation_check_detail_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "root_reconciliation_id" "$finance_script" || ! grep -q "root_reconciliation_id" scripts/stage2-completion-audit-gate.sh || ! grep -q "root_reconciliation_id" scripts/verify-stage2-evidence-archive.sh; then
  echo "Finance evidence gates must require audited reconciliation check details bound to the reconciliation id" >&2
  exit 1
fi

if ! grep -q "finance_reconciliation_id" "$finance_script" || ! grep -q "finance reconciliation_id is not a true ERP/accounting system identity" "$finance_script"; then
  echo "Finance evidence script must require a true ERP/accounting reconciliation id" >&2
  exit 1
fi

if ! grep -q "invalid finance reconciliation check status count" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must reject failed finance reconciliation checks" >&2
  exit 1
fi

if ! grep -q "missing finance reconciliation check evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject finance reconciliation evidence without checks" >&2
  exit 1
fi

if ! grep -q "invalid finance reconciliation check status evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject failed finance reconciliation checks" >&2
  exit 1
fi

if ! grep -q "missing finance reconciliation check audit evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject reconciliation checks without audit details" >&2
  exit 1
fi

if ! grep -q "usage-export-csv-evidence.json" scripts/verify-stage2-evidence-archive.sh || ! grep -q "finance-export-delivery-evidence.json" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must require finance CSV and export delivery evidence" >&2
  exit 1
fi

if ! grep -q "finance_export_delivery_system_id" "$finance_script"; then
  echo "finance evidence script must record the ERP/accounting system id" >&2
  exit 1
fi

if ! grep -q "finance_export_delivery_receipt_count" "$finance_script" || ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*posted_at.*delivered_at.*received_at.*accepted_at.*timestamp" "$finance_script"; then
  echo "finance evidence script must record ERP/accounting delivery receipt details" >&2
  exit 1
fi

if ! grep -q "delivery_id: Uuid" crates/mandoforge-api/src/main.rs || ! grep -q "file_name: String" crates/mandoforge-api/src/main.rs || ! grep -q "export_bytes: usize" crates/mandoforge-api/src/main.rs || ! grep -q "record_count: usize" crates/mandoforge-api/src/main.rs; then
  echo "Finance export delivery API must expose file and receipt binding fields for ERP/accounting evidence" >&2
  exit 1
fi

if ! grep -q "root_system_id" "$finance_script" || ! grep -q "root_system_id" scripts/stage2-completion-audit-gate.sh || ! grep -q "root_system_id" scripts/verify-stage2-evidence-archive.sh; then
  echo "Finance evidence gates must bind ERP/accounting delivery receipts to the observer system id" >&2
  exit 1
fi

if ! grep -q "root_file_name" "$finance_script" || ! grep -q "root_byte_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "root_file_name" scripts/verify-stage2-evidence-archive.sh; then
  echo "Finance evidence gates must bind ERP/accounting delivery receipts to the current export file and byte count" >&2
  exit 1
fi

if ! grep -q "true ERP/accounting system identity" "$finance_script"; then
  echo "finance evidence script must reject artifact-store ERP/accounting system ids" >&2
  exit 1
fi

if ! grep -q "delivery_receipt_count" scripts/stage2-completion-audit-gate.sh || ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*posted_at.*delivered_at.*received_at.*accepted_at.*timestamp" scripts/stage2-completion-audit-gate.sh; then
  echo "Stage 2 completion audit must require ERP/accounting delivery receipt details" >&2
  exit 1
fi

if ! grep -q "delivery_receipt_count" scripts/verify-stage2-evidence-archive.sh || ! grep -q "audit_id.*audit_log_id.*trace_id.*run_id.*posted_at.*delivered_at.*received_at.*accepted_at.*timestamp" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must require ERP/accounting delivery receipt details" >&2
  exit 1
fi

if ! grep -q "missing finance delivery receipt evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject finance delivery without ERP/accounting receipt details" >&2
  exit 1
fi

if ! grep -q "missing finance delivery receipt audit evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject finance delivery receipts without audit or posting details" >&2
  exit 1
fi

if ! grep -q "mismatched finance delivery receipt system evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject delivery receipts for a different ERP/accounting system" >&2
  exit 1
fi

if ! grep -q "mismatched finance delivery receipt export evidence" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier self-test must reject delivery receipts for a different export file" >&2
  exit 1
fi

if ! grep -q "backend_kind=.* is not production KMS/HSM" scripts/verify-stage2-evidence-archive.sh; then
  echo "Stage 2 archive verifier must reject non-production KMS/HSM archive evidence" >&2
  exit 1
fi

if grep -R "completion_blocked // true" scripts/stage2-completion-audit-gate.sh scripts/stage2-production-evidence-gate.sh scripts/verify-stage2-evidence-archive.sh >/dev/null; then
  echo "Stage 2 scripts must not use jq // true for completion_blocked because false is a valid value" >&2
  exit 1
fi

if grep -R "production_blocked // true" scripts/policy-rollout-evidence-gate.sh scripts/scheduler-evidence-gate.sh scripts/finance-evidence-gate.sh >/dev/null; then
  echo "Stage 2 scripts must not use jq // true for production_blocked because false is a valid value" >&2
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
