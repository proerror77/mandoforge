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

MandoForge 要贴近的是 Claude Managed Agents 风格的产品模型：

```text
Agent -> Environment -> Session -> Events -> Threads
```

通用治理执行链路仍然重要，但它应该位于这层 managed-agent 表面之下：

```text
创建或恢复 Session
-> 写入 user/tool/approval Event
-> worker queue claim session-loop work
-> 调用模型
-> 解析工具调用
-> 检查策略
-> 需要时暂停审批
-> 人类批准、拒绝，或回写 tool result
-> 在 workspace / sandbox / Codex / MCP / Remote Computer Environment 中执行工具
-> 生成 artifact
-> 写入 Events、Threads 和 Audit
-> 实时回流并回放完整 timeline
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

当前最重要的下一步不是做某个行业 demo，而是让 Agent 中台贴近 Claude
Managed Agents 的产品模型，同时保持自托管、provider-neutral 和 policy-governed：

```text
Agent -> Environment -> Session -> Events -> Threads
```

近期重点是 **Managed Session Runtime**：

```text
创建或恢复 Session，写入 user events，让 Orchestrator loop 通过受治理的 Environment queue 执行，并把 model/tool/session events 实时回流到 UI。
```

这是对之前 Remote Computer-first 叙述的修正。Remote Computer 仍然重要，
但它应该是 Environment 的一种实现，而不是最高层产品对象。

短期先做 event-driven session state：

- 添加 first-class Environment 资源，放在 runtime profiles 和 Remote Computer profiles 之上。
- 让 `POST /api/sessions/:id/events` 成为驱动任务的主入口。
- 把 `POST /api/sessions/:id/run` 降级为向 session 写入 user event 的兼容 wrapper。
- 把 Orchestrator 执行从 API request path 里移出来，交给 queue-claimed session loop。
- 把 session status、model spans、tool use、approvals、artifacts、child threads 实时回流到 UI。

当前 managed-agent baseline：

- `GET/POST /api/environments` 和 `GET/PATCH/DELETE /api/environments/:id` 管理第一等 Environment 资源。
- `POST /api/sessions` 接受 `environment_id`，并在 session event log 写入 `session.environment_bound`。
- `POST /api/sessions/:id/events` 会 enqueue 可被 worker lease claim 的 `session_loop_job`；`mandoforge-worker` 在 API request path 之外执行 orchestrator loop。
- session 执行会写入 managed-agent 风格的 `session.status_*`、`span.model_request_*`、`agent.tool_use`、`agent.tool_result` 和 `thread.*` timeline events。
- `GET /api/sessions/:id/threads` 暴露 durable `session_threads`；Manager 到 Specialist 的 typed handoff 会创建挂在 parent session 下的 child specialist thread。
- `Environment(type=remote_computer)` 现在负责自动 Remote Computer 分配：approved execution jobs 只会自动 claim 与 session environment contract 匹配的 lease 或 warm-pool resource；绑定 remote environment 但未启用 Remote Computer execution transport 时会 fail closed，不会静默退回本地执行。
- UI 的开始任务表单会加载 environments，并把新 session 绑定到选择的环境。
- UI 的运行路径先围绕 managed-session 对象组织：Agent、Environment、Event Stream、Blocking Actions、Artifacts 和 Threads；worker、Remote Computer、provider、Vault、MCP、tenant 等底层设施保留在系统状态和高级面板里。

当前对齐缺口记录在 [Claude Managed Agents Alignment](docs/claude-managed-agents-alignment.md)
和 [Stage 2 / Stage 3 Roadmap](docs/stage2-stage3-roadmap.md)。最重要的剩余工作是把
Claude-style contract 做成端到端闭环：可恢复的非 terminal idle session、基于
event cursor 的 loop processing、live streaming、environment queue binding、
lease-fenced job finalization，以及能证明 worker restart / session recovery 的生产证据。

剩余生产化工作也包括集群证据：runtime 已强制执行
`Environment(type=remote_computer)` policy，但真实 Kubernetes Pod execution 仍依赖已配置的
Remote Computer transport，以及外部 state-sync / sidecar / worker-pool evidence gates。

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

常驻 worker / orchestrator loop：

```bash
MANDOFORGE_EXECUTION_WORKER=queue \
MANDOFORGE_DEV_ADMIN_TOKEN=local-worker-token \
cargo run -p mandoforge-api

BASE_URL=http://127.0.0.1:8787 \
MANDOFORGE_WORKER_TOKEN=local-worker-token \
cargo run -p mandoforge-api --bin mandoforge-worker
```

这个 worker 会消费 session-loop jobs 和已批准的 execution jobs。用户从 UI
或 `/api/sessions/:id/events` 写入任务后，session-loop job 由 worker claim，
再进入 provider / tool / approval / execution queue 路径。

受管 coding-agent CLI profile：

```bash
# Codex CLI profile
curl -sS -X POST "$BASE_URL/api/agent-runtime-profiles" \
  -H 'content-type: application/json' \
  -H 'x-mandoforge-subject: admin-1' \
  -H 'x-mandoforge-roles: admin' \
  -d '{
    "name": "codex-cli-worker",
    "runtime_type": "codex_cli",
    "command": "/usr/bin/codex",
    "default_args": ["exec", "--json"],
    "remote_computer_required": true
  }'

# Claude Code CLI profile
curl -sS -X POST "$BASE_URL/api/agent-runtime-profiles" \
  -H 'content-type: application/json' \
  -H 'x-mandoforge-subject: admin-1' \
  -H 'x-mandoforge-roles: admin' \
  -d '{
    "name": "claude-code-worker",
    "runtime_type": "claude_code",
    "command": "/usr/local/bin/claude",
    "default_args": ["--print"],
    "remote_computer_required": true
  }'
```

把其中一个 profile 绑定到 specialist agent 或 handoff assignment 后，再用
`agent_cli.exec` 传入匹配的 `profile` 和 `task`。批准后的 execution job 由
`mandoforge-worker` 消费，结果会为了兼容旧路径继续记录 `profile`、
`runtime_type`、`stdout`、`stderr`、截断标记和退出状态。受管 `codex_cli`、
`claude_code`、Gemini、OpenCode、Aider profile 会被当作 runtime adapter：
它们的 JSONL 或 stream-json 输出会被写成 `runtime_adapter.event` session
events，并带基础 secret-key redaction 和 event-count limits。这样 CLI-backed
agents 仍然在 Tool Router、Policy Engine、Approval Engine、worker lease、
Remote Computer、event log、audit path 之内，同时产品语义会往
Environment-owned runtime adapter 推进。`agent_cli.exec` 仍然是兼容 facade；
目标 Managed Agents 模型是 `Agent -> Environment -> Session -> runtime
adapter -> Events`。

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
- [Claude Managed Agents Alignment](docs/claude-managed-agents-alignment.md)
- [Whiskey Adoption Runbook](docs/whiskey-adoption-runbook.md)
- [Whiskey Adoption Status](docs/whiskey-adoption-status.md)
- [MandoForge Roadmap v2](docs/mandoforge-roadmap-v2.md)
- [Stage 2 / Stage 3 Roadmap](docs/stage2-stage3-roadmap.md)
- [Workflow Pack Adaptation Plan](docs/workflow-pack-adaptation-plan.md)
- [WorkflowPack Manifest Contract](docs/workflow-pack-manifest-contract.md)
- [Agent Remote Computer Plan](docs/agent-remote-computer-plan.md)
- [Deployment And Demo Guide](docs/deployment-guide.md)

## 一句话总结

MandoForge 的定位是：

```text
一个可以被不同行业 Agent 复用的 Agent 中台内核。
它负责运行、治理、审批、审计和回放 Agent 作业。
业务 Agent 在它上面生长，而不是每个业务 Agent 都重做一套运行时。
```
