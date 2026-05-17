#!/usr/bin/env bash
set -euo pipefail

cargo test -p mandoforge-api --locked scheduler -- --nocapture
bash -n scripts/scheduler-evidence-gate.sh

echo "stage3 scheduler lane ok"
