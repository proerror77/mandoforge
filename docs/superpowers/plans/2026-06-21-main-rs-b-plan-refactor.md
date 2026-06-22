# main.rs B 方案重构计划

目标：把 `crates/mandoforge-api/src/main.rs` 从巨型 composition 文件拆成稳定的类型层和 handler 层。采用 B 方案，但必须用 owner-domain-first 的方式执行，避免把 `common.rs` 变成新的 `main.rs`。

## 当前前置条件

- 先落地当前非重构改动：AgentVersion snapshot 字段、ontology onboarding migration、workflow schedules migration、task_grants constraints migration。
- 前置改动必须单独提交；之后的每个拆分 commit 必须是纯移动/可见性调整，不混入业务行为变化。
- 所有拆分步骤保持现有 `#[cfg(test)] mod tests` 在 `main.rs`，暂不迁移测试。

## 模块归属规则

- 类型归属按业务 owner，不按引用数量。
- `Agent` / `AgentVersion` / runtime profile / environment 归 `types::agent`。
- `Session` / session events / session loop jobs / session threads 归 `types::session`。
- `WorkflowDefinition` / `WorkflowRun` / step runs / transitions / task grants / dynamic workflow plans 归 `types::workflow`。
- `WorkflowPack*` 归 `types::workflow_pack`。
- `Ontology*` 归 `types::ontology`，semantic graph/memory 类型归 `types::semantic`。
- `Approval*` 归 `types::approval`，RBAC/tenant/org/team/project 类型归 `types::tenant`。
- `common.rs` 只放无业务 owner 的通用 DTO 或 tiny primitives。不要放 `Agent`、`WorkflowRun`、`Session` 这类被多处引用的核心实体。
- `AppState` 后续拆到 `state.rs`，不是业务 type。
- `AppError` 后续拆到 `error.rs`，不是 `types::common`。

## 目标结构

```text
src/
  error.rs
  state.rs
  types/
    mod.rs
    agent.rs
    session.rs
    workflow.rs
    workflow_pack.rs
    ontology.rs
    semantic.rs
    approval.rs
    tenant.rs
    remote_computer.rs
    usage.rs
    provider.rs
    mcp.rs
    eval.rs
    tools.rs
    deployment.rs
    common.rs
  handlers/
    mod.rs
    agent.rs
    session.rs
    workflow.rs
    ontology.rs
    semantic.rs
    approval.rs
    tenant.rs
    remote_computer.rs
    usage.rs
    provider.rs
    mcp.rs
    eval.rs
    tools.rs
    deployment.rs
```

## 执行顺序

- [ ] Phase -1: 清理当前功能改动，单独验证并提交。
  - AgentVersion snapshot 字段和 migration 一起提交。
  - Ontology onboarding / workflow schedule / task_grant constraints migrations 修到可运行。
  - 验证：`cargo check --manifest-path crates/mandoforge-api/Cargo.toml`，以及相关 targeted tests。

- [ ] Phase 0: 建立类型模块骨架，不移动任何类型。
  - 新增 `src/types/mod.rs`，先只放模块声明占位。
  - `main.rs` 加 `mod types;`，不改变行为。
  - 验证并提交。

- [ ] Phase 1: 拆 leaf / low-coupling 类型。
  - `types::tools`: `ToolDescriptor` 和 tool marker structs。
  - `types::eval`: eval dataset/case/run/judge profile DTO。
  - `types::deployment`: deployment/readiness DTO。
  - 每个 domain 单独 commit。

- [ ] Phase 2: 拆核心实体 owner 类型。
  - `types::agent`：Agent、AgentVersion、runtime profile、environment、agent release DTO。
  - `types::session`：Session、SessionStatus、SessionEvent、SessionLoopJob、SessionThread。
  - `types::tenant`：Organization、Team、Project、Membership、tenant isolation DTO。
  - 每个 domain 单独 commit。

- [ ] Phase 3: 拆 workflow / ontology 大块类型。
  - 先拆 `types::workflow_pack`，再拆 `types::workflow`。
  - 先拆 `types::semantic`，再拆 `types::ontology`。
  - 这些模块引用多，禁止和 handler 移动同 commit。

- [ ] Phase 4: 拆 `AppError` 和 `AppState`。
  - `AppError` 移到 `error.rs`，统一 `crate::AppError` re-export。
  - `AppState` 移到 `state.rs`，保持 store modules 的 `use crate::AppState` 不变。

- [ ] Phase 5: 拆 handlers。
  - 每次只迁移一个 router/handler domain。
  - handler 迁移必须保持现有 routes 不变。
  - 对每个迁移 domain 跑对应 API/targeted tests。

## 每步检查

- `cargo check --manifest-path crates/mandoforge-api/Cargo.toml`
- 对应 targeted tests，例如：
  - `cargo test --manifest-path crates/mandoforge-api/Cargo.toml reads_agent_versions_for_agent -- --nocapture`
  - `cargo test --manifest-path crates/mandoforge-api/Cargo.toml workflow_run -- --nocapture`
  - `cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_release -- --nocapture`
- `git diff --check`

## 禁止事项

- 不把类型移动和 SQL migration、业务行为改动混在同一个 commit。
- 不一次移动 50+ 个类型。
- 不为了减少 import 把核心实体塞进 `common.rs`。
- 不迁移测试模块，除非后续专门做测试拆分计划。
