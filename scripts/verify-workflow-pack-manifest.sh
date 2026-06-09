#!/usr/bin/env bash
set -euo pipefail

cargo test -p mandoforge-api workflow_pack -- --nocapture
cargo test -p mandoforge-api ecommerce_expansion_packs_stage_semantic_context_os_skeletons -- --nocapture
scripts/verify-ecommerce-platform-closed-loop.sh

test -f schemas/workflow-pack-manifest.schema.json
test -f packs/ai-governance/package.yaml
test -f packs/ecommerce-amazon/package.yaml
test -f packs/ecommerce-core/package.yaml
test -f packs/ecommerce-taobao/package.yaml
test -f packs/ecommerce-tiktok-shop/package.yaml
test -f packs/ecommerce-tmall/package.yaml
test -f packs/ecommerce-xianyu/package.yaml
test -f packs/ecommerce-xiaohongshu/package.yaml
test -f packs/legal/package.yaml
test -f scripts/verify-ecommerce-platform-closed-loop.sh

echo "workflow pack manifest contract ok"
