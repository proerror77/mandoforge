#!/usr/bin/env bash
set -euo pipefail

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ecommerce Tmall Context OS verification requires $1" >&2
    exit 1
  fi
}

require_cmd cargo
require_cmd jq
require_cmd rg

jq -e '
  .properties.semantic_scopes != null
  and (.["$defs"].semantic_scopes.required | index("domain_scope") != null)
  and (.["$defs"].semantic_scopes.required | index("workflow_scope") != null)
  and (.["$defs"].semantic_scopes.required | index("share_policy") != null)
' schemas/workflow-pack-manifest.schema.json >/dev/null

rg -q '^semantic_scopes:' packs/ecommerce-tmall/package.yaml
rg -q '^  domain_scope: ecommerce$' packs/ecommerce-tmall/package.yaml
rg -q '^  workflow_scope: tmall$' packs/ecommerce-tmall/package.yaml
rg -q '^  share_policy: isolated$' packs/ecommerce-tmall/package.yaml
rg -q '^  lane_scope:' packs/ecommerce-tmall/workflows
rg -q '^actions:' packs/ecommerce-tmall/package.yaml
rg -q '^  - id: submit-review-explanation$' packs/ecommerce-tmall/package.yaml
rg -q '^[[:space:]]+- id: tmall_review$' packs/ecommerce-tmall/profiles/ontology_seed.yaml

cargo test -p mandoforge-api validates_ecommerce_tmall_domain_pack_fixture -- --nocapture
cargo test -p mandoforge-api ecommerce_tmall_pack_stages_semantic_context_os_contract -- --nocapture
cargo test -p mandoforge-api ecommerce_tmall_connector_quality_checks_account_secrets_and_lane_readiness -- --nocapture

echo "ecommerce Tmall Context OS semantic contract ok"
