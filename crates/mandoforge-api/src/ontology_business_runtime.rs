//! Authority and evidence shared by every governed business runtime adapter.
//! Receipt IDs refer to server-owned immutable records, never caller attestations.
use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::{DateTime, Duration, SubsecRound, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::{
    AppError, AppState, ContextPacket, ExecuteTool, OntologyRelease, OntologyReleaseCatalogV1,
    OntologySdkApplication, Principal, Role, SemanticObject, TaskGrant, ToolCall, ToolDescriptor,
    ToolExecutor, consumer_objects, consumer_relations,
    context_packet_and_grant_for_tool_invocation, normalized_json_sha256,
    resolve_consumer_application, resolved_catalog_for_subset, task_grant_session_matches,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OntologyRuntimeBinding {
    pub(crate) application_id: Uuid,
    pub(crate) work_item_id: Uuid,
    pub(crate) required_source_object_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct OntologyReadReceipt {
    pub(crate) id: Uuid,
    pub(crate) tenant_id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) task_grant_id: Uuid,
    pub(crate) application_id: Uuid,
    pub(crate) source_tool_call_id: Uuid,
    pub(crate) kind: String,
    pub(crate) context_packet_id: Uuid,
    pub(crate) context_packet_version: i64,
    pub(crate) ontology_release_id: Uuid,
    pub(crate) release_version: String,
    pub(crate) catalog_digest: String,
    pub(crate) subset_digest: String,
    pub(crate) grant_scope_digest: String,
    pub(crate) work_item_id: Uuid,
    pub(crate) work_item_version: String,
    pub(crate) object_versions: BTreeMap<Uuid, String>,
    pub(crate) relation_versions: BTreeMap<Uuid, String>,
    pub(crate) action_tool_call_id: Option<Uuid>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) expires_at: DateTime<Utc>,
}

pub(crate) fn ontology_runtime_binding(
    grant: &TaskGrant,
) -> Result<Option<OntologyRuntimeBinding>, AppError> {
    grant
        .approval_policy
        .get("ontology_runtime")
        .cloned()
        .map(|v| {
            serde_json::from_value(v)
                .map_err(|_| AppError::forbidden("invalid Ontology runtime binding"))
        })
        .transpose()
}

pub(crate) fn ontology_runtime_authority_digest(grant: &TaskGrant) -> String {
    normalized_json_sha256(
        &json!({"id":grant.id,"parent":grant.parent_grant_id,"agent":grant.grantee_agent_id,
        "session":grant.grantee_session_id,"context":grant.context_packet_id,"tool_scope":grant.tool_scope,
        "memory_scope":grant.memory_scope,"semantic_scopes":grant.semantic_scopes,
        "connector_scope":grant.connector_scope,"approval_policy":grant.approval_policy}),
    )
}

pub(crate) fn ontology_runtime_subject(agent_id: Uuid) -> String {
    format!("agent-runtime:{agent_id}")
}

pub(crate) fn ontology_object_version(object: &SemanticObject) -> String {
    normalized_json_sha256(&json!({"id":object.id,"object_type":object.object_type,
        "object_key":object.object_key,"status":object.status,"archived_at":object.archived_at,
        "content":object.content,"semantic_scopes":object.semantic_scopes,"updated_at":object.updated_at}))
}

pub(crate) struct OntologyRuntimeContext {
    pub(crate) packet: ContextPacket,
    pub(crate) grant: TaskGrant,
    pub(crate) binding: OntologyRuntimeBinding,
    pub(crate) principal: Principal,
    pub(crate) application: OntologySdkApplication,
    pub(crate) release: OntologyRelease,
    pub(crate) catalog: OntologyReleaseCatalogV1,
}

pub(crate) async fn ontology_runtime_context(
    state: &AppState,
    input: &ExecuteTool,
) -> Result<OntologyRuntimeContext, AppError> {
    let (packet, grant) = context_packet_and_grant_for_tool_invocation(state, input).await?;
    let grant = grant.ok_or_else(|| AppError::forbidden("business runtime requires TaskGrant"))?;
    if grant.status != "active" || grant.expires_at.is_some_and(|v| v <= Utc::now()) {
        return Err(AppError::forbidden(
            "business TaskGrant is revoked or expired",
        ));
    }
    let run = state.get_workflow_run(grant.workflow_run_id).await?;
    if !task_grant_session_matches(&grant, &run, input.session_id) {
        return Err(AppError::forbidden("business TaskGrant session mismatch"));
    }
    let session = state.get_session(input.session_id).await?;
    if grant.grantee_agent_id != Some(session.agent_id) || packet.agent_id != session.agent_id {
        return Err(AppError::forbidden(
            "business runtime Agent identity mismatch",
        ));
    }
    let binding = ontology_runtime_binding(&grant)?
        .ok_or_else(|| AppError::forbidden("business runtime binding is required"))?;
    let principal = Principal {
        tenant_id: state.current_tenant_id(),
        subject_id: ontology_runtime_subject(session.agent_id),
        roles: vec![Role::Operator],
    };
    crate::authorize_principal_request(
        state,
        &principal,
        crate::Permission::SessionsRead,
        "session",
        Some(input.session_id),
    )
    .await?;
    crate::authorize_principal_request(
        state,
        &principal,
        crate::Permission::SessionsRead,
        "work_item",
        Some(binding.work_item_id),
    )
    .await?;
    let (application, release, catalog) =
        resolve_consumer_application(state, &principal, binding.application_id).await?;
    let pinned = packet
        .replay_summary
        .get("ontology_release")
        .cloned()
        .unwrap_or(Value::Null);
    if pinned["id"].as_str() != Some(&release.id.to_string())
        || pinned["version"].as_str() != Some(&release.version)
        || pinned["catalog_digest"].as_str() != Some(&application.catalog_digest)
        || ["id", "version", "domain_scope", "catalog_digest"]
            .iter()
            .any(|key| grant.approval_policy["ontology_release_snapshot"][key] != pinned[key])
    {
        return Err(AppError::forbidden(
            "business runtime release/context binding mismatch",
        ));
    }
    let ids = grant.approval_policy["ontology_consumer_scope"]["object_ids"]
        .as_array()
        .ok_or_else(|| {
            AppError::forbidden("business runtime requires explicit Ontology object IDs")
        })?;
    if ids.is_empty()
        || ids.len() > 128
        || ids
            .iter()
            .any(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()).is_none())
    {
        return Err(AppError::forbidden(
            "invalid business Ontology object scope",
        ));
    }
    if binding.required_source_object_ids.is_empty()
        || binding.required_source_object_ids.len() > 128
        || binding
            .required_source_object_ids
            .iter()
            .any(|id| !ids.iter().any(|v| v.as_str() == Some(&id.to_string())))
    {
        return Err(AppError::forbidden(
            "business completion requires explicit source objects inside the grant",
        ));
    }
    Ok(OntologyRuntimeContext {
        packet,
        grant,
        binding,
        principal,
        application,
        release,
        catalog,
    })
}

impl AppState {
    pub(crate) async fn insert_ontology_read_receipt(
        &self,
        receipt: &OntologyReadReceipt,
    ) -> Result<(), AppError> {
        if receipt.tenant_id != self.current_tenant_id() {
            return Err(AppError::forbidden("read receipt tenant mismatch"));
        }
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store.ontology_read_receipts.contains_key(&receipt.id) {
                    return Err(AppError::conflict("read receipt already exists"));
                }
                store
                    .ontology_read_receipts
                    .insert(receipt.id, receipt.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query("INSERT INTO ontology_runtime_read_receipts (id,tenant_id,session_id,task_grant_id,application_id,source_tool_call_id,kind,expires_at,payload,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
                    .bind(receipt.id).bind(receipt.tenant_id).bind(receipt.session_id).bind(receipt.task_grant_id)
                    .bind(receipt.application_id).bind(receipt.source_tool_call_id).bind(&receipt.kind)
                    .bind(receipt.expires_at).bind(serde_json::to_value(receipt)?).bind(receipt.created_at).execute(pool).await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn get_ontology_read_receipt(
        &self,
        id: Uuid,
    ) -> Result<OntologyReadReceipt, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .ontology_read_receipts
                .get(&id)
                .filter(|r| r.tenant_id == self.current_tenant_id())
                .cloned()
                .ok_or_else(|| AppError::forbidden("unknown Ontology read receipt")),
            StoreBackend::Postgres(pool) => {
                let payload = sqlx::query_scalar::<_, Value>("SELECT payload FROM ontology_runtime_read_receipts WHERE tenant_id=$1 AND id=$2")
                    .bind(self.current_tenant_id()).bind(id).fetch_optional(pool).await?
                    .ok_or_else(|| AppError::forbidden("unknown Ontology read receipt"))?;
                Ok(serde_json::from_value(payload)?)
            }
        }
    }
}

fn new_receipt(
    state: &AppState,
    context: &OntologyRuntimeContext,
    call: &ToolCall,
    kind: &str,
) -> OntologyReadReceipt {
    let now = Utc::now();
    OntologyReadReceipt {
        id: Uuid::new_v4(),
        tenant_id: state.current_tenant_id(),
        session_id: context.packet.session_id,
        task_grant_id: context.grant.id,
        application_id: context.application.id,
        source_tool_call_id: call.id,
        kind: kind.into(),
        context_packet_id: context.packet.id,
        context_packet_version: context.packet.version,
        ontology_release_id: context.release.id,
        release_version: context.release.version.clone(),
        catalog_digest: context.application.catalog_digest.clone(),
        subset_digest: context.application.subset_digest.clone(),
        grant_scope_digest: ontology_runtime_authority_digest(&context.grant),
        work_item_id: context.binding.work_item_id,
        work_item_version: String::new(),
        object_versions: BTreeMap::new(),
        relation_versions: BTreeMap::new(),
        action_tool_call_id: None,
        created_at: now,
        expires_at: context
            .grant
            .expires_at
            .unwrap_or(now + Duration::minutes(15))
            .min(now + Duration::minutes(15)),
    }
}

pub(crate) struct OntologyContextReadTool;
#[async_trait]
impl ToolExecutor for OntologyContextReadTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "ontology.context.read",
            risk: "low",
            description: "Read authorized task, Ontology objects, relationships, rules and Action contracts; issue bound read evidence",
        }
    }
    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        call: &ToolCall,
    ) -> Result<Value, AppError> {
        let context = ontology_runtime_context(state, input).await?;
        let mut objects = Vec::new();
        let mut receipt = new_receipt(state, &context, call, "context");
        for api_name in &context.application.subset_manifest.objects {
            if !context.grant.approval_policy["ontology_consumer_scope"]["objects"]
                .as_array()
                .is_some_and(|names| names.iter().any(|n| n.as_str() == Some(api_name)))
            {
                continue;
            }
            for object in consumer_objects(
                state,
                &context.principal,
                &context.application,
                &context.catalog,
                api_name,
                Some(&context.grant),
            )
            .await?
            {
                receipt
                    .object_versions
                    .insert(object.id, object.version.clone());
                objects.push(object);
            }
        }
        let authorized = context.grant.approval_policy["ontology_consumer_scope"]["object_ids"]
            .as_array()
            .expect("validated IDs");
        if authorized.iter().any(|id| {
            Uuid::parse_str(id.as_str().unwrap())
                .ok()
                .is_none_or(|id| !receipt.object_versions.contains_key(&id))
        }) {
            return Err(AppError::forbidden(
                "an authorized Ontology object is unavailable or outside the application subset",
            ));
        }
        let relations = consumer_relations(
            state,
            &context.principal,
            &context.application,
            &context.catalog,
            Some(&context.grant),
            &Default::default(),
        )
        .await?;
        receipt.relation_versions = relations
            .iter()
            .map(|r| (r.id, r.version.clone()))
            .collect();
        let work = state
            .list_work_items()
            .await?
            .into_iter()
            .find(|w| w.id == context.binding.work_item_id && w.archived_at.is_none())
            .ok_or_else(|| AppError::forbidden("bound business WorkItem is unavailable"))?;
        receipt.work_item_version = normalized_json_sha256(&serde_json::to_value(&work)?);
        let catalog =
            resolved_catalog_for_subset(&context.catalog, &context.application.subset_manifest)?;
        state.insert_ontology_read_receipt(&receipt).await?;
        state.append_event("tool",Some(call.id),input.session_id,"ontology.context.read",json!({"receipt_id":receipt.id,"task_grant_id":receipt.task_grant_id,"context_packet_id":receipt.context_packet_id,"release_id":receipt.ontology_release_id,"object_versions":receipt.object_versions})).await?;
        Ok(
            json!({"read_receipt_id":receipt.id,"expires_at":receipt.expires_at,"task":{"objective":context.grant.objective,"work_item":work},"ontology_release_id":context.release.id,"release_version":context.release.version,"catalog_digest":receipt.catalog_digest,"objects":objects,"relations":relations,"catalog":catalog}),
        )
    }
}

pub(crate) async fn validate_ontology_read_receipt(
    state: &AppState,
    input: &ExecuteTool,
    kind: &str,
    argument: &str,
) -> Result<OntologyReadReceipt, AppError> {
    let context = ontology_runtime_context(state, input).await?;
    let id = crate::uuid_tool_arg(
        &input.args,
        &[argument],
        "Action requires server-issued Ontology read evidence",
    )?;
    let receipt = state.get_ontology_read_receipt(id).await?;
    validate_receipt_binding(&receipt, &context, state.current_tenant_id(), kind)?;
    let work = state
        .list_work_items()
        .await?
        .into_iter()
        .find(|w| w.id == receipt.work_item_id && w.archived_at.is_none())
        .ok_or_else(|| AppError::forbidden("bound WorkItem no longer exists"))?;
    if normalized_json_sha256(&serde_json::to_value(&work)?) != receipt.work_item_version {
        return Err(AppError::forbidden(
            "business WorkItem changed after Ontology read",
        ));
    }
    let calls = state.list_tool_calls(Some(input.session_id)).await?;
    if !calls.iter().any(|c| {
        c.id == receipt.source_tool_call_id
            && c.task_grant_id == Some(context.grant.id)
            && c.status == "completed"
            && matches!(
                c.policy_decision["origin"].as_str(),
                Some("runtime_adapter" | "session_loop")
            )
            && c.tool_name
                == if kind == "context" {
                    "ontology.context.read"
                } else {
                    "ontology.action.result.read"
                }
    }) {
        return Err(AppError::forbidden(
            "Ontology read did not complete in this session",
        ));
    }
    let visible = crate::visible_semantic_objects_for_principal(state, &context.principal).await?;
    if receipt
        .object_versions
        .keys()
        .any(|id| !visible.iter().any(|object| object.id == *id))
    {
        return Err(AppError::forbidden(
            "Ontology object visibility was revoked after read",
        ));
    }
    for (id, version) in &receipt.object_versions {
        let object = state.get_semantic_object(*id).await?;
        if object.archived_at.is_some()
            || object.status != "active"
            || ontology_object_version(&object) != *version
        {
            return Err(AppError::forbidden(
                "Ontology read evidence is stale; re-read current objects",
            ));
        }
    }
    let links = state.list_semantic_links().await?;
    for (id, version) in &receipt.relation_versions {
        let current = links
            .iter()
            .find(|link| link.id == *id && link.status == "active" && link.archived_at.is_none())
            .ok_or_else(|| AppError::forbidden("Ontology relation read evidence is stale"))?;
        if normalized_json_sha256(&serde_json::to_value(current)?) != *version {
            return Err(AppError::forbidden(
                "Ontology relation read evidence is stale",
            ));
        }
    }
    Ok(receipt)
}

pub(crate) fn validate_receipt_binding(
    receipt: &OntologyReadReceipt,
    context: &OntologyRuntimeContext,
    tenant_id: Uuid,
    kind: &str,
) -> Result<(), AppError> {
    if receipt.tenant_id != tenant_id
        || receipt.session_id != context.packet.session_id
        || receipt.task_grant_id != context.grant.id
        || receipt.application_id != context.application.id
        || receipt.context_packet_id != context.packet.id
        || receipt.context_packet_version != context.packet.version
        || receipt.ontology_release_id != context.release.id
        || receipt.release_version != context.release.version
        || receipt.catalog_digest != context.application.catalog_digest
        || receipt.grant_scope_digest != ontology_runtime_authority_digest(&context.grant)
        || receipt.subset_digest != context.application.subset_digest
        || receipt.work_item_id != context.binding.work_item_id
        || receipt.kind != kind
        || receipt.expires_at <= Utc::now()
    {
        return Err(AppError::forbidden(
            "Ontology read evidence is expired or belongs to a different session, grant, release or task",
        ));
    }
    Ok(())
}

/// A child cannot opt an inherited business task out of its governed runtime.
pub(crate) async fn validate_ontology_runtime_lineage(
    state: &AppState,
    grant: &TaskGrant,
) -> Result<(), AppError> {
    let binding = grant.approval_policy.get("ontology_runtime");
    let mut parent = grant.parent_grant_id;
    let mut seen = std::collections::HashSet::new();
    while let Some(id) = parent {
        if !seen.insert(id) {
            return Err(AppError::forbidden("business grant lineage cycle"));
        }
        let ancestor = state.get_task_grant(id).await?;
        if let Some(required) = ancestor.approval_policy.get("ontology_runtime") {
            if binding != Some(required) {
                return Err(AppError::forbidden(
                    "business runtime authority cannot be removed or replaced by a child grant",
                ));
            }
            for kind in ["objects", "relations", "actions", "object_ids"] {
                let child = grant.approval_policy["ontology_consumer_scope"][kind]
                    .as_array()
                    .ok_or_else(|| {
                        AppError::forbidden(
                            "business child grant must preserve explicit Ontology scopes",
                        )
                    })?;
                let allowed = ancestor.approval_policy["ontology_consumer_scope"][kind]
                    .as_array()
                    .ok_or_else(|| AppError::forbidden("business parent grant scope is invalid"))?;
                if child.iter().any(|v| !allowed.contains(v)) {
                    return Err(AppError::forbidden(
                        "business child grant expands Ontology authority",
                    ));
                }
            }
        }
        parent = ancestor.parent_grant_id;
    }
    Ok(())
}

pub(crate) fn enforce_ontology_business_tool_boundary(
    grant: Option<&TaskGrant>,
    tool: &str,
) -> Result<(), AppError> {
    if grant.is_some_and(|g| g.approval_policy.get("ontology_runtime").is_some())
        && !matches!(
            tool,
            "codex.exec"
                | "agent_cli.exec"
                | "ontology.context.read"
                | "ontology.action.execute"
                | "ontology.action.result.read"
        )
    {
        return Err(AppError::forbidden(
            "business Agent operations must use Ontology reads and registered Actions; generic tool bypass is forbidden",
        ));
    }
    Ok(())
}

pub(crate) fn internal_business_action_kind(
    spec: &crate::OntologyOnboardingToolSpec,
) -> Option<&str> {
    match spec.executor.get("type").and_then(Value::as_str) {
        Some("internal_followup_draft") => Some("draft"),
        Some("internal_work_item_closeout") => Some("closeout"),
        _ => None,
    }
}

pub(crate) async fn validate_business_action_read(
    state: &AppState,
    input: &ExecuteTool,
    spec: &crate::OntologyOnboardingToolSpec,
) -> Result<Option<OntologyReadReceipt>, AppError> {
    let grant_id = input
        .task_grant_id
        .ok_or_else(|| AppError::forbidden("Action requires TaskGrant"))?;
    let grant = state.get_task_grant(grant_id).await?;
    if ontology_runtime_binding(&grant)?.is_none() && internal_business_action_kind(spec).is_none()
    {
        return Ok(None);
    }
    let context = ontology_runtime_context(state, input).await?;
    let receipt =
        validate_ontology_read_receipt(state, input, "context", "ontology_read_receipt_id").await?;
    let action = context
        .catalog
        .actions
        .iter()
        .find(|a| a.runtime_name == spec.name)
        .ok_or_else(|| AppError::forbidden("Action is outside the runtime application catalog"))?;
    if normalized_json_sha256(&serde_json::to_value(spec)?) != action.contract_digest {
        return Err(AppError::forbidden(
            "Action contract differs from the authorized catalog",
        ));
    }
    crate::ontology_sdk_consumer_runtime::require_consumer_allowlist(
        Some(&context.grant),
        "actions",
        &action.api_name,
    )?;
    if !context
        .application
        .subset_manifest
        .actions
        .contains(&action.api_name)
    {
        return Err(AppError::forbidden(
            "Action is outside the runtime application subset",
        ));
    }
    let work = state
        .list_work_items()
        .await?
        .into_iter()
        .find(|w| w.id == context.binding.work_item_id && w.archived_at.is_none())
        .ok_or_else(|| AppError::forbidden("business WorkItem is unavailable"))?;
    if matches!(work.status.as_str(), "done" | "canceled" | "blocked") {
        return Err(AppError::forbidden(
            "business WorkItem is not accepting new Actions",
        ));
    }
    if internal_business_action_kind(spec).is_some() {
        let expected_target = if internal_business_action_kind(spec) == Some("draft") {
            "FollowupDraft"
        } else {
            "WorkItem"
        };
        if spec.target_object != expected_target {
            return Err(AppError::forbidden(
                "internal Action target does not match its registered executor",
            ));
        }
        if !spec.approval_required
            || spec.read_only
            || spec.transaction_profile
                != crate::OntologyActionTransactionProfile::LocalSerializable
        {
            return Err(AppError::forbidden(
                "internal business contracts must require approval and local serializable execution",
            ));
        }
        let parameters = input
            .args
            .get("parameters")
            .ok_or_else(|| AppError::bad_request("internal Action requires parameters"))?;
        if parameters["work_item_id"].as_str() != Some(&context.binding.work_item_id.to_string()) {
            return Err(AppError::forbidden(
                "Action WorkItem is outside this business task",
            ));
        }
        if internal_business_action_kind(spec) == Some("draft") {
            let id = crate::uuid_tool_arg(
                parameters,
                &["source_object_id"],
                "draft Action requires source_object_id",
            )?;
            if !receipt.object_versions.contains_key(&id) {
                return Err(AppError::forbidden(
                    "Action object was not read in this authorized context",
                ));
            }
            if !spec.approval_required
                || spec.read_only
                || spec.transaction_profile
                    != crate::OntologyActionTransactionProfile::LocalSerializable
            {
                return Err(AppError::forbidden(
                    "internal draft contract must require approval and local serializable execution",
                ));
            }
        }
    }
    Ok(Some(receipt))
}

pub(crate) async fn prepare_internal_business_draft(
    state: &AppState,
    input: &ExecuteTool,
    spec: &crate::OntologyOnboardingToolSpec,
    receipt: &OntologyReadReceipt,
) -> Result<SemanticObject, AppError> {
    let context = ontology_runtime_context(state, input).await?;
    let p = &input.args["parameters"];
    let source_id =
        crate::uuid_tool_arg(p, &["source_object_id"], "draft source object is required")?;
    let source = state.get_semantic_object(source_id).await?;
    let source_type = spec.executor["source_object_type"]
        .as_str()
        .ok_or_else(|| AppError::forbidden("draft executor must declare source_object_type"))?;
    if source.content["object_type"].as_str() != Some(source_type)
        || !receipt.object_versions.contains_key(&source_id)
    {
        return Err(AppError::forbidden(
            "draft source is not an authorized instance of the declared type",
        ));
    }
    let text = |key: &str| -> Result<String, AppError> {
        let value = p[key]
            .as_str()
            .map(str::trim)
            .filter(|v| !v.is_empty() && v.len() <= 16384)
            .ok_or_else(|| {
                AppError::bad_request(format!("draft {key} must contain 1–16384 bytes"))
            })?;
        Ok(value.into())
    };
    let title = text("draft_title")?;
    let body = text("draft_body")?;
    let output_type = context
        .catalog
        .objects
        .iter()
        .find(|o| o.object_type == spec.target_object)
        .ok_or_else(|| AppError::forbidden("draft output object type is not published"))?;
    if !context
        .application
        .subset_manifest
        .objects
        .contains(&output_type.api_name)
    {
        return Err(AppError::forbidden(
            "draft output object type is outside the runtime application",
        ));
    }
    let mut scopes = source.semantic_scopes.clone();
    let scopes_object = scopes
        .as_object_mut()
        .ok_or_else(|| AppError::forbidden("source semantic scope is invalid"))?;
    for (key, value) in context
        .grant
        .semantic_scopes
        .as_object()
        .ok_or_else(|| AppError::forbidden("grant semantic scope is invalid"))?
    {
        if scopes_object
            .get(key)
            .is_some_and(|existing| existing != value)
        {
            return Err(AppError::forbidden("draft source and task scopes conflict"));
        }
        scopes_object.insert(key.clone(), value.clone());
    }
    let id = crate::deterministic_record_id(
        receipt.work_item_id,
        "internal-followup-draft",
        &[&source_id.to_string()],
    );
    let now = Utc::now().trunc_subsecs(6);
    Ok(SemanticObject {
        id,
        source_id: None,
        object_type: "business_object".into(),
        object_key: format!("followup_draft:{}:{source_id}", receipt.work_item_id),
        title: title.clone(),
        summary: format!("Internal follow-up draft for {source_id}"),
        content: json!({"domain_scope":context.release.domain_scope,"object_type":spec.target_object,"properties":{"source_object_id":source_id,"work_item_id":receipt.work_item_id,"title":title,"body":body,"status":"draft"},"source_version":receipt.object_versions[&source_id]}),
        semantic_scopes: scopes,
        source_uri: Some(format!("mandoforge://business-actions/{id}")),
        provenance: json!({"source":"ontology_action.internal_followup_draft","session_id":input.session_id,"task_grant_id":context.grant.id,"ontology_release_id":context.release.id,"action":spec.name}),
        trust_level: "source_attested".into(),
        freshness: "current".into(),
        status: "active".into(),
        created_at: now,
        updated_at: now,
        archived_at: None,
    })
}

pub(crate) struct OntologyActionResultReadTool;
#[async_trait]
impl ToolExecutor for OntologyActionResultReadTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "ontology.action.result.read",
            risk: "low",
            description: "Read the actual business result of this task's registered Action and issue independent readback evidence",
        }
    }
    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        call: &ToolCall,
    ) -> Result<Value, AppError> {
        let context = ontology_runtime_context(state, input).await?;
        let id = crate::uuid_tool_arg(
            &input.args,
            &["action_tool_call_id"],
            "Action result requires action_tool_call_id",
        )?;
        let action = state.get_tool_call(id).await?;
        if action.session_id != input.session_id
            || action.task_grant_id != Some(context.grant.id)
            || action.tool_name != "ontology.action.execute"
        {
            return Err(AppError::forbidden(
                "Action result belongs to a different business task",
            ));
        }
        if action.status != "completed" {
            return Ok(
                json!({"status":action.status,"action_tool_call_id":action.id,"result":action.result,"error":action.error}),
            );
        }
        let result = action
            .result
            .as_ref()
            .ok_or_else(|| AppError::forbidden("Action has no committed result"))?;
        let work = state
            .list_work_items()
            .await?
            .into_iter()
            .find(|w| w.id == context.binding.work_item_id && w.archived_at.is_none())
            .ok_or_else(|| AppError::forbidden("bound WorkItem is missing"))?;
        let mut receipt = new_receipt(state, &context, call, "action_result");
        receipt.action_tool_call_id = Some(action.id);
        receipt.work_item_version = normalized_json_sha256(&serde_json::to_value(&work)?);
        let business_object = match result["status"].as_str() {
            Some("business_draft_created") => {
                let object_id = crate::uuid_tool_arg(
                    result,
                    &["business_object_id"],
                    "Action business object is missing",
                )?;
                let object = state.get_semantic_object(object_id).await?;
                if object.provenance["task_grant_id"].as_str()
                    != Some(&context.grant.id.to_string())
                    || object.content["properties"]["work_item_id"].as_str()
                        != Some(&work.id.to_string())
                {
                    return Err(AppError::forbidden(
                        "Action result object authority mismatch",
                    ));
                }
                let kind = context
                    .catalog
                    .objects
                    .iter()
                    .find(|o| {
                        Some(o.object_type.as_str()) == object.content["object_type"].as_str()
                    })
                    .ok_or_else(|| {
                        AppError::forbidden("Action result object type is not published")
                    })?;
                // Authority to read this additional object comes only from this task's committed Action.
                // The application property projection and principal visibility still apply.
                let projected = crate::consumer_object_by_id(
                    state,
                    &context.principal,
                    &context.application,
                    &context.catalog,
                    &kind.api_name,
                    object_id,
                    None,
                )
                .await?;
                receipt
                    .object_versions
                    .insert(projected.id, projected.version.clone());
                serde_json::to_value(projected)?
            }
            Some("work_item_completed") => {
                if work.status != "done"
                    || work.metadata["ontology_closeout"]["task_grant_id"].as_str()
                        != Some(&context.grant.id.to_string())
                {
                    return Err(AppError::forbidden(
                        "business closeout state is not confirmed",
                    ));
                }
                serde_json::to_value(&work)?
            }
            _ => {
                return Err(AppError::forbidden(
                    "Action has no executed internal business result",
                ));
            }
        };
        state.insert_ontology_read_receipt(&receipt).await?;
        state.append_event("tool",Some(call.id),input.session_id,"ontology.action.readback",json!({"receipt_id":receipt.id,"action_tool_call_id":action.id,"object_versions":receipt.object_versions,"work_item_version":receipt.work_item_version})).await?;
        Ok(
            json!({"status":"readback_verified","result_read_receipt_id":receipt.id,"action_tool_call_id":action.id,"execution_receipt":result,"business_object":business_object,"work_item_status":work.status}),
        )
    }
}

pub(crate) async fn validate_business_closeout_inputs(
    state: &AppState,
    input: &ExecuteTool,
    receipts: &[OntologyReadReceipt],
) -> Result<(), AppError> {
    let context = ontology_runtime_context(state, input).await?;
    let mut covered = std::collections::BTreeSet::new();
    for receipt in receipts {
        let action_id = receipt
            .action_tool_call_id
            .ok_or_else(|| AppError::forbidden("closeout evidence is not an Action readback"))?;
        let action = state.get_tool_call(action_id).await?;
        let result = action
            .result
            .as_ref()
            .ok_or_else(|| AppError::forbidden("closeout Action has no result"))?;
        if action.status != "completed"
            || action.session_id != input.session_id
            || action.task_grant_id != input.task_grant_id
            || result["status"] != "business_draft_created"
        {
            return Err(AppError::forbidden(
                "closeout requires this task's successful internal draft Action",
            ));
        }
        let approval_id = crate::uuid_tool_arg(
            result,
            &["approval_id"],
            "closeout requires approved Action evidence",
        )?;
        let approval = state.get_approval(approval_id).await?;
        if approval.status != "approved" || approval.tool_call_id != Some(action.id) {
            return Err(AppError::forbidden("closeout Action was not approved"));
        }
        let object_id = crate::uuid_tool_arg(
            result,
            &["business_object_id"],
            "closeout result object missing",
        )?;
        if !receipt.object_versions.contains_key(&object_id) {
            return Err(AppError::forbidden(
                "closeout draft was not independently read",
            ));
        }
        let object = state.get_semantic_object(object_id).await?;
        if object.content["properties"]["work_item_id"].as_str()
            != Some(&context.binding.work_item_id.to_string())
            || object.content["properties"]["status"] != "draft"
        {
            return Err(AppError::forbidden(
                "closeout draft does not satisfy the bound business objective",
            ));
        }
        let source = crate::uuid_tool_arg(
            &object.content["properties"],
            &["source_object_id"],
            "closeout draft source missing",
        )?;
        if !context.binding.required_source_object_ids.contains(&source) {
            return Err(AppError::forbidden(
                "closeout source is outside the required task scope",
            ));
        }
        covered.insert(source);
    }
    if covered
        != context
            .binding
            .required_source_object_ids
            .iter()
            .copied()
            .collect()
    {
        return Err(AppError::forbidden(
            "closeout is missing approved, read-back results for required business objects",
        ));
    }
    Ok(())
}
