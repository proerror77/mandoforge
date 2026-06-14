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

## Agent OS 分层

MandoForge 的主架构是 Agent OS，不是生产验收证据包：

```text
Existing Work Surfaces
  Slack / 飞书 / GitHub / Jira / Linear / Email
        |
Collaboration Layer
  WorkItem / Project / Assignment / Review
  Agent Teammate / Squad / Activity Feed
        |
Manager Agent / Work Coordination Layer
  Task decomposition / routing / escalation / review
        |
Managed Runtime Layer
  Session / Event Log / Tool Router / Policy
  Approval / Audit / Artifact / Eval
        |
Semantic Layer / Ontology Service
  Business Objects / Metrics / Relations
  Actions / Permissions / Tool Bindings
  Retrieval Context / Data Contracts
        |
Enterprise Data Foundation
  Warehouse / Lakehouse / Postgres / Vector
  Graph / Docs / APIs / Event Streams
```

Claude Managed Agents 的 `Agent -> Environment -> Session -> Events -> Threads`
只适合作为 Managed Runtime Layer 的参考模型，不是整个 Agent OS 的产品架构。
上层协作、Manager Agent、Semantic Layer 仍然是 MandoForge 自己的 Agent OS 产品层。

Runtime 边界要固定成：

```text
MandoForge Agent Runtime
  -> Codex CLI / Claude Code CLI / Codex App Server runtime adapters
  -> normalized events, tool calls, artifacts, audit logs
```

MandoForge 负责 session、context、policy、approval、audit、artifact、resume
cursor、streaming 和 worker lease。Codex CLI / Claude Code CLI 是被
MandoForge 调用和监管的执行后端。Manager Agent 是跑在这个 runtime 上的
managed agent，负责 WorkItem / Assignment 协调，不拥有另一套执行栈。

## 它不是哪些东西？

- 它不是一个现成的垂直行业 SaaS。
- 它不是只服务电商、金融、客服或代码场景。
- 它不是单纯的 Prompt 管理工具。
- 它不是只包装 OpenAI API 的轻量 demo。
- 它现在也还不是生产级完整平台。

它当前更准确的状态是：**一个 Rust 写的 Agent OS Kernel 雏形**。Managed Runtime Layer 已经支撑 repo-controlled pilot；Collaboration、Manager Agent 和 Semantic Layer 是下一步产品化重点。

## 核心运行闭环

Managed Runtime Layer 可以参考 Claude Managed Agents 风格的运行时模型：

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

当前实现状态记录在
[docs/runtime-truth-audit.md](docs/runtime-truth-audit.md)：哪些 runtime 能力已经落地，哪些仍是核心缺口。

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
- Secret reference / credential 边界。
- Worker queue、Redis / NATS handoff、worker readiness。
- Approval v2：修改参数、委托审批、过期、升级、通知。
- MCP Gateway 管理。
- Codex App Server adapter。
- Eval / Release gate / rollback。
- Observability、usage、cost tracking。
- Scheduler due-run。
- Remote Computer readiness skeleton。

Stage 2 的 repo-controlled pilot 已完成。核心完成证据应该是 runtime action record：session events、tool calls、approvals、artifacts 和 audit logs。

## 现在最重要的设计方向

当前最重要的下一步不是做某个行业 demo，而是围绕 runtime kernel 往完整
Agent OS 分层上推进：

```text
Work Surfaces -> Collaboration -> Manager Agent / Work Coordination -> Managed Runtime -> Semantic Layer -> Data Foundation
```

近期重点仍然是 **Managed Session Runtime**，因为上层 Collaboration / Manager
Agent / Semantic Layer 都依赖一个可靠的 event-driven runtime：

```text
创建或恢复 Session，写入 user events，让 runtime session loop 通过受治理的 Environment queue 执行，调用选定的 CLI/runtime adapter，并把 model/tool/session events 实时回流到 UI。
```

这是对之前 Remote Computer-first 叙述的修正。Remote Computer 仍然重要，
但它应该是 Environment 的一种实现，而不是最高层产品对象。

短期先做 event-driven session state：

- 添加 first-class Environment 资源，放在 runtime profiles 和 Remote Computer profiles 之上。
- 让 `POST /api/sessions/:id/events` 成为驱动任务的主入口。
- 把 `POST /api/sessions/:id/run` 降级为向 session 写入 user event 的兼容 wrapper。
- 把 runtime session-loop 执行从 API request path 里移出来，交给 queue-claimed worker path。
- 把 session status、model spans、tool use、approvals、artifacts、child threads 实时回流到 UI。

当前 managed-agent baseline：

- `GET/POST /api/environments` 和 `GET/PATCH/DELETE /api/environments/:id` 管理第一等 Environment 资源。
- `POST /api/sessions` 接受 `environment_id`，并在 session event log 写入 `session.environment_bound`。
- `Environment.runtime_profile_id` 是 session 的 canonical managed runtime-adapter 绑定。`agent_cli.exec` 仍是 CLI-backed adapters 的兼容 facade，但请求里的 profile 必须先匹配绑定的 Environment profile，之后才回退到 handoff 或 agent runtime profile；旧的 env-var allowlist 只在没有 managed binding 时生效。
- `POST /api/sessions/:id/events` 会 enqueue 可被 worker lease claim 的 `session_loop_job`；`mandoforge-worker` 在 API request path 之外执行 runtime session loop。
- session 执行会写入 managed-agent 风格的 `session.status_*`、`span.model_request_*`、`agent.tool_use`、`agent.tool_result` 和 `thread.*` timeline events。
- `GET /api/sessions/:id/threads` 暴露 durable `session_threads`；Manager 到 Specialist 的 typed handoff 会创建挂在 parent session 下的 child specialist thread。
- `Environment(type=remote_computer)` 现在负责自动 Remote Computer 分配：approved execution jobs 只会自动 claim 与 session environment contract 匹配的 lease 或 warm-pool resource；绑定 remote environment 但未启用 Remote Computer execution transport 时会 fail closed，不会静默退回本地执行。
- UI 的开始任务表单会加载 environments，并把新 session 绑定到选择的环境。
- UI 的运行路径先围绕 managed-session 对象组织：Agent、Environment、Event Stream、Blocking Actions、Artifacts 和 Threads；worker、Remote Computer、provider、secret、MCP、tenant 等底层设施保留在系统状态和高级面板里。
- Enterprise Ontology Fast-Onboarding 已通过 Semantic console 和 `/api/ontology/onboarding/*` 暴露：seed packs、demo runs、schema understanding、review graph、proposal review、materialization、calibration 和 compiled tool specs。用法见 [Ontology Builder Usage](docs/ontology-builder-usage.md)。

Runtime 对齐状态记录在 [Claude Managed Agents Alignment](docs/claude-managed-agents-alignment.md)
和 [Agent OS Product Roadmap](docs/stage2-stage3-roadmap.md)。核心 runtime contract
现在围绕可恢复的 idle session、基于 event cursor 的 loop processing、live
streaming、Environment-bound worker claim 和 lease-fenced job finalization 展开。
第一个 WorkItem intake、assignment-routing、review、Activity Feed、Agent
Teammate/Squad 和 Manager Plan binding 切片现在已经能持久化 collaboration work 并写入
audit evidence；下一步主线应该继续往 UI workflow surfaces、Semantic Objects 推进，而
不是继续扩展旁支部署验证包。

## 本地运行

启动 API：

```bash
MANDOFORGE_INSECURE_DEV_AUTH=1 \
MANDOFORGE_ALLOW_HOST_SHELL_EXEC=1 \
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

Ontology Builder fast-onboarding gate：

```bash
BASE_URL=http://127.0.0.1:8787 \
./scripts/verify-enterprise-ontology-fast-onboarding.sh
```

Postgres-backed restart/resume core evidence：

```bash
START_POSTGRES=1 ./scripts/managed-session-restart-resume-core-gate.sh
```

这个 gate 需要 Docker Desktop 或已有 `DATABASE_URL`。它会把 session event、
tool call、audit log、restart/resume、cursor、thread lineage 和 runtime turn
证据写到 `.mandoforge/managed-session-restart-resume-core-evidence/`。

常驻 worker / runtime session loop：

```bash
MANDOFORGE_EXECUTION_WORKER=queue \
MANDOFORGE_DEV_ADMIN_TOKEN=local-worker-token \
cargo run -p mandoforge-api

BASE_URL=http://127.0.0.1:8787 \
MANDOFORGE_WORKER_TOKEN=local-worker-token \
WORKER_POOL=managed-agent \
cargo run -p mandoforge-api --bin mandoforge-worker
```

这个 worker 会消费 session-loop jobs 和已批准的 execution jobs。用户从 UI
或 `/api/sessions/:id/events` 写入任务后，session-loop job 由 worker claim，
再进入 provider / tool / approval / execution queue 路径。`WORKER_ENVIRONMENT_ID`
会把 worker 绑定到单个 Environment id；`WORKER_POOL` 或 `WORKER_QUEUE`
会绑定到 `worker_queue_binding` 里同名 pool 的 Environments。

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

把其中一个 profile 绑定到 Environment、specialist agent 或 handoff assignment
后，MandoForge Agent Runtime 会通过 session-loop worker path 调用选定的
CLI/runtime adapter。`agent_cli.exec` 仍可以作为兼容 facade，传入匹配的
`profile` 和 `task`；批准后的 execution job 由 `mandoforge-worker` 消费。

受管 `codex_cli`、`claude_code`、Gemini、OpenCode、Aider profile 会被当作
runtime adapter：它们的 JSONL 或 stream-json 输出会被写成
`runtime_adapter.event` session events，并带基础 secret-key redaction 和
event-count limits。Codex CLI 和 Claude Code CLI 输出也会映射成 normalized
runtime turn records，覆盖 turn start、items/tool calls、usage、final
message、artifact 和 completion；Codex App Server turn API 也会用同一套
taxonomy 记录 thread/turn lineage。

这样 CLI-backed agents 仍然在 Tool Router、Policy Engine、Approval Engine、
worker lease、Remote Computer、event log、audit path 之内，同时产品语义会往
Environment-owned runtime adapter 推进。目标 Managed Agents 模型是：

```text
Agent -> Environment -> Session -> runtime adapter -> Events
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
- [Stage 2 Completion Audit](docs/stage2-completion-audit.md)
- [Claude Managed Agents Alignment](docs/claude-managed-agents-alignment.md) - 仅作为 runtime-layer 参考
- [MandoForge Roadmap v2](docs/mandoforge-roadmap-v2.md)
- [Agent OS Product Roadmap](docs/stage2-stage3-roadmap.md)
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
