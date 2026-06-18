# Changelog

All notable changes to MandoForge are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)

## [0.1.1] - 2026-06-15

### Added

- **K8s on-demand Pod provisioning**: When no warm-pool Remote Computer is available, the execution engine now automatically creates a Kubernetes Pod, polls until it reaches Running phase, persists the DB record, and leases it to the job. Previously jobs failed silently when the warm pool was empty.
- `poll_kubernetes_pod_running` helper polls the K8s `/pods/{name}` endpoint with configurable timeout (`MANDOFORGE_REMOTE_COMPUTER_POD_READY_TIMEOUT_SECONDS`, default 60s) and interval (`MANDOFORGE_REMOTE_COMPUTER_POD_READY_POLL_INTERVAL_MS`, default 2000ms).
- Stale-reclaim task (`execute_remote_computer_stale_reclaim`) now issues a `live_delete` mutation for expired on-demand Pod leases, preventing orphaned Pods in Kubernetes.
- DB migration `0060`: unique index on `remote_computers(tenant_id, pod_name)` to prevent double-provisioning when concurrent workers process jobs for the same session.
- Ontology topology graph with batch review UI — radial layout, selectable nodes, evidence summaries, batch accept/reject actions.
- `AppError::internal` constructor that logs the full error detail server-side while returning a generic `"internal server error"` to API callers.

### Changed

- On-demand Pod creation is race-safe: if two workers collide on the same pod name (unique constraint violation), the losing worker re-reads the winner's record instead of returning an error.
- Poll client built once outside the loop (avoids TLS-stack allocation on every tick); per-request 10s timeout added to prevent hung K8s API calls from stalling the deadline guard.
- Deadline checked at the top of each poll iteration so the function fails fast if the timeout has already elapsed before the first attempt.
- Internal Kubernetes error messages are no longer forwarded verbatim in 500 API responses.

### Fixed

- `provision_remote_computer_pod_for_job` doc comment referenced wrong env var (`MANDOFORGE_REMOTE_COMPUTER_RUNNER` → `MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT`).
- Magic number `900` (default lease seconds) extracted to `REMOTE_COMPUTER_DEFAULT_LEASE_SECONDS` constant shared between warm-pool and on-demand paths.
- Gate tests serialized with a `Mutex` to prevent flaky failures from parallel env-var mutation.
