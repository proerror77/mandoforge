# MandoForge

[English README](README.md)

## 它是什么？

MandoForge 是一个 **Agent 中台 / Agent 作业系统内核**。

它不是一个单独的聊天机器人，也不是某个行业的固定 Agent 应用。它更像企业内部运行 Agent 的“操作系统底座”：负责管理 Agent 怎么创建、怎么运行、怎么调用工具、怎么审批高风险动作、怎么留下审计记录、怎么回放整个执行过程。

更简单地说：

```text
业务 Agent 负责解决具体问题。
MandoForge 负责让这些 Agent 可控、可审计、可扩展地运行。
```

## 为什么说它是 Agent 中台？

一个真正能进企业或复杂业务场景的 Agent，不只是“调用一次模型”。它通常需要：

- 有身份和权限：谁能创建、运行、审批、查看 Agent。
- 有任务过程：一次 Agent 作业从开始到结束要能追踪。
- 有工具调用：读文件、查数据库、执行命令、调用 MCP、调用 Codex。
- 有风险控制：高风险动作必须暂停，等人审批。
- 有审计记录：事后能知道模型说了什么、工具做了什么、谁批准了什么。
- 有运行环境：不同 Agent 不能随便污染彼此的 workspace。
- 有成本和治理：provider、token、预算、发布、回滚、评估都要能管理。

MandoForge 做的就是这一层公共能力。上面可以长出各行各业的 Agent：

- 电商运营 Agent
- 金融分析 Agent
- 客服质检 Agent
- 数据分析 Agent
- 内部办公 Agent
- 代码执行 Agent
- 法务 / 合同 / 审批 Agent
- 任何需要“工具 + 审批 + 审计 + 回放”的企业 Agent

## 它不是哪些东西？

- 它不是一个现成的垂直行业 SaaS。
- 它不是只服务电商、金融、客服或代码场景。
- 它不是单纯的 Prompt 管理工具。
- 它不是只包装 OpenAI API 的轻量 demo。
- 它现在也还不是生产级完整平台。

它当前更准确的状态是：**一个 Rust 写的 Agent OS Kernel 雏形，Stage 1 和 repo-controlled Stage 2 governed-runtime pilot 已经完成。** 真实生产环境还需要按 adoption backlog 跑环境级证据，才能声明某个生产部署已经验证。

## 核心运行闭环

MandoForge 要沉淀的是这条通用 Agent 作业链路：

```text
创建 Agent
-> 创建 Session
-> 调用模型
-> 解析工具调用
-> 检查策略
-> 需要时暂停审批
-> 人类批准或拒绝
-> 在 workspace / sandbox 中执行工具
-> 生成 artifact
-> 写入事件和审计
-> 回放完整 timeline
```

这条链路一旦稳定，就可以被不同业务 Agent 复用。业务层只需要换 Agent 配置、工具、数据源和审批策略，不需要每次重新发明运行时底座。

## 当前已经有什么？

Stage 1 已经实现了通用运行时内核：

- Rust + Axum API server。
- Postgres 持久化存储，缺少 `DATABASE_URL` 时可以回退到内存模式。
- Agent、Agent Version、Session、Event、Tool Call、Approval、Artifact、Audit Log。
- Tool Router：统一管理工具调用。
- Policy Engine：决定工具调用是允许、拒绝还是需要审批。
- Approval Queue：高风险动作暂停后由人处理。
- Session Timeline：回放一次 Agent 作业的完整过程。
- 静态 Web Console：能看 Agent、Session、Timeline、Approval、Artifact、Audit。
- Docker Compose 和 Kubernetes skeleton。

Stage 2 已经补上 repo-controlled pilot 需要的治理能力：

- 组织 / 团队 / 项目 scope。
- RBAC 权限控制。
- Workflow Pack / Domain Pack：可安装、可版本化、可审计的行业工作流包。
- Provider 管理、模型 allowlist、预算和健康检查。
- Vault secret reference 边界。
- Worker queue、Redis / NATS handoff、worker readiness。
- Approval v2：修改参数、委托审批、过期、升级、通知。
- MCP Gateway 管理。
- Codex App Server adapter。
- Eval / Release gate / rollback。
- Observability、usage、cost、finance operations。
- Scheduler due-run。
- Remote Computer readiness skeleton。

Stage 2 的 repo-controlled pilot 已完成。严格审计和真实外部生产 adoption backlog 见：[Stage 2 Completion Audit](docs/stage2-completion-audit.md)。

## 现在最重要的设计方向

当前最重要的下一步不是做某个行业 demo，而是把 Agent 中台的运行底座继续打稳。

近期重点是 **Remote Computer**：

```text
让一次 Agent 作业可以绑定到一个可租用、可隔离、可审计的远程执行环境。
```

短期先做 session attach state：

- 把 Session 绑定到一个 Remote Computer lease。
- 持久化 attach / release 状态。
- 检测 stale attachment。
- 暂时不把工具执行搬进 Pod。
- 保证不绕过 Tool Router、Policy Engine 和 Approval Engine。

长期方向是让 Remote Computer 成为主要 sandbox substrate：Agent 可以在隔离 Pod / workspace 中执行任务，同步 artifact 和 timeline，保留完整审计链路。

## 本地运行

启动 API：

```bash
cargo run -p mandoforge-api
```

打开控制台：

```text
http://127.0.0.1:8787
```

基础检查：

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/smoke.sh
```

完整 Stage 1 demo：

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/stage1-demo.sh
```

最终 gate：

```bash
./scripts/stage1-final-gate.sh
```

Docker Desktop 可用时：

```bash
RUN_LIVE=1 START_LIVE_STACK=1 ./scripts/stage1-final-gate.sh
```

## Docker

```bash
docker compose up --build
```

API 默认在：

```text
http://127.0.0.1:8787
```

## Kubernetes

Kubernetes skeleton 在 [deploy/k8s](deploy/k8s)。

```bash
kubectl apply -k deploy/k8s
kubectl -n agent-os port-forward svc/mandoforge-api 8787:8787
```

这些 manifest 是自托管 pilot 起点，不是生产加固声明。

## 重要文档

- [Runtime Architecture](docs/architecture.md)
- [Stage 1 Plan](docs/stage1-plan.md)
- [Stage 1 Completion Audit](docs/stage1-completion-audit.md)
- [Stage 2 Gap Audit](docs/stage2-gap-audit.md)
- [Stage 2 Completion Audit](docs/stage2-completion-audit.md)
- [Stage 2 Production Adoption Runbook](docs/stage2-production-adoption-runbook.md)
- [Stage 2 / Stage 3 Roadmap](docs/stage2-stage3-roadmap.md)
- [Workflow Pack Adaptation Plan](docs/workflow-pack-adaptation-plan.md)
- [Agent Remote Computer Plan](docs/agent-remote-computer-plan.md)
- [Deployment And Demo Guide](docs/deployment-guide.md)

## 一句话总结

MandoForge 的定位是：

```text
一个可以被不同行业 Agent 复用的 Agent 中台内核。
它负责运行、治理、审批、审计和回放 Agent 作业。
业务 Agent 在它上面生长，而不是每个业务 Agent 都重做一套运行时。
```
