#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
scripts/stage3-verify-scheduler.sh
scripts/stage3-verify-codex-traces.sh
scripts/stage3-verify-remote-computer.sh
scripts/stage3-verify-agent-handoffs.sh
scripts/stage3-verify-workflow-packs.sh
cargo test --workspace --locked --all-targets -- --test-threads=1
git diff --check

echo "stage3 integration owner gate ok"
