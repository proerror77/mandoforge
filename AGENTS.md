# Codex User Instructions

- Work autonomously on clear, reversible tasks.
- Ask only when the next step is destructive, irreversible, or genuinely ambiguous.
- Prefer evidence over assumption; verify before claiming completion.
- Preserve unrelated user changes and avoid broad cleanup unless requested.
- Use project-local `AGENTS.md` files when present.

## Commit Discipline

- Use atomic commits: each commit should represent one coherent, reviewable unit of work.
- Do not mix unrelated refactors, docs, tests, and feature changes unless they are required for the same deliverable.
- Before committing, verify the commit diff matches the stated intent and does not include unrelated local artifacts.
- Commit messages should state the concrete behavior or artifact changed, not vague progress.

## Review and Merge Discipline

- Before modifying implementation files, perform a scope review: identify the intended files, existing dirty changes, unrelated local artifacts, and verification plan.
- For substantial runtime, policy, security, or orchestration changes, perform a deep review before committing. The review should check correctness, authorization boundaries, failure modes, test coverage, and whether unrelated changes are being mixed in.
- Before merge, record a review verdict (`APPROVE` or `REQUEST_CHANGES`) against the exact current PR head SHA, including scope and checks. Re-review after any subsequent change; fixing findings alone does not transfer an earlier approval to a new head.
- Merge requires `APPROVE` for that exact head with no unresolved blocking findings. `REQUEST_CHANGES` blocks merge until the findings are fixed and the resulting head is approved.
- Local or agent review is distinct from GitHub's required reviewer approvals and branch protection. Satisfy both applicable requirements; never describe a local verdict as a GitHub approval or bypass protection to complete a task.
- Merge only the reviewed PR contents. Do not include unrelated local worktree changes, generated artifacts, or cleanup unless they are explicitly part of the reviewed PR scope.

## Delivery Discipline

- Routine documentation edits and localized reversible fixes use focused checks. Substantial work includes new runtime behavior, authorization or policy changes, persistence changes, and orchestration across components; file count alone does not determine the workflow.
- Substantial work starts from a GitHub issue with explicit acceptance criteria, and its branch and pull request reference that issue.
- Refresh `origin/main` before branching. Keep local `main` aligned with `origin/main`; do not use it for unpublished commits.
- Open substantial pull requests as drafts early. Do not stack unrelated work on an unreviewed pull request.
- Report `Code`, `CI`, `Review`, `Merge`, `Publish`, `Deploy`, `Runtime`, and `Readback` separately. Never treat an earlier state as proof of a later one.
- Stop at the requested delivery state. An issue or draft PR required by this workflow does not authorize merge, release, deployment, external messages, or business writes. Reuse still-valid checks for the same head and environment; repeat when code, inputs, environment, or required checks change.
- Skills must follow this scope and existing user authorization. Stage only task-owned files. Keep screenshots local unless sharing to a specified destination is authorized; generic skill instructions do not authorize public uploads.

## Project Notes

- This is a Rust-native Managed Agents / Enterprise Agent OS. Before changing architecture or claiming runtime completion, read `docs/runtime-truth-audit.md` and the relevant implementation. Use `docs/enterprise-product-completion-contract.md` for production completion claims; the Stage 1 PRD is historical context, not a current scope ceiling.
- High-risk business actions must stay draft/approval-only until a later production policy explicitly enables execution.

## Agent skills

- Use diff review for a specified PR, branch, or working-tree change; use repository analysis for architecture and workflow audits. Preserve staged, unstaged, and untracked review scope explicitly.
- Business skills in `packs/*/skills/` use package-manifest IDs and paths, not the developer-skill frontmatter convention. Verify source loading, pinned runtime instructions, and behavior separately from manifest validation.
### Issue tracker

Issues are tracked in GitHub; external PRs are not a triage request surface. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the default Matt Pocock triage label vocabulary. See `docs/agents/triage-labels.md`.

### Domain docs

This is a single-context repository. See `docs/agents/domain.md`.
