#!/usr/bin/env bash
set -euo pipefail

archive_path="${1:-}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

sha256_value() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
  else
    echo "sha256sum or shasum is required to verify the Stage 2 evidence archive" >&2
    exit 1
  fi
}

verify_archive() {
  local archive="$1"
  local checksum_file="${archive}.sha256"
  local manifest_file="${archive}.manifest.txt"
  local tmpdir
  local checklist
  local expected_sha
  local actual_sha
  local manifest_sha
  local completion_blocked
  local missing_total

  if [[ ! -s "$archive" ]]; then
    echo "missing or empty Stage 2 evidence archive: $archive" >&2
    exit 1
  fi

  if [[ ! -s "$checksum_file" ]]; then
    echo "missing Stage 2 evidence checksum sidecar: $checksum_file" >&2
    exit 1
  fi

  if [[ ! -s "$manifest_file" ]]; then
    echo "missing Stage 2 evidence release manifest: $manifest_file" >&2
    exit 1
  fi

  expected_sha="$(awk '{print $1}' "$checksum_file")"
  actual_sha="$(sha256_value "$archive")"

  if [[ "$expected_sha" != "$actual_sha" ]]; then
    echo "Stage 2 evidence archive checksum mismatch for $archive" >&2
    echo "expected=$expected_sha" >&2
    echo "actual=$actual_sha" >&2
    exit 1
  fi

  manifest_sha="$(grep -E '^archive_sha256=' "$manifest_file" | sed 's/^archive_sha256=//')"
  if [[ "$manifest_sha" != "$actual_sha" ]]; then
    echo "Stage 2 evidence archive manifest checksum mismatch for $archive" >&2
    echo "manifest=$manifest_sha" >&2
    echo "actual=$actual_sha" >&2
    exit 1
  fi

  if ! grep -q "^archive_path=$archive$" "$manifest_file"; then
    echo "Stage 2 evidence archive manifest does not point at $archive" >&2
    exit 1
  fi

  tar tzf "$archive" >/dev/null

  if ! command -v jq >/dev/null 2>&1; then
    echo "jq is required to verify the Stage 2 completion checklist inside the archive" >&2
    exit 1
  fi

  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' RETURN
  tar xzf "$archive" -C "$tmpdir"
  checklist="$tmpdir/completion-audit/checklist.json"
  if [[ ! -s "$checklist" ]]; then
    echo "Stage 2 evidence archive is missing completion-audit/checklist.json" >&2
    exit 1
  fi

  completion_blocked="$(jq -r 'if has("completion_blocked") then .completion_blocked else true end' "$checklist")"
  missing_total="$(jq -r '
    [
      .missing_readiness_endpoint_count,
      .missing_validation_endpoint_count,
      .missing_required_evidence_artifact_count,
      .unresolved_endpoint_count,
      .missing_evidence_script_count,
      .missing_evidence_job_manifest_count,
      .missing_required_flag_count
    ]
    | map(. // 0)
    | add
  ' "$checklist")"

  if [[ "$completion_blocked" == "true" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Stage 2 evidence archive checklist is still blocked; set ALLOW_BLOCKED=1 only for inventory archives." >&2
    exit 1
  fi

  if [[ "$missing_total" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Stage 2 evidence archive checklist still has missing evidence metadata or artifacts: $missing_total" >&2
    exit 1
  fi
}

self_test() {
  local tmpdir
  local archive
  local sha

  tmpdir="$(mktemp -d)"
  mkdir -p "$tmpdir/evidence/completion-audit"
  echo "stage2_status=blocked" >"$tmpdir/evidence/summary.txt"
  cat >"$tmpdir/evidence/completion-audit/checklist.json" <<'JSON'
{
  "completion_blocked": false,
  "missing_readiness_endpoint_count": 0,
  "missing_validation_endpoint_count": 0,
  "missing_required_evidence_artifact_count": 0,
  "unresolved_endpoint_count": 0,
  "missing_evidence_script_count": 0,
  "missing_evidence_job_manifest_count": 0,
  "missing_required_flag_count": 0
}
JSON
  archive="$tmpdir/stage2-evidence.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  verify_archive "$archive"
}

if [[ "$archive_path" == "--self-test" ]]; then
  self_test
  echo "stage2 evidence archive verifier self-test ok"
  exit 0
fi

if [[ -z "$archive_path" ]]; then
  echo "usage: scripts/verify-stage2-evidence-archive.sh <archive.tar.gz>" >&2
  echo "       scripts/verify-stage2-evidence-archive.sh --self-test" >&2
  exit 1
fi

verify_archive "$archive_path"
echo "stage2 evidence archive verified: $archive_path"
