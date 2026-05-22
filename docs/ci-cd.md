# CI/CD

Mandoforge keeps the default CI path fast and moves full evidence runs to explicit workflows.

## Fast CI

`.github/workflows/ci.yml` runs on every push and pull request:

- `Static gates` builds the React/Vite web UI, verifies the emitted `web/` assets, checks JavaScript and shell syntax, Kubernetes manifest renderability, and evidence manifest validators without compiling Rust.
- `Rust tests` installs the stable Rust toolchain, restores Cargo registry and `target/` cache, runs `cargo fetch --locked`, checks formatting, and runs `cargo test --workspace --locked --all-targets -- --test-threads=1`.

The Rust job intentionally does not run both `cargo check` and `cargo test` on every PR. `cargo test --all-targets` already compiles the workspace test targets and avoids a second full compile path in ordinary CI.

## Evidence Gates

`.github/workflows/evidence.yml` is for heavier verification:

- Scheduled runs execute `./scripts/stage1-final-gate.sh`.
- Manual runs can opt into `RUN_LIVE=1 START_LIVE_STACK=1 ./scripts/stage1-final-gate.sh`.
- The workflow also performs a Docker image build dry run.

Use this workflow for full gate confidence when runtime, Docker, or Postgres behavior matters. Keep these checks out of the default PR path unless the change requires them.

## Deploy

`.github/workflows/deploy.yml` is manual and uses the `stage2-production` GitHub Environment. It builds the React/Vite web UI, builds a `linux/amd64` Docker image, and pushes it to GHCR by default.

To update the Whiskey adoption stack through GitHub Actions, run the workflow with:

- `image_tag`: the release tag to deploy, for example `whiskey-YYYYMMDD-<sha>`.
- `publish_image`: `true`.
- `deploy_whiskey`: `true`.
- `whiskey_remote_host`: the Whiskey host or IP, unless the `WHISKEY_REMOTE_HOST` secret is configured.
- `whiskey_remote_user`: optional; falls back to `WHISKEY_REMOTE_USER`, then `root`.
- `whiskey_remote_root`: defaults to `/opt/mandoforge-adoption`.

Whiskey deployment also requires the `WHISKEY_SSH_PRIVATE_KEY` repository or environment secret. The workflow pushes the image with GitHub's package token, then runs `scripts/whiskey-adoption-deploy.sh` over SSH so Whiskey only pulls and restarts the adoption stack; it does not compile Rust on the Whiskey host.

It does not apply Kubernetes manifests or execute production business actions. Stage 2 high-risk actions remain approval-gated until production policy explicitly enables them.

## Rust Compile Cost Rules

- Prefer one Rust compile lane in PR CI.
- Cache Cargo registry, git dependencies, and `target/` by `Cargo.lock` plus `Cargo.toml` hashes.
- Use `cargo fetch --locked` to make dependency resolution deterministic before the test compile.
- Put Docker, live Postgres, and full final gates in scheduled or manual workflows.
- Add release builds only to deploy/release workflows, not to every pull request.
