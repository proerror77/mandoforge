#!/usr/bin/env bash
set -euo pipefail

scripts/verify-workflow-pack-manifest.sh
cargo test -p mandoforge-api --locked workflow_pack_install -- --nocapture

echo "stage3 workflow packs lane ok"
