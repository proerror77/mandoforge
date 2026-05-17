#!/usr/bin/env bash
set -euo pipefail

cargo test -p mandoforge-api --locked agent_handoff -- --nocapture

echo "stage3 agent handoffs lane ok"
