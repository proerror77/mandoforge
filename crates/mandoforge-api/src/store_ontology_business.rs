//! Transaction helpers for registered, internal-only business effects.
//! Called inside the existing approval/claim transaction, never from a raw data-write route.
use chrono::{SubsecRound, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::store_backend::MemoryStore;
use crate::store_rows::{semantic_link_from_row, semantic_object_from_row, task_grant_from_row};
use crate::store_workflows::{TASK_GRANT_COLUMNS, task_grant_runtime_denial};
use crate::{
    AppError, Artifact, OntologyReadReceipt, SemanticObject, TaskGrant, ToolCall,
    normalized_json_sha256, ontology_object_version, ontology_runtime_authority_digest,
    ontology_runtime_binding,
};

fn receipt_authority(
    receipt: &OntologyReadReceipt,
    grant: &TaskGrant,
    call: &ToolCall,
    tenant: Uuid,
) -> Result<(), AppError> {
    let binding = ontology_runtime_binding(grant)?
        .ok_or_else(|| AppError::forbidden("internal business effect requires runtime binding"))?;
    if receipt.tenant_id != tenant
        || receipt.session_id != call.session_id
        || call.task_grant_id != Some(grant.id)
        || receipt.task_grant_id != grant.id
        || receipt.application_id != binding.application_id
        || receipt.work_item_id != binding.work_item_id
        || receipt.context_packet_id != grant.context_packet_id.unwrap_or_default()
        || receipt.grant_scope_digest != ontology_runtime_authority_digest(grant)
        || receipt.expires_at <= Utc::now()
        || receipt.created_at > call.created_at
    {
        return Err(AppError::forbidden(
            "Ontology evidence authority changed before commit",
        ));
    }
    if let Some((reason, _)) = task_grant_runtime_denial(grant, Utc::now()) {
        return Err(AppError::forbidden(reason));
    }
    Ok(())
}

pub(crate) fn validate_receipt_memory(
    store: &MemoryStore,
    tenant: Uuid,
    receipt: &OntologyReadReceipt,
    call: &ToolCall,
) -> Result<(), AppError> {
    let persisted = store
        .ontology_read_receipts
        .get(&receipt.id)
        .filter(|r| r.tenant_id == tenant)
        .ok_or_else(|| AppError::forbidden("Ontology read receipt is not server-owned"))?;
    if serde_json::to_value(persisted)? != serde_json::to_value(receipt)? {
        return Err(AppError::forbidden("Ontology read receipt was altered"));
    }
    let grant = store
        .task_grants
        .get(&receipt.task_grant_id)
        .ok_or_else(|| AppError::forbidden("TaskGrant missing at commit"))?;
    receipt_authority(receipt, grant, call, tenant)?;
    let source = store
        .tool_calls
        .get(&receipt.source_tool_call_id)
        .ok_or_else(|| AppError::forbidden("Ontology read call missing"))?;
    if source.status != "completed"
        || !matches!(
            source.policy_decision["origin"].as_str(),
            Some("runtime_adapter" | "session_loop")
        )
        || source.session_id != call.session_id
        || source.task_grant_id != call.task_grant_id
    {
        return Err(AppError::forbidden(
            "Ontology read was not completed by this task",
        ));
    }
    if !store
        .context_packets
        .get(&receipt.context_packet_id)
        .is_some_and(|p| {
            p.version == receipt.context_packet_version && p.session_id == call.session_id
        })
    {
        return Err(AppError::forbidden("Ontology context version changed"));
    }
    let work = store
        .work_items
        .get(&receipt.work_item_id)
        .filter(|w| w.archived_at.is_none())
        .ok_or_else(|| AppError::forbidden("read WorkItem missing at commit"))?;
    if normalized_json_sha256(&serde_json::to_value(work)?) != receipt.work_item_version {
        return Err(AppError::forbidden("WorkItem changed after Ontology read"));
    }
    for (id, version) in &receipt.object_versions {
        let object = store
            .semantic_objects
            .get(id)
            .ok_or_else(|| AppError::forbidden("read object missing at commit"))?;
        if object.status != "active"
            || object.archived_at.is_some()
            || ontology_object_version(object) != *version
        {
            return Err(AppError::forbidden("Ontology object changed after read"));
        }
    }
    for (id, version) in &receipt.relation_versions {
        let link = store
            .semantic_links
            .get(id)
            .ok_or_else(|| AppError::forbidden("read relation missing at commit"))?;
        if link.status != "active"
            || link.archived_at.is_some()
            || normalized_json_sha256(&serde_json::to_value(link)?) != *version
        {
            return Err(AppError::forbidden("Ontology relation changed after read"));
        }
    }
    Ok(())
}

pub(crate) async fn validate_receipt_postgres(
    connection: &mut sqlx::PgConnection,
    tenant: Uuid,
    receipt: &OntologyReadReceipt,
    call: &ToolCall,
) -> Result<(), AppError> {
    let persisted = sqlx::query_scalar::<_, Value>(
        "SELECT payload FROM ontology_runtime_read_receipts WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant)
    .bind(receipt.id)
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| AppError::forbidden("Ontology read receipt is not server-owned"))?;
    if persisted != serde_json::to_value(receipt)? {
        return Err(AppError::forbidden("Ontology read receipt was altered"));
    }
    let row = sqlx::query(&format!(
        "SELECT {TASK_GRANT_COLUMNS} FROM task_grants WHERE tenant_id=$1 AND id=$2 FOR UPDATE"
    ))
    .bind(tenant)
    .bind(receipt.task_grant_id)
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| AppError::forbidden("TaskGrant missing at commit"))?;
    let grant = task_grant_from_row(row)?;
    receipt_authority(receipt, &grant, call, tenant)?;
    let valid=sqlx::query_scalar::<_,i32>("SELECT 1 FROM tool_calls WHERE tenant_id=$1 AND id=$2 AND session_id=$3 AND task_grant_id=$4 AND status='completed' AND policy_decision->>'origin' IN ('runtime_adapter','session_loop')")
        .bind(tenant).bind(receipt.source_tool_call_id).bind(call.session_id).bind(receipt.task_grant_id)
        .fetch_optional(&mut *connection).await?.is_some();
    if !valid {
        return Err(AppError::forbidden(
            "Ontology read was not completed by this task",
        ));
    }
    let context=sqlx::query_scalar::<_,i32>("SELECT 1 FROM context_packets WHERE tenant_id=$1 AND id=$2 AND session_id=$3 AND version=$4")
        .bind(tenant).bind(receipt.context_packet_id).bind(call.session_id).bind(receipt.context_packet_version)
        .fetch_optional(&mut *connection).await?.is_some();
    if !context {
        return Err(AppError::forbidden("Ontology context version changed"));
    }
    let row = sqlx::query("SELECT * FROM work_items WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
        .bind(tenant)
        .bind(receipt.work_item_id)
        .fetch_optional(&mut *connection)
        .await?
        .ok_or_else(|| AppError::forbidden("read WorkItem missing at commit"))?;
    let work = crate::store_rows::work_item_from_row(row)?;
    if work.archived_at.is_some()
        || normalized_json_sha256(&serde_json::to_value(&work)?) != receipt.work_item_version
    {
        return Err(AppError::forbidden("WorkItem changed after Ontology read"));
    }
    for (id, version) in &receipt.object_versions {
        let row =
            sqlx::query("SELECT * FROM semantic_objects WHERE tenant_id=$1 AND id=$2 FOR SHARE")
                .bind(tenant)
                .bind(id)
                .fetch_optional(&mut *connection)
                .await?
                .ok_or_else(|| AppError::forbidden("read object missing at commit"))?;
        let object = semantic_object_from_row(row)?;
        if object.status != "active"
            || object.archived_at.is_some()
            || ontology_object_version(&object) != *version
        {
            return Err(AppError::forbidden("Ontology object changed after read"));
        }
    }
    for (id, version) in &receipt.relation_versions {
        let row =
            sqlx::query("SELECT * FROM semantic_links WHERE tenant_id=$1 AND id=$2 FOR SHARE")
                .bind(tenant)
                .bind(id)
                .fetch_optional(&mut *connection)
                .await?
                .ok_or_else(|| AppError::forbidden("read relation missing at commit"))?;
        let link = semantic_link_from_row(row)?;
        if link.status != "active"
            || link.archived_at.is_some()
            || normalized_json_sha256(&serde_json::to_value(link)?) != *version
        {
            return Err(AppError::forbidden("Ontology relation changed after read"));
        }
    }
    Ok(())
}

pub(crate) fn receipt_from_artifact(
    artifact: &Artifact,
) -> Result<Option<OntologyReadReceipt>, AppError> {
    artifact
        .content
        .get("ontology_read_receipt")
        .filter(|r| !r.is_null())
        .cloned()
        .map(|r| serde_json::from_value(r).map_err(AppError::from))
        .transpose()
}

/// None means the pre-existing proposal-only executor: it still has no business effect.
fn draft_candidate(artifact: &Artifact) -> Result<Option<SemanticObject>, AppError> {
    artifact
        .content
        .get("internal_business_object")
        .filter(|v| !v.is_null())
        .cloned()
        .map(|value| serde_json::from_value(value).map_err(AppError::from))
        .transpose()
}

fn existing_draft(candidate: &SemanticObject, existing: &SemanticObject) -> Result<(), AppError> {
    if existing.status != "active"
        || existing.archived_at.is_some()
        || existing.object_type != candidate.object_type
        || existing.object_key != candidate.object_key
        || existing.content != candidate.content
        || existing.semantic_scopes != candidate.semantic_scopes
        || existing.provenance["task_grant_id"] != candidate.provenance["task_grant_id"]
    {
        return Err(AppError::conflict(
            "business draft identity already exists with a different effect or authority",
        ));
    }
    Ok(())
}

pub(crate) fn prepare_business_effect_memory(
    store: &MemoryStore,
    tenant: Uuid,
    artifact: &mut Artifact,
    call: &ToolCall,
    result: &mut Value,
) -> Result<Option<MemoryBusinessEffect>, AppError> {
    let Some(receipt) = receipt_from_artifact(artifact)? else {
        return Ok(None);
    };
    validate_receipt_memory(store, tenant, &receipt, call)?;
    let Some(candidate) = draft_candidate(artifact)? else {
        return prepare_closeout_memory(store, tenant, artifact, call, result, &receipt);
    };
    let work = store
        .work_items
        .get(&receipt.work_item_id)
        .ok_or_else(|| AppError::forbidden("business WorkItem is missing"))?;
    if work.archived_at.is_some() || matches!(work.status.as_str(), "canceled" | "blocked") {
        return Err(AppError::forbidden(
            "business WorkItem cannot accept this effect",
        ));
    }
    let existing = store.semantic_objects.get(&candidate.id);
    if let Some(existing) = existing {
        existing_draft(&candidate, existing)?;
    } else if work.status == "done" {
        return Err(AppError::forbidden(
            "completed WorkItem cannot create a new business effect",
        ));
    }
    let object = existing.unwrap_or(&candidate);
    result["business_object_id"] = json!(object.id);
    result["business_object_version"] = json!(ontology_object_version(object));
    result["effect_reused"] = json!(existing.is_some());
    artifact.content["internal_business_object"] = serde_json::to_value(object)?;
    Ok(existing
        .is_none()
        .then_some(MemoryBusinessEffect::Draft(candidate)))
}

pub(crate) async fn apply_business_effect_postgres(
    connection: &mut sqlx::PgConnection,
    tenant: Uuid,
    artifact: &mut Artifact,
    call: &ToolCall,
    result: &mut Value,
) -> Result<(), AppError> {
    let Some(receipt) = receipt_from_artifact(artifact)? else {
        return Ok(());
    };
    validate_receipt_postgres(connection, tenant, &receipt, call).await?;
    let Some(candidate) = draft_candidate(artifact)? else {
        return apply_closeout_postgres(connection, tenant, artifact, call, result, &receipt).await;
    };
    let work = sqlx::query_as::<_, (String, Option<chrono::DateTime<Utc>>)>(
        "SELECT status,archived_at FROM work_items WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
    )
    .bind(tenant)
    .bind(receipt.work_item_id)
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| AppError::forbidden("business WorkItem is missing"))?;
    if work.1.is_some() || matches!(work.0.as_str(), "canceled" | "blocked") {
        return Err(AppError::forbidden(
            "business WorkItem cannot accept this effect",
        ));
    }
    let existing =
        sqlx::query("SELECT * FROM semantic_objects WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
            .bind(tenant)
            .bind(candidate.id)
            .fetch_optional(&mut *connection)
            .await?
            .map(semantic_object_from_row)
            .transpose()?;
    let reused = existing.is_some();
    let object = if let Some(existing) = existing {
        existing_draft(&candidate, &existing)?;
        existing
    } else {
        if work.0 == "done" {
            return Err(AppError::forbidden(
                "completed WorkItem cannot create a new business effect",
            ));
        }
        insert_business_object_postgres(connection, tenant, &candidate).await?;
        candidate
    };
    result["business_object_id"] = json!(object.id);
    result["business_object_version"] = json!(ontology_object_version(&object));
    result["effect_reused"] = json!(reused);
    artifact.content["internal_business_object"] = serde_json::to_value(object)?;
    Ok(())
}

pub(crate) async fn insert_business_object_postgres(
    connection: &mut sqlx::PgConnection,
    tenant: Uuid,
    object: &SemanticObject,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO semantic_objects (id,tenant_id,source_id,object_type,object_key,title,summary,content,semantic_scopes,source_uri,provenance,trust_level,freshness,status,created_at,updated_at,archived_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,NULL)")
        .bind(object.id).bind(tenant).bind(object.source_id).bind(&object.object_type).bind(&object.object_key).bind(&object.title).bind(&object.summary).bind(&object.content).bind(&object.semantic_scopes).bind(&object.source_uri).bind(&object.provenance).bind(&object.trust_level).bind(&object.freshness).bind(&object.status).bind(object.created_at).bind(object.updated_at).execute(connection).await?;
    Ok(())
}

pub(crate) enum MemoryBusinessEffect {
    Draft(SemanticObject),
    Closeout {
        work: crate::WorkItem,
        projection: Box<Option<SemanticObject>>,
        activity: crate::WorkItemActivityEntry,
    },
}

pub(crate) fn commit_memory_business_effect(store: &mut MemoryStore, effect: MemoryBusinessEffect) {
    match effect {
        MemoryBusinessEffect::Draft(object) => {
            store.semantic_objects.insert(object.id, object);
        }
        MemoryBusinessEffect::Closeout {
            work,
            projection,
            activity,
        } => {
            store.work_items.insert(work.id, work);
            if let Some(object) = *projection {
                store.semantic_objects.insert(object.id, object);
            }
            store
                .work_item_activity_entries
                .insert(activity.id, activity);
        }
    }
}

fn closeout_receipts(artifact: &Artifact) -> Result<Vec<OntologyReadReceipt>, AppError> {
    Ok(serde_json::from_value(
        artifact
            .content
            .get("closeout_read_receipts")
            .cloned()
            .unwrap_or_else(|| json!([])),
    )?)
}

fn validate_closeout_effect(
    grant: &TaskGrant,
    context: &OntologyReadReceipt,
    read: &OntologyReadReceipt,
    action: &ToolCall,
    object: &SemanticObject,
    approval: &crate::Approval,
) -> Result<Uuid, AppError> {
    let result = action
        .result
        .as_ref()
        .ok_or_else(|| AppError::forbidden("closeout Action has no committed result"))?;
    if action.status != "completed"
        || action.tool_name != "ontology.action.execute"
        || action.session_id != context.session_id
        || action.task_grant_id != Some(grant.id)
        || read.action_tool_call_id != Some(action.id)
        || result["status"] != "business_draft_created"
        || result["business_object_id"].as_str() != Some(&object.id.to_string())
        || !read.object_versions.contains_key(&object.id)
        || approval.status != "approved"
        || approval.tool_call_id != Some(action.id)
        || result["approval_id"].as_str() != Some(&approval.id.to_string())
        || object.content["properties"]["status"] != "draft"
        || object.content["properties"]["work_item_id"].as_str()
            != Some(&context.work_item_id.to_string())
        || object.provenance["task_grant_id"].as_str() != Some(&grant.id.to_string())
    {
        return Err(AppError::forbidden(
            "business closeout evidence is incomplete or belongs to another task",
        ));
    }
    let source = crate::uuid_tool_arg(
        &object.content["properties"],
        &["source_object_id"],
        "draft source missing",
    )?;
    let binding = ontology_runtime_binding(grant)?
        .ok_or_else(|| AppError::forbidden("closeout authority missing"))?;
    if !binding.required_source_object_ids.contains(&source) {
        return Err(AppError::forbidden(
            "closeout result is outside the required business scope",
        ));
    }
    Ok(source)
}

fn close_work_item(
    mut work: crate::WorkItem,
    grant: &TaskGrant,
    call: &ToolCall,
    context: &OntologyReadReceipt,
    reads: &[OntologyReadReceipt],
    covered: &std::collections::BTreeSet<Uuid>,
) -> Result<(crate::WorkItem, crate::WorkItemActivityEntry), AppError> {
    let binding = ontology_runtime_binding(grant)?
        .ok_or_else(|| AppError::forbidden("closeout authority missing"))?;
    if *covered != binding.required_source_object_ids.iter().copied().collect() {
        return Err(AppError::forbidden(
            "not all required business results have approved readback",
        ));
    }
    if work.id != binding.work_item_id
        || work.archived_at.is_some()
        || !matches!(work.status.as_str(), "open" | "in_progress" | "review")
    {
        return Err(AppError::forbidden(
            "WorkItem is not eligible for governed closeout",
        ));
    }
    let details = json!({"task_grant_id":grant.id,"context_read_receipt_id":context.id,"result_read_receipt_ids":reads.iter().map(|r|r.id).collect::<Vec<_>>(),"draft_action_ids":reads.iter().filter_map(|r|r.action_tool_call_id).collect::<Vec<_>>(),"closing_action_tool_call_id":call.id,"required_source_object_ids":binding.required_source_object_ids});
    work.metadata
        .as_object_mut()
        .ok_or_else(|| AppError::forbidden("WorkItem metadata must be an object"))?
        .insert("ontology_closeout".into(), details.clone());
    work.status = "done".into();
    work.updated_at = Utc::now().trunc_subsecs(6);
    let activity = crate::WorkItemActivityEntry {
        id: crate::deterministic_record_id(call.id, "work-item-closeout", &[&work.id.to_string()]),
        work_item_id: work.id,
        event_type: "work_item.completed".into(),
        actor_subject: Some(crate::ontology_runtime_subject(
            grant.grantee_agent_id.unwrap_or_default(),
        )),
        subject_type: Some("tool_call".into()),
        subject_id: Some(call.id),
        summary:
            "Completed by registered Action after approved business effects and Agent readback"
                .into(),
        metadata: details,
        created_at: work.updated_at,
    };
    Ok((work, activity))
}

fn update_work_item_projection(
    mut object: SemanticObject,
    work: &crate::WorkItem,
) -> Result<SemanticObject, AppError> {
    if object.source_uri.as_deref() != Some(&format!("mandoforge://work-items/{}", work.id))
        || object.content["work_item_id"].as_str() != Some(&work.id.to_string())
        || !object.content.is_object()
    {
        return Err(AppError::forbidden(
            "WorkItem semantic projection identity mismatch",
        ));
    }
    object.content["status"] = json!(work.status);
    object.content["metadata"] = work.metadata.clone();
    object.updated_at = work.updated_at;
    object.provenance["closeout_action_id"] =
        work.metadata["ontology_closeout"]["closing_action_tool_call_id"].clone();
    Ok(object)
}

fn prepare_closeout_memory(
    store: &MemoryStore,
    tenant: Uuid,
    artifact: &Artifact,
    call: &ToolCall,
    result: &mut Value,
    context: &OntologyReadReceipt,
) -> Result<Option<MemoryBusinessEffect>, AppError> {
    let reads = closeout_receipts(artifact)?;
    if reads.is_empty() {
        return Ok(None);
    }
    let grant = store
        .task_grants
        .get(&context.task_grant_id)
        .ok_or_else(|| AppError::forbidden("closeout TaskGrant missing"))?;
    let mut covered = std::collections::BTreeSet::new();
    for read in &reads {
        validate_receipt_memory(store, tenant, read, call)?;
        let id = read
            .action_tool_call_id
            .ok_or_else(|| AppError::forbidden("closeout requires Action readback"))?;
        let action = store
            .tool_calls
            .get(&id)
            .ok_or_else(|| AppError::forbidden("closeout source Action missing"))?;
        let data = action
            .result
            .as_ref()
            .ok_or_else(|| AppError::forbidden("closeout source Action failed"))?;
        let object_id =
            crate::uuid_tool_arg(data, &["business_object_id"], "draft object missing")?;
        let object = store
            .semantic_objects
            .get(&object_id)
            .ok_or_else(|| AppError::forbidden("draft object missing"))?;
        let approval_id = crate::uuid_tool_arg(data, &["approval_id"], "draft approval missing")?;
        let approval = store
            .approvals
            .get(&approval_id)
            .ok_or_else(|| AppError::forbidden("draft approval missing"))?;
        covered.insert(validate_closeout_effect(
            grant, context, read, action, object, approval,
        )?);
        let source_receipt_id = crate::uuid_tool_arg(
            data,
            &["ontology_read_receipt_id"],
            "draft context evidence missing",
        )?;
        let source_receipt = store
            .ontology_read_receipts
            .get(&source_receipt_id)
            .ok_or_else(|| AppError::forbidden("draft source read missing"))?;
        validate_receipt_memory(store, tenant, source_receipt, action)?;
    }
    let work = store
        .work_items
        .get(&context.work_item_id)
        .cloned()
        .ok_or_else(|| AppError::forbidden("closeout WorkItem missing"))?;
    let (work, activity) = close_work_item(work, grant, call, context, &reads, &covered)?;
    if store.work_item_activity_entries.contains_key(&activity.id) {
        return Err(AppError::conflict("closeout activity already exists"));
    }
    let projection = store
        .semantic_objects
        .values()
        .find(|o| {
            o.object_type == "work_item"
                && o.object_key == format!("work_item:{}", work.id)
                && o.archived_at.is_none()
        })
        .cloned()
        .map(|o| update_work_item_projection(o, &work))
        .transpose()?;
    result["work_item_id"] = json!(work.id);
    result["work_item_status"] = json!(work.status);
    result["work_item_version"] = json!(normalized_json_sha256(&serde_json::to_value(&work)?));
    Ok(Some(MemoryBusinessEffect::Closeout {
        work,
        projection: Box::new(projection),
        activity,
    }))
}

async fn apply_closeout_postgres(
    connection: &mut sqlx::PgConnection,
    tenant: Uuid,
    artifact: &Artifact,
    call: &ToolCall,
    result: &mut Value,
    context: &OntologyReadReceipt,
) -> Result<(), AppError> {
    let reads = closeout_receipts(artifact)?;
    if reads.is_empty() {
        return Ok(());
    }
    let row = sqlx::query(&format!(
        "SELECT {TASK_GRANT_COLUMNS} FROM task_grants WHERE tenant_id=$1 AND id=$2 FOR UPDATE"
    ))
    .bind(tenant)
    .bind(context.task_grant_id)
    .fetch_one(&mut *connection)
    .await?;
    let grant = task_grant_from_row(row)?;
    let mut covered = std::collections::BTreeSet::new();
    for read in &reads {
        validate_receipt_postgres(connection, tenant, read, call).await?;
        let id = read
            .action_tool_call_id
            .ok_or_else(|| AppError::forbidden("closeout requires Action readback"))?;
        let row = sqlx::query("SELECT * FROM tool_calls WHERE tenant_id=$1 AND id=$2 FOR SHARE")
            .bind(tenant)
            .bind(id)
            .fetch_optional(&mut *connection)
            .await?
            .ok_or_else(|| AppError::forbidden("closeout source Action missing"))?;
        let action = crate::store_rows::tool_call_from_row(row)?;
        let data = action
            .result
            .as_ref()
            .ok_or_else(|| AppError::forbidden("closeout source Action failed"))?;
        let object_id =
            crate::uuid_tool_arg(data, &["business_object_id"], "draft object missing")?;
        let row =
            sqlx::query("SELECT * FROM semantic_objects WHERE tenant_id=$1 AND id=$2 FOR SHARE")
                .bind(tenant)
                .bind(object_id)
                .fetch_optional(&mut *connection)
                .await?
                .ok_or_else(|| AppError::forbidden("draft object missing"))?;
        let object = semantic_object_from_row(row)?;
        let approval_id = crate::uuid_tool_arg(data, &["approval_id"], "draft approval missing")?;
        let row = sqlx::query("SELECT * FROM approvals WHERE tenant_id=$1 AND id=$2 FOR SHARE")
            .bind(tenant)
            .bind(approval_id)
            .fetch_optional(&mut *connection)
            .await?
            .ok_or_else(|| AppError::forbidden("draft approval missing"))?;
        let approval = crate::store_rows::approval_from_row(row)?;
        covered.insert(validate_closeout_effect(
            &grant, context, read, &action, &object, &approval,
        )?);
        let source_receipt_id = crate::uuid_tool_arg(
            data,
            &["ontology_read_receipt_id"],
            "draft source read missing",
        )?;
        let payload = sqlx::query_scalar::<_, Value>(
            "SELECT payload FROM ontology_runtime_read_receipts WHERE tenant_id=$1 AND id=$2",
        )
        .bind(tenant)
        .bind(source_receipt_id)
        .fetch_optional(&mut *connection)
        .await?
        .ok_or_else(|| AppError::forbidden("draft source read missing"))?;
        validate_receipt_postgres(
            connection,
            tenant,
            &serde_json::from_value(payload)?,
            &action,
        )
        .await?;
    }
    let row = sqlx::query("SELECT * FROM work_items WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
        .bind(tenant)
        .bind(context.work_item_id)
        .fetch_one(&mut *connection)
        .await?;
    let (work, activity) = close_work_item(
        crate::store_rows::work_item_from_row(row)?,
        &grant,
        call,
        context,
        &reads,
        &covered,
    )?;
    sqlx::query(
        "UPDATE work_items SET status=$1,metadata=$2,updated_at=$3 WHERE tenant_id=$4 AND id=$5",
    )
    .bind(&work.status)
    .bind(&work.metadata)
    .bind(work.updated_at)
    .bind(tenant)
    .bind(work.id)
    .execute(&mut *connection)
    .await?;
    let projection=sqlx::query("SELECT * FROM semantic_objects WHERE tenant_id=$1 AND object_type='work_item' AND object_key=$2 AND archived_at IS NULL FOR UPDATE")
        .bind(tenant).bind(format!("work_item:{}",work.id)).fetch_optional(&mut *connection).await?.map(semantic_object_from_row).transpose()?;
    if let Some(object) = projection {
        let object = update_work_item_projection(object, &work)?;
        sqlx::query("UPDATE semantic_objects SET content=$1,provenance=$2,updated_at=$3 WHERE tenant_id=$4 AND id=$5").bind(&object.content).bind(&object.provenance).bind(object.updated_at).bind(tenant).bind(object.id).execute(&mut *connection).await?;
    }
    sqlx::query("INSERT INTO work_item_activity_entries (id,tenant_id,work_item_id,event_type,actor_subject,subject_type,subject_id,summary,metadata,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
        .bind(activity.id).bind(tenant).bind(activity.work_item_id).bind(&activity.event_type).bind(&activity.actor_subject).bind(&activity.subject_type).bind(activity.subject_id).bind(&activity.summary).bind(&activity.metadata).bind(activity.created_at).execute(&mut *connection).await?;
    result["work_item_id"] = json!(work.id);
    result["work_item_status"] = json!(work.status);
    result["work_item_version"] = json!(normalized_json_sha256(&serde_json::to_value(&work)?));
    Ok(())
}
