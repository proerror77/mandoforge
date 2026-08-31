use super::*;

#[tokio::test]
async fn scheduler_due_run_processes_semantic_aging() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state.seed_demo_agent().await.expect("seed demo agent");
    let app = build_router(state.clone());
    let admin_headers = [
        ("x-mandoforge-subject", "admin-1"),
        ("x-mandoforge-roles", "admin"),
    ];
    let due_at = Utc::now() - chrono::Duration::minutes(5);
    let source: SemanticSource = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/semantic-sources",
            json!({
                "source_type": "memory",
                "source_uri": "memory://semantic-aging-test",
                "display_name": "Semantic aging test"
            }),
            &admin_headers,
        ),
    )
    .await;
    let stale: SemanticObject = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/semantic-objects",
            json!({
                "source_id": source.id,
                "object_type": "memory",
                "object_key": "memory:legal:stale-aging-test",
                "title": "Stale legal memory",
                "summary": "This memory should be archived by aging policy.",
                "content": {"rule": "old"},
                "semantic_scopes": {
                    "domain_scope": "legal",
                    "workflow_scope": "contract-review",
                    "memory_scope": "legal-policy"
                },
                "trust_level": "source_attested",
                "freshness": "stale"
            }),
            &admin_headers,
        ),
    )
    .await;
    state
        .create_workflow_pack_runtime_objects(vec![WorkflowPackRuntimeObject {
            id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            binding_id: Uuid::new_v4(),
            pack_id: "semantic-aging-pack".to_string(),
            pack_version: "0.1.0".to_string(),
            object_type: "schedule".to_string(),
            object_key: "semantic-aging:legal-policy".to_string(),
            runtime_kind: "semantic_aging_policy".to_string(),
            status: "released".to_string(),
            spec: json!({
                "domain_scope": "legal",
                "memory_scope": "legal-policy",
                "archive_stale": true,
                "schedule_policy": {"mode": "scheduler", "due_at": due_at, "one_shot": true}
            }),
            created_at: due_at,
            updated_at: due_at,
        }])
        .await
        .expect("semantic aging policy");

    let scheduler_run: Value = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/scheduler/run-due",
            json!({"idempotency_key": "semantic-aging-test"}),
            &admin_headers,
        ),
    )
    .await;
    assert_eq!(
        scheduler_run["semantic_aging_policies"]["archived_count"],
        json!(1)
    );
    assert!(
        scheduler_run["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|action| { action == "semantic_aging_policies_processed" })
    );

    let remaining_objects: Vec<SemanticObject> = request_json(
        app,
        Request::builder()
            .uri("/api/semantic-objects")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(!remaining_objects.iter().any(|object| object.id == stale.id));
}

#[tokio::test]
async fn scheduler_due_run_activates_workflow_scheduled_retry_steps() {
    let app = test_app().await;
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let agent = agents.first().expect("seeded agent");

    let definition: Value = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/workflow-definitions",
            json!({
                "name": "Scheduler due retry workflow",
                "entrypoint": "scheduler-due-retry-workflow",
                "trigger_type": "manual",
                "default_agent_id": agent.id,
                "step_graph": {
                    "steps": [
                        {"key": "collect", "type": "agent", "start": true},
                        {
                            "key": "normalize",
                            "type": "agent",
                            "depends_on": ["collect"],
                            "retry": {"max_attempts": 2, "delay_seconds": 1}
                        }
                    ]
                },
                "release_state": "released"
            }),
            &[("x-mandoforge-roles", "admin")],
        ),
    )
    .await;
    let run: Value = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/workflow-runs",
            json!({
                "workflow_definition_id": definition["id"],
                "title": "scheduler due retry"
            }),
            &[("x-mandoforge-roles", "operator")],
        ),
    )
    .await;
    let run_id = run["id"].as_str().expect("run id");

    let steps: Vec<Value> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/workflow-runs/{run_id}/steps"))
            .header("x-mandoforge-roles", "operator")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let collect_step_id = steps[0]["id"]
        .as_str()
        .expect("collect step id")
        .to_string();
    let _: Value = request_json(
        app.clone(),
        json_request_with_headers(
            "PATCH",
            &format!("/api/workflow-step-runs/{collect_step_id}"),
            json!({"status": "completed"}),
            &[("x-mandoforge-roles", "operator")],
        ),
    )
    .await;

    let steps: Vec<Value> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/workflow-runs/{run_id}/steps"))
            .header("x-mandoforge-roles", "operator")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let normalize_step_id = steps
        .iter()
        .find(|step| step["step_key"] == json!("normalize"))
        .and_then(|step| step["id"].as_str())
        .expect("normalize step id")
        .to_string();
    let _: Value = request_json(
        app.clone(),
        json_request_with_headers(
            "PATCH",
            &format!("/api/workflow-step-runs/{normalize_step_id}"),
            json!({
                "status": "failed",
                "output_payload": {"error": "temporary source outage"}
            }),
            &[("x-mandoforge-roles", "operator")],
        ),
    )
    .await;

    tokio::time::sleep(Duration::from_millis(1_100)).await;

    let graph: WorkflowRunGraphConsole = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/workflow-runs/{run_id}/graph"))
            .header("x-mandoforge-roles", "operator")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(graph.due_scheduled_count, 1);

    let scheduler_plan: SchedulerDuePlan = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/scheduler/due-plan")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let workflow_plan = scheduler_plan
        .actions
        .iter()
        .find(|item| item.action == "workflow_scheduled_step_activation")
        .expect("workflow scheduled step plan item");
    assert_eq!(workflow_plan.area, "workflows");
    assert_eq!(workflow_plan.status, "due");
    assert_eq!(workflow_plan.due_count, 1);

    let scheduler_run: SchedulerDueRun = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/scheduler/run-due",
            json!({"idempotency_key": "workflow-scheduled-step-test"}),
            &[("x-mandoforge-roles", "admin")],
        ),
    )
    .await;
    let workflow_activation = scheduler_run
        .workflow_scheduled_steps
        .as_ref()
        .expect("workflow scheduled activation sweep");
    assert_eq!(workflow_activation.status, "completed");
    assert_eq!(workflow_activation.activated_count, 1);
    assert!(
        scheduler_run
            .actions
            .iter()
            .any(|action| action == "workflow_scheduled_steps_activated")
    );

    let steps_after_scheduler: Vec<Value> = request_json(
        app,
        Request::builder()
            .uri(format!("/api/workflow-runs/{run_id}/steps"))
            .header("x-mandoforge-roles", "operator")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let activated_retry = steps_after_scheduler
        .iter()
        .find(|step| {
            step["step_key"] == json!("normalize")
                && step["id"] == json!(workflow_activation.activated_step_ids[0])
        })
        .expect("activated normalize retry step");
    assert_eq!(activated_retry["status"], json!("queued"));
}

#[tokio::test]
async fn scheduler_due_run_materializes_due_semantic_synthesis_schedule_once() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state.seed_demo_agent().await.expect("seed demo agent");
    let agent = state
        .list_agents()
        .await
        .expect("agents")
        .into_iter()
        .next()
        .expect("seeded agent");
    let session = state
        .create_session(CreateSession {
            agent_id: agent.id,
            environment_id: None,
            title: "scheduled semantic synthesis".to_string(),
            message: Some("Build a governed memory trail.".to_string()),
        })
        .await
        .expect("session");
    state
        .append_event(
            "system",
            None,
            session.id,
            "session.goal.completed",
            json!({"objective": "Build a governed memory trail."}),
        )
        .await
        .expect("checkpoint");

    let due_at = Utc::now() - chrono::Duration::minutes(5);
    let runtime_object_id = Uuid::new_v4();
    state
            .create_workflow_pack_runtime_objects(vec![WorkflowPackRuntimeObject {
                id: runtime_object_id,
                installation_id: Uuid::new_v4(),
                binding_id: Uuid::new_v4(),
                pack_id: "semantic-memory-pack".to_string(),
                pack_version: "0.1.0".to_string(),
                object_type: "schedule".to_string(),
                object_key: "semantic-synthesis:scheduled-memory-trail".to_string(),
                runtime_kind: "semantic_synthesis_schedule".to_string(),
                status: "released".to_string(),
                spec: json!({
                    "session_id": session.id,
                    "synthesis_type": "dreaming_synthesis",
                    "goal_attempted": "Synthesize durable lessons from completed managed work.",
                    "context_used": ["scheduler_due_run", "session_events"],
                    "worked": ["due schedule remained audit-bound"],
                    "failed_or_corrected": [],
                    "unsafe_assumptions": ["scheduler must not promote memory directly"],
                    "durable_memory_candidates": [
                        {
                            "proposed_object_key": "memory:scheduled-semantic-synthesis:review-required",
                            "title": "Scheduled synthesis remains review gated",
                            "summary": "Scheduler-owned semantic synthesis should create review candidates, not durable memory directly.",
                            "content": {"rule": "scheduled-candidate-first"},
                            "trust_level": "source_attested",
                            "freshness": "current"
                        }
                    ],
                    "schedule_policy": {
                        "mode": "scheduler",
                        "due_at": due_at,
                        "one_shot": true
                    }
                }),
                created_at: due_at,
                updated_at: due_at,
            }])
            .await
            .expect("runtime object");

    let plan = build_scheduler_due_plan(&state).await.expect("due plan");
    let semantic_plan = plan
        .actions
        .iter()
        .find(|item| item.action == "semantic_synthesis_schedule_run")
        .expect("semantic synthesis schedule plan item");
    assert_eq!(semantic_plan.area, "memory");
    assert_eq!(semantic_plan.status, "due");
    assert_eq!(semantic_plan.due_count, 1);

    let first_run = execute_scheduler_due_tasks(
        &state,
        Some(SchedulerRunDueRequest {
            idempotency_key: Some("semantic-synthesis-schedule-test".to_string()),
            owner: Some("test".to_string()),
            run_window_start: None,
            run_window_end: None,
            retry_policy: None,
        }),
    )
    .await
    .expect("first due run");
    let first_run_value = serde_json::to_value(&first_run).expect("run value");
    assert_eq!(
        first_run_value["semantic_synthesis_schedules"]["status"],
        json!("completed")
    );
    assert_eq!(
        first_run_value["semantic_synthesis_schedules"]["created_count"],
        json!(1)
    );
    assert!(
        first_run
            .actions
            .iter()
            .any(|action| action == "semantic_synthesis_schedules_processed")
    );

    let artifacts = state
        .list_artifacts(session.id)
        .await
        .expect("session artifacts");
    assert_eq!(
        artifacts
            .iter()
            .filter(|artifact| artifact.artifact_type == "semantic_dreaming_report")
            .count(),
        1
    );
    let candidates = state
        .list_memory_writeback_candidates(Some(session.id))
        .await
        .expect("memory writeback candidates");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].candidate_type, "dreaming_synthesis");
    assert_eq!(candidates[0].status, "pending");

    let second_run = execute_scheduler_due_tasks(
        &state,
        Some(SchedulerRunDueRequest {
            idempotency_key: Some("semantic-synthesis-schedule-test-2".to_string()),
            owner: Some("test".to_string()),
            run_window_start: None,
            run_window_end: None,
            retry_policy: None,
        }),
    )
    .await
    .expect("second due run");
    let second_run_value = serde_json::to_value(&second_run).expect("run value");
    assert_eq!(
        second_run_value["semantic_synthesis_schedules"]["created_count"],
        json!(0)
    );
    let artifacts_after_second = state
        .list_artifacts(session.id)
        .await
        .expect("session artifacts");
    assert_eq!(
        artifacts_after_second
            .iter()
            .filter(|artifact| artifact.artifact_type == "semantic_dreaming_report")
            .count(),
        1
    );
}

#[tokio::test]
async fn scheduler_due_run_materializes_workflow_bound_semantic_synthesis_schedule() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state.seed_demo_agent().await.expect("seed demo agent");
    let app = build_router(state.clone());
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let agent = agents.first().expect("seeded agent");
    let definition: WorkflowDefinition = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/workflow-definitions",
            json!({
                "name": "Workflow-bound semantic synthesis",
                "entrypoint": "workflow-bound-semantic-synthesis",
                "trigger_type": "manual",
                "default_agent_id": agent.id,
                "step_graph": {
                    "steps": [
                        {"key": "collect", "type": "agent", "start": true}
                    ]
                },
                "handoff_rules": {
                    "root_task_grant": {
                        "memory_scope": {
                            "mode": "candidate_writeback",
                            "allowed_scope_keys": [
                                "workflow_definition_id",
                                "workflow_id"
                            ],
                            "allowed_object_types": ["memory"],
                            "allowed_source_types": [
                                "semantic_synthesis",
                                "artifact",
                                "session_event"
                            ],
                            "allowed_object_ids": [],
                            "minimum_trust_level": "source_attested",
                            "max_objects": 5,
                            "approval_memory_allowed": false,
                            "handoff_memory_allowed": false,
                            "writeback_allowed": true
                        }
                    }
                },
                "release_state": "released"
            }),
            &[("x-mandoforge-roles", "admin")],
        ),
    )
    .await;
    let workflow_run: WorkflowRun = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/workflow-runs",
            json!({
                "workflow_definition_id": definition.id,
                "title": "workflow-bound semantic synthesis run"
            }),
            &[("x-mandoforge-roles", "operator")],
        ),
    )
    .await;
    let steps: Vec<WorkflowStepRun> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/workflow-runs/{}/steps", workflow_run.id))
            .header("x-mandoforge-roles", "operator")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let collect = steps.first().expect("collect step");
    let _: WorkflowStepRun = request_json(
        app.clone(),
        json_request_with_headers(
            "PATCH",
            &format!("/api/workflow-step-runs/{}", collect.id),
            json!({"status": "completed"}),
            &[("x-mandoforge-roles", "operator")],
        ),
    )
    .await;
    let completed_run: WorkflowRun = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/workflow-runs/{}", workflow_run.id))
            .header("x-mandoforge-roles", "operator")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(completed_run.status, "completed");
    let runtime_object_id = Uuid::new_v4();
    let due_at = Utc::now() - chrono::Duration::minutes(5);
    state
            .create_workflow_pack_runtime_objects(vec![WorkflowPackRuntimeObject {
                id: runtime_object_id,
                installation_id: Uuid::new_v4(),
                binding_id: Uuid::new_v4(),
                pack_id: "workflow-bound-semantic-memory-pack".to_string(),
                pack_version: "0.1.0".to_string(),
                object_type: "schedule".to_string(),
                object_key: "workflow:workflow-bound:semantic-synthesis".to_string(),
                runtime_kind: "semantic_synthesis_schedule".to_string(),
                status: "released".to_string(),
                spec: json!({
                    "workflow_definition_id": definition.id,
                    "workflow_id": "workflow-bound-semantic-synthesis",
                    "synthesis_type": "dreaming_synthesis",
                    "goal_attempted": "Synthesize completed workflow runs into governed memory candidates.",
                    "context_used": ["workflow_run", "session_events"],
                    "worked": ["workflow run completed"],
                    "failed_or_corrected": [],
                    "unsafe_assumptions": ["do not promote memory directly"],
                    "durable_memory_candidates": [
                        {
                            "proposed_object_key": "memory:workflow-bound:scheduled-synthesis",
                            "title": "Workflow-bound scheduled synthesis",
                            "summary": "Workflow-bound semantic synthesis should select completed workflow sessions.",
                            "content": {"rule": "workflow-definition-selector"},
                            "trust_level": "source_attested",
                            "freshness": "current"
                        }
                    ],
                    "schedule_policy": {
                        "mode": "scheduler",
                        "due_at": due_at
                    },
                    "session_selector": {
                        "source": "completed_workflow_runs",
                        "status": "completed"
                    }
                }),
                created_at: due_at,
                updated_at: due_at,
            }])
            .await
            .expect("runtime object");

    let scheduler_run: SchedulerDueRun = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/scheduler/run-due",
            json!({
                "idempotency_key": "workflow-bound-semantic-synthesis-test",
                "owner": "test"
            }),
            &[("x-mandoforge-roles", "admin")],
        ),
    )
    .await;
    let semantic_synthesis = scheduler_run
        .semantic_synthesis_schedules
        .as_ref()
        .expect("semantic synthesis schedule sweep");
    assert_eq!(semantic_synthesis.created_count, 1);
    assert_eq!(semantic_synthesis.failed_count, 0);
    assert_eq!(
        semantic_synthesis.runs[0].session_id,
        Some(workflow_run.primary_session_id)
    );

    let artifacts: Vec<Artifact> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!(
                "/api/sessions/{}/artifacts",
                workflow_run.primary_session_id
            ))
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(artifacts.iter().any(|artifact| {
        artifact.artifact_type == "semantic_dreaming_report"
            && artifact.content["metadata"]["workflow_definition_id"] == json!(definition.id)
            && artifact.content["metadata"]["schedule_runtime_object_id"]
                == json!(runtime_object_id)
    }));
}

#[tokio::test]
async fn scheduler_due_run_orchestrates_due_automation_across_teams() {
    let app = test_app().await;
    let (status, error) = request_value(
        app.clone(),
        Request::builder()
            .uri("/api/scheduler/due-plan")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(
        error["error"]
            .as_str()
            .is_some_and(|message| message.contains("not allowed"))
    );

    let organization: Organization = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/organizations")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"name": "Scheduler Org", "slug": "scheduler-org"}).to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let team: Team = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/organizations/{}/teams", organization.id))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"name": "Scheduler Team", "slug": "scheduler-team"}).to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let _server: McpServerRecord = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/teams/{}/mcp-servers", team.id))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "name": "scheduled-mcp",
                    "transport": "http",
                    "config": {"health_check": {"interval_seconds": 1}},
                    "tool_allowlist": ["ping"]
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let session: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"agent_id": agents[0].id, "title": "scheduler remote computer reclaim"})
                    .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let computer: RemoteComputer = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"name": "scheduler-remote-computer", "profile": "workspace-write"})
                    .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let lease: RemoteComputerLease = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/remote-computers/{}/leases", computer.id))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "worker_id": "scheduler-remote-manager",
                    "lease_seconds": 30
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let _attachment: RemoteComputerAttachment = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/remote-computer-leases/{}/attach", lease.id))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "attached_by": "scheduler-remote-manager",
                    "stale_after_seconds": -1
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let _provider: ProviderRecord = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/providers")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "provider_type": "mock",
                    "name": "scheduler-alert-mock",
                    "default_model": "gpt-5.5-mini",
                    "config": {
                        "budget": {"daily_request_limit": 1},
                        "pricing": {"per_request_cents": 1.0}
                    }
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let alert_agent: Agent = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/agents")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "name": "scheduler alert agent",
                    "kind": "orchestrator",
                    "provider": "scheduler-alert-mock",
                    "model": "gpt-5.5-mini",
                    "tools": ["file.read", "sql.get_schema", "sql.query", "shell.exec"]
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let alert_session: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"agent_id": alert_agent.id, "title": "scheduler alert session"}).to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let _alert_run: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/sessions/{}/run", alert_session.id))
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    run_next_session_loop_job(app.clone(), alert_session.id, "scheduler-alert-worker").await;
    let _alert_route: CostAlertRoute = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/usage/alert-routes")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "name": "scheduler-critical-email",
                    "channel": "email",
                    "target": "ops@example.com",
                    "severity_filter": "critical"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;

    let plan: SchedulerDuePlan = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/scheduler/due-plan")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(plan.status, "ready");
    assert_eq!(plan.team_count, 1);
    assert!(plan.item_count >= 8);
    assert!(plan.actionable_count >= 2);
    let mcp_health_plan = plan
        .actions
        .iter()
        .find(|item| item.action == "mcp_scheduled_health_checks")
        .expect("mcp health plan item");
    assert_eq!(mcp_health_plan.area, "mcp");
    assert_eq!(mcp_health_plan.status, "due");
    assert_eq!(mcp_health_plan.due_count, 1);
    assert_eq!(mcp_health_plan.target_count, 1);
    let remote_computer_plan = plan
        .actions
        .iter()
        .find(|item| item.action == "remote_computer_stale_reclaim")
        .expect("remote computer reclaim plan item");
    assert_eq!(remote_computer_plan.area, "remote_computers");
    assert_eq!(remote_computer_plan.status, "due");
    assert_eq!(remote_computer_plan.due_count, 1);
    let remote_computer_sidecar_plan = plan
        .actions
        .iter()
        .find(|item| item.action == "remote_computer_sidecar_supervision")
        .expect("remote computer sidecar supervision plan item");
    assert_eq!(remote_computer_sidecar_plan.area, "remote_computers");
    assert_eq!(remote_computer_sidecar_plan.status, "due");
    assert_eq!(remote_computer_sidecar_plan.due_count, 1);
    let cost_alert_plan = plan
        .actions
        .iter()
        .find(|item| item.action == "usage_cost_alert_delivery")
        .expect("cost alert delivery plan item");
    assert_eq!(cost_alert_plan.area, "usage");
    assert_eq!(cost_alert_plan.status, "due");
    assert_eq!(cost_alert_plan.due_count, 1);

    let run: SchedulerDueRun = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/scheduler/run-due")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(run.status, "completed");
    assert_eq!(run.owner, "manual");
    assert_eq!(run.idempotency_key, None);
    assert!(!run.replayed);
    assert_eq!(run.retry_policy.max_attempts, 1);
    assert_eq!(run.retry_policy.backoff_seconds, 0);
    assert!(run.run_window_end > run.run_window_start);
    assert_eq!(run.team_count, 1);
    assert_eq!(run.mcp_health_runs.len(), 1);
    assert_eq!(run.mcp_health_runs[0].team_id, team.id);
    assert_eq!(run.mcp_health_runs[0].due_count, 1);
    assert_eq!(run.mcp_rollout_runs.len(), 1);
    assert_eq!(run.codex_app_server_stale_polls.candidate_count, 0);
    let cost_alert_delivery = run
        .cost_alert_delivery
        .as_ref()
        .expect("scheduler cost alert delivery");
    assert_eq!(cost_alert_delivery.status, "reserved");
    assert_eq!(cost_alert_delivery.channel, "routes");
    assert_eq!(
        cost_alert_delivery.alerts[0].provider_name,
        "scheduler-alert-mock"
    );
    assert_eq!(run.remote_computer_reclaim.status, "completed");
    assert_eq!(run.remote_computer_reclaim.reclaimed_attachment_count, 1);
    assert_eq!(run.remote_computer_reclaim.reclaimed_lease_count, 0);
    assert_eq!(run.remote_computer_sidecar_supervision.status, "attention");
    assert_eq!(
        run.remote_computer_sidecar_supervision
            .missing_heartbeat_count,
        1
    );
    if !usage_finance_export_schedule_enabled() {
        assert_eq!(run.usage_finance_export.status, "disabled");
    }
    assert!(
        run.actions
            .iter()
            .any(|action| action == "mcp_health_checks_processed")
    );
    assert!(
        run.actions
            .iter()
            .any(|action| action == "remote_computer_reclaim_processed")
    );
    assert!(
        run.actions
            .iter()
            .any(|action| action == "remote_computer_sidecar_supervision_processed")
    );
    assert!(
        run.actions
            .iter()
            .any(|action| action == "usage_cost_alert_delivery_processed")
    );

    let summary: SchedulerOrchestrationSummary = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/scheduler/summary")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(summary.last_run_status.as_deref(), Some("completed"));
    assert_eq!(summary.recent_run_count, 1);
    assert_eq!(summary.recent_runs[0].run_id, Some(run.run_id));
    assert_eq!(summary.recent_runs[0].owner.as_deref(), Some("manual"));
    assert_eq!(summary.recent_runs[0].idempotency_key, None);
    assert_eq!(summary.deployment_readiness.status, "blocked");
    assert!(summary.deployment_readiness.production_blocked);
    assert!(summary.deployment_readiness.scheduler_manifest_present);
    assert!(
        summary
            .deployment_readiness
            .service_account_manifest_present
    );
    assert_eq!(
        summary.deployment_readiness.service_account_name.as_deref(),
        Some("mandoforge-scheduler")
    );
    assert!(
        summary
            .deployment_readiness
            .automount_service_account_token_disabled
    );
    assert!(summary.deployment_readiness.subject_from_secret);
    assert!(summary.deployment_readiness.roles_from_secret);
    assert!(summary.deployment_readiness.token_from_secret);
    assert!(summary.deployment_readiness.token_header_present);
    assert!(summary.deployment_readiness.hardcoded_admin_headers_absent);
    assert!(!summary.deployment_readiness.shared_token_runtime_configured);
    assert!(summary.attention_items.iter().any(|item| {
        item.kind == "scheduler_deployment_blocked" && item.severity == "critical"
    }));
    assert_eq!(summary.recent_runs[0].team_count, 1);
    assert!(
        summary.recent_runs[0]
            .actions
            .iter()
            .any(|action| action == "mcp_health_checks_processed")
    );
    assert!(
        summary.recent_runs[0]
            .actions
            .iter()
            .any(|action| action == "remote_computer_reclaim_processed")
    );
    assert!(
        summary.recent_runs[0]
            .actions
            .iter()
            .any(|action| action == "remote_computer_sidecar_supervision_processed")
    );
    assert!(
        summary.recent_runs[0]
            .actions
            .iter()
            .any(|action| action == "usage_cost_alert_delivery_processed")
    );

    let audit_logs: Vec<AuditLog> = request_json(
        app,
        Request::builder()
            .uri("/api/audit-logs")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(audit_logs.iter().any(|log| {
        log.action == "scheduler.run_due"
            && log.details["team_count"] == 1
            && log.details["status"] == "completed"
            && log.details["run_id"] == json!(run.run_id)
            && log.details["owner"] == "manual"
            && log.details["run"]["run_id"] == json!(run.run_id)
            && log.details["cost_alert_delivery_status"] == "reserved"
            && log.details["remote_computer_sidecar_supervision_status"] == "attention"
    }));
    assert!(audit_logs.iter().any(|log| {
        log.action == "remote_computer.sidecar_supervision_run"
            && log.details["status"] == "attention"
            && log.details["missing_heartbeat_count"] == 1
    }));
}

#[tokio::test]
async fn scheduler_due_run_replays_by_idempotency_key_and_validates_request() {
    let app = test_app().await;
    let payload = json!({
        "idempotency_key": "stage3-scheduler:2026-05-17T00:00Z",
        "owner": "k8s-cronjob",
        "run_window_start": "2026-05-17T00:00:00Z",
        "run_window_end": "2026-05-17T00:05:00Z",
        "retry_policy": {
            "max_attempts": 3,
            "backoff_seconds": 30
        }
    });

    let first_run: SchedulerDueRun = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/scheduler/run-due")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(payload.to_string()))
            .expect("valid request"),
    )
    .await;
    assert_eq!(first_run.status, "noop");
    assert_eq!(
        first_run.idempotency_key.as_deref(),
        Some("stage3-scheduler:2026-05-17T00:00Z")
    );
    assert_eq!(first_run.owner, "k8s-cronjob");
    assert_eq!(first_run.retry_policy.max_attempts, 3);
    assert_eq!(first_run.retry_policy.backoff_seconds, 30);
    assert!(!first_run.replayed);

    let second_run: SchedulerDueRun = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/scheduler/run-due")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(payload.to_string()))
            .expect("valid request"),
    )
    .await;
    assert_eq!(second_run.run_id, first_run.run_id);
    assert_eq!(second_run.checked_at, first_run.checked_at);
    assert!(second_run.replayed);

    let summary: SchedulerOrchestrationSummary = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/scheduler/summary")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(summary.recent_run_count, 1);
    assert_eq!(summary.recent_runs[0].run_id, Some(first_run.run_id));
    assert_eq!(
        summary.recent_runs[0].idempotency_key.as_deref(),
        Some("stage3-scheduler:2026-05-17T00:00Z")
    );
    assert_eq!(summary.recent_runs[0].owner.as_deref(), Some("k8s-cronjob"));

    let audit_logs: Vec<AuditLog> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/audit-logs")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let matching_runs: Vec<_> = audit_logs
        .iter()
        .filter(|log| {
            log.action == "scheduler.run_due"
                && log.details["idempotency_key"] == json!("stage3-scheduler:2026-05-17T00:00Z")
        })
        .collect();
    assert_eq!(matching_runs.len(), 1);
    assert_eq!(matching_runs[0].details["run"]["replayed"], false);

    let (status, error) = request_value(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/scheduler/run-due")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "idempotency_key": "invalid key with spaces",
                    "retry_policy": {"max_attempts": 1, "backoff_seconds": 0}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        error["error"]
            .as_str()
            .is_some_and(|message| message.contains("idempotency_key"))
    );

    let (status, error) = request_value(
        app,
        Request::builder()
            .method("POST")
            .uri("/api/scheduler/run-due")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "idempotency_key": "stage3-scheduler:bad-retry",
                    "run_window_start": "2026-05-17T00:05:00Z",
                    "run_window_end": "2026-05-17T00:05:00Z",
                    "retry_policy": {"max_attempts": 0, "backoff_seconds": 0}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        error["error"]
            .as_str()
            .is_some_and(|message| message.contains("run_window_end"))
    );
}

#[tokio::test]
async fn scheduler_due_run_records_task_error_and_continues_remaining_tasks() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));

    let run = execute_scheduler_due_tasks(
        &state,
        Some(SchedulerRunDueRequest {
            idempotency_key: Some("scheduler-forced-task-failure".to_string()),
            owner: Some("__force_policy_rollout_failure".to_string()),
            run_window_start: None,
            run_window_end: None,
            retry_policy: None,
        }),
    )
    .await
    .expect("scheduler run should continue after task failure");

    assert_eq!(run.status, "failed");
    assert_eq!(run.policy_rollout.status, "failed");
    assert_eq!(run.task_errors.len(), 1);
    assert_eq!(run.task_errors[0].task, "policy_rollout");
    assert!(
        run.task_errors[0]
            .message
            .contains("forced scheduler task failure")
    );
    assert!(run.workflow_scheduled_steps.is_some());
    assert!(run.semantic_synthesis_schedules.is_some());
    assert!(run.semantic_aging_policies.is_some());
    assert!(run.ontology_release_workflow_triggers.is_some());
    assert_eq!(run.remote_computer_reclaim.status, "noop");
    assert_eq!(run.usage_finance_export.status, "disabled");

    let audit_logs = state.list_audit_logs(None).await.expect("audit logs");
    let scheduler_audit = audit_logs
        .iter()
        .find(|log| log.action == "scheduler.run_due")
        .expect("scheduler run audit");
    assert_eq!(scheduler_audit.details["status"], json!("failed"));
    assert_eq!(scheduler_audit.details["task_error_count"], json!(1));
    assert_eq!(
        scheduler_audit.details["task_errors"][0]["task"],
        json!("policy_rollout")
    );
}

#[tokio::test]
async fn scheduler_due_run_surfaces_deferred_remote_computer_reclaim() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: "scheduler-reclaim-attention".to_string(),
            profile: Some("agent-sandbox".to_string()),
            namespace: None,
            pod_name: Some("scheduler-reclaim-attention-pod".to_string()),
            workspace_path: None,
            state_mount_path: None,
            metadata: Some(json!({"on_demand": true})),
        })
        .await
        .expect("create on-demand computer");
    let lease = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: None,
                worker_id: Some("scheduler-reclaim-worker".to_string()),
                lease_seconds: Some(60),
                metadata: Some(json!({"on_demand": true})),
            },
        )
        .await
        .expect("create lease");
    let StoreBackend::Memory(inner) = &state.store else {
        panic!("test requires memory store");
    };
    inner
        .write()
        .await
        .remote_computer_leases
        .get_mut(&lease.id)
        .expect("persisted lease")
        .lease_expires_at = Some(Utc::now() - chrono::Duration::seconds(1));

    let run = execute_scheduler_due_tasks(&state, None)
        .await
        .expect("scheduler run");

    assert_eq!(run.status, "failed");
    assert_eq!(run.remote_computer_reclaim.status, "attention");
    assert!(
        run.task_errors
            .iter()
            .any(|error| error.task == "remote_computer_reclaim")
    );
    assert!(
        run.actions
            .iter()
            .any(|action| action == "remote_computer_reclaim_processed")
    );
}
