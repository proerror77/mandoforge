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

## Project Notes

- This repo is the Rust-native Managed Agents / Enterprise Agent OS runtime described in the Stage 1 PRD.
- Stage 1 is a generic Agent OS Kernel loop: session event log, provider harness, tool router, policy approval, workspace execution, artifacts, audit logs, timeline UI, and final gates.
- High-risk business actions must stay draft/approval-only until a later production policy explicitly enables execution.
