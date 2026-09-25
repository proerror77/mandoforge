use super::*;
use crate::execution_queue::ExecutionJob;
use chrono::SubsecRound;

pub(crate) struct BusinessFixture {
    pub(crate) state: AppState,
    pub(crate) session: Session,
    pub(crate) grant: TaskGrant,
    pub(crate) work: WorkItem,
    pub(crate) customer: SemanticObject,
    pub(crate) rule: SemanticObject,
    pub(crate) draft_action: String,
    pub(crate) close_action: String,
}

fn fixture_proposal(
    run_id: Uuid,
    kind: &str,
    name: &str,
    content: Value,
) -> OntologyOnboardingProposalDraft {
    OntologyOnboardingProposalDraft {
        id: Uuid::new_v4(),
        run_id,
        proposal_type: kind.into(),
        name: name.into(),
        source_mapping: "isolated_fixture".into(),
        confidence: 1.0,
        evidence: json!({"class":"synthetic_initial_definition"}),
        recommendation: "approve".into(),
        review_status: "approved".into(),
        content,
    }
}

pub(crate) async fn business_fixture(mut state: AppState) -> BusinessFixture {
    state.process_role = ProcessRole::Worker;
    let now = Utc::now().trunc_subsecs(6);
    let domain = format!("business_fixture_{}", Uuid::new_v4().simple());
    let scopes = json!({"domain_scope":domain,"workflow_scope":"customer-followup","share_policy":"tenant_only"});
    let profile=state.create_agent_runtime_profile(serde_json::from_value(json!({"name":format!("business-codex-{}",Uuid::new_v4().simple()),"runtime_type":"codex_cli","command":"codex","default_args":[],"env":{},"remote_computer_required":false})).unwrap()).await.unwrap();
    let agent=state.create_agent(serde_json::from_value(json!({"name":format!("Ontology business Agent {}",Uuid::new_v4()),"kind":"specialist","agent_role":"specialist","provider":"openai-compatible","model":"gpt-6-astra","runtime_profile_id":profile.id,"release_state":"active","tools":["codex.exec","ontology.context.read","ontology.action.execute","ontology.action.result.read"],"semantic_scopes":scopes})).unwrap()).await.unwrap();
    let session = state
        .create_session(CreateSession {
            agent_id: agent.id,
            environment_id: None,
            title: "Complete the assigned customer follow-up work using Ontology and Actions"
                .into(),
            message: None,
        })
        .await
        .unwrap();
    let work=state.create_work_item(serde_json::from_value(json!({"title":"Customer follow-up work","description":"Use current customer and policy facts to create the required internal follow-up drafts. After independent approval and business-result readback, invoke the registered closeout Action and verify the completed WorkItem. Never send customer messages.","metadata":{"semantic_scopes":scopes,"fixture":true}})).unwrap()).await.unwrap();
    project_work_item_semantic_object(&state, &work)
        .await
        .unwrap();
    let run_id = Uuid::new_v4();
    state
        .create_ontology_onboarding_run(
            OntologyOnboardingRunRecord {
                id: run_id,
                industry: "customer_success".into(),
                source_mode: "fixture".into(),
                domain_scope: domain.clone(),
                source_dataset_manifest: Some(ontology_onboarding_source_dataset_manifest(&[])),
                source_profiles: Some(Vec::new()),
                status: "pending_review".into(),
                dataset_count: 0,
                profile_count: 0,
                proposal_count: 0,
                approved_count: 0,
                materialized_count: 0,
                actor_subject: "fixture-admin".into(),
                created_at: now,
                updated_at: now,
            },
            Vec::new(),
            new_audit_log(
                None,
                "user",
                None,
                "ontology_onboarding.demo_run_created",
                "ontology_onboarding_run",
                Some(run_id),
                json!({"initial_data_only":true}),
            ),
        )
        .await
        .unwrap();
    let mut proposals = vec![
        fixture_proposal(
            run_id,
            "object",
            "Customer",
            json!({"object_type":"Customer","properties":[{"name":"name","type":"string","nullable":false},{"name":"days_since_last_contact","type":"integer","nullable":false}]}),
        ),
        fixture_proposal(
            run_id,
            "object",
            "FollowupPolicy",
            json!({"object_type":"FollowupPolicy","properties":[{"name":"threshold_days","type":"integer","nullable":false},{"name":"instruction","type":"string","nullable":false}]}),
        ),
        fixture_proposal(
            run_id,
            "object",
            "FollowupDraft",
            json!({"object_type":"FollowupDraft","properties":[{"name":"source_object_id","type":"uuid","nullable":false},{"name":"work_item_id","type":"uuid","nullable":false},{"name":"title","type":"string","nullable":false},{"name":"body","type":"string","nullable":false},{"name":"status","type":"string","nullable":false}]}),
        ),
        fixture_proposal(
            run_id,
            "object",
            "WorkItem",
            json!({"object_type":"WorkItem","properties":[{"name":"status","type":"string"}]}),
        ),
        fixture_proposal(
            run_id,
            "relation",
            "followsPolicy",
            json!({"from_object":"Customer","relation":"follows_policy","to_object":"FollowupPolicy"}),
        ),
    ];
    proposals.push(fixture_proposal(run_id,"action","create_followup_draft",json!({"target_object":"FollowupDraft","inputs":{"type":"object","properties":{"source_object_id":{"type":"uuid"},"work_item_id":{"type":"uuid"},"draft_title":{"type":"string"},"draft_body":{"type":"string"}},"required":["source_object_id","work_item_id","draft_title","draft_body"]},"effects":[{"operation":"create_internal_draft"}],"executor":{"type":"internal_followup_draft","source_object_type":"Customer"},"transaction_profile":"local_serializable","policy":{"approval_required":true}})));
    proposals.push(fixture_proposal(run_id,"action","close_work_item",json!({"target_object":"WorkItem","inputs":{"type":"object","properties":{"work_item_id":{"type":"uuid"},"result_read_receipt_ids":{"type":"array","items":{"type":"uuid"}}},"required":["work_item_id","result_read_receipt_ids"]},"effects":[{"operation":"close_after_approved_readback"}],"executor":{"type":"internal_work_item_closeout"},"transaction_profile":"local_serializable","policy":{"approval_required":true}})));
    let (catalog, digest) = build_ontology_release_catalog(&domain, &proposals, None).unwrap();
    let mut evidence = vec![catalog_evidence(&catalog, &digest)];
    for p in proposals.iter().filter(|p| p.proposal_type == "action") {
        let spec = ontology_tool_spec_from_action_proposal(run_id, p).unwrap();
        let value = serde_json::to_value(spec).unwrap();
        evidence.push(json!({"contract_digest":normalized_json_sha256(&value),"tool_spec":value}));
    }
    let release = state
        .create_ontology_release(OntologyRelease {
            id: Uuid::new_v4(),
            version: format!("fixture-{}", Uuid::new_v4().simple()),
            domain_scope: domain.clone(),
            source_run_id: Some(run_id),
            parent_release_id: None,
            rollback_target_release_id: None,
            status: "active".into(),
            release_class: "repo_controlled".into(),
            object_count: 4,
            relation_count: 1,
            action_count: 2,
            migration_policy: json!({}),
            gate_result: json!({"status":"passed","fixture_initial_registration":true}),
            materialized_object_ids: json!([]),
            materialized_link_ids: json!([]),
            evidence_refs: json!(evidence),
            promoted_by: Some("fixture-admin".into()),
            promoted_at: Some(now),
            rolled_back_by: None,
            rolled_back_at: None,
            archived_at: None,
            created_at: now,
            updated_at: now,
        })
        .await
        .unwrap();
    let subset = OntologySdkSubsetManifest {
        objects: catalog.objects.iter().map(|o| o.api_name.clone()).collect(),
        relations: catalog
            .relations
            .iter()
            .map(|r| r.api_name.clone())
            .collect(),
        actions: catalog.actions.iter().map(|a| a.api_name.clone()).collect(),
    };
    let (subset, subset_digest) = normalize_and_validate_subset(&catalog, &subset).unwrap();
    let application_id = Uuid::new_v4();
    let application = state
        .create_ontology_sdk_application(
            OntologySdkApplication {
                id: application_id,
                tenant_id: state.current_tenant_id(),
                subject: ontology_runtime_subject(agent.id),
                ontology_release_id: release.id,
                release_version: release.version.clone(),
                domain_scope: domain.clone(),
                catalog_digest: digest.clone(),
                subset_manifest: subset,
                subset_digest,
                status: "active".into(),
                created_at: now,
            },
            new_audit_log(
                None,
                "user",
                None,
                "ontology_sdk.application_created",
                "ontology_sdk_application",
                Some(application_id),
                json!({"initial_data_only":true}),
            ),
        )
        .await
        .unwrap();
    let customer=state.create_semantic_object(CreateSemanticObject {source_id:None,object_type:"business_object".into(),object_key:format!("customer:{}",Uuid::new_v4()),title:"Aster Fixture Customer".into(),summary:"Synthetic customer facts".into(),content:json!({"domain_scope":domain,"object_type":"Customer","properties":{"name":"Aster Fixture Customer","days_since_last_contact":45}}),semantic_scopes:scopes.clone(),source_uri:Some("mandoforge://fixture/customer".into()),provenance:json!({"fixture":true}),trust_level:"source_attested".into(),freshness:"current".into(),status:"active".into()}).await.unwrap();
    let rule=state.create_semantic_object(CreateSemanticObject {source_id:None,object_type:"business_object".into(),object_key:format!("rule:{}",Uuid::new_v4()),title:"Follow-up policy".into(),summary:"Synthetic business rule".into(),content:json!({"domain_scope":domain,"object_type":"FollowupPolicy","properties":{"threshold_days":30,"instruction":"If the observed contact gap exceeds threshold_days, prepare a courteous follow-up draft. The body must include the exact customer name and observed contact gap as digits. Never send the draft. Complete the work only after the draft Action is approved, executed and read back."}}),semantic_scopes:scopes.clone(),source_uri:Some("mandoforge://fixture/rule".into()),provenance:json!({"fixture":true}),trust_level:"source_attested".into(),freshness:"current".into(),status:"active".into()}).await.unwrap();
    state
        .create_semantic_link(CreateSemanticLink {
            from_entity_type: "semantic_object".into(),
            from_entity_id: customer.id.to_string(),
            relation_type: "follows_policy".into(),
            to_entity_type: "semantic_object".into(),
            to_entity_id: rule.id.to_string(),
            metadata: json!({}),
            provenance: json!({"fixture":true}),
            confidence: 1.0,
            status: "active".into(),
        })
        .await
        .unwrap();
    let definition = state
        .create_workflow_definition(WorkflowDefinition {
            id: Uuid::new_v4(),
            pack_installation_id: None,
            pack_id: None,
            pack_version: None,
            name: "Ontology-first business fixture".into(),
            entrypoint: format!("business-fixture-{}", Uuid::new_v4()),
            trigger_type: "manual".into(),
            default_agent_id: agent.id,
            default_environment_id: None,
            input_schema_ref: None,
            output_schema_ref: None,
            step_graph: json!({}),
            handoff_rules: json!({}),
            execution_strategy: "managed_graph".into(),
            runtime_adapter: None,
            runtime_mode: None,
            runtime_capability_contract: json!({}),
            event_ingestion_policy: default_event_ingestion_policy(),
            approval_policy_ref: None,
            eval_gate_refs: vec![],
            release_state: "released".into(),
            created_at: now,
            updated_at: now,
            archived_at: None,
        })
        .await
        .unwrap();
    let snapshot = json!({"id":release.id,"version":release.version,"domain_scope":domain,"catalog_digest":digest});
    let run = state
        .create_workflow_run(WorkflowRun {
            id: Uuid::new_v4(),
            workflow_definition_id: definition.id,
            pack_installation_id: None,
            source_event_id: None,
            source_work_item_id: Some(work.id),
            source_schedule_id: None,
            status: "running".into(),
            primary_session_id: session.id,
            root_task_grant_id: None,
            input_payload: json!({}),
            input_digest: format!("fixture-{}", Uuid::new_v4()),
            execution_strategy: "managed_graph".into(),
            runtime_adapter: None,
            runtime_mode: None,
            delegation_status: None,
            external_run_ref: None,
            runtime_event_cursor: None,
            runtime_envelope: json!({"ontology_release":snapshot}),
            started_at: Some(now),
            completed_at: None,
            audit_trace_id: None,
            created_at: now,
            updated_at: now,
        })
        .await
        .unwrap();
    let version = state.agent_version_for_session(session.id).await.unwrap();
    let packet = state
        .create_context_packet(ContextPacket {
            id: Uuid::new_v4(),
            session_id: session.id,
            agent_id: agent.id,
            agent_version_id: Some(version.id),
            version: 1,
            generated_at: now,
            task: json!({"work_item_id":work.id}),
            agent: ContextPacketAgent {
                id: agent.id,
                name: agent.name.clone(),
                kind: agent.kind.clone(),
                agent_role: agent.agent_role.clone(),
                release_state: "active".into(),
                tools: agent.tools.clone(),
                mcp_server_ids: vec![],
                skill_ids: vec![],
                workflow_pack_ids: vec![],
                remote_computer_profile: json!({}),
            },
            runtime_profile: Some(ContextPacketRuntimeProfile {
                id: profile.id,
                name: profile.name,
                runtime_type: "codex_cli".into(),
                remote_computer_required: false,
                status: "enabled".into(),
            }),
            semantic_scopes: scopes.clone(),
            tool_policy: json!({}),
            policy_reminders: vec![],
            freshness_warnings: vec![],
            source_refs: vec![],
            retrieved_objects: vec![],
            replay_summary: json!({"ontology_release":snapshot}),
            audit_trace_id: None,
            created_at: now,
        })
        .await
        .unwrap();
    let grant=state.create_task_grant(TaskGrant {id:Uuid::new_v4(),workflow_run_id:run.id,workflow_step_run_id:None,session_id:Some(session.id),parent_grant_id:None,source_event_id:None,source_handoff_id:None,issuer_subject:"fixture-admin".into(),grantee_agent_id:Some(agent.id),grantee_session_id:Some(session.id),agent_class:None,objective:work.description.clone().unwrap(),risk_level:"low".into(),status:"active".into(),expires_at:Some(now+chrono::Duration::minutes(20)),max_turns:Some(10),max_tool_calls:Some(50),max_runtime_seconds:Some(600),max_cost_usd_micros:None,turns_used:0,tool_calls_used:0,cost_usd_micros_used:0,semantic_scopes:scopes.clone(),memory_scope:json!({}),tool_scope:json!({"read":["ontology.context.read","ontology.action.result.read"],"write":["ontology.action.execute","codex.exec","agent_cli.exec"]}),connector_scope:json!({}),approval_policy:json!({"ontology_release_snapshot":snapshot,"ontology_consumer_scope":{"objects":["Customer","FollowupPolicy"],"relations":catalog.relations.iter().map(|r|r.api_name.clone()).collect::<Vec<_>>(),"actions":catalog.actions.iter().map(|a|a.api_name.clone()).collect::<Vec<_>>(),"object_ids":[customer.id,rule.id]},"ontology_runtime":{"application_id":application.id,"work_item_id":work.id,"required_source_object_ids":[customer.id]}}),external_effects:json!({}),context_packet_id:Some(packet.id),policy_revision_id:None,immutable_args_hash:None,audit_trace_id:None,created_at:now,updated_at:now}).await.unwrap();
    state
        .update_workflow_run_root_task_grant(run.id, grant.id)
        .await
        .unwrap();
    let draft_action = catalog
        .actions
        .iter()
        .find(|a| a.internal_executor.as_deref() == Some("internal_followup_draft"))
        .unwrap()
        .runtime_name
        .clone();
    let close_action = catalog
        .actions
        .iter()
        .find(|a| a.internal_executor.as_deref() == Some("internal_work_item_closeout"))
        .unwrap()
        .runtime_name
        .clone();
    BusinessFixture {
        state,
        session,
        grant,
        work,
        customer,
        rule,
        draft_action,
        close_action,
    }
}

impl BusinessFixture {
    pub(crate) fn input(&self, mut args: Value) -> ExecuteTool {
        args["context_packet_id"] = json!(self.grant.context_packet_id);
        ExecuteTool {
            session_id: self.session.id,
            task_grant_id: Some(self.grant.id),
            args,
        }
    }
    async fn read(&self) -> Value {
        execute_tool_invocation(
            &self.state,
            "ontology.context.read",
            self.input(json!({})),
            ToolInvocationOrigin::RuntimeAdapter,
        )
        .await
        .unwrap()
    }
    fn draft_input(&self, receipt: &Value) -> ExecuteTool {
        self.input(json!({"action":self.draft_action,"ontology_read_receipt_id":receipt["read_receipt_id"],"parameters":{"work_item_id":self.work.id,"source_object_id":self.customer.id,"draft_title":"Follow up with Aster","draft_body":"Hello Aster Fixture Customer, it has been 45 days since our last contact."}}))
    }
    async fn approve_and_execute(&self, result: &Value) -> Result<ExecutionJob, AppError> {
        let id = Uuid::parse_str(result["approval_id"].as_str().unwrap()).unwrap();
        let app = build_router(self.state.clone());
        let (status, body) = request_value(
            app,
            json_request_with_headers(
                "POST",
                &format!("/api/approvals/{id}/approve"),
                json!({}),
                &[
                    ("x-mandoforge-subject", "independent-test-approver"),
                    ("x-mandoforge-roles", "approver"),
                ],
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let job = self
            .state
            .execution_queue
            .list()
            .await
            .unwrap()
            .into_iter()
            .find(|j| j.approval_id == id)
            .unwrap();
        execution::run_execution_job(&self.state, job.id, "fixture-executor").await
    }
}

#[tokio::test]
async fn ontology_business_read_before_action_and_atomic_closeout() {
    let _lock = env_lock().lock().unwrap();
    let f = business_fixture(test_state_with_worker(Arc::new(QueueBackedExecutionWorker))).await;
    assert_read_before_action_and_atomic_closeout(f).await;
}

async fn assert_read_before_action_and_atomic_closeout(f: BusinessFixture) {
    assert!(
        execute_tool_invocation(
            &f.state,
            "ontology.action.execute",
            f.draft_input(&json!({"read_receipt_id":Uuid::new_v4()})),
            ToolInvocationOrigin::RuntimeAdapter
        )
        .await
        .is_err()
    );
    let read = f.read().await;
    assert_eq!(read["objects"].as_array().unwrap().len(), 2);
    let submitted = execute_tool_invocation(
        &f.state,
        "ontology.action.execute",
        f.draft_input(&read),
        ToolInvocationOrigin::RuntimeAdapter,
    )
    .await
    .unwrap();
    assert_eq!(submitted["status"], "approval_required");
    assert!(
        !f.state
            .list_semantic_objects()
            .await
            .unwrap()
            .iter()
            .any(|o| o.content["object_type"] == "FollowupDraft"
                && o.content["properties"]["work_item_id"] == json!(f.work.id))
    );
    let job = f.approve_and_execute(&submitted).await.unwrap();
    assert_eq!(job.status, ExecutionJobStatus::Completed);
    let call = f.state.get_tool_call(job.tool_call_id).await.unwrap();
    assert_eq!(
        call.result.as_ref().unwrap()["status"],
        "business_draft_created"
    );
    assert!(
        apply_provider_completion(
            &f.state,
            f.session.id,
            Some(f.grant.id),
            "completed",
            "model says done"
        )
        .await
        .is_err()
    );
    let rb = execute_tool_invocation(
        &f.state,
        "ontology.action.result.read",
        f.input(json!({"action_tool_call_id":call.id})),
        ToolInvocationOrigin::RuntimeAdapter,
    )
    .await
    .unwrap();
    let close=execute_tool_invocation(&f.state,"ontology.action.execute",f.input(json!({"action":f.close_action,"ontology_read_receipt_id":read["read_receipt_id"],"parameters":{"work_item_id":f.work.id,"result_read_receipt_ids":[rb["result_read_receipt_id"]]}})),ToolInvocationOrigin::RuntimeAdapter).await.unwrap();
    let close_job = f.approve_and_execute(&close).await.unwrap();
    assert_eq!(close_job.status, ExecutionJobStatus::Completed);
    assert!(
        !ontology_codex_runtime::business_completion_verified(
            &f.state,
            f.session.id,
            f.grant.id,
            None
        )
        .await
        .unwrap()
    );
    execute_tool_invocation(
        &f.state,
        "ontology.action.result.read",
        f.input(json!({"action_tool_call_id":close_job.tool_call_id})),
        ToolInvocationOrigin::RuntimeAdapter,
    )
    .await
    .unwrap();
    assert!(
        ontology_codex_runtime::business_completion_verified(
            &f.state,
            f.session.id,
            f.grant.id,
            None
        )
        .await
        .unwrap()
    );
    let work = f
        .state
        .list_work_items()
        .await
        .unwrap()
        .into_iter()
        .find(|w| w.id == f.work.id)
        .unwrap();
    assert_eq!(work.status, "done");
    let projection = f
        .state
        .list_semantic_objects()
        .await
        .unwrap()
        .into_iter()
        .find(|o| o.object_key == format!("work_item:{}", f.work.id))
        .unwrap();
    assert_eq!(projection.content["status"], "done");
}

#[tokio::test]
async fn ontology_business_rejects_driver_receipts_session_and_object_substitution() {
    let _lock = env_lock().lock().unwrap();
    let f = business_fixture(test_state_with_worker(Arc::new(QueueBackedExecutionWorker))).await;
    let manual = execute_tool_invocation(
        &f.state,
        "ontology.context.read",
        f.input(json!({})),
        ToolInvocationOrigin::ManualRoute,
    )
    .await
    .unwrap();
    assert!(
        execute_tool_invocation(
            &f.state,
            "ontology.action.execute",
            f.draft_input(&manual),
            ToolInvocationOrigin::RuntimeAdapter
        )
        .await
        .is_err()
    );
    let read = f.read().await;
    let mut wrong = f.draft_input(&read);
    wrong.args["parameters"]["source_object_id"] = json!(Uuid::new_v4());
    assert!(
        execute_tool_invocation(
            &f.state,
            "ontology.action.execute",
            wrong,
            ToolInvocationOrigin::RuntimeAdapter
        )
        .await
        .is_err()
    );
    let mut wrong = f.draft_input(&read);
    wrong.args["parameters"]["source_object_id"] = json!(f.rule.id);
    assert!(
        execute_tool_invocation(
            &f.state,
            "ontology.action.execute",
            wrong,
            ToolInvocationOrigin::RuntimeAdapter
        )
        .await
        .is_err()
    );
    let other = f
        .state
        .create_session(CreateSession {
            agent_id: f.session.agent_id,
            environment_id: None,
            title: "engineering session".into(),
            message: None,
        })
        .await
        .unwrap();
    let mut wrong = f.draft_input(&read);
    wrong.session_id = other.id;
    assert!(
        execute_tool_invocation(
            &f.state,
            "ontology.action.execute",
            wrong,
            ToolInvocationOrigin::RuntimeAdapter
        )
        .await
        .is_err()
    );
    if let StoreBackend::Memory(inner) = &f.state.store {
        let mut store = inner.write().await;
        let grant = store.task_grants.get_mut(&f.grant.id).unwrap();
        grant.tool_scope["write"]
            .as_array_mut()
            .unwrap()
            .extend([json!("shell.exec"), json!("mcp.call")]);
    }
    assert!(
        execute_tool_invocation(
            &f.state,
            "shell.exec",
            f.input(json!({"command":"true"})),
            ToolInvocationOrigin::RuntimeAdapter
        )
        .await
        .is_err()
    );
    assert!(
        execute_tool_invocation(
            &f.state,
            "mcp.call",
            f.input(json!({"server":"anything","tool":"write"})),
            ToolInvocationOrigin::RuntimeAdapter
        )
        .await
        .is_err()
    );
    assert!(f.state.execution_queue.list().await.unwrap().is_empty());
}

#[tokio::test]
async fn ontology_business_stale_and_revoked_authority_rejects_effects() {
    let _lock = env_lock().lock().unwrap();
    let f = business_fixture(test_state_with_worker(Arc::new(QueueBackedExecutionWorker))).await;
    let read = f.read().await;
    let mut content = f.customer.content.clone();
    content["properties"]["days_since_last_contact"] = json!(75);
    f.state
        .update_semantic_object(
            f.customer.id,
            serde_json::from_value(json!({"content":content})).unwrap(),
        )
        .await
        .unwrap();
    assert!(
        execute_tool_invocation(
            &f.state,
            "ontology.action.execute",
            f.draft_input(&read),
            ToolInvocationOrigin::RuntimeAdapter
        )
        .await
        .is_err()
    );
    let fresh = f.read().await;
    let proposed = execute_tool_invocation(
        &f.state,
        "ontology.action.execute",
        f.draft_input(&fresh),
        ToolInvocationOrigin::RuntimeAdapter,
    )
    .await
    .unwrap();
    let mut content = f.rule.content.clone();
    content["properties"]["threshold_days"] = json!(80);
    f.state
        .update_semantic_object(
            f.rule.id,
            serde_json::from_value(json!({"content":content})).unwrap(),
        )
        .await
        .unwrap();
    assert!(f.approve_and_execute(&proposed).await.is_err());
    assert!(
        !f.state
            .list_semantic_objects()
            .await
            .unwrap()
            .iter()
            .any(|o| o.content["object_type"] == "FollowupDraft"
                && o.content["properties"]["work_item_id"] == json!(f.work.id))
    );
    f.state
        .update_task_grant_status(f.grant.id, "revoked")
        .await
        .unwrap();
    assert!(
        execute_tool_invocation(
            &f.state,
            "ontology.context.read",
            f.input(json!({})),
            ToolInvocationOrigin::RuntimeAdapter
        )
        .await
        .is_err()
    );
    assert!(
        apply_provider_completion(
            &f.state,
            f.session.id,
            Some(f.grant.id),
            "completed",
            "false completion"
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn ontology_business_duplicate_approved_actions_do_not_duplicate_drafts() {
    let _lock = env_lock().lock().unwrap();
    let f = business_fixture(test_state_with_worker(Arc::new(QueueBackedExecutionWorker))).await;
    assert_duplicate_actions_do_not_duplicate_drafts(f).await;
}

async fn assert_duplicate_actions_do_not_duplicate_drafts(f: BusinessFixture) {
    let read = f.read().await;
    let first = execute_tool_invocation(
        &f.state,
        "ontology.action.execute",
        f.draft_input(&read),
        ToolInvocationOrigin::RuntimeAdapter,
    )
    .await
    .unwrap();
    let first_job = f.approve_and_execute(&first).await.unwrap();
    assert_eq!(first_job.status, ExecutionJobStatus::Completed);
    let second = execute_tool_invocation(
        &f.state,
        "ontology.action.execute",
        f.draft_input(&read),
        ToolInvocationOrigin::RuntimeAdapter,
    )
    .await
    .unwrap();
    let second_job = f.approve_and_execute(&second).await.unwrap();
    assert_eq!(second_job.status, ExecutionJobStatus::Completed);
    let second_call = f
        .state
        .get_tool_call(second_job.tool_call_id)
        .await
        .unwrap();
    assert_eq!(second_call.result.unwrap()["effect_reused"], true);
    assert_eq!(
        f.state
            .list_semantic_objects()
            .await
            .unwrap()
            .iter()
            .filter(|o| o.content["object_type"] == "FollowupDraft"
                && o.content["properties"]["work_item_id"] == json!(f.work.id))
            .count(),
        1
    );
    assert!(execute_tool_invocation(&f.state,"ontology.action.execute",f.input(json!({"action":f.close_action,"ontology_read_receipt_id":read["read_receipt_id"],"parameters":{"work_item_id":f.work.id,"result_read_receipt_ids":[]}})),ToolInvocationOrigin::RuntimeAdapter).await.is_err());
}

#[tokio::test]
async fn ontology_business_cannot_drop_binding_or_reuse_runtime_credential_at_general_api() {
    let _lock = env_lock().lock().unwrap();
    let f = business_fixture(test_state_with_worker(Arc::new(QueueBackedExecutionWorker))).await;
    let mut child = f.grant.clone();
    child.id = Uuid::new_v4();
    child.parent_grant_id = Some(f.grant.id);
    child
        .approval_policy
        .as_object_mut()
        .unwrap()
        .remove("ontology_runtime");
    assert!(
        validate_ontology_runtime_lineage(&f.state, &child)
            .await
            .is_err()
    );
    child.approval_policy = f.grant.approval_policy.clone();
    child.approval_policy["ontology_consumer_scope"]["object_ids"] = json!([Uuid::new_v4()]);
    assert!(
        validate_ontology_runtime_lineage(&f.state, &child)
            .await
            .is_err()
    );
    for token in [
        "Bearer mforuntime-v1.fixture",
        "  Bearer   mforuntime-v1.fixture ",
        "bearer mforuntime-v1.fixture",
    ] {
        let headers = HeaderMap::from_iter([
            ("authorization".parse().unwrap(), token.parse().unwrap()),
            (
                "x-mandoforge-subject".parse().unwrap(),
                "admin".parse().unwrap(),
            ),
            (
                "x-mandoforge-roles".parse().unwrap(),
                "admin".parse().unwrap(),
            ),
        ]);
        assert!(principal_from_request(&f.state, &headers).await.is_err());
    }
}

#[tokio::test]
async fn ontology_business_expired_receipt_and_grant_fail_closed() {
    let _lock = env_lock().lock().unwrap();
    let f = business_fixture(test_state_with_worker(Arc::new(QueueBackedExecutionWorker))).await;
    let read = f.read().await;
    let id = Uuid::parse_str(read["read_receipt_id"].as_str().unwrap()).unwrap();
    if let StoreBackend::Memory(inner) = &f.state.store {
        inner
            .write()
            .await
            .ontology_read_receipts
            .get_mut(&id)
            .unwrap()
            .expires_at = Utc::now() - chrono::Duration::seconds(1);
    }
    assert!(
        execute_tool_invocation(
            &f.state,
            "ontology.action.execute",
            f.draft_input(&read),
            ToolInvocationOrigin::RuntimeAdapter
        )
        .await
        .is_err()
    );
    if let StoreBackend::Memory(inner) = &f.state.store {
        inner
            .write()
            .await
            .task_grants
            .get_mut(&f.grant.id)
            .unwrap()
            .expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
    }
    assert!(
        execute_tool_invocation(
            &f.state,
            "ontology.context.read",
            f.input(json!({})),
            ToolInvocationOrigin::RuntimeAdapter
        )
        .await
        .is_err()
    );
}

pub(crate) async fn memory_business_fixture() -> BusinessFixture {
    business_fixture(test_state_with_worker(Arc::new(QueueBackedExecutionWorker))).await
}

pub(crate) fn business_fixture_env_lock() -> std::sync::MutexGuard<'static, ()> {
    env_lock().lock().unwrap()
}

async fn postgres_business_state() -> AppState {
    let database_url =
        std::env::var("MANDOFORGE_TEST_POSTGRES_URL").expect("isolated Postgres required");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .unwrap();
    run_migrations(&pool).await.unwrap();
    let mut state = test_state_with_worker(Arc::new(QueueBackedExecutionWorker));
    state.tenant_id = Uuid::new_v4();
    state.workspace_root =
        std::env::temp_dir().join(format!("mandoforge-business-{}", state.tenant_id));
    sqlx::query("INSERT INTO tenants (id,name,slug) VALUES ($1,'Isolated business fixture',$2)")
        .bind(state.tenant_id)
        .bind(format!("business-fixture-{}", state.tenant_id))
        .execute(&pool)
        .await
        .unwrap();
    state.execution_queue = ExecutionQueue::postgres(pool.clone(), state.tenant_id);
    state.store = StoreBackend::Postgres(pool);
    state
}

/// Opt-in real-provider test. The driver registers synthetic initial data, starts
/// a normal workflow, acts as an independent test Approver, and reads evidence.
/// It never calls the Agent's Ontology tools or creates its business effects.
#[tokio::test]
#[ignore = "requires isolated MANDOFORGE_TEST_POSTGRES_URL and MANDOFORGE_RUN_REAL_CODEX=1 with local Codex login"]
async fn ontology_business_real_codex_native_worker_closes_work_item() {
    let _lock = env_lock().lock().unwrap();
    assert_eq!(
        std::env::var("MANDOFORGE_RUN_REAL_CODEX").as_deref(),
        Ok("1")
    );
    let f = business_fixture(postgres_business_state().await).await;
    let state = &f.state;
    let environment = state.create_environment(serde_json::from_value(json!({"name":format!("ontology real fixture {}",Uuid::new_v4()),"release_state":"active"})).unwrap()).await.unwrap();
    let original_run = state
        .get_workflow_run(f.grant.workflow_run_id)
        .await
        .unwrap();
    let mut definition = state
        .get_workflow_definition(original_run.workflow_definition_id)
        .await
        .unwrap();
    definition.id = Uuid::new_v4();
    definition.entrypoint = format!("ontology-real-{}", definition.id);
    definition.default_environment_id = Some(environment.id);
    definition.execution_strategy = "native_steps".into();
    definition.step_graph = json!({"steps":[{"key":"business-work","type":"agent","start":true}]});
    let mut policy = f.grant.approval_policy.clone();
    policy
        .as_object_mut()
        .unwrap()
        .remove("ontology_release_snapshot");
    policy["ontology_runtime"]
        .as_object_mut()
        .unwrap()
        .remove("work_item_id");
    definition.handoff_rules = json!({"root_task_grant":{"max_turns":10,"max_tool_calls":50,"max_runtime_seconds":600,"semantic_scopes":f.grant.semantic_scopes,"tool_scope":f.grant.tool_scope,"approval_policy":policy}});
    let definition = state.create_workflow_definition(definition).await.unwrap();
    let app = build_router(state.clone());
    let (status, body) = request_value(app.clone(), json_request_with_headers("POST","/api/workflow-runs",json!({"workflow_definition_id":definition.id,"source_work_item_id":f.work.id,"title":"Complete the assigned business work through Ontology","input_payload":{}}),&[("x-mandoforge-subject","fixture-operator"),("x-mandoforge-roles","operator")])).await;
    assert!(status.is_success(), "{status}: {body}");
    let run: WorkflowRun = serde_json::from_value(body).unwrap();
    let root_id = run.root_task_grant_id.unwrap();
    let root = state.get_task_grant(root_id).await.unwrap();
    assert_eq!(
        root.approval_policy["ontology_runtime"]["work_item_id"],
        json!(f.work.id)
    );
    assert_ne!(root.id, f.grant.id);
    let _database = EnvVarGuard::set(
        "DATABASE_URL",
        &std::env::var("MANDOFORGE_TEST_POSTGRES_URL").unwrap(),
    );
    let _token = EnvVarGuard::set(
        "MANDOFORGE_WORKER_TOKEN",
        "ontology-isolated-test-worker-token",
    );
    let _worker_id = EnvVarGuard::set("WORKER_ID", "ontology-real-native-worker");
    let _environment = EnvVarGuard::set("WORKER_ENVIRONMENT_ID", &environment.id.to_string());
    let _pool = EnvVarGuard::remove("WORKER_POOL");
    let _queue = EnvVarGuard::remove("WORKER_QUEUE");
    let _once = EnvVarGuard::remove("RUN_ONCE");
    let _max = EnvVarGuard::set("MAX_JOBS", "0");
    let _poll = EnvVarGuard::set("POLL_INTERVAL_SECONDS", "0.2");
    let _concurrency = EnvVarGuard::set("WORKER_CONCURRENCY", "1");
    let _provider = EnvVarGuard::remove("MANDOFORGE_PROVIDER_BASE_URL");
    let _provider_key = EnvVarGuard::remove("MANDOFORGE_PROVIDER_API_KEY");
    let mut approvals = std::collections::BTreeSet::new();
    let observe_and_approve = async {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(480);
        loop {
            let current = state.get_workflow_run(run.id).await.unwrap();
            let calls = state
                .list_tool_calls(Some(run.primary_session_id))
                .await
                .unwrap();
            for call in calls.iter().filter(|c| {
                c.tool_name == "ontology.action.execute" && c.status == "waiting_approval"
            }) {
                assert_eq!(call.task_grant_id, Some(root_id));
                assert_eq!(call.policy_decision["origin"], "runtime_adapter");
                let action = call.args["action"].as_str().unwrap();
                assert!(
                    action == f.draft_action || action == f.close_action,
                    "unexpected Action {action}"
                );
                assert_eq!(call.args["parameters"]["work_item_id"], json!(f.work.id));
                if action == f.draft_action {
                    assert_eq!(
                        call.args["parameters"]["source_object_id"],
                        json!(f.customer.id)
                    );
                    let draft_body = call.args["parameters"]["draft_body"].as_str().unwrap();
                    assert!(
                        draft_body.contains("Aster Fixture Customer") && draft_body.contains("45"),
                        "Agent must use actual Ontology facts: {draft_body}"
                    );
                }
                let pending = state
                    .list_approvals()
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|a| a.tool_call_id == Some(call.id))
                    .unwrap();
                if approvals.insert(pending.id) {
                    let (status, body) = request_value(
                        app.clone(),
                        json_request_with_headers(
                            "POST",
                            &format!("/api/approvals/{}/approve", pending.id),
                            json!({}),
                            &[
                                ("x-mandoforge-subject", "independent-test-approver"),
                                ("x-mandoforge-roles", "approver"),
                            ],
                        ),
                    )
                    .await;
                    assert_eq!(status, StatusCode::OK, "test approval: {body}");
                    eprintln!(
                        "Test Approver approved {action}; Agent proposal {}",
                        call.id
                    );
                }
            }
            if matches!(current.status.as_str(), "completed" | "failed" | "canceled")
                || tokio::time::Instant::now() >= deadline
            {
                break current;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    };
    let terminal = tokio::select! {
        result=crate::worker_daemon::run_worker_daemon(state.clone())=>panic!("worker stopped before independently observed completion: {result:?}"),
        result=observe_and_approve=>result,
    };
    let calls = state
        .list_tool_calls(Some(run.primary_session_id))
        .await
        .unwrap();
    let events = state.list_events(run.primary_session_id).await.unwrap();
    let work = state
        .list_work_items()
        .await
        .unwrap()
        .into_iter()
        .find(|w| w.id == f.work.id)
        .unwrap();
    let objects = state.list_semantic_objects().await.unwrap();
    let drafts = objects
        .iter()
        .filter(|o| {
            o.content["object_type"] == "FollowupDraft"
                && o.content["properties"]["work_item_id"] == json!(f.work.id)
        })
        .collect::<Vec<_>>();
    let session = state.get_session(run.primary_session_id).await.unwrap();
    if let Ok(directory) = std::env::var("MANDOFORGE_BUSINESS_EVIDENCE_DIR") {
        std::fs::create_dir_all(&directory).unwrap();
        let evidence = json!({"test":"real_codex_native_worker","approval_class":"independent_test_role_dev_auth_not_human_approval","workflow_run":terminal,"session":session,"work_item":work,"drafts":drafts,"tool_calls":calls,"events":events,"approval_ids":approvals});
        std::fs::write(
            std::path::Path::new(&directory).join("real-business-loop.json"),
            serde_json::to_vec_pretty(&evidence).unwrap(),
        )
        .unwrap();
    }
    assert_eq!(
        terminal.status, "completed",
        "workflow did not complete; session {session:?}; calls {calls:?}"
    );
    assert_eq!(session.status, SessionStatus::Terminated);
    assert_eq!(work.status, "done");
    assert_eq!(drafts.len(), 1);
    assert_eq!(approvals.len(), 2);
    assert!(
        ontology_codex_runtime::business_completion_verified(state, session.id, root_id, None)
            .await
            .unwrap()
    );
    assert!(calls.iter().any(|c| c.tool_name == "ontology.context.read"
        && c.policy_decision["origin"] == "runtime_adapter"));
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.tool_name == "ontology.action.execute" && c.status == "completed")
            .count(),
        2
    );
    assert!(
        events
            .iter()
            .any(|e| e.event_type == "business_agent.native_event"
                && e.payload.to_string().contains("mcp_tool_call")),
        "real Codex must emit native MCP evidence"
    );
    let artifact = state.list_artifacts(session.id).await.unwrap();
    assert!(
        artifact
            .iter()
            .any(|a| a.artifact_type == "business_result")
    );
    tokio::fs::remove_dir_all(&state.workspace_root)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires isolated MANDOFORGE_TEST_POSTGRES_URL"]
async fn ontology_business_postgres_read_before_action_and_atomic_closeout() {
    let _lock = env_lock().lock().unwrap();
    assert_read_before_action_and_atomic_closeout(
        business_fixture(postgres_business_state().await).await,
    )
    .await;
}

#[tokio::test]
#[ignore = "requires isolated MANDOFORGE_TEST_POSTGRES_URL"]
async fn ontology_business_postgres_receipts_are_immutable_tenant_scoped_and_stale_effects_roll_back()
 {
    let _lock = env_lock().lock().unwrap();
    let f = business_fixture(postgres_business_state().await).await;
    let read = f.read().await;
    let id = Uuid::parse_str(read["read_receipt_id"].as_str().unwrap()).unwrap();
    let StoreBackend::Postgres(pool) = &f.state.store else {
        unreachable!()
    };
    assert!(
        sqlx::query("UPDATE ontology_runtime_read_receipts SET payload='{}' WHERE id=$1")
            .bind(id)
            .execute(pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM ontology_runtime_read_receipts WHERE id=$1")
            .bind(id)
            .execute(pool)
            .await
            .is_err()
    );
    let mut other = f.state.clone();
    other.tenant_id = Uuid::new_v4();
    assert!(other.get_ontology_read_receipt(id).await.is_err());
    // Exercise RLS as a non-superuser; the fixture connection otherwise owns the DB.
    let mut tx = pool.begin().await.unwrap();
    let role = format!("receipt_reader_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE ROLE {role} NOLOGIN"))
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(&format!(
        "GRANT SELECT ON ontology_runtime_read_receipts TO {role}"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(&format!("SET LOCAL ROLE {role}"))
        .execute(&mut *tx)
        .await
        .unwrap();
    for (tenant, expected) in [(f.state.tenant_id, 1_i64), (other.tenant_id, 0_i64)] {
        sqlx::query("SELECT set_config('mandoforge.tenant_id',$1,true)")
            .bind(tenant.to_string())
            .execute(&mut *tx)
            .await
            .unwrap();
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM ontology_runtime_read_receipts WHERE id=$1",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert_eq!(count, expected, "receipt RLS tenant boundary");
    }
    tx.rollback().await.unwrap();
    let submitted = execute_tool_invocation(
        &f.state,
        "ontology.action.execute",
        f.draft_input(&read),
        ToolInvocationOrigin::RuntimeAdapter,
    )
    .await
    .unwrap();
    let mut content = f.rule.content.clone();
    content["properties"]["threshold_days"] = json!(90);
    f.state
        .update_semantic_object(
            f.rule.id,
            serde_json::from_value(json!({"content":content})).unwrap(),
        )
        .await
        .unwrap();
    assert!(f.approve_and_execute(&submitted).await.is_err());
    assert!(
        !f.state
            .list_semantic_objects()
            .await
            .unwrap()
            .iter()
            .any(|o| o.content["object_type"] == "FollowupDraft"
                && o.content["properties"]["work_item_id"] == json!(f.work.id))
    );
    assert!(
        !ontology_codex_runtime::business_completion_verified(
            &f.state,
            f.session.id,
            f.grant.id,
            None
        )
        .await
        .unwrap()
    );
}

#[tokio::test]
async fn ontology_business_revocation_after_proposal_prevents_approved_effects() {
    let _lock = env_lock().lock().unwrap();
    let f = business_fixture(test_state_with_worker(Arc::new(QueueBackedExecutionWorker))).await;
    let read = f.read().await;
    let submitted = execute_tool_invocation(
        &f.state,
        "ontology.action.execute",
        f.draft_input(&read),
        ToolInvocationOrigin::RuntimeAdapter,
    )
    .await
    .unwrap();
    f.state
        .update_task_grant_status(f.grant.id, "revoked")
        .await
        .unwrap();
    if let Ok(job) = f.approve_and_execute(&submitted).await {
        assert_ne!(
            job.status,
            ExecutionJobStatus::Completed,
            "revoked grant cannot execute"
        );
    }
    assert!(
        !f.state
            .list_semantic_objects()
            .await
            .unwrap()
            .iter()
            .any(|o| o.content["object_type"] == "FollowupDraft")
    );
    assert!(
        !ontology_codex_runtime::business_completion_verified(
            &f.state,
            f.session.id,
            f.grant.id,
            None
        )
        .await
        .unwrap()
    );
}

#[tokio::test]
#[ignore = "requires isolated MANDOFORGE_TEST_POSTGRES_URL"]
async fn ontology_business_postgres_duplicate_effects_are_idempotent() {
    let _lock = env_lock().lock().unwrap();
    assert_duplicate_actions_do_not_duplicate_drafts(
        business_fixture(postgres_business_state().await).await,
    )
    .await;
}
