use super::*;

fn postgres_workflow_pack_installation(
    pack_id: &str,
    now: DateTime<Utc>,
) -> WorkflowPackInstallation {
    WorkflowPackInstallation {
        id: Uuid::new_v4(),
        pack_id: pack_id.to_string(),
        kind: "WorkflowPack".to_string(),
        version: "0.1.0".to_string(),
        manifest_path: format!("packs/{pack_id}/package.yaml"),
        manifest: json!({}),
        validation_report: json!({}),
        status: "staged".to_string(),
        eval_gate_status: "pending".to_string(),
        release_gate_status: "pending".to_string(),
        gate_evidence: json!({}),
        staged_at: Some(now),
        released_at: None,
        archived_at: None,
        created_at: now,
        updated_at: now,
    }
}

fn postgres_workflow_pack_release_request(
    installation: &WorkflowPackInstallation,
    agent_id: Uuid,
    agent_version_id: Uuid,
    environment: &str,
    now: DateTime<Utc>,
) -> crate::store_workflow_pack_lifecycle::WorkflowPackLifecycleTransitionRequest {
    crate::store_workflow_pack_lifecycle::WorkflowPackLifecycleTransitionRequest {
        installation_id: installation.id,
        expected_status: "staged".to_string(),
        next_status: "released".to_string(),
        eval_gate_status: "passed".to_string(),
        release_gate_status: "passed".to_string(),
        gate_evidence: json!({"test": "postgres-shared-release"}),
        staged_at: installation.staged_at,
        released_at: Some(now),
        occurred_at: now,
        audit_action: "workflow_pack.released".to_string(),
        audit_details: json!({"test": "postgres-shared-release"}),
        agent_release_transition: crate::store_workflow_pack_lifecycle::WorkflowPackAgentReleaseTransition::PromoteFromPack {
            targets: vec![(agent_id, agent_version_id)],
            environment: environment.to_string(),
            promoted_by: "postgres-review".to_string(),
            gate_evidence: json!({"test": "postgres-shared-release"}),
        },
    }
}

fn postgres_workflow_pack_rollback_request(
    installation: &WorkflowPackInstallation,
    now: DateTime<Utc>,
) -> crate::store_workflow_pack_lifecycle::WorkflowPackLifecycleTransitionRequest {
    crate::store_workflow_pack_lifecycle::WorkflowPackLifecycleTransitionRequest {
        installation_id: installation.id,
        expected_status: "released".to_string(),
        next_status: "rolled_back".to_string(),
        eval_gate_status: installation.eval_gate_status.clone(),
        release_gate_status: installation.release_gate_status.clone(),
        gate_evidence: json!({"test": "postgres-shared-rollback"}),
        staged_at: installation.staged_at,
        released_at: installation.released_at,
        occurred_at: now,
        audit_action: "workflow_pack.rolled_back".to_string(),
        audit_details: json!({"test": "postgres-shared-rollback"}),
        agent_release_transition:
            crate::store_workflow_pack_lifecycle::WorkflowPackAgentReleaseTransition::RollbackPackPromotions,
    }
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_shared_workflow_pack_release_survives_until_last_rollback() {
    let _env = env_lock().lock().expect("env lock");
    let _provider_runtime = EnvVarGuard::remove("MANDOFORGE_PROVIDER_RUNTIME_ENV");
    let _release_enforcement = EnvVarGuard::remove("MANDOFORGE_AGENT_RELEASE_ENFORCEMENT");
    let database_url = std::env::var("MANDOFORGE_TEST_POSTGRES_URL")
        .expect("MANDOFORGE_TEST_POSTGRES_URL is required");
    let mut state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let tenant_id = state.tenant_id;
    let tenant_setting = format!("SET mandoforge.tenant_id = '{tenant_id}'");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .after_connect(move |connection, _metadata| {
            let tenant_setting = tenant_setting.clone();
            Box::pin(async move {
                sqlx::query(&tenant_setting).execute(connection).await?;
                Ok(())
            })
        })
        .connect(&database_url)
        .await
        .expect("connect test postgres");
    run_migrations(&pool).await.expect("run migrations");
    seed_demo_tenant(&pool, tenant_id)
        .await
        .expect("seed tenant");
    state.store = StoreBackend::Postgres(pool.clone());

    let agent = state
        .create_agent(CreateAgent {
            name: format!("Postgres shared release {}", Uuid::new_v4()),
            kind: "orchestrator".to_string(),
            provider: "openai-compatible".to_string(),
            model: "gpt-5.5-mini".to_string(),
            team_id: None,
            project_id: None,
            runtime_profile_id: None,
            agent_role: "specialist".to_string(),
            system_prompt: "Verify shared workflow pack release references.".to_string(),
            runtime_config: json!({}),
            tools: vec!["file.read".to_string()],
            tool_policy: json!({}),
            mcp_server_ids: vec![],
            skill_ids: vec![],
            workflow_pack_ids: vec![],
            remote_computer_profile: json!({}),
            semantic_scopes: json!({}),
            release_state: "draft".to_string(),
        })
        .await
        .expect("create postgres agent");
    let agent_version = state
        .current_agent_version(agent.id)
        .await
        .expect("postgres agent version");
    let now = Utc::now();
    let first = postgres_workflow_pack_installation(
        format!("postgres-shared-first-{}", Uuid::new_v4()).as_str(),
        now,
    );
    let second = postgres_workflow_pack_installation(
        format!("postgres-shared-second-{}", Uuid::new_v4()).as_str(),
        now,
    );
    for installation in [&first, &second] {
        state
            .create_workflow_pack_installation_with_profile_assets(installation.clone(), &[])
            .await
            .expect("create postgres workflow pack installation");
    }
    let environment = format!("postgres-shared-{}", Uuid::new_v4());
    let first_state = state.clone();
    let second_state = state.clone();
    let (first_released, second_released) = tokio::join!(
        first_state.transition_workflow_pack_lifecycle(postgres_workflow_pack_release_request(
            &first,
            agent.id,
            agent_version.id,
            &environment,
            now,
        )),
        second_state.transition_workflow_pack_lifecycle(postgres_workflow_pack_release_request(
            &second,
            agent.id,
            agent_version.id,
            &environment,
            now,
        )),
    );
    let first_released = first_released.expect("release first postgres workflow pack");
    let second_released = second_released.expect("release second postgres workflow pack");

    let release_for_target = |releases: Vec<AgentRelease>| {
        releases
            .into_iter()
            .find(|release| {
                release.agent_id == agent.id
                    && release.agent_version_id == agent_version.id
                    && release.environment == environment
            })
            .expect("shared postgres AgentRelease")
    };
    let shared = release_for_target(
        state
            .list_all_agent_releases()
            .await
            .expect("list postgres AgentReleases"),
    );
    assert_eq!(shared.status, "promoted");
    assert_eq!(
        shared.automation_policy["workflow_pack_installation_ids"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );

    state
        .transition_workflow_pack_lifecycle(postgres_workflow_pack_rollback_request(
            &first_released,
            Utc::now(),
        ))
        .await
        .expect("rollback first postgres workflow pack");
    let after_first_rollback = release_for_target(
        state
            .list_all_agent_releases()
            .await
            .expect("list shared release after first rollback"),
    );
    assert_eq!(after_first_rollback.status, "promoted");
    assert_eq!(
        after_first_rollback.automation_policy["workflow_pack_installation_ids"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        state
            .get_workflow_pack_installation(second.id)
            .await
            .expect("second postgres workflow pack")
            .status,
        "released"
    );

    state
        .transition_workflow_pack_lifecycle(postgres_workflow_pack_rollback_request(
            &second_released,
            Utc::now(),
        ))
        .await
        .expect("rollback second postgres workflow pack");
    let after_last_rollback = release_for_target(
        state
            .list_all_agent_releases()
            .await
            .expect("list shared release after final rollback"),
    );
    assert_eq!(after_last_rollback.status, "rolled_back");
    assert_eq!(
        after_last_rollback.automation_policy["workflow_pack_installation_ids"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );

    let dataset = state
        .create_eval_dataset(CreateEvalDataset {
            name: format!("Postgres manual release race {}", Uuid::new_v4()),
            description: None,
        })
        .await
        .expect("create postgres eval dataset");
    state
        .create_eval_case(
            dataset.id,
            CreateEvalCase {
                input: json!({"final_answer": "release ready"}),
                expected: Some(json!({"contains": ["release"]})),
                grading_policy: json!({"kind": "final_answer"}),
            },
        )
        .await
        .expect("create postgres eval case");
    let eval_run = state
        .create_eval_run(dataset.id, CreateEvalRun { agent_id: agent.id })
        .await
        .expect("create postgres eval run");
    assert_eq!(eval_run.status, "completed");

    let manual_environment = format!("postgres-manual-race-{}", Uuid::new_v4());
    let automated = crate::store_releases::new_workflow_pack_agent_release(
        Uuid::new_v4(),
        agent.id,
        agent_version.id,
        &manual_environment,
        "workflow-pack-release",
        &json!({"test": "manual-race"}),
        Utc::now(),
    );
    let mut automated_tx = pool.begin().await.expect("begin automated release tx");
    crate::store_releases::insert_or_get_promoted_agent_release_tx(
        &mut automated_tx,
        tenant_id,
        &automated,
    )
    .await
    .expect("insert uncommitted automated release");

    let manual_state = state.clone();
    let manual_environment_for_request = manual_environment.clone();
    let manual_promotion = tokio::spawn(async move {
        manual_state
            .create_agent_release(
                agent.id,
                CreateAgentRelease {
                    agent_version_id: Some(agent_version.id),
                    eval_run_id: eval_run.id,
                    environment: manual_environment_for_request,
                    min_score: Some(1.0),
                },
                "independent-reviewer".to_string(),
            )
            .await
    });
    let mut observed_unique_conflict_wait = false;
    for _ in 0..100 {
        let waiting: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1
                FROM pg_stat_activity
                WHERE wait_event_type = 'Lock'
                  AND query ILIKE '%INSERT INTO agent_releases%'
            )",
        )
        .fetch_one(&pool)
        .await
        .expect("inspect postgres lock wait");
        if waiting {
            observed_unique_conflict_wait = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(
        observed_unique_conflict_wait,
        "manual promotion must reach the uncommitted automated release conflict"
    );
    automated_tx
        .commit()
        .await
        .expect("commit automated release");
    let manual = manual_promotion
        .await
        .expect("join manual promotion")
        .expect("complete independent promotion");
    assert_ne!(manual.id, automated.id);
    assert_eq!(manual.status, "promoted");
    assert_ne!(
        manual.automation_policy["source"],
        json!("workflow_pack_release")
    );
    let race_releases = state
        .list_all_agent_releases()
        .await
        .expect("list manual race releases")
        .into_iter()
        .filter(|release| {
            release.agent_id == agent.id
                && release.agent_version_id == agent_version.id
                && release.environment == manual_environment
        })
        .collect::<Vec<_>>();
    assert!(
        race_releases
            .iter()
            .any(|release| { release.id == automated.id && release.status == "superseded" })
    );
    assert_eq!(
        race_releases
            .iter()
            .filter(|release| release.status == "promoted")
            .count(),
        1
    );

    pool.close().await;
}
