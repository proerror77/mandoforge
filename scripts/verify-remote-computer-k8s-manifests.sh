#!/usr/bin/env bash
set -euo pipefail

for required_command in kubectl jq; do
  if ! command -v "$required_command" >/dev/null 2>&1; then
    echo "$required_command is required to verify Remote Computer manifests" >&2
    exit 1
  fi
done

dry_run_manifests=(
  deploy/k8s/agent-remote-computer.yaml
  deploy/k8s/remote-computer-state-pvc.yaml
  deploy/k8s/remote-computer-state-contract.yaml
  deploy/k8s/remote-computer-state-juicefs-profile.yaml
  deploy/k8s/remote-computer-warm-pool.yaml
  deploy/k8s/agent-sandbox-runtime.yaml
  deploy/k8s/agent-sandbox-egress-networkpolicy.yaml
  deploy/agent-sandbox-smoke/sandbox-claim.yaml
)
runner_source="crates/mandoforge-api/src/remote_computer_runner.rs"
runtime_protocol_source="crates/mandoforge-api/src/sandbox_runtime_protocol.rs"
runtime_launcher_source="crates/mandoforge-api/src/bin/mandoforge-sandbox-runtime.rs"
runtime_dockerfile="Dockerfile.agent-sandbox"
runtime_build_script="scripts/build-agent-sandbox-runtime-image.sh"
runtime_publish_workflow=".github/workflows/deploy.yml"
agent_sandbox_contract="deploy/k8s/agent-sandbox-controller-contract.yaml"

for manifest in "${dry_run_manifests[@]}"; do
  if [[ ! -f "$manifest" ]]; then
    echo "missing Remote Computer manifest: $manifest" >&2
    exit 1
  fi
done

if [[ ! -f "$agent_sandbox_contract" ]]; then
  echo "missing Agent Sandbox controller contract: $agent_sandbox_contract" >&2
  exit 1
fi

k8s_render="$(mktemp -t mandoforge-k8s-render.XXXXXX.out)"
default_render="$(mktemp -t mandoforge-default-render.XXXXXX.out)"
pilot_render="$(mktemp -t mandoforge-remote-computer-pilot-render.XXXXXX.out)"
agent_sandbox_render="$(mktemp -t mandoforge-agent-sandbox-pilot-render.XXXXXX.out)"
agent_sandbox_smoke_render="$(mktemp -t mandoforge-agent-sandbox-smoke-render.XXXXXX.out)"
agent_sandbox_cache_render="$(mktemp -t mandoforge-agent-sandbox-cache-render.XXXXXX.out)"
agent_sandbox_template_render="$(mktemp -t mandoforge-agent-sandbox-template-render.XXXXXX.out)"
agent_sandbox_warm_pool_render="$(mktemp -t mandoforge-agent-sandbox-warm-pool-render.XXXXXX.out)"
agent_sandbox_egress_render="$(mktemp -t mandoforge-agent-sandbox-egress-render.XXXXXX.out)"
agent_sandbox_claim_render="$(mktemp -t mandoforge-agent-sandbox-claim-render.XXXXXX.out)"
cleanup() {
  rm -f \
    "$k8s_render" \
    "$default_render" \
    "$pilot_render" \
    "$agent_sandbox_render" \
    "$agent_sandbox_smoke_render" \
    "$agent_sandbox_cache_render" \
    "$agent_sandbox_template_render" \
    "$agent_sandbox_warm_pool_render" \
    "$agent_sandbox_egress_render" \
    "$agent_sandbox_claim_render"
}
trap cleanup EXIT

extract_rendered_resource() {
  local render_file="$1"
  local expected_kind="$2"
  local expected_name="$3"
  local output_file="$4"

  awk -v expected_kind="$expected_kind" -v expected_name="$expected_name" '
    function reset_document() {
      document = ""
      kind = ""
      name = ""
      in_metadata = 0
    }
    function emit_document() {
      if (kind == expected_kind && name == expected_name) {
        printf "%s", document
      }
    }
    BEGIN { reset_document() }
    /^---[[:space:]]*$/ {
      emit_document()
      reset_document()
      next
    }
    {
      document = document $0 ORS
      if ($0 ~ /^kind:[[:space:]]*/) {
        kind = $0
        sub(/^kind:[[:space:]]*/, "", kind)
      }
      if ($0 == "metadata:") {
        in_metadata = 1
        next
      }
      if ($0 ~ /^[^[:space:]]/ && $0 != "metadata:") {
        in_metadata = 0
      }
      if (in_metadata && $0 ~ /^  name:[[:space:]]*/) {
        name = $0
        sub(/^  name:[[:space:]]*/, "", name)
        in_metadata = 0
      }
    }
    END { emit_document() }
  ' "$render_file" >"$output_file"

  if [[ ! -s "$output_file" ]]; then
    echo "render is missing $expected_kind/$expected_name" >&2
    exit 1
  fi
}

render_manifest_json() {
  kubectl patch --local=true --type=merge --patch '{}' -f "$1" -o json
}

kubectl kustomize deploy/k8s >"$k8s_render"
kubectl kustomize deploy >"$default_render"
kubectl kustomize deploy/remote-computer-pilot --load-restrictor LoadRestrictionsNone >"$pilot_render"
kubectl kustomize deploy/agent-sandbox-pilot --load-restrictor LoadRestrictionsNone >"$agent_sandbox_render"
kubectl kustomize deploy/agent-sandbox-smoke --load-restrictor LoadRestrictionsNone >"$agent_sandbox_smoke_render"

if ! grep -q "kind: SandboxTemplate" "$k8s_render" \
  || ! grep -q "kind: SandboxWarmPool" "$k8s_render" \
  || ! grep -q "name: mandoforge-agent-sandbox-egress" "$k8s_render"; then
  echo "default production render must include Agent Sandbox template, warm pool, and network policy" >&2
  exit 1
fi

if ! grep -q "MANDOFORGE_REMOTE_COMPUTER_RUNNER: agent-sandbox" "$k8s_render"; then
  echo "default production render must select the Agent Sandbox runner" >&2
  exit 1
fi

if grep -q "name: mandoforge-agent-remote-computer-template" "$k8s_render"; then
  echo "default production render must not include the legacy direct-Pod runtime template" >&2
  exit 1
fi

if ! grep -q "serviceAccountName: mandoforge-api" "$k8s_render" \
  || ! grep -q "name: mandoforge-api-agent-sandbox" "$k8s_render"; then
  echo "API must use an explicit Agent Sandbox ServiceAccount and scoped RBAC" >&2
  exit 1
fi

role_json="$(render_manifest_json deploy/k8s/api-agent-sandbox-rbac.yaml \
  | jq -cs 'map(select(.kind == "Role" and .metadata.name == "mandoforge-api-agent-sandbox"))[0]')"
if ! jq -e '
  def exact_rule($group; $resource; $verbs):
    .apiGroups == [$group]
    and .resources == [$resource]
    and (.verbs | sort) == ($verbs | sort);
  .rules as $rules
  | ($rules | length) == 4
    and any($rules[]; exact_rule(""; "pods"; ["get"]))
    and any($rules[]; exact_rule(""; "pods/exec"; ["create", "get"]))
    and any($rules[]; exact_rule("extensions.agents.x-k8s.io"; "sandboxclaims"; ["get", "create", "delete"]))
    and any($rules[]; exact_rule("agents.x-k8s.io"; "sandboxes"; ["get"]))
' <<<"$role_json" >/dev/null; then
  echo "API Agent Sandbox RBAC must contain only the required resource/verb tuples" >&2
  exit 1
fi

for worker_manifest in deploy/k8s/worker.yaml deploy/k8s/worker-isolated-pool.yaml; do
  if ! render_manifest_json "$worker_manifest" \
    | jq -e '.spec.template.spec.automountServiceAccountToken == false' >/dev/null; then
    echo "$worker_manifest must set automountServiceAccountToken: false" >&2
    exit 1
  fi
done

if grep -q "mountPath: /var/run/secrets/kubernetes.io/serviceaccount" deploy/k8s/worker.yaml \
  || grep -q "mountPath: /var/run/secrets/kubernetes.io/serviceaccount" deploy/k8s/worker-isolated-pool.yaml; then
  echo "queue workers must not mount Kubernetes API credentials" >&2
  exit 1
fi

if grep -q "app: agent-remote-computer" deploy/k8s/worker-isolated-pool-networkpolicy.yaml \
  || grep -q "port: 8080" deploy/k8s/worker-isolated-pool-networkpolicy.yaml; then
  echo "queue workers must reach remote runtimes only through the MandoForge API" >&2
  exit 1
fi

if ! grep -q 'MANDOFORGE_AGENT_SANDBOX_CONTROLLER_VERSION: "v0.5.1"' "$agent_sandbox_contract" \
  || ! grep -q 'MANDOFORGE_AGENT_SANDBOX_EXTENSIONS_API: "extensions.agents.x-k8s.io/v1beta1"' "$agent_sandbox_contract" \
  || ! grep -q 'MANDOFORGE_AGENT_SANDBOX_CORE_INSTALL_ASSET: "manifest.yaml"' "$agent_sandbox_contract" \
  || ! grep -q 'MANDOFORGE_AGENT_SANDBOX_CORE_INSTALL_SHA256: "8cfdf0a878f66b91d2e7103e77859d1412d850ce3f5fe5c3fa134c36bd55504a"' "$agent_sandbox_contract" \
  || ! grep -q 'MANDOFORGE_AGENT_SANDBOX_EXTENSIONS_INSTALL_ASSET: "extensions.yaml"' "$agent_sandbox_contract" \
  || ! grep -q 'MANDOFORGE_AGENT_SANDBOX_EXTENSIONS_INSTALL_SHA256: "7c22b450e24ede3fddbcd5ae0ee7c78ea102d6c30635ff860cc486578a55932e"' "$agent_sandbox_contract"; then
  echo "Agent Sandbox controller contract must pin v0.5.1, v1beta1, and both release asset digests" >&2
  exit 1
fi

extract_rendered_resource "$agent_sandbox_render" PersistentVolumeClaim \
  mandoforge-agent-sandbox-mandoforge-cache "$agent_sandbox_cache_render"
extract_rendered_resource "$agent_sandbox_render" SandboxTemplate \
  mandoforge-agent-runtime "$agent_sandbox_template_render"
extract_rendered_resource "$agent_sandbox_render" SandboxWarmPool \
  mandoforge-agent-runtime "$agent_sandbox_warm_pool_render"
extract_rendered_resource "$agent_sandbox_render" NetworkPolicy \
  mandoforge-agent-sandbox-egress "$agent_sandbox_egress_render"
extract_rendered_resource "$agent_sandbox_smoke_render" SandboxClaim \
  mandoforge-agent-runtime-smoke "$agent_sandbox_claim_render"

if ! grep -q "name: mandoforge-remote-computer-state" "$k8s_render" \
  || ! grep -q "name: mandoforge-remote-computer-state-contract" "$k8s_render"; then
  echo "base Remote Computer render is missing the state PVC or state contract" >&2
  exit 1
fi

if grep -Eq 'kind:[[:space:]]*Secret|replace-me|s3\.example\.com' "$default_render"; then
  echo "default deploy render must not include placeholder Remote Computer state Secrets" >&2
  exit 1
fi

if grep -q "mandoforge-api:8080" "$k8s_render" "$pilot_render" deploy/k8s/remote-computer-artifact-discovery-sidecar.yaml; then
  echo "Remote Computer artifact sidecar must target the mandoforge-api service port 8787, not 8080" >&2
  exit 1
fi

if ! grep -q "http://mandoforge-api:8787" "$k8s_render"; then
  echo "base Remote Computer render is missing the in-cluster API URL on port 8787" >&2
  exit 1
fi

if ! grep -q "driver: csi.juicefs.com" "$pilot_render"; then
  echo "Remote Computer pilot render is missing JuiceFS CSI storage" >&2
  exit 1
fi

if ! grep -q "volumeName: mandoforge-remote-computer-state-juicefs-pv" "$pilot_render"; then
  echo "Remote Computer pilot render does not bind the Pod-mounted state PVC to the JuiceFS PV" >&2
  exit 1
fi

if ! grep -q "kind: ScaledObject" "$pilot_render"; then
  echo "Remote Computer pilot render is missing the KEDA ScaledObject example" >&2
  exit 1
fi

if ! awk '
  /name: mandoforge-agent-remote-computer-warm-pool/ { in_warm_pool = 1 }
  in_warm_pool && /name: artifact-discovery/ { found_sidecar = 1 }
  in_warm_pool && /name: mandoforge-remote-computer-artifact-discovery/ { found_config = 1 }
  END { exit !(found_sidecar && found_config) }
' "$pilot_render"; then
  echo "Remote Computer warm-pool render is missing artifact discovery sidecar parity" >&2
  exit 1
fi

if grep -Eq 'kind:[[:space:]]*Secret|replace-me|s3\.example\.com' deploy/k8s/agent-sandbox-runtime.yaml; then
  echo "Agent Sandbox runtime manifest must not include placeholder Secrets" >&2
  exit 1
fi

if [[ ! -f "$runtime_dockerfile" ]]; then
  echo "Agent Sandbox runtime Dockerfile is missing" >&2
  exit 1
fi

if [[ ! -x "$runtime_build_script" ]]; then
  echo "Agent Sandbox tracked-context image builder is missing or not executable" >&2
  exit 1
fi

if [[ ! -f "$runtime_publish_workflow" ]]; then
  echo "Agent Sandbox runtime publish workflow is missing" >&2
  exit 1
fi

for runtime_contract in \
  'rust:1.97.0-bookworm@sha256:' \
  'node:24.18.0-bookworm-slim@sha256:' \
  'ghcr.io/astral-sh/uv:0.11.28@sha256:' \
  'sccache --version 0.16.0' \
  'pnpm@${PNPM_VERSION}' \
  '@openai/codex@${CODEX_VERSION}' \
  '@anthropic-ai/claude-code@${CLAUDE_CODE_VERSION}' \
  '/usr/local/bin/mandoforge-sandbox-runtime' \
  'env PATH=/usr/local/bin:/usr/bin:/bin rustc --version' \
  'env PATH=/usr/local/bin:/usr/bin:/bin claude --version' \
  'MANDOFORGE_SHARED_CARGO_CACHE_ROOT=/cache/project/cargo' \
  'MANDOFORGE_SOURCE_TREE' \
  '.mandoforge-tracked-context' \
  'git config --system --add safe.directory /opt/mandoforge-source' \
  'git -C /opt/mandoforge-source init'; do
  if ! grep -Fq "$runtime_contract" "$runtime_dockerfile"; then
    echo "Agent Sandbox runtime image is missing pinned contract: $runtime_contract" >&2
    exit 1
  fi
done

for build_contract in \
  'git diff --quiet --' \
  'git checkout-index --all --force' \
  'git write-tree' \
  'MANDOFORGE_SOURCE_TREE' \
  'mandoforge-agent-sandbox-runtime:0.1.5'; do
  if ! grep -Fq "$build_contract" "$runtime_build_script"; then
    echo "Agent Sandbox image builder is missing tracked-context contract: $build_contract" >&2
    exit 1
  fi
done

if ! grep -q 'image: ghcr.io/proerror77/mandoforge/mandoforge-agent-sandbox-runtime@sha256:77b6327baca38493fd9d688e6641737ff1f507e5650e7fc6e9f07e5f42e1469a' "$agent_sandbox_template_render" \
  || grep -q 'mandoforge-adoption-api:latest' deploy/k8s/agent-sandbox-runtime.yaml; then
  echo "Agent Sandbox template must use the dedicated pinned runtime image" >&2
  exit 1
fi

for publish_contract in \
  'runtime_only:' \
  'RUNTIME_IMAGE_NAME: ghcr.io/${{ github.repository }}/mandoforge-agent-sandbox-runtime' \
  'RUNTIME_IMAGE_TAG: ${{ inputs.runtime_image_tag }}' \
  'MANDOFORGE_AGENT_SANDBOX_IMAGE="$RUNTIME_IMAGE_NAME:$RUNTIME_IMAGE_TAG"' \
  "if: inputs.runtime_only != 'true'" \
  "if: inputs.runtime_only == 'true' || inputs.publish_image == 'true'" \
  'docker push "$RUNTIME_IMAGE_NAME:$RUNTIME_IMAGE_TAG"'; do
  if ! grep -Fq "$publish_contract" "$runtime_publish_workflow"; then
    echo "Agent Sandbox deploy workflow is missing publish contract: $publish_contract" >&2
    exit 1
  fi
done

if grep -q "kind: SandboxClaim" "$agent_sandbox_render"; then
  echo "Agent Sandbox pilot render must not create live SandboxClaim resources" >&2
  exit 1
fi

if grep -Eq 'kind:[[:space:]]*Secret|replace-me|s3\.example\.com|mandoforge-agent-remote-computer-warm-pool|remote-computer-state-juicefs' "$agent_sandbox_render"; then
  echo "Agent Sandbox pilot render must not inherit Remote Computer pilot secrets, JuiceFS, or legacy warm-pool resources" >&2
  exit 1
fi

if ! grep -q "kind: SandboxClaim" "$agent_sandbox_claim_render"; then
  echo "Agent Sandbox smoke render is missing the live SandboxClaim example" >&2
  exit 1
fi

if ! grep -q "apiVersion: extensions.agents.x-k8s.io/v1beta1" "$agent_sandbox_template_render" \
  || ! grep -q "apiVersion: extensions.agents.x-k8s.io/v1beta1" "$agent_sandbox_warm_pool_render" \
  || ! grep -q "apiVersion: extensions.agents.x-k8s.io/v1beta1" "$agent_sandbox_claim_render"; then
  echo "Agent Sandbox pilot must use the current v1beta1 extensions API" >&2
  exit 1
fi

if ! grep -q "networkPolicyManagement: Unmanaged" "$agent_sandbox_template_render"; then
  echo "Agent Sandbox template must delegate egress isolation to the reviewed MandoForge policies" >&2
  exit 1
fi

if ! grep -q "volumeClaimTemplates:" "$agent_sandbox_template_render" \
  || ! grep -q "name: workspace-data" "$agent_sandbox_template_render"; then
  echo "Agent Sandbox pilot must define the per-sandbox workspace PVC template" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-agent-sandbox-mandoforge-cache" "$agent_sandbox_template_render" \
  || ! grep -q "mandoforge.io/cache-scope: single-project" "$agent_sandbox_cache_render" \
  || ! grep -q "MANDOFORGE_SHARED_CARGO_CACHE_ROOT" "$agent_sandbox_template_render" \
  || ! grep -q "SCCACHE_DIR" "$agent_sandbox_template_render" \
  || ! grep -q "PNPM_STORE_DIR" "$agent_sandbox_template_render" \
  || ! grep -q "UV_CACHE_DIR" "$agent_sandbox_template_render"; then
  echo "Agent Sandbox pilot must mount explicit single-project dependency cache paths" >&2
  exit 1
fi

if ! grep -Fq '.env("RUSTC_WRAPPER", "sccache")' "$runtime_launcher_source" \
  || ! grep -Fq '.env("CARGO_HOME", cargo_home)' "$runtime_launcher_source" \
  || ! grep -Fq '.env("CARGO_TARGET_DIR", cargo_target)' "$runtime_launcher_source" \
  || ! grep -Fq 'for cache_name in ["registry", "git"]' "$runtime_launcher_source"; then
  echo "Agent Sandbox launcher must isolate Cargo identity/target while sharing download and compile caches" >&2
  exit 1
fi

if grep -q "CARGO_TARGET_DIR" deploy/k8s/agent-sandbox-runtime.yaml \
  || grep -q 'CARGO_HOME=/cache/project/cargo' "$runtime_dockerfile" \
  || grep -q 'name: agent-state' deploy/k8s/agent-sandbox-runtime.yaml \
  || grep -q 'name: artifact-discovery' deploy/k8s/agent-sandbox-runtime.yaml \
  || grep -q 'value: "/workspace/session"' deploy/k8s/agent-sandbox-runtime.yaml \
  || ! grep -q 'mountPath: /workspace/sessions' "$agent_sandbox_template_render" \
  || grep -q 'mountPath: /workspace$' "$agent_sandbox_template_render"; then
  echo "Agent Sandbox mutable target, agent state, sidecar, and workspace paths must remain session-private" >&2
  exit 1
fi

if ! grep -q "fsGroup: 1000" "$agent_sandbox_template_render" \
  || ! grep -q "fsGroupChangePolicy: OnRootMismatch" "$agent_sandbox_template_render"; then
  echo "Agent Sandbox PVC mounts must be writable by the non-root runtime user" >&2
  exit 1
fi

if ! grep -q "sandboxTemplateRef:" "$agent_sandbox_warm_pool_render" \
  || ! grep -q "type: Recreate" "$agent_sandbox_warm_pool_render" \
  || ! grep -q "warmPoolRef:" "$agent_sandbox_claim_render"; then
  echo "Agent Sandbox pilot/smoke overlays must wire WarmPool and Claim with current reference fields" >&2
  exit 1
fi

if grep -q "templateRef:" "$agent_sandbox_render" "$agent_sandbox_smoke_render" \
  || grep -q "warmpool:" "$agent_sandbox_render" "$agent_sandbox_smoke_render"; then
  echo "Agent Sandbox pilot must not use retired SandboxClaim reference fields" >&2
  exit 1
fi

network_policy_json="$(render_manifest_json deploy/k8s/agent-sandbox-egress-networkpolicy.yaml)"
if ! jq -e '
  def ports_exact($expected):
    (.ports | sort_by([.protocol, .port])) == ($expected | sort_by([.protocol, .port]));
  .spec as $spec
  | $spec.podSelector.matchLabels == {
      "app": "mandoforge-agent-remote-computer",
      "mandoforge.io/runtime-substrate": "agent-sandbox"
    }
    and ($spec.policyTypes | sort) == (["Ingress", "Egress"] | sort)
    and $spec.ingress == []
    and ($spec.egress | length) == 3
    and any($spec.egress[];
      (.to | length) == 1
      and .to[0].namespaceSelector.matchLabels["kubernetes.io/metadata.name"] == "kube-system"
      and .to[0].podSelector.matchLabels["k8s-app"] == "kube-dns"
      and ports_exact([{"protocol":"UDP","port":53},{"protocol":"TCP","port":53}]))
    and any($spec.egress[];
      (.to | length) == 1
      and .to[0].podSelector.matchLabels.app == "mandoforge-api"
      and ports_exact([{"protocol":"TCP","port":8787}]))
    and any($spec.egress[];
      (.to | length) == 2
      and ([.to[].ipBlock.cidr] | sort) == (["0.0.0.0/0", "::/0"] | sort)
      and any(.to[];
        .ipBlock.cidr == "0.0.0.0/0"
        and (["10.0.0.0/8", "127.0.0.0/8", "169.254.0.0/16", "172.16.0.0/12", "192.168.0.0/16"] - .ipBlock.except | length) == 0)
      and any(.to[];
        .ipBlock.cidr == "::/0"
        and (["::1/128", "fc00::/7", "fe80::/10"] - .ipBlock.except | length) == 0)
      and ports_exact([{"protocol":"TCP","port":443}]))
' <<<"$network_policy_json" >/dev/null; then
  echo "Agent Sandbox pilot must deny ingress and allow only bounded DNS, API, and external HTTPS egress" >&2
  exit 1
fi

if ! grep -q "shutdownPolicy: Delete" "$agent_sandbox_claim_render" \
  || ! grep -q "ttlSecondsAfterFinished: 300" "$agent_sandbox_claim_render"; then
  echo "Agent Sandbox smoke claim must carry explicit cleanup lifecycle fields" >&2
  exit 1
fi

for tracking_key in \
  "mandoforge.io/session-id" \
  "mandoforge.io/remote-computer-id" \
  "mandoforge.io/tenant-id" \
  "mandoforge.io/lease-id" \
  "mandoforge.io/lifecycle"; do
  if ! grep -q "$tracking_key" "$runner_source"; then
    echo "Remote Computer runner is missing Kubernetes Pod tracking metadata: $tracking_key" >&2
    exit 1
  fi
done

if ! grep -q "parse_kubernetes_exec_stdin" "$runner_source" \
  || ! grep -q "SANDBOX_RUNTIME_EXECUTABLE" "$runner_source" \
  || ! grep -q "stdin=true" "$runner_source" \
  || ! grep -q "MAX_SANDBOX_RUNTIME_ENVELOPE_BYTES" "$runtime_protocol_source"; then
  echo "Remote Computer runner must use the bounded fixed-launcher stdin protocol" >&2
  exit 1
fi

if grep -q "parse_kubernetes_exec_command" "$runner_source" \
  || grep -q 'command_query' "$runner_source"; then
  echo "Remote Computer runner must not retain dynamic Kubernetes exec command query construction" >&2
  exit 1
fi

echo "remote computer k8s manifests ok"
