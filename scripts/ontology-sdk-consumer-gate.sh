#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
example_dir="$repo_root/examples/mandoforge-osdk-typescript"

cargo test --manifest-path "$repo_root/Cargo.toml" -p mandoforge-api --bins \
  ontology_sdk_consumer_http_enforces_subject_subset_visibility_and_proposal_boundary \
  -- --test-threads=1
npm --prefix "$example_dir" ci --ignore-scripts
npm --prefix "$example_dir" run typecheck

if [[ "${MANDOFORGE_RUN_LIVE:-0}" == "1" ]]; then
  npm --prefix "$example_dir" run live
fi
