# Stage 2 Production Adoption And Stage 3 Roadmap

This roadmap separates two different kinds of work:

- Stage 2 production adoption proves the completed repo-controlled Governed Runtime Pilot against real external targets.
- Stage 3 turns the runtime into a productized Enterprise Agent OS with external scheduling, richer traces, production-grade Remote Computer execution, workflow packs, and typed agent-team handoffs.

Do not treat Stage 2 production adoption as a reason to reopen the repo-controlled Stage 2 completion decision. The Stage 2 pilot is complete in this repository. Production adoption is environment-specific evidence work.

## Stage 2 Production Adoption

### Goal

Run the existing fail-closed evidence gates against real deployment targets and archive the resulting proof.

The output should answer:

- Which environment was validated.
- Which controller targets were used.
- Which evidence artifacts were captured.
- Which gates stayed blocked.
- Which adoption backlog items remain for that environment.

### Workstream S2-A: Evidence Baseline

Owner: integration owner.

Deliverables:

- Production adoption runbook.
- Controller environment matrix.
- Secret rendering checklist.
- Evidence PVC and archive procedure.
- Required artifact map for each production target.

Verification:

```bash
scripts/verify-stage2-controller-env-template.sh
scripts/verify-stage2-evidence-k8s-manifests.sh
scripts/verify-stage2-evidence-archive.sh --self-test
```

### Workstream S2-B: Cluster And Isolation Proof

Primary lanes: Lane A, Lane C, Lane D, Lane E.

Deliverables:

- Tenant routing evidence against a real multi-tenant deployment.
- Worker load-validation evidence against a real queue and cluster profile.
- Remote Computer state-sync and sidecar-recovery evidence against real distributed state storage.
- OTel collector deployment, rollout, and remediation evidence.

Required evidence gates:

```bash
scripts/tenant-isolation-evidence-gate.sh
scripts/worker-evidence-gate.sh
scripts/remote-computer-evidence-gate.sh
scripts/observability-collector-evidence-gate.sh
```

### Workstream S2-C: Governance Control Plane Proof

Primary lanes: Lane B and Lane E.

Deliverables:

- Policy rollout orchestration proof.
- Provider gate, rollout, and rollback proof.
- MCP connector deployment, rollout, and rollback proof.
- Eval/release rollout, orchestration, and rollback proof.

Required evidence gates:

```bash
scripts/policy-rollout-evidence-gate.sh
scripts/provider-governance-evidence-gate.sh
scripts/mcp-gateway-evidence-gate.sh
scripts/eval-release-evidence-gate.sh
```

### Workstream S2-D: Business Safety Proof

Primary lanes: Lane B, Lane C, and Lane E.

Deliverables:

- Vault/KMS/HSM rotation and recovery evidence.
- Approval notification delivery evidence against real delivery targets.
- Codex App Server deployment and production-ops evidence.
- Finance close, export delivery, and reconciliation evidence.

Required evidence gates:

```bash
scripts/vault-evidence-gate.sh
scripts/approval-notification-evidence-gate.sh
scripts/codex-app-server-evidence-gate.sh
scripts/finance-evidence-gate.sh
```

### Workstream S2-E: Adoption Closeout

Owner: integration owner.

Deliverables:

- Strict production evidence bundle run.
- Evidence archive, checksum, and manifest.
- Updated completion audit section naming the validated environment.
- Residual adoption backlog for anything still blocked.

Verification:

```bash
scripts/stage2-production-evidence-gate.sh
scripts/stage2-completion-audit-gate.sh
scripts/archive-stage2-production-evidence.sh
scripts/verify-stage2-evidence-archive.sh <archive>
```

## Stage 3 Product Roadmap

### Goal

Build the product layer above the governed runtime: scheduled operations, operator-grade traces, real execution substrates, portable workflow packs, and native multi-agent handoffs.

### Workstream S3-A: External Scheduler Integration

Primary lane: Lane E.

Scope:

- Promote scheduler due-plan and run-due APIs into a real external scheduler contract.
- Add idempotency keys, run windows, retry policy, and run ownership metadata.
- Persist scheduler execution plans and outcomes as first-class audit evidence.
- Keep mutable actions fail-closed unless their readiness gates pass.

Acceptance criteria:

- Operators can preview, trigger, replay, and audit scheduled work.
- Scheduler runs are safe to retry.
- Stage 2 evidence jobs can use the same scheduler contract instead of bespoke orchestration.

### Workstream S3-B: Codex App Server Trace Dashboard

Primary lane: Lane C.

Scope:

- Expand per-turn traces across thread, turn, command, poll, interrupt, artifact sync, worker lease, and retry events.
- Add timeline views for terminal state, stuck turns, stale polls, and fallback to CLI.
- Surface artifact lineage from App Server response to MandoForge artifact row.

Acceptance criteria:

- An operator can answer why a Codex turn is stuck or failed without reading raw logs.
- Trace views distinguish provider failure, worker lease failure, non-terminal polling, and artifact sync failure.
- Long-running turn supervision has evidence suitable for scheduler and production ops gates.

### Workstream S3-C: Production Remote Computer Execution

Primary lane: Lane D.

Scope:

- Productize the assigned-Pod execution transport that Stage 2 keeps behind explicit gates.
- Keep Tool Router, Policy Engine, Approval Engine, event log, and audit paths authoritative.
- Support cancellation through assigned Pod deletion or command interruption.
- Capture stdout/stderr bounds, final artifacts, sidecar heartbeats, state locks, and assignment lifecycle.

Acceptance criteria:

- Approved `file.write`, `shell.exec`, and `codex.exec` can run through an assigned Remote Computer without bypassing policy or audit.
- Failed or stale Remote Computer assignments are recoverable through worker lease retry.
- Artifact discovery and push sync produce replayable evidence.

### Workstream S3-D: WorkflowPack / DomainPack Platform

Primary lanes: Lane A, Lane B, and Lane E.

Scope:

- Define the `WorkflowPack` and `DomainPack` manifest contract.
- Validate pack layout, connectors, schemas, policies, evals, profiles, and worker roles.
- Add profile onboarding contracts for company, department, approval matrix, connector map, risk policy, and output style.
- Ship the first AI Governance Pack slice.

Acceptance criteria:

- A pack can be installed, validated, staged, evaluated, and released without silently changing runtime safety.
- Connectors declare provenance, tenant scope, write gating, and prompt-injection boundaries.
- Pack behavior has eval fixtures and release gates.

### Workstream S3-E: Typed Agent Team Handoffs

Primary lanes: Lane A, Lane B, and Lane C.

Scope:

- Add typed `agent_handoff_events`.
- Enforce allowlisted source agent, target agent, intent enum, schema version, risk level, and approval requirement.
- Persist request, accept, reject, fail, and complete transitions in timeline and audit logs.
- Support Reader / Analyzer / Writer role boundaries for untrusted input workflows.

Acceptance criteria:

- Agent-to-agent routing is not free-form natural language.
- High-risk handoffs require approval before downstream work starts.
- Handoff chains are replayable and visible in the operator console.

### Workstream S3-F: Product Hardening

Primary owner: integration owner.

Scope:

- Split remaining global hotspots before they block parallel work.
- Keep readiness summaries consistent across lanes.
- Add migration numbering discipline for parallel branches.
- Preserve strict CI gates and add targeted Stage 3 verification scripts.

Acceptance criteria:

- Multiple lane agents can develop Stage 3 features without repeatedly colliding in `main.rs`.
- Every Stage 3 feature has a lane-local verification path and an integration gate.

## Recommended Execution Order

1. Create the Stage 2 production adoption runbook and controller matrix.
2. Start Stage 3 with the WorkflowPack manifest contract because it is mostly additive and clarifies product direction.
3. In parallel, prototype typed handoff events because WorkflowPack worker roles depend on them.
4. Move Remote Computer execution after the handoff/event model is stable enough to preserve audit semantics.
5. Expand Codex traces and scheduler integration as the operational layer over the execution substrate.

## Non-Goals

- Do not claim a production environment is validated from local mock controller evidence.
- Do not allow Workflow Packs to bypass tenant scope, policy, approval, or audit.
- Do not let Remote Computer execution become a side channel around the Tool Router.
- Do not add free-form agent handoff messages as the primary routing contract.
- Do not mix Stage 2 production adoption evidence changes with unrelated Stage 3 product work in one commit.
