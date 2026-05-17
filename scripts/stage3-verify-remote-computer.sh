#!/usr/bin/env bash
set -euo pipefail

cargo test -p mandoforge-api --locked remote_computer -- --nocapture
bash -n scripts/remote-computer-evidence-gate.sh
bash -n scripts/verify-remote-computer-k8s-manifests.sh
bash -n scripts/whiskey-remote-computer-k3s-host-inventory.sh
bash -n scripts/whiskey-remote-computer-k3s-constrained-pilot.sh

echo "stage3 remote computer lane ok"
