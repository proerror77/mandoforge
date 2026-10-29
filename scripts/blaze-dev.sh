#!/usr/bin/env bash
# Optional local experiment. Standard Cargo remains the validation authority.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
usage() { echo "Usage: $0 [build|test] [test-filter]"; }
[[ ${1:-} != --help && ${1:-} != -h ]] || { usage; exit 0; }
(($# <= 2)) || { usage >&2; exit 2; }
action=${1:-build}
case "$action" in build|test) ;; *) usage >&2; exit 2 ;; esac
filter=${2:-}
[[ $filter != -* ]] || { echo 'Expected a test name, not a build flag.' >&2; exit 2; }
[[ $action == test || $# -le 1 ]] || { usage >&2; exit 2; }
cd "$root"
args=("$action" --locked -p mandoforge-api --bin mandoforge-api)
[[ -z $filter ]] || args+=("$filter")
command -v blaze >/dev/null 2>&1 || {
  echo 'Blaze is not installed. See https://github.com/indexable-inc/blaze#install' >&2
  exit 127
}
# Compare actual test runs, without Blaze reusing a prior passing test result.
export BLAZE_TESTS=all
exec blaze "${args[@]}"
