//! A per-execution MCP capability. It never authenticates at the general API.
//! Session/grant/application identity comes from the persisted approved ToolCall.
use crate::{
    AppError, AppState, ExecuteTool, ToolCall, ToolInvocationOrigin, execute_tool_invocation,
    normalized_json_sha256, ontology_runtime_authority_digest, ontology_runtime_context,
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::{process::Stdio, sync::Arc, time::Duration};
use tokio::{process::Command, sync::Mutex};
use uuid::Uuid;

#[derive(Clone)]
struct RuntimeMcpScope {
    state: AppState,
    session_id: Uuid,
    grant_id: Uuid,
    context_id: Uuid,
    authority_digest: String,
    token: String,
    expires_at: DateTime<Utc>,
    // Serialize tool calls within this capability; business commits retain their own DB locks.
    calls: Arc<Mutex<()>>,
    claim: RuntimeClaim,
    parent_call_id: Uuid,
}

#[derive(Clone)]
enum RuntimeClaim {
    Execution {
        id: Uuid,
        worker: String,
        generation: i64,
    },
    SessionLoop {
        id: Uuid,
        worker: String,
        attempt: i32,
    },
}

impl RuntimeClaim {
    async fn check(&self, state: &AppState) -> Result<(), AppError> {
        let valid = match self {
            Self::Execution {
                id,
                worker,
                generation,
            } => {
                let job = state.execution_queue.get(*id).await?;
                job.status == crate::ExecutionJobStatus::Executing
                    && job.worker_id.as_ref() == Some(worker)
                    && job.claim_generation == *generation
                    && job.lease_expires_at.is_some_and(|t| t > Utc::now())
            }
            Self::SessionLoop {
                id,
                worker,
                attempt,
            } => {
                let job = state.get_session_loop_job(*id).await?;
                job.status == crate::SessionLoopJobStatus::Running
                    && job.worker_id.as_ref() == Some(worker)
                    && job.attempt_count == *attempt
                    && job.lease_expires_at.is_some_and(|t| t > Utc::now())
            }
        };
        if !valid {
            return Err(AppError::forbidden(
                "runtime capability lost its worker execution claim",
            ));
        }
        Ok(())
    }
}

impl RuntimeMcpScope {
    fn input(&self, args: Value) -> ExecuteTool {
        let mut args = args;
        args["context_packet_id"] = json!(self.context_id);
        args["runtime_parent_tool_call_id"] = json!(self.parent_call_id);
        ExecuteTool {
            session_id: self.session_id,
            task_grant_id: Some(self.grant_id),
            args,
        }
    }
    async fn authorize(&self, headers: &HeaderMap) -> Result<(), AppError> {
        let expected = format!("Bearer {}", self.token);
        let supplied = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let equal = supplied.len() == expected.len()
            && supplied
                .bytes()
                .zip(expected.bytes())
                .fold(0u8, |difference, (a, b)| difference | (a ^ b))
                == 0;
        if !equal || Utc::now() >= self.expires_at {
            return Err(AppError::forbidden(
                "runtime capability is invalid or expired",
            ));
        }
        self.claim.check(&self.state).await?;
        let grant = self.state.get_task_grant(self.grant_id).await?;
        if ontology_runtime_authority_digest(&grant) != self.authority_digest {
            return Err(AppError::forbidden(
                "runtime authority changed; this capability cannot change tasks",
            ));
        }
        crate::preview_task_grant_for_tool_invocation(
            &self.state,
            "ontology.context.read",
            &self.input(json!({})),
        )
        .await?;
        ontology_runtime_context(&self.state, &self.input(json!({}))).await?;
        Ok(())
    }
}

fn mcp_tools() -> Value {
    json!({"tools":[
        {"name":"ontology_read_context","description":"Read your assigned business task, authorized Ontology objects, relationships, rules, registered Action contracts, completion requirements and Action history. Required before any business Action.","inputSchema":{"type":"object","properties":{},"additionalProperties":false}},
        {"name":"ontology_propose_action","description":"Submit a registered Action from the Ontology contract using a server-issued context read receipt. Approval and execution are separate. If approval is required, end this Agent turn and wait; you cannot approve it yourself.","inputSchema":{"type":"object","properties":{"action":{"type":"string"},"parameters":{"type":"object"},"ontology_read_receipt_id":{"type":"string"}},"required":["action","parameters","ontology_read_receipt_id"],"additionalProperties":false}},
        {"name":"ontology_read_action_result","description":"Read the actual result of this task's Action. Successful business object readback issues evidence required by the closeout Action. A pending/failed Action has no successful business readback.","inputSchema":{"type":"object","properties":{"action_tool_call_id":{"type":"string"}},"required":["action_tool_call_id"],"additionalProperties":false}}
    ]})
}

fn check_keys(args: &Value, allowed: &[&str]) -> Result<(), AppError> {
    let object = args
        .as_object()
        .ok_or_else(|| AppError::bad_request("tool arguments must be an object"))?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(AppError::forbidden(
            "runtime tools cannot override session, grant, application, mode or other undeclared arguments",
        ));
    }
    Ok(())
}

async fn runtime_tool(scope: &RuntimeMcpScope, name: &str, args: Value) -> Result<Value, AppError> {
    let _guard = scope.calls.lock().await;
    scope.claim.check(&scope.state).await?;
    let current = scope.state.get_task_grant(scope.grant_id).await?;
    if Utc::now() >= scope.expires_at
        || ontology_runtime_authority_digest(&current) != scope.authority_digest
    {
        return Err(AppError::forbidden(
            "runtime authority changed while the request was queued",
        ));
    }
    let mut result = match name {
        "ontology_read_context" => {
            check_keys(&args, &[])?;
            execute_tool_invocation(
                &scope.state,
                "ontology.context.read",
                scope.input(json!({})),
                ToolInvocationOrigin::RuntimeAdapter,
            )
            .await?
        }
        "ontology_propose_action" => {
            check_keys(&args, &["action", "parameters", "ontology_read_receipt_id"])?;
            let context = ontology_runtime_context(&scope.state, &scope.input(json!({}))).await?;
            let requested = args["action"]
                .as_str()
                .ok_or_else(|| AppError::bad_request("Action name is required"))?;
            let action = context
                .catalog
                .actions
                .iter()
                .find(|a| a.runtime_name == requested || a.api_name == requested)
                .ok_or_else(|| {
                    AppError::forbidden("Action is not registered in the authorized Ontology")
                })?;
            execute_tool_invocation(&scope.state,"ontology.action.execute",scope.input(json!({"action":action.runtime_name,"parameters":args["parameters"],"ontology_read_receipt_id":args["ontology_read_receipt_id"]})),ToolInvocationOrigin::RuntimeAdapter).await?
        }
        "ontology_read_action_result" => {
            check_keys(&args, &["action_tool_call_id"])?;
            execute_tool_invocation(
                &scope.state,
                "ontology.action.result.read",
                scope.input(args),
                ToolInvocationOrigin::RuntimeAdapter,
            )
            .await?
        }
        _ => {
            return Err(AppError::forbidden(
                "tool is not available to this business runtime",
            ));
        }
    };
    if let Some(id) = result
        .get("approval_id")
        .and_then(Value::as_str)
        .and_then(|v| Uuid::parse_str(v).ok())
    {
        let approval = scope.state.get_approval(id).await?;
        result["action_tool_call_id"] = json!(approval.tool_call_id);
        result["next_step"] = json!(
            "End this turn and wait for an independent approval decision and execution. Do not claim business completion."
        );
    }
    if name == "ontology_read_context" {
        let history=scope.state.list_tool_calls(Some(scope.session_id)).await?.into_iter()
            .filter(|c|c.task_grant_id==Some(scope.grant_id) && c.tool_name=="ontology.action.execute")
            .map(|c|json!({"action_tool_call_id":c.id,"action":c.args["action"],"parameters":c.args["parameters"],"status":c.status,"result":c.result,"error":c.error})).collect::<Vec<_>>();
        result["action_history"] = json!(history);
        let context = ontology_runtime_context(&scope.state, &scope.input(json!({}))).await?;
        result["completion_requirements"] = json!({"required_source_object_ids":context.binding.required_source_object_ids,"rules":["All required business sources need an approved draft Action and an actual result readback.","Use the registered closeout Action; a Review or model statement does not close the WorkItem.","After closeout execution, read that Action result before ending as completed."]});
    }
    Ok(result)
}

async fn mcp(
    State(scope): State<RuntimeMcpScope>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Response {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    if let Err(error) = scope.authorize(&headers).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32001,"message":error.message}})),
        )
            .into_response();
    }
    if request["jsonrpc"] != "2.0" {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let method = request["method"].as_str().unwrap_or("");
    if method.starts_with("notifications/") {
        return StatusCode::ACCEPTED.into_response();
    }
    let result = match method {
        "initialize" => Ok(
            json!({"protocolVersion":request["params"]["protocolVersion"].as_str().unwrap_or("2025-03-26"),"capabilities":{"tools":{}},"serverInfo":{"name":"MandoForge Ontology Runtime","version":"1"}}),
        ),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(mcp_tools()),
        "tools/call" => {
            let name = request["params"]["name"].as_str().unwrap_or("");
            let args = request["params"]
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let response = runtime_tool(&scope, name, args).await;
            Ok(match response {
                Ok(value) => {
                    json!({"content":[{"type":"text","text":value.to_string()}],"isError":false})
                }
                Err(error) => {
                    json!({"content":[{"type":"text","text":json!({"status":"rejected","reason":error.message}).to_string()}],"isError":true})
                }
            })
        }
        _ => Err("method is not available"),
    };
    match result {
        Ok(result) => Json(json!({"jsonrpc":"2.0","id":id,"result":result})).into_response(),
        Err(message) => {
            Json(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":message}}))
                .into_response()
        }
    }
}

struct RuntimeServer(tokio::task::JoinHandle<()>);
impl Drop for RuntimeServer {
    fn drop(&mut self) {
        self.0.abort();
    }
}
struct RuntimeProcessGroup(Option<u32>);
impl Drop for RuntimeProcessGroup {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
    }
}

pub(crate) async fn run_ontology_codex(
    state: &AppState,
    call: &ToolCall,
    command_path: &str,
    _task: &str,
) -> Result<Value, AppError> {
    let grant_id = call
        .task_grant_id
        .ok_or_else(|| AppError::forbidden("business runtime requires persisted TaskGrant"))?;
    let grant = state.get_task_grant(grant_id).await?;
    let context_id = grant
        .context_packet_id
        .ok_or_else(|| AppError::forbidden("business runtime context is missing"))?;
    let input = ExecuteTool {
        session_id: call.session_id,
        task_grant_id: Some(grant_id),
        args: json!({"context_packet_id":context_id}),
    };
    let context = ontology_runtime_context(state, &input).await?;
    if grant.max_cost_usd_micros.is_some() {
        return Err(AppError::forbidden(
            "business Codex monetary budgets require a configured metering adapter; refusing unmetered execution",
        ));
    }
    let timeout_seconds = grant.max_runtime_seconds.unwrap_or(180).clamp(1, 600) as u64;
    let token = format!(
        "mforuntime-v1.{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    let expires_at = grant
        .expires_at
        .unwrap_or(Utc::now() + chrono::Duration::seconds(timeout_seconds as i64))
        .min(Utc::now() + chrono::Duration::seconds(timeout_seconds as i64));
    let mut runtime_state = state.clone();
    runtime_state.tenant_id = state.current_tenant_id();
    let claim = if call.policy_decision["origin"] == "runtime_model" {
        let id = crate::uuid_tool_arg(
            &call.args,
            &["session_loop_job_id"],
            "business model claim is missing",
        )?;
        let job = state.get_session_loop_job(id).await?;
        if job.session_id != call.session_id {
            return Err(AppError::forbidden("business model claim session mismatch"));
        }
        RuntimeClaim::SessionLoop {
            id,
            worker: job
                .worker_id
                .ok_or_else(|| AppError::forbidden("business model claim owner missing"))?,
            attempt: job.attempt_count,
        }
    } else {
        let job = state
            .execution_queue
            .list()
            .await?
            .into_iter()
            .find(|j| j.tool_call_id == call.id && j.status == crate::ExecutionJobStatus::Executing)
            .ok_or_else(|| AppError::forbidden("business adapter has no owned execution claim"))?;
        RuntimeClaim::Execution {
            id: job.id,
            worker: job
                .worker_id
                .ok_or_else(|| AppError::forbidden("business execution owner missing"))?,
            generation: job.claim_generation,
        }
    };
    claim.check(state).await?;
    let scope = RuntimeMcpScope {
        state: runtime_state,
        session_id: call.session_id,
        grant_id,
        context_id,
        authority_digest: ontology_runtime_authority_digest(&grant),
        token: token.clone(),
        expires_at,
        calls: Arc::new(Mutex::new(())),
        claim,
        parent_call_id: call.id,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/mcp", listener.local_addr()?);
    let router = Router::new()
        .route("/mcp", post(mcp))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .with_state(scope);
    let _server = RuntimeServer(tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    }));
    let workspace = state.workspace_root.join(call.session_id.to_string());
    tokio::fs::create_dir_all(&workspace).await?;
    let started_at = Utc::now();
    state.append_event("runtime_adapter",Some(call.id),call.session_id,"business_agent.started",json!({"task_grant_id":grant_id,"context_packet_id":context_id,"application_id":context.application.id,"work_item_id":context.binding.work_item_id,"runner":"codex-cli-ontology"})).await?;
    let agent_version = state.agent_version_for_session(call.session_id).await?;
    let mut command = Command::new(command_path);
    command.env_clear();
    for key in [
        "PATH",
        "HOME",
        "CODEX_HOME",
        "TMPDIR",
        "TMP",
        "TEMP",
        "LANG",
        "LC_ALL",
        "SSL_CERT_FILE",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("MANDOFORGE_ONTOLOGY_RUNTIME_TOKEN", token);
    command
        .kill_on_drop(true)
        .process_group(0)
        .current_dir(&workspace)
        .args([
            "exec",
            "--json",
            "--ephemeral",
            "--skip-git-repo-check",
            "--ignore-user-config",
            "--sandbox",
            "read-only",
            "--model",
            &agent_version.model,
        ]);
    // Native features are constrained per process; user-global configuration is not edited.
    for feature in [
        "shell_tool",
        "unified_exec",
        "apps",
        "plugins",
        "browser_use",
        "browser_use_external",
        "browser_use_full_cdp_access",
        "computer_use",
        "multi_agent",
        "multi_agent_v2",
        "skill_search",
    ] {
        command.arg("--disable").arg(feature);
    }
    for config in [
        "approval_policy=\"never\"".to_string(),
        "web_search=\"disabled\"".to_string(),
        format!("mcp_servers.mandoforge.url={}",serde_json::to_string(&endpoint)?),
        "mcp_servers.mandoforge.bearer_token_env_var=\"MANDOFORGE_ONTOLOGY_RUNTIME_TOKEN\"".to_string(),
        "mcp_servers.mandoforge.required=true".to_string(),
        // These are transport permissions only. Action effects still require
        // the platform's separate, authenticated approval and execution path.
        "mcp_servers.mandoforge.enabled_tools=[\"ontology_read_context\",\"ontology_propose_action\",\"ontology_read_action_result\"]".to_string(),
        "mcp_servers.mandoforge.tools.ontology_read_context.approval_mode=\"approve\"".to_string(),
        "mcp_servers.mandoforge.tools.ontology_propose_action.approval_mode=\"approve\"".to_string(),
        "mcp_servers.mandoforge.tools.ontology_read_action_result.approval_mode=\"approve\"".to_string(),
        "mcp_servers.mandoforge.startup_timeout_sec=15".to_string(),
        "developer_instructions=\"You are a governed business Agent. First call ontology_read_context to learn your actual objective, authorized facts, rules and Action contracts. Use only the MandoForge Ontology tools for business work. Do not invent facts. Submit registered Actions yourself using read receipts. Never approve actions. When an Action awaits approval or execution, end this turn and report waiting; the platform will resume you after the independent decision. A model answer or Review is not business completion. After the closeout Action executes, use ontology_read_action_result to verify its actual state before reporting completion.\"".to_string(),
    ] {command.arg("-c").arg(config);}
    command
        .arg("Use ontology_read_context to retrieve your assigned business objective, authorized facts, rules and Action contracts. Complete that task through the registered platform Actions. The task authority is supplied by the server, not by caller-provided execution text.")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn()?;
    let mut group = RuntimeProcessGroup(child.id());
    let output = tokio::time::timeout(
        Duration::from_secs(timeout_seconds),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| {
        AppError::bad_request(
            "business Codex deadline exceeded; inspect persisted Action state before retry",
        )
    })??;
    group.0 = None;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    for native in crate::execution::parse_codex_jsonl(&stdout) {
        state
            .append_event(
                "runtime_adapter",
                Some(call.id),
                call.session_id,
                "business_agent.native_event",
                json!({"runtime_type":"codex_cli","event":native}),
            )
            .await?;
    }
    let calls = state.list_tool_calls(Some(call.session_id)).await?;
    let pending = calls
        .iter()
        .filter(|c| {
            c.task_grant_id == Some(grant_id)
                && c.tool_name == "ontology.action.execute"
                && c.status == "waiting_approval"
        })
        .map(|c| json!({"action_tool_call_id":c.id,"action":c.args["action"],"result":c.result}))
        .collect::<Vec<_>>();
    let completed = output.status.success()
        && business_completion_verified(state, call.session_id, grant_id, Some(started_at)).await?;
    let mut final_artifact_id = None;
    if completed {
        let work = state
            .list_work_items()
            .await?
            .into_iter()
            .find(|w| w.id == context.binding.work_item_id)
            .ok_or_else(|| AppError::forbidden("completed WorkItem missing at final readback"))?;
        let artifact=state.insert_artifact(crate::Artifact {id:Uuid::new_v4(),session_id:call.session_id,artifact_type:"business_result".into(),name:"verified-business-result.json".into(),path:None,
            content:json!({"status":"completed","task_grant_id":grant_id,"work_item":work,"source":"ontology_action_readback"}),created_at:Utc::now()}).await?;
        final_artifact_id = Some(artifact.id);
        state.append_event("system",Some(artifact.id),call.session_id,"artifact.created",json!({"artifact_id":artifact.id,"name":artifact.name,"artifact_type":artifact.artifact_type,"source":"business_agent.verified_completion"})).await?;
    }
    let business_status = if completed {
        "completed"
    } else if !pending.is_empty() {
        "waiting_approval"
    } else {
        "blocked"
    };
    let result = json!({"runner":"codex-cli-ontology","runtime_type":"codex_cli","status":output.status.code(),"business_status":business_status,"work_item_id":context.binding.work_item_id,"final_artifact_id":final_artifact_id,"pending_actions":pending,"stdout":crate::execution::truncate_output(&stdout,65536).text,"stderr":crate::execution::truncate_output(&stderr,8192).text});
    state
        .append_event(
            "runtime_adapter",
            Some(call.id),
            call.session_id,
            if completed {
                "business_agent.completed"
            } else {
                "business_agent.not_completed"
            },
            result.clone(),
        )
        .await?;
    if !output.status.success() || business_status == "blocked" {
        return Err(AppError::bad_request(format!(
            "business Agent did not reach approved completion or a pending Action (exit {:?})",
            output.status.code()
        ))
        .with_known_execution_outcome());
    }
    Ok(result)
}

pub(crate) async fn business_completion_verified(
    state: &AppState,
    session_id: Uuid,
    grant_id: Uuid,
    after: Option<DateTime<Utc>>,
) -> Result<bool, AppError> {
    let grant = state.get_task_grant(grant_id).await?;
    let Some(binding) = crate::ontology_runtime_binding(&grant)? else {
        return Ok(false);
    };
    let Some(work) =
        state.list_work_items().await?.into_iter().find(|w| {
            w.id == binding.work_item_id && w.archived_at.is_none() && w.status == "done"
        })
    else {
        return Ok(false);
    };
    let closing = work.metadata["ontology_closeout"]["closing_action_tool_call_id"]
        .as_str()
        .and_then(|id| Uuid::parse_str(id).ok());
    let calls = state.list_tool_calls(Some(session_id)).await?;
    let closure = calls.iter().find(|c| {
        Some(c.id) == closing
            && c.task_grant_id == Some(grant_id)
            && c.tool_name == "ontology.action.execute"
            && c.status == "completed"
            && c.result
                .as_ref()
                .is_some_and(|r| r["status"] == "work_item_completed")
    });
    if closure.is_none() {
        return Ok(false);
    }
    Ok(calls.iter().any(|c| {
        c.task_grant_id == Some(grant_id)
            && c.tool_name == "ontology.action.result.read"
            && c.status == "completed"
            && matches!(
                c.policy_decision["origin"].as_str(),
                Some("runtime_adapter" | "session_loop")
            )
            && after.is_none_or(|time| c.created_at >= time)
            && c.result.as_ref().is_some_and(|r| {
                r["status"] == "readback_verified"
                    && r["action_tool_call_id"].as_str()
                        == closing.as_ref().map(Uuid::to_string).as_deref()
                    && r["business_object"]["status"] == "done"
                    && r["execution_receipt"]["work_item_version"]
                        == json!(normalized_json_sha256(
                            &serde_json::to_value(&work).unwrap_or(Value::Null)
                        ))
            })
    }))
}

/// Native session-loop backend: the worker's existing model-turn lease authorizes
/// this constrained reasoning turn. It cannot execute business effects without Actions.
pub(crate) async fn run_ontology_session_turn(
    state: &AppState,
    job: &crate::SessionLoopJob,
    grant: &crate::TaskGrant,
) -> Result<crate::Session, AppError> {
    let version = state.agent_version_for_session(job.session_id).await?;
    let command = crate::execution::business_codex_command(state, job.session_id).await?;
    let now = Utc::now();
    let call = ToolCall {
        id: Uuid::new_v4(),
        session_id: job.session_id,
        event_id: Some(Uuid::new_v4()),
        tool_name: "codex.exec".into(),
        args: json!({"session_loop_job_id":job.id,"context_packet_id":grant.context_packet_id,"task":"retrieve assigned objective from Ontology","sandbox_mode":"read-only"}),
        task_grant_id: Some(grant.id),
        normalized_args_hash: None,
        target_binding: json!({}),
        status: "running".into(),
        risk_level: "low".into(),
        policy_decision: json!({"decision":"allowed","origin":"runtime_model","reason":"native business model turn under an owned session-loop claim; only scoped Ontology MCP capabilities"}),
        result: None,
        error: None,
        started_at: Some(now),
        completed_at: None,
        created_at: now,
    };
    let (call, _) = state
        .commit_tool_invocation_start(call, version.id, version.version)
        .await?;
    match run_ontology_codex(state, &call, &command, "").await {
        Ok(result) => {
            state
                .commit_tool_invocation_result(
                    call.id,
                    "completed",
                    result.clone(),
                    "runtime_model",
                )
                .await?;
            if result["business_status"] == "completed" {
                crate::apply_provider_completion(state,job.session_id,Some(grant.id),"completed","Registered business Actions executed and independently read back; WorkItem closed").await
            } else {
                crate::set_managed_session_status(
                    state,
                    job.session_id,
                    crate::SessionStatus::RequiresAction,
                    "business Agent is waiting for an independent Action approval",
                )
                .await?;
                state.get_session(job.session_id).await
            }
        }
        Err(error) => {
            state
                .update_tool_call_status(
                    call.id,
                    "failed",
                    None,
                    Some(json!({"error":error.message})),
                )
                .await?;
            Err(error)
        }
    }
}

#[cfg(test)]
#[allow(clippy::await_holding_lock)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use tower::ServiceExt;

    #[tokio::test]
    async fn ontology_business_private_mcp_rejects_identity_override_and_lost_claim() {
        let _lock = crate::tests::ontology_business_runtime_tests::business_fixture_env_lock();
        let f = crate::tests::ontology_business_runtime_tests::memory_business_fixture().await;
        let queued = f
            .state
            .enqueue_session_loop_job(f.session.id, None, "capability boundary fixture")
            .await
            .unwrap();
        let job = f
            .state
            .start_session_loop_job(queued.id, "owned-worker")
            .await
            .unwrap();
        let scope = RuntimeMcpScope {
            state: f.state.clone(),
            session_id: f.session.id,
            grant_id: f.grant.id,
            context_id: f.grant.context_packet_id.unwrap(),
            authority_digest: ontology_runtime_authority_digest(&f.grant),
            token: "mforuntime-v1.fixture-private-token".into(),
            expires_at: Utc::now() + chrono::Duration::minutes(1),
            calls: Arc::new(Mutex::new(())),
            claim: RuntimeClaim::SessionLoop {
                id: job.id,
                worker: "owned-worker".into(),
                attempt: job.attempt_count,
            },
            parent_call_id: Uuid::new_v4(),
        };
        let router = Router::new()
            .route("/mcp", post(mcp))
            .with_state(scope.clone());
        let request = |token: &str, args: Value| {
            axum::http::Request::builder().method("POST").uri("/mcp").header("content-type","application/json").header("authorization",format!("Bearer {token}")).body(Body::from(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"ontology_read_context","arguments":args}}).to_string())).unwrap()
        };
        let denied = router
            .clone()
            .oneshot(request("wrong-token", json!({})))
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
        for args in [
            json!({"session_id":Uuid::new_v4()}),
            json!({"task_grant_id":Uuid::new_v4()}),
            json!({"mode":"engineering"}),
            json!({"application_id":Uuid::new_v4()}),
        ] {
            let response = router
                .clone()
                .oneshot(request(&scope.token, args))
                .await
                .unwrap();
            let value: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 1048576).await.unwrap())
                    .unwrap();
            assert_eq!(value["result"]["isError"], true);
        }
        assert!(
            f.state
                .list_tool_calls(Some(f.session.id))
                .await
                .unwrap()
                .is_empty()
        );
        if let crate::StoreBackend::Memory(inner) = &f.state.store {
            inner
                .write()
                .await
                .session_loop_jobs
                .get_mut(&job.id)
                .unwrap()
                .lease_expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
        }
        let denied = router
            .oneshot(request(&scope.token, json!({})))
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    }
}
