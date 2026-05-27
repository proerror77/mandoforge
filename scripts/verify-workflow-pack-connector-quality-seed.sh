#!/usr/bin/env bash
set -euo pipefail

source_path="${1:-scripts/workflow-pack-evidence-gate.sh}"

if [[ ! -f "$source_path" ]]; then
  echo "missing workflow pack evidence script: $source_path" >&2
  exit 1
fi

if grep -Fq 'retrieved_at: "2026-05-17T00:00:00Z"' "$source_path"; then
  echo "workflow pack connector quality ready proof must not use a stale fixed retrieved_at timestamp" >&2
  exit 1
fi

for required_arg in retrieved_at citation_url source_id reference retrieval_actor title snippet; do
  if ! grep -Fq -- "--arg $required_arg" "$source_path"; then
    echo "workflow pack connector quality proof is missing live jq arg: $required_arg" >&2
    exit 1
  fi
done

echo "Workflow pack connector quality ready proof uses fresh live connector evidence"
