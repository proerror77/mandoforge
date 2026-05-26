#!/usr/bin/env bash
set -euo pipefail

source_path="${1:-scripts/whiskey-adoption-evidence.sh}"

if [[ ! -f "$source_path" ]]; then
  echo "missing Whiskey evidence script: $source_path" >&2
  exit 1
fi

seed_block="$(awk '
  /^seed_eval_release_evidence\(\) \{/ { in_block = 1 }
  in_block { print }
  /^seed_observability_remediation_evidence\(\) \{/ { exit }
' "$source_path")"

if [[ -z "$seed_block" ]]; then
  echo "could not locate seed_eval_release_evidence block" >&2
  exit 1
fi

if grep -Fq "jq -r '.[0].id // empty'" <<<"$seed_block"; then
  echo "Whiskey eval/release seed must not select the first arbitrary agent" >&2
  exit 1
fi

if grep -Fq '"tools":[]' <<<"$seed_block"; then
  echo "Whiskey eval/release seed must not create an agent with an empty tools list" >&2
  exit 1
fi

for required_tool in file.read file.write sql.query artifact.create; do
  if ! grep -Fq "\"$required_tool\"" <<<"$seed_block"; then
    echo "Whiskey eval/release seed is missing required tool: $required_tool" >&2
    exit 1
  fi
done

echo "Whiskey eval/release seed agent selection is constrained to Stage 2 regression-capable agents"
