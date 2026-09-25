# AX Coding Agent pilot — issue #37

This optional backend runs a Coding Agent inside an AX / Agent Substrate actor.
MandoForge retains session identity, Tool Router policy, approval, audit and final
acceptance. `ax` is the transport/control CLI; **Codex is the agent in the guest**.
No production manifest, default agent profile, workflow or scheduler is changed.

## Pinned contract

- Google AX: `acb6c1709b405eeaf0507031556426bf20493d52`, plus the checked-in
  `scripts/ax-pilot/ax-force-router.patch` for the host CLI only. Apply this patch
  with `git apply --unidiff-zero` before building `cmd/ax`; the controller and runner
  remain upstream unchanged.
  The helper sets `AX_SSH_FORCE_ROUTER=1`: direct TCP probing can succeed through
  a macOS tunnel/proxy even when the worker gRPC endpoint is unreachable. Explicit
  atenet routing avoids that observed false-positive path.
- Its Go module pins Substrate `672533541dbf` and env `4468a200b170`.
- Guest Codex CLI: `0.155.1`; the image is built from the pinned AX runner plus
  `scripts/ax-pilot/ax-coding-agent.py` using the adjacent Dockerfile.
- Managed configuration must specify the built AX CLI's SHA-256 and the final
  runner image digest. `ax version` does not identify a source revision; record
  `git rev-parse HEAD` and `go version -m` when building it. The helper's revision
  field identifies the supported contract, not independent binary provenance.

The [upstream runner contract](https://github.com/google/ax/blob/acb6c1709b405eeaf0507031556426bf20493d52/docs/runner.md)
requires `/usr/local/bin/ax-task-runner`, port 80 readiness and persistent
`/workspace`. AX does **not** read a command's exit status. A running actor is not
a successful Coding Agent turn. This adapter reads a nonce-bound result through
AX guest services, checks the exit code and Codex completion/final-message events,
then emits the existing normalized runtime event family and final artifact.
Events are returned in a batch after result readback, not streamed live.

## Scope and authority

Only managed profile type `ax_pilot` with command `mandoforge-ax-pilot` is added.
It uses the existing `agent_cli.exec` approval flow. No new business controller or
HTTP endpoint is introduced. The Orchestrator selects the managed profile when
calling `agent_cli.exec` and supplies the JSON task above. Existing direct CLI
profiles and the `delegated_runtime` workflow enum are not automatically migrated.
The executable also requires
`MANDOFORGE_AX_PILOT_ENABLED=1`. There is no enabled profile by default.

The pilot accepts one JSON task argument:

```json
{"operation":"run","key":"a-generated-uuid","prompt":"Explain a small Rust ownership example. Do not change files."}
```

Omit `prompt` for a fixed diagnostic only. Unknown fields and extra CLI arguments
are rejected. Arbitrary executables, shell snippets, workspace repositories,
model configuration and environment variables cannot be supplied by this task.
The Codex path runs `codex exec --json --ephemeral --sandbox read-only` with
`approval_policy="never"`; requests beyond that sandbox cannot ask AX to approve
them. MandoForge's approval authorizes the bounded turn, not unrestricted tools.
This first integration is for coding analysis, not write-enabled code editing.

The helper derives the session UUID from the existing session workspace, hashes
it with the key to name the AX task, and stores the immutable request and target
in `.ax-pilot/`. An OS lock excludes concurrent reconciliations; a fsynced intent
precedes submission. After an interrupted/ambiguous submit, the same identity
only reads the existing task. Missing state does not trigger another apply.
Do not delete these receipts to retry. Retain and reconcile them with AX first.
Local workspace durability is required; this is not cross-host distributed CAS.

The guest writes its own durable start marker before Codex. A marker without a
result fails closed on restart. The same result can be read again without
running another Codex turn. A changed prompt or target is rejected for that key.

Operations `read` and `cancel` reuse the same key. `cancel` calls AX delete and
keeps the receipt; deleting the AX resource is not independent proof that every
Substrate artifact has been removed. Operator readback/cleanup is required.
`run` with the original input also reattaches to existing result polling.

`suspend` / `resume` are available only for the fixed diagnostic. They wait for
AX state and, after resume, independently re-read the diagnostic result. Codex
checkpoint/resume is deliberately rejected: AX restores files into a new process
tree, and this ephemeral Codex pilot does not implement CLI conversation resume.
A new approved key is a new turn, not continuation of the old conversation.

## Local verification

Build the API and helper with the existing workspace dependencies:

```sh
cargo build -p mandoforge-api --bin mandoforge-api --bin mandoforge-ax-pilot
cargo test -p mandoforge-api --bin mandoforge-ax-pilot
cargo test -p mandoforge-api --bin mandoforge-api ax_pilot_metadata
python3 -m unittest discover -s scripts/ax-pilot -p 'test_*.py'
```

`scripts/ax-pilot/local-lab.yaml` records the test-only namespace, controller,
Redis and worker-pool fixture used in this trial. It requires the existing
Substrate installation and the recorded images in the local registry; it is not
a clean-cluster installer. It grants no cluster-wide secret access. Never apply
it to a cloud context. Build `cmd/ax-controller` and `cmd/ax-task-runner` from the
pinned source for the cluster CPU architecture. The runner Dockerfile expects
that binary named `ax-task-runner-linux` in its build context. Forward the lab
Redis service to a loopback port, run the upstream `ax-server` bound to another
loopback port with `--redis-addr`, and retain those processes for the test. The
controller uses the fixture snapshot bucket backed by the local Substrate store.

Use a dedicated local AX server backed by its own Redis and an explicitly named
local Kubernetes context. The executable rejects non-loopback AX server addresses
and non-local context names. These checks prevent accidental target selection;
context names are not a security boundary. Upstream AX's CLI uses plaintext gRPC,
so do not expose the server publicly. The local fixture is single-operator.

Configure an admin-created managed profile with these environment values:

| Variable | Required value |
| --- | --- |
| `MANDOFORGE_AX_PILOT_ENABLED` | `1` |
| `MANDOFORGE_AX_CLI` | Absolute path to pinned, compiled AX CLI |
| `MANDOFORGE_AX_CLI_SHA256` | SHA-256 of that executable |
| `MANDOFORGE_AX_SERVER` | Loopback IP and port, e.g. `127.0.0.1:18087` |
| `MANDOFORGE_AX_CONTEXT` | Explicit local `kind-*` or `docker-desktop` context |
| `MANDOFORGE_AX_IMAGE` | Runner image including `@sha256:...` |

Use a profile timeout of at least 240 seconds. The helper bounds each CLI call
and result polling; timeout preserves the unresolved identity for operator action.
AX sandbox debug services are enabled solely to read the fixed result file. No
external task is permitted to pass its own guest-service command through this
helper. Operators with direct AX access still have guest execution capability.

`scripts/ax-pilot/verify.py --output /absolute/new/evidence-directory` creates a
local managed profile, agent and session, asserts no execution before approval,
approves the diagnostic, and archives events, tool calls and artifacts. It needs
the variables above (computes CLI hash itself), plus
`MANDOFORGE_AX_PILOT_BIN` pointing to the helper. Add `--prompt '...'` to exercise
Codex. It exits nonzero unless result and final-event readback pass. Use only a
dedicated local MandoForge API; its default URL is port 18787. The script creates
records and approves only its own isolated test action.

The Python unit tests substitute the Codex process and are **contract evidence**,
not an LLM or sandbox success claim. Live evidence is reported separately below.

## Credentials, limits and remaining work

No host Codex login, customer repository, connector secret or model credential is
copied to the actor. The current AX controller only injects its Gemini credential,
which is not a ready-made Codex authentication path. A secure, explicitly scoped
Codex credential delivery mechanism is a prerequisite for a real model-backed
turn; do not put keys in task manifests, Redis payloads or images. This pilot does
not invent that credential service or silently use a host subscription.

The local worker pools supply aggregate CPU/memory limits. AX does not set a
worker selector on its generated template; this local run can use an available
worker outside the pilot namespace. A separate actor/atespace is not a dedicated
worker or tenant isolation guarantee. The inspected AX
controller does not propagate Task resource settings into its generated template,
so this pilot makes no per-task quota guarantee. AX's default egress is not a
MandoForge enterprise network policy. Read-only Codex, a nonsecret prompt, and a
credential-free isolated target bound this trial; they do not establish tenant
isolation, enterprise budget enforcement or production readiness.

AX provides declarative actor provisioning and file checkpoint lifecycle for
Coding Agent workloads. Its cost is an extra Redis/gRPC controller, Substrate
worker infrastructure, and an adapter-owned result/recovery protocol. Direct
Substrate integration would remove AX's layer but would also require MandoForge
to implement its task/workspace lifecycle; it is not substituted for this pilot.
