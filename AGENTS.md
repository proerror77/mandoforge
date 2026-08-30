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
- Pull requests must be reviewed before merge. Do not merge a PR until the review result is `APPROVE` or all blocking findings have been fixed and re-reviewed.
- Merge only the reviewed PR contents. Do not include unrelated local worktree changes, generated artifacts, or cleanup unless they are explicitly part of the reviewed PR scope.

## Delivery Discipline

- Substantial work starts from a GitHub issue with explicit acceptance criteria, and its branch and pull request reference that issue.
- Refresh `origin/main` before branching. Keep local `main` aligned with `origin/main`; do not use it for unpublished commits.
- Open substantial pull requests as drafts early. Do not stack unrelated work on an unreviewed pull request.
- Report `Code`, `CI`, `Review`, `Merge`, `Publish`, `Deploy`, `Runtime`, and `Readback` separately. Never treat an earlier state as proof of a later one.

## Project Notes

- This repo is the Rust-native Managed Agents / Enterprise Agent OS runtime described in the Stage 1 PRD.
- Stage 1 is a generic Agent OS Kernel loop: session event log, provider harness, tool router, policy approval, workspace execution, artifacts, audit logs, timeline UI, and final gates.
- High-risk business actions must stay draft/approval-only until a later production policy explicitly enables execution.

## Agent skills

### Issue tracker

Issues are tracked in GitHub; external PRs are not a triage request surface. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the default Matt Pocock triage label vocabulary. See `docs/agents/triage-labels.md`.

### Domain docs

This is a single-context repository. See `docs/agents/domain.md`.
