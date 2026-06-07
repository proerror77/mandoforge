#!/usr/bin/env bash
set -euo pipefail

CONTRACT_PATH="${CONTRACT_PATH:-docs/enterprise-product-completion-contract.md}"
AUDIT_DIR="${AUDIT_DIR:-.mandoforge/enterprise-product-completion}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

required_lanes=(
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
  docs/workflow-pack-manifest-contract.md
  docs/drafts/context-os-memory-architecture.md
  docs/agent-remote-computer-plan.md
  docs/whiskey-adoption-completion-audit.md
  scripts/agent-os-core-evidence-gate.sh
  scripts/managed-session-runtime-evidence-gate.sh
  scripts/worker-remote-computer-evidence-gate.sh
  scripts/enterprise-security-admin-readiness-gate.sh
  scripts/native-connector-production-readiness-gate.sh
  scripts/ontology-engine-readiness-gate.sh
  scripts/verify-ecommerce-tmall-context-os.sh
  scripts/verify-workflow-pack-manifest.sh
  scripts/workflow-pack-evidence-gate.sh
  scripts/tenant-isolation-evidence-gate.sh
  scripts/vault-evidence-gate.sh
  scripts/observability-collector-evidence-gate.sh
  scripts/enterprise-product-readiness-gate.sh
  scripts/verify-ui-api-truth-gate.mjs
)

mkdir -p "$AUDIT_DIR"

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

blocked_lanes=(
  remote-computer-multinode
  live-connector-production
  ontology-engine
  enterprise-security-admin
  observability-ops
  product-surfaces
)

status="blocked"
if [[ "${#missing[@]}" -gt 0 ]]; then
  status="invalid_contract"
fi

checklist_json="$AUDIT_DIR/checklist.json"
{
  printf '{\n'
  printf '  "source": "enterprise-product-completion-contract-gate",\n'
  printf '  "contract_path": "%s",\n' "$CONTRACT_PATH"
  printf '  "enterprise_product_status": "%s",\n' "$status"
  printf '  "completion_blocked": %s,\n' "$(if [[ "$status" == "blocked" || "$status" == "invalid_contract" ]]; then echo true; else echo false; fi)"
  printf '  "required_evidence_class": "customer_grade",\n'
  printf '  "required_lanes": [\n'
  for i in "${!required_lanes[@]}"; do
    comma=","
    if [[ "$i" -eq $((${#required_lanes[@]} - 1)) ]]; then
      comma=""
    fi
    printf '    "%s"%s\n' "${required_lanes[$i]}" "$comma"
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

summary_file="$AUDIT_DIR/summary.txt"
{
  echo "enterprise_product_status=$status"
  echo "completion_blocked=$(if [[ "$status" == "blocked" || "$status" == "invalid_contract" ]]; then echo true; else echo false; fi)"
  echo "required_evidence_class=customer_grade"
  echo "required_lane_count=${#required_lanes[@]}"
  echo "blocked_lane_count=${#blocked_lanes[@]}"
  echo "missing_contract_item_count=${#missing[@]}"
  echo "checklist_json=$checklist_json"
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
