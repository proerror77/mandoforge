# Script Instructions

- Gate scripts must fail closed by default. Missing required evidence, missing scripts, or malformed summaries should return a non-zero exit unless the script documents an explicit static inventory mode.
- Use `STATIC_ONLY=1` only for repo wiring checks. Do not let static mode claim customer-grade production readiness.
- If a gate supports blocked inventory runs, use `ALLOW_BLOCKED=1` only to report known missing evidence; do not convert blocked evidence into ready evidence.
- New production gates should emit a stable summary file, be wired into `scripts/production-launch-preflight.sh`, and be covered by `scripts/verify-stage2-evidence-k8s-manifests.sh` plus the enterprise readiness contract.
- When changing shell scripts, run `bash -n` on every touched `.sh` file. For a new gate, also verify its static path, blocked path, and a temporary ready-path fixture when practical.

