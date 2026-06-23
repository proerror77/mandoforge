use super::*;
use std::sync::Arc;

#[tokio::test]
async fn ontology_release_promote_triggers_workflow_run_with_action_catalog() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let definition =
        ontology_release_trigger_workflow_definition_for_test(&state, "commerce").await;
    let release =
        ontology_release_candidate_for_test(&state, "commerce-vtest-trigger-workflow").await;
    gate_ontology_release_with_actor(&state, release.id, "test")
        .await
        .expect("gate");

    let active = promote_ontology_release_with_actor(&state, release.id, "test")
        .await
        .expect("promote");

    let runs = state.list_workflow_runs().await.expect("workflow runs");
    let run = runs
        .iter()
        .find(|run| {
            run.workflow_definition_id == definition.id
                && run.input_payload["ontology_release_id"] == json!(active.id)
        })
        .expect("workflow run triggered by ontology release");
    assert_eq!(run.status, "queued");
    assert_eq!(
        run.input_payload["trigger"],
        json!("ontology_release.promoted")
    );
    assert_eq!(run.input_payload["ontology_version"], json!(active.version));
    assert_eq!(run.input_payload["domain_scope"], json!("commerce"));
    assert_eq!(
        run.input_payload["ontology_release"]["release_class"],
        json!("repo_controlled")
    );
    assert_eq!(run.input_payload["action_catalog"]["tool_count"], json!(1));
    let tool_specs = run.input_payload["action_catalog"]["tool_specs"]
        .as_array()
        .expect("tool specs");
    assert!(
        tool_specs
            .iter()
            .any(|spec| spec["name"] == json!("commerce.refund_order"))
    );

    let events = state
        .list_events(run.primary_session_id)
        .await
        .expect("session events");
    assert!(events.iter().any(|event| {
        event.event_type == "ontology_release.workflow_run_triggered"
            && event.payload["ontology_release_id"] == json!(active.id)
    }));
    let audit_logs = state.list_audit_logs(None).await.expect("audit logs");
    assert!(audit_logs.iter().any(|log| {
        log.action == "ontology_release.workflow_run_triggered"
            && log.details["workflow_run_id"] == json!(run.id)
    }));
    let triggers = state
        .list_ontology_release_workflow_triggers()
        .await
        .expect("workflow triggers");
    let trigger = triggers
        .iter()
        .find(|trigger| {
            trigger.ontology_release_id == active.id
                && trigger.workflow_definition_id == definition.id
        })
        .expect("durable workflow trigger");
    assert_eq!(trigger.status, "triggered");
    assert_eq!(trigger.workflow_run_id, Some(run.id));
    assert!(trigger.error_message.is_none());
}

#[tokio::test]
async fn ontology_release_promote_triggers_all_matching_workflow_definitions() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let first_definition =
        ontology_release_trigger_workflow_definition_for_test(&state, "commerce").await;
    let second_definition =
        ontology_release_trigger_workflow_definition_for_test(&state, "commerce").await;
    let release =
        ontology_release_candidate_for_test(&state, "commerce-vtest-trigger-all-workflows").await;
    gate_ontology_release_with_actor(&state, release.id, "test")
        .await
        .expect("gate");

    let active = promote_ontology_release_with_actor(&state, release.id, "test")
        .await
        .expect("promote");

    let runs = state.list_workflow_runs().await.expect("workflow runs");
    let matching_runs: Vec<_> = runs
        .iter()
        .filter(|run| {
            [first_definition.id, second_definition.id].contains(&run.workflow_definition_id)
                && run.input_payload["ontology_release_id"] == json!(active.id)
        })
        .collect();
    assert_eq!(matching_runs.len(), 2);
    assert!(matching_runs.iter().all(|run| {
        run.input_payload["trigger"] == json!("ontology_release.promoted")
            && run.input_payload["action_catalog"]["tool_count"] == json!(1)
    }));

    let triggers = state
        .list_ontology_release_workflow_triggers()
        .await
        .expect("workflow triggers");
    let matching_triggers: Vec<_> = triggers
        .iter()
        .filter(|trigger| {
            trigger.ontology_release_id == active.id
                && [first_definition.id, second_definition.id]
                    .contains(&trigger.workflow_definition_id)
        })
        .collect();
    assert_eq!(matching_triggers.len(), 2);
    assert!(
        matching_triggers
            .iter()
            .all(|trigger| { trigger.status == "triggered" && trigger.workflow_run_id.is_some() })
    );
}

#[tokio::test]
async fn ontology_release_workflow_trigger_failure_does_not_block_other_definitions() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let valid_definition =
        ontology_release_trigger_workflow_definition_for_test(&state, "commerce").await;
    let mut invalid_definition = valid_definition.clone();
    invalid_definition.id = Uuid::new_v4();
    invalid_definition.name = "Ontology release downstream workflow with missing agent".to_string();
    invalid_definition.entrypoint = "ontology-release-downstream-missing-agent".to_string();
    invalid_definition.default_agent_id = Uuid::new_v4();
    invalid_definition.created_at = Utc::now() + ChronoDuration::seconds(1);
    invalid_definition.updated_at = invalid_definition.created_at;
    state
        .create_workflow_definition(invalid_definition.clone())
        .await
        .expect("invalid matching definition");
    let release = ontology_release_candidate_for_test(
        &state,
        "commerce-vtest-trigger-definition-failure-isolation",
    )
    .await;
    gate_ontology_release_with_actor(&state, release.id, "test")
        .await
        .expect("gate");

    let active = promote_ontology_release_with_actor(&state, release.id, "test")
        .await
        .expect("promote");

    let runs = state.list_workflow_runs().await.expect("workflow runs");
    assert!(runs.iter().any(|run| {
        run.workflow_definition_id == valid_definition.id
            && run.input_payload["ontology_release_id"] == json!(active.id)
    }));
    assert!(!runs.iter().any(|run| {
        run.workflow_definition_id == invalid_definition.id
            && run.input_payload["ontology_release_id"] == json!(active.id)
    }));

    let triggers = state
        .list_ontology_release_workflow_triggers()
        .await
        .expect("workflow triggers");
    let valid_trigger = triggers
        .iter()
        .find(|trigger| {
            trigger.ontology_release_id == active.id
                && trigger.workflow_definition_id == valid_definition.id
        })
        .expect("valid trigger");
    assert_eq!(valid_trigger.status, "triggered");
    assert!(valid_trigger.workflow_run_id.is_some());
    let invalid_trigger = triggers
        .iter()
        .find(|trigger| {
            trigger.ontology_release_id == active.id
                && trigger.workflow_definition_id == invalid_definition.id
        })
        .expect("invalid trigger");
    assert_eq!(invalid_trigger.status, "failed");
    assert!(invalid_trigger.workflow_run_id.is_none());
    assert!(invalid_trigger.error_message.is_some());

    let audit_logs = state.list_audit_logs(None).await.expect("audit logs");
    assert!(audit_logs.iter().any(|log| {
        log.action == "ontology_release.workflow_definition_trigger_failed"
            && log.resource_id == Some(invalid_definition.id)
            && log.details["ontology_release_id"] == json!(active.id)
    }));
}

#[tokio::test]
async fn ontology_release_workflow_trigger_is_idempotent_per_release() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let definition =
        ontology_release_trigger_workflow_definition_for_test(&state, "commerce").await;
    let release =
        ontology_release_candidate_for_test(&state, "commerce-vtest-trigger-idempotent").await;
    gate_ontology_release_with_actor(&state, release.id, "test")
        .await
        .expect("gate");
    let active = promote_ontology_release_with_actor(&state, release.id, "test")
        .await
        .expect("promote");

    trigger_workflow_run_from_ontology_release(&state, &active, "test")
        .await
        .expect("repeat trigger");

    let matching_runs = state
        .list_workflow_runs()
        .await
        .expect("workflow runs")
        .into_iter()
        .filter(|run| {
            run.workflow_definition_id == definition.id
                && run.input_payload["ontology_release_id"] == json!(active.id)
        })
        .count();
    assert_eq!(matching_runs, 1);
    let matching_triggers = state
        .list_ontology_release_workflow_triggers()
        .await
        .expect("workflow triggers")
        .into_iter()
        .filter(|trigger| {
            trigger.ontology_release_id == active.id
                && trigger.workflow_definition_id == definition.id
        })
        .count();
    assert_eq!(matching_triggers, 1);
}

#[tokio::test]
async fn ontology_release_workflow_trigger_reclaims_stale_pending_claim() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let definition =
        ontology_release_trigger_workflow_definition_for_test(&state, "commerce").await;
    let release =
        ontology_release_candidate_for_test(&state, "commerce-vtest-trigger-stale-pending").await;

    let first_claim = state
        .claim_ontology_release_workflow_trigger(release.id, definition.id)
        .await
        .expect("claim trigger")
        .expect("first claim");
    assert_eq!(first_claim.status, "pending");
    assert_eq!(first_claim.attempt_count, 1);
    assert!(first_claim.claimed_at.is_some());

    let fresh_retry = trigger_workflow_run_from_ontology_release(&state, &release, "test")
        .await
        .expect("fresh retry");
    assert!(fresh_retry.is_none());

    if let StoreBackend::Memory(inner) = &state.store {
        let mut store = inner.write().await;
        let trigger = store
            .ontology_release_workflow_triggers
            .get_mut(&first_claim.id)
            .expect("trigger");
        let stale_time = Utc::now() - ChronoDuration::seconds(600);
        trigger.claimed_at = Some(stale_time);
        trigger.updated_at = stale_time;
    }

    let run = trigger_workflow_run_from_ontology_release(&state, &release, "test")
        .await
        .expect("stale retry")
        .expect("workflow run");

    let triggers = state
        .list_ontology_release_workflow_triggers()
        .await
        .expect("workflow triggers");
    let trigger = triggers
        .iter()
        .find(|trigger| trigger.id == first_claim.id)
        .expect("same durable trigger");
    assert_eq!(trigger.status, "triggered");
    assert_eq!(trigger.attempt_count, 2);
    assert_eq!(trigger.workflow_run_id, Some(run.id));
    assert!(trigger.error_message.is_none());
}

#[tokio::test]
async fn ontology_release_workflow_trigger_relinks_existing_run_after_stale_claim() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let definition =
        ontology_release_trigger_workflow_definition_for_test(&state, "commerce").await;
    let release =
        ontology_release_candidate_for_test(&state, "commerce-vtest-trigger-relink-run").await;

    let first_claim = state
        .claim_ontology_release_workflow_trigger(release.id, definition.id)
        .await
        .expect("claim trigger")
        .expect("first claim");
    let existing_run = create_workflow_run_from_definition(
        &state,
        &definition,
        "preexisting ontology release run".to_string(),
        json!({
            "trigger": "ontology_release.promoted",
            "ontology_release_id": release.id,
            "ontology_version": release.version,
            "domain_scope": release.domain_scope,
        }),
        json!({
            "trigger": "ontology_release.promoted",
            "ontology_release_id": release.id,
            "domain_scope": release.domain_scope,
        }),
    )
    .await
    .expect("existing workflow run");

    if let StoreBackend::Memory(inner) = &state.store {
        let mut store = inner.write().await;
        let trigger = store
            .ontology_release_workflow_triggers
            .get_mut(&first_claim.id)
            .expect("trigger");
        let stale_time = Utc::now() - ChronoDuration::seconds(600);
        trigger.claimed_at = Some(stale_time);
        trigger.updated_at = stale_time;
    }

    let recovered_run = trigger_workflow_run_from_ontology_release(&state, &release, "test")
        .await
        .expect("stale retry")
        .expect("existing workflow run");
    assert_eq!(recovered_run.id, existing_run.id);

    let matching_runs = state
        .list_workflow_runs()
        .await
        .expect("workflow runs")
        .into_iter()
        .filter(|run| {
            run.workflow_definition_id == definition.id
                && run.input_payload["ontology_release_id"] == json!(release.id)
        })
        .count();
    assert_eq!(matching_runs, 1);

    let triggers = state
        .list_ontology_release_workflow_triggers()
        .await
        .expect("workflow triggers");
    let trigger = triggers
        .iter()
        .find(|trigger| trigger.id == first_claim.id)
        .expect("same durable trigger");
    assert_eq!(trigger.status, "triggered");
    assert_eq!(trigger.attempt_count, 2);
    assert_eq!(trigger.workflow_run_id, Some(existing_run.id));
    assert!(trigger.error_message.is_none());
}

#[tokio::test]
async fn ontology_release_workflow_trigger_drain_retries_failed_trigger() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let release =
        ontology_release_candidate_for_test(&state, "commerce-vtest-trigger-drain-retry").await;
    gate_ontology_release_with_actor(&state, release.id, "test")
        .await
        .expect("gate");
    let active = promote_ontology_release_with_actor(&state, release.id, "test")
        .await
        .expect("promote without matching definition");
    let definition =
        ontology_release_trigger_workflow_definition_for_test(&state, "commerce").await;
    let failed_claim = state
        .claim_ontology_release_workflow_trigger(active.id, definition.id)
        .await
        .expect("claim trigger")
        .expect("claimed trigger");
    state
        .complete_ontology_release_workflow_trigger(
            failed_claim.id,
            "failed",
            None,
            Some("simulated trigger failure".to_string()),
        )
        .await
        .expect("mark trigger failed");

    let drain = drain_due_ontology_release_workflow_triggers(&state, "test", 10)
        .await
        .expect("drain workflow triggers");

    assert_eq!(drain.retryable_count, 1);
    assert_eq!(drain.triggered_count, 1);
    assert_eq!(drain.failed_count, 0);
    assert_eq!(drain.status, "triggered");
    let runs = state.list_workflow_runs().await.expect("workflow runs");
    assert!(runs.iter().any(|run| {
        run.workflow_definition_id == definition.id
            && run.input_payload["ontology_release_id"] == json!(active.id)
    }));
    let trigger = state
        .list_ontology_release_workflow_triggers()
        .await
        .expect("workflow triggers")
        .into_iter()
        .find(|trigger| trigger.id == failed_claim.id)
        .expect("drained trigger");
    assert_eq!(trigger.status, "triggered");
    assert!(trigger.workflow_run_id.is_some());
    assert!(trigger.error_message.is_none());
}

#[tokio::test]
async fn ontology_release_workflow_trigger_drain_skips_inactive_release_terminally() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let release =
        ontology_release_candidate_for_test(&state, "commerce-vtest-trigger-inactive-skip").await;
    let definition =
        ontology_release_trigger_workflow_definition_for_test(&state, "commerce").await;
    let failed_claim = state
        .claim_ontology_release_workflow_trigger(release.id, definition.id)
        .await
        .expect("claim trigger")
        .expect("claimed trigger");
    state
        .complete_ontology_release_workflow_trigger(
            failed_claim.id,
            "failed",
            None,
            Some("simulated trigger failure".to_string()),
        )
        .await
        .expect("mark trigger failed");

    let drain = drain_due_ontology_release_workflow_triggers(&state, "test", 10)
        .await
        .expect("drain workflow triggers");

    assert_eq!(drain.retryable_count, 1);
    assert_eq!(drain.triggered_count, 0);
    assert_eq!(drain.skipped_count, 1);
    assert_eq!(drain.failed_count, 0);
    assert_eq!(drain.status, "skipped");

    let trigger = state
        .list_ontology_release_workflow_triggers()
        .await
        .expect("workflow triggers")
        .into_iter()
        .find(|trigger| trigger.id == failed_claim.id)
        .expect("drained trigger");
    assert_eq!(
        trigger.status,
        ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_SKIPPED
    );
    assert!(trigger.workflow_run_id.is_none());
    assert!(
        trigger
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("candidate")
    );

    let retryable = state
        .retryable_ontology_release_workflow_triggers(10)
        .await
        .expect("retryable triggers");
    assert!(
        !retryable
            .iter()
            .any(|trigger| trigger.id == failed_claim.id)
    );
}

#[tokio::test]
async fn ontology_release_workflow_trigger_rejects_unknown_status() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let trigger = state
        .claim_ontology_release_workflow_trigger(Uuid::new_v4(), Uuid::new_v4())
        .await
        .expect("claim trigger")
        .expect("claimed trigger");

    let error = state
        .complete_ontology_release_workflow_trigger(trigger.id, "done", None, None)
        .await
        .expect_err("unknown trigger status should be rejected");
    assert!(
        error
            .message
            .contains("unsupported ontology release workflow trigger status"),
        "{error:?}"
    );

    let stored = state
        .list_ontology_release_workflow_triggers()
        .await
        .expect("workflow triggers")
        .into_iter()
        .find(|stored| stored.id == trigger.id)
        .expect("stored trigger");
    assert_eq!(
        stored.status,
        ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_PENDING
    );
    assert!(stored.workflow_run_id.is_none());
    assert!(stored.error_message.is_none());
}
