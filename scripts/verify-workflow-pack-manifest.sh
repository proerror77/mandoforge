#!/usr/bin/env bash
set -euo pipefail

cargo test -p mandoforge-api workflow_pack -- --nocapture

test -f schemas/workflow-pack-manifest.schema.json
test -f packs/ai-governance/package.yaml
test -f packs/ecommerce-tmall/package.yaml

echo "workflow pack manifest contract ok"
