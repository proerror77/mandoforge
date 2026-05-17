#!/usr/bin/env bash
set -euo pipefail

cargo test -p mandoforge-api --locked codex_app_server -- --nocapture
bash -n scripts/codex-app-server-evidence-gate.sh

echo "stage3 codex traces lane ok"
