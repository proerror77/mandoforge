# CI/CD

Mandoforge keeps the default CI path fast and moves full evidence runs to explicit workflows.

## Fast CI

`.github/workflows/ci.yml` runs on pull requests and pushes to `main`:

- `Static gates` builds the Yew/Trunk WebAssembly web UI into `web/`, verifies the emitted static assets, checks JavaScript and shell syntax, Kubernetes manifest renderability, and evidence manifest validators without compiling the API workspace.
- `Rust tests` installs the repository's pinned Rust toolchain, restores Cargo registry and `target/` cache, runs `cargo fetch --locked`, checks formatting and Clippy, and runs `cargo test --workspace --locked --all-targets -- --test-threads=1`.

Local development, GitHub Actions, and the Agent Sandbox runtime use Rust 1.97.0. Update `rust-toolchain.toml`, the sandbox runtime image, and every `dtolnay/rust-toolchain` workflow reference together when intentionally upgrading it.

The Rust job intentionally does not run both `cargo check` and `cargo test` on every PR. `cargo test --all-targets` already compiles the workspace test targets and avoids a second full compile path in ordinary CI.

## Evidence Gates

`.github/workflows/evidence.yml` is for heavier verification:

- Scheduled runs execute `./scripts/stage1-final-gate.sh`.
- Manual runs can opt into `RUN_LIVE=1 START_LIVE_STACK=1 ./scripts/stage1-final-gate.sh`.
- The workflow also performs a Docker image build dry run.
- Scheduled runs also start the API and execute
  `./scripts/managed-workflow-runtime-evidence-gate.sh`; manual runs can opt in
  with `managed_workflow_runtime=true`.

Use this workflow for full gate confidence when runtime, Docker, Postgres, or
managed workflow graph behavior matters. Keep these checks out of the default PR
path unless the change requires them.

## Deploy

`.github/workflows/deploy.yml` is manual and its jobs run only when dispatched from `main`. Its build job publishes images without creating a production deployment record. The optional Whiskey deployment job uses the `stage2-production` GitHub Environment.

To update the Whiskey adoption stack through GitHub Actions, run the workflow with:

- `image_tag`: optional; defaults to the exact source commit SHA.
- `publish_image`: `true`.
- `deploy_whiskey`: `true`.
- `whiskey_remote_host`: the Whiskey host or IP, unless the `WHISKEY_REMOTE_HOST` secret is configured.
- `whiskey_remote_user`: optional; falls back to `WHISKEY_REMOTE_USER`, then `root`.
- `whiskey_remote_root`: defaults to `/opt/mandoforge-adoption`.

Whiskey deployment also requires the `WHISKEY_SSH_PRIVATE_KEY` repository or environment secret. The workflow pushes the image with GitHub's package token, then runs `scripts/whiskey-adoption-deploy.sh` over SSH so Whiskey only pulls and restarts the adoption stack; it does not compile Rust on the Whiskey host. The deployment fails unless both the running container's image-baked tag/revision labels and the runtime endpoint match the requested image tag and exact Git SHA. The inspected image metadata is archived separately from the runtime response.

Publishing an image is `Publish` evidence only. A successful environment-gated job plus the script's independent container-image and runtime readbacks is `Deploy` and `Readback` evidence; neither is implied by CI or merge status.

It does not apply Kubernetes manifests or execute production business actions. Stage 2 high-risk actions remain approval-gated until production policy explicitly enables them.

## Rust Compile Cost Rules

- Prefer one Rust compile lane in PR CI.
- Cache Cargo registry, git dependencies, and `target/` by `Cargo.lock` plus `Cargo.toml` hashes.
- Use `cargo fetch --locked` to make dependency resolution deterministic before the test compile.
- Put Docker, live Postgres, and full final gates in scheduled or manual workflows.
- Add release builds only to deploy/release workflows, not to every pull request.
