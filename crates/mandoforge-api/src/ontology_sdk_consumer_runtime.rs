use std::collections::{BTreeMap, BTreeSet};

use axum::http::HeaderMap;
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::{
    AppError, AppState, ExecuteTool, OntologyRelease, OntologyReleaseCatalogV1,
    OntologySdkApplication, OntologySdkCatalogObject, Permission, Principal, SemanticObject,
    TaskGrant, ToolInvocationOrigin, authorize_collection_request, authorize_principal_request,
    execute_tool_invocation, new_audit_log, normalize_and_validate_subset,
    ontology_action_tool_spec_for_release, ontology_release_current_status,
    release_catalog_from_evidence, task_grant_allows_tool, task_grant_session_matches,
    validate_ontology_action_parameters, visible_semantic_links_for_principal,
    visible_semantic_objects_for_principal,
};

#[derive(Debug, Deserialize, Default)]
pub(crate) struct OntologySdkConsumerReadQuery {
    pub(crate) task_grant_id: Option<Uuid>,
    pub(crate) object_id: Option<Uuid>,
    pub(crate) relation_api_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OntologySdkConsumerActionRequest {
    pub(crate) session_id: Uuid,
    pub(crate) task_grant_id: Uuid,
    pub(crate) context_packet_id: Uuid,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) parameters: Value,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct OntologySdkConsumerObject {
    pub(crate) id: Uuid,
    pub(crate) api_name: String,
    pub(crate) object_type: String,
    pub(crate) object_key: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) properties: Value,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct OntologySdkConsumerRelation {
    pub(crate) id: Uuid,
    pub(crate) api_name: String,
    pub(crate) relation_type: String,
    pub(crate) from_object_id: Uuid,
    pub(crate) to_object_id: Uuid,
}

pub(crate) async fn authorize_consumer_read(
    state: &AppState,
    headers: &HeaderMap,
    application_id: Uuid,
) -> Result<
    (
        Principal,
        OntologySdkApplication,
        OntologyRelease,
        OntologyReleaseCatalogV1,
    ),
    AppError,
> {
    let principal = authorize_collection_request(
        state,
        headers,
        Permission::SessionsRead,
        "ontology_sdk_consumer",
    )
    .await?;
    let application = state.get_ontology_sdk_application(application_id).await?;
    if application.subject != principal.subject_id {
        return Err(AppError::forbidden(
            "ontology SDK application is bound to a different authenticated subject",
        ));
    }
    if application.status != crate::ONTOLOGY_SDK_APPLICATION_STATUS_ACTIVE {
        return Err(AppError::forbidden(
            "ontology SDK application is not active",
        ));
    }
    let release = state
        .get_ontology_release(application.ontology_release_id)
        .await?;
    if !ontology_release_current_status(&release.status)
        || release.version != application.release_version
        || release.domain_scope != application.domain_scope
    {
        return Err(AppError::forbidden(
            "ontology SDK application release is no longer the exact active release",
        ));
    }
    let (catalog, catalog_digest) = release_catalog_from_evidence(&release)?;
    if catalog_digest != application.catalog_digest {
        return Err(AppError::forbidden(
            "ontology SDK application catalog digest no longer matches the release",
        ));
    }
    let (normalized_subset, subset_digest) =
        normalize_and_validate_subset(&catalog, &application.subset_manifest)?;
    if subset_digest != application.subset_digest
        || normalized_subset != application.subset_manifest
    {
        return Err(AppError::forbidden(
            "ontology SDK application subset manifest is invalid or tampered",
        ));
    }
    Ok((principal, application, release, catalog))
}

pub(crate) async fn task_grant_for_consumer_read(
    state: &AppState,
    grant_id: Option<Uuid>,
) -> Result<Option<TaskGrant>, AppError> {
    let Some(grant_id) = grant_id else {
        return Ok(None);
    };
    let grant = state.get_task_grant(grant_id).await?;
    if grant.status != "active"
        || grant
            .expires_at
            .is_some_and(|expires_at| expires_at <= Utc::now())
    {
        return Err(AppError::forbidden(
            "ontology SDK read TaskGrant is inactive or expired",
        ));
    }
    Ok(Some(grant))
}

pub(crate) fn require_consumer_allowlist(
    grant: Option<&TaskGrant>,
    kind: &str,
    api_name: &str,
) -> Result<(), AppError> {
    let Some(grant) = grant else {
        return Ok(());
    };
    if !consumer_scope_contains(grant, kind, api_name) {
        return Err(AppError::forbidden(
            "TaskGrant ontology_consumer_scope does not allow this API name",
        ));
    }
    Ok(())
}

fn consumer_scope_contains(grant: &TaskGrant, kind: &str, api_name: &str) -> bool {
    grant
        .approval_policy
        .get("ontology_consumer_scope")
        .and_then(Value::as_object)
        .and_then(|scope| scope.get(kind))
        .and_then(Value::as_array)
        .is_some_and(|names| names.iter().any(|name| name.as_str() == Some(api_name)))
}

pub(crate) async fn consumer_objects(
    state: &AppState,
    principal: &Principal,
    application: &OntologySdkApplication,
    catalog: &OntologyReleaseCatalogV1,
    api_name: &str,
    grant: Option<&TaskGrant>,
) -> Result<Vec<OntologySdkConsumerObject>, AppError> {
    require_consumer_allowlist(grant, "objects", api_name)?;
    if !application
        .subset_manifest
        .objects
        .iter()
        .any(|name| name == api_name)
    {
        return Err(AppError::forbidden(
            "object is not declared by the application subset",
        ));
    }
    let catalog_object = catalog
        .objects
        .iter()
        .find(|object| object.api_name == api_name)
        .ok_or_else(|| AppError::not_found("ontology SDK object API name not found"))?;
    let objects = visible_semantic_objects_for_principal(state, principal).await?;
    let mut projected = Vec::new();
    for object in objects {
        if object.object_type != "business_object"
            || object.status != "active"
            || object.archived_at.is_some()
            || object.content.get("domain_scope").and_then(Value::as_str)
                != Some(application.domain_scope.as_str())
            || object.content.get("object_type").and_then(Value::as_str)
                != Some(catalog_object.object_type.as_str())
        {
            continue;
        }
        match project_object(object, catalog_object) {
            Ok(projected_object) => projected.push(projected_object),
            Err(error) => {
                state
                    .append_audit_log(new_audit_log(
                        None,
                        "consumer",
                        None,
                        "ontology_sdk.consumer_projection_failed",
                        "ontology_sdk_application",
                        Some(application.id),
                        json!({"api_name": api_name, "reason": error.message.clone()}),
                    ))
                    .await?;
                return Err(error);
            }
        }
    }
    Ok(projected)
}

pub(crate) async fn consumer_object_by_id(
    state: &AppState,
    principal: &Principal,
    application: &OntologySdkApplication,
    catalog: &OntologyReleaseCatalogV1,
    api_name: &str,
    object_id: Uuid,
    grant: Option<&TaskGrant>,
) -> Result<OntologySdkConsumerObject, AppError> {
    let mut objects =
        consumer_objects(state, principal, application, catalog, api_name, grant).await?;
    objects
        .drain(..)
        .find(|object| object.id == object_id)
        .ok_or_else(|| AppError::not_found("ontology SDK object instance not found"))
}

pub(crate) async fn consumer_relations(
    state: &AppState,
    principal: &Principal,
    application: &OntologySdkApplication,
    catalog: &OntologyReleaseCatalogV1,
    grant: Option<&TaskGrant>,
    query: &OntologySdkConsumerReadQuery,
) -> Result<Vec<OntologySdkConsumerRelation>, AppError> {
    if let Some(api_name) = query.relation_api_name.as_deref() {
        require_consumer_allowlist(grant, "relations", api_name)?;
        if !application
            .subset_manifest
            .relations
            .iter()
            .any(|name| name == api_name)
        {
            return Err(AppError::forbidden(
                "relation is not declared by the application subset",
            ));
        }
    } else if grant.is_some()
        && grant
            .and_then(|grant| grant.approval_policy.get("ontology_consumer_scope"))
            .and_then(|scope| scope.get("relations"))
            .and_then(Value::as_array)
            .is_none()
    {
        return Err(AppError::forbidden(
            "TaskGrant ontology_consumer_scope relation allowlist is required",
        ));
    }
    let objects = visible_semantic_objects_for_principal(state, principal)
        .await?
        .into_iter()
        .filter(|object| {
            object.object_type == "business_object"
                && object.status == "active"
                && object.archived_at.is_none()
                && object.content.get("domain_scope").and_then(Value::as_str)
                    == Some(application.domain_scope.as_str())
        })
        .map(|object| (object.id, object))
        .collect::<BTreeMap<_, _>>();
    let visible_links = visible_semantic_links_for_principal(state, principal).await?;
    let subset_relations = application
        .subset_manifest
        .relations
        .iter()
        .collect::<BTreeSet<_>>();
    let mut projected = Vec::new();
    for link in visible_links {
        if link.from_entity_type != "semantic_object"
            || link.to_entity_type != "semantic_object"
            || link.status != "active"
            || link.archived_at.is_some()
        {
            continue;
        }
        let Ok(from_id) = Uuid::parse_str(&link.from_entity_id) else {
            continue;
        };
        let Ok(to_id) = Uuid::parse_str(&link.to_entity_id) else {
            continue;
        };
        let Some(from) = objects.get(&from_id) else {
            continue;
        };
        let Some(to) = objects.get(&to_id) else {
            continue;
        };
        let from_api_name = catalog
            .objects
            .iter()
            .find(|object| {
                object.object_type
                    == from
                        .content
                        .get("object_type")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
            })
            .map(|object| object.api_name.as_str());
        let to_api_name = catalog
            .objects
            .iter()
            .find(|object| {
                object.object_type
                    == to
                        .content
                        .get("object_type")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
            })
            .map(|object| object.api_name.as_str());
        let Some(relation) = catalog.relations.iter().find(|relation| {
            relation.relation_type == link.relation_type
                && Some(relation.from_object_api_name.as_str()) == from_api_name
                && Some(relation.to_object_api_name.as_str()) == to_api_name
        }) else {
            continue;
        };
        if !subset_relations.contains(&relation.api_name)
            || query
                .relation_api_name
                .as_deref()
                .is_some_and(|name| name != relation.api_name)
            || query
                .object_id
                .is_some_and(|id| id != from_id && id != to_id)
            || (grant.is_some()
                && !consumer_scope_contains(grant.unwrap(), "relations", &relation.api_name))
        {
            continue;
        }
        projected.push(OntologySdkConsumerRelation {
            id: link.id,
            api_name: relation.api_name.clone(),
            relation_type: relation.relation_type.clone(),
            from_object_id: from_id,
            to_object_id: to_id,
        });
    }
    Ok(projected)
}

pub(crate) async fn propose_consumer_action(
    state: &AppState,
    principal: &Principal,
    application: &OntologySdkApplication,
    release: &OntologyRelease,
    catalog: &OntologyReleaseCatalogV1,
    api_name: &str,
    input: OntologySdkConsumerActionRequest,
) -> Result<Value, AppError> {
    // The read collection check above intentionally remains SessionsRead so a
    // consumer can inspect its subset.  Proposal submission is a separate
    // privileged operation: require both execution and session-run permission,
    // and scope every referenced resource to the authenticated principal before
    // loading or using the grant/context UUIDs.
    authorize_principal_request(
        state,
        principal,
        Permission::ToolsExecute,
        "session",
        Some(input.session_id),
    )
    .await?;
    authorize_principal_request(
        state,
        principal,
        Permission::SessionsRun,
        "session",
        Some(input.session_id),
    )
    .await?;
    authorize_principal_request(
        state,
        principal,
        Permission::SessionsRead,
        "task_grant",
        Some(input.task_grant_id),
    )
    .await?;
    authorize_principal_request(
        state,
        principal,
        Permission::SessionsRead,
        "context_packet",
        Some(input.context_packet_id),
    )
    .await?;
    if !application
        .subset_manifest
        .actions
        .iter()
        .any(|name| name == api_name)
    {
        return Err(AppError::forbidden(
            "action is not declared by the application subset",
        ));
    }
    let action = catalog
        .actions
        .iter()
        .find(|action| action.api_name == api_name)
        .ok_or_else(|| AppError::not_found("ontology SDK action API name not found"))?;
    let grant = state.get_task_grant(input.task_grant_id).await?;
    if grant.status != "active"
        || grant
            .expires_at
            .is_some_and(|expires_at| expires_at <= Utc::now())
    {
        return Err(AppError::forbidden(
            "ontology action TaskGrant is inactive or expired",
        ));
    }
    let run = state.get_workflow_run(grant.workflow_run_id).await?;
    if !task_grant_session_matches(&grant, &run, input.session_id)
        || grant.context_packet_id != Some(input.context_packet_id)
    {
        return Err(AppError::forbidden(
            "ontology action TaskGrant is not bound to the requested session/context",
        ));
    }
    if !consumer_scope_contains(&grant, "actions", api_name) {
        return Err(AppError::forbidden(
            "TaskGrant ontology_consumer_scope does not allow this action",
        ));
    }
    if !task_grant_allows_tool(&grant, "ontology.action.execute") {
        return Err(AppError::forbidden(
            "TaskGrant tool scope does not allow ontology.action.execute",
        ));
    }
    let packet = state.get_context_packet(input.context_packet_id).await?;
    if packet.session_id != input.session_id {
        return Err(AppError::forbidden(
            "context packet does not belong to the requested session",
        ));
    }
    let packet_snapshot = packet
        .replay_summary
        .get("ontology_release")
        .ok_or_else(|| {
            AppError::forbidden("context packet ontology release snapshot is missing")
        })?;
    if packet_snapshot.get("id").and_then(Value::as_str) != Some(&release.id.to_string())
        || packet_snapshot.get("version").and_then(Value::as_str) != Some(release.version.as_str())
        || packet_snapshot.get("domain_scope").and_then(Value::as_str)
            != Some(release.domain_scope.as_str())
        || packet_snapshot
            .get("catalog_digest")
            .and_then(Value::as_str)
            != Some(&application.catalog_digest)
    {
        return Err(AppError::forbidden(
            "context packet ontology release snapshot does not match the application release",
        ));
    }
    let snapshot = grant
        .approval_policy
        .get("ontology_release_snapshot")
        .ok_or_else(|| AppError::forbidden("TaskGrant ontology release snapshot is missing"))?;
    if snapshot.get("id").and_then(Value::as_str) != Some(&release.id.to_string())
        || snapshot.get("version").and_then(Value::as_str) != Some(release.version.as_str())
        || snapshot.get("domain_scope").and_then(Value::as_str)
            != Some(release.domain_scope.as_str())
        || snapshot.get("catalog_digest").and_then(Value::as_str)
            != Some(&application.catalog_digest)
    {
        return Err(AppError::forbidden(
            "TaskGrant ontology release snapshot does not match the application release",
        ));
    }
    let (spec, _) = ontology_action_tool_spec_for_release(release, &action.runtime_name)?;
    if spec.execution_mode != "proposal_only" {
        return Err(AppError::forbidden(
            "consumer actions must remain proposal_only",
        ));
    }
    validate_ontology_action_parameters(&spec.input_schema, &input.parameters)?;
    let result = execute_tool_invocation(
        state,
        "ontology.action.execute",
        ExecuteTool {
            session_id: input.session_id,
            task_grant_id: Some(input.task_grant_id),
            args: json!({
                "action": action.runtime_name,
                "parameters": input.parameters,
                "context_packet_id": input.context_packet_id,
            }),
        },
        ToolInvocationOrigin::ManualRoute,
    )
    .await?;
    match result.get("status").and_then(Value::as_str) {
        Some("approval_required") | Some("proposal_created") => Ok(result),
        _ => Err(AppError::forbidden(
            "consumer ontology action did not produce an approval or proposal result",
        )),
    }
}

fn project_object(
    object: SemanticObject,
    catalog_object: &OntologySdkCatalogObject,
) -> Result<OntologySdkConsumerObject, AppError> {
    let source_values = object_property_values(&object.content);
    let mut projected = Map::new();
    for property in &catalog_object.properties {
        let value = source_values
            .get(&property.source_name)
            .cloned()
            .unwrap_or(Value::Null);
        if value.is_null() {
            if !property.nullable {
                return Err(AppError::forbidden(
                    "business object property is missing a non-nullable value",
                ));
            }
        } else if !property_value_matches(&value, &property.value_type) {
            return Err(AppError::forbidden(
                "business object property type does not match the release catalog",
            ));
        }
        projected.insert(property.api_name.clone(), value);
    }
    Ok(OntologySdkConsumerObject {
        id: object.id,
        api_name: catalog_object.api_name.clone(),
        object_type: catalog_object.object_type.clone(),
        object_key: object.object_key,
        title: object.title,
        summary: object.summary,
        properties: Value::Object(projected),
    })
}

fn object_property_values(content: &Value) -> BTreeMap<String, Value> {
    let mut values = BTreeMap::new();
    if let Some(properties) = content.get("properties") {
        match properties {
            Value::Array(entries) => {
                for entry in entries {
                    let Some(name) = entry
                        .get("source_name")
                        .or_else(|| entry.get("name"))
                        .and_then(Value::as_str)
                    else {
                        continue;
                    };
                    values.insert(
                        name.to_string(),
                        entry.get("value").cloned().unwrap_or(Value::Null),
                    );
                }
            }
            Value::Object(entries) => {
                values.extend(
                    entries
                        .iter()
                        .map(|(name, value)| (name.clone(), value.clone())),
                );
            }
            _ => {}
        }
    }
    if let Some(entries) = content.as_object() {
        for (name, value) in entries {
            values.entry(name.clone()).or_insert_with(|| value.clone());
        }
    }
    values
}

fn property_value_matches(value: &Value, value_type: &str) -> bool {
    match value_type.trim().to_ascii_lowercase().as_str() {
        "unknown" | "" => true,
        "string" | "text" => value.is_string(),
        "uuid" => value
            .as_str()
            .is_some_and(|value| Uuid::parse_str(value).is_ok()),
        "integer" | "int" | "int32" | "int64" => {
            value.as_i64().is_some() || value.as_u64().is_some()
        }
        "number" | "decimal" | "float" | "double" => value.is_number(),
        "boolean" | "bool" => value.is_boolean(),
        "object" => value.is_object(),
        "json" => !value.is_null(),
        "array" => value.is_array(),
        "date" => value
            .as_str()
            .is_some_and(|value| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()),
        "timestamp" | "datetime" => value
            .as_str()
            .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_ok()),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant_with_scope(scope: Value) -> TaskGrant {
        let now = Utc::now();
        TaskGrant {
            id: Uuid::new_v4(),
            workflow_run_id: Uuid::new_v4(),
            workflow_step_run_id: None,
            session_id: None,
            parent_grant_id: None,
            source_event_id: None,
            source_handoff_id: None,
            issuer_subject: "test".to_string(),
            grantee_agent_id: None,
            grantee_session_id: None,
            agent_class: None,
            objective: "test".to_string(),
            risk_level: "low".to_string(),
            status: "active".to_string(),
            expires_at: None,
            max_turns: None,
            max_tool_calls: None,
            max_runtime_seconds: None,
            max_cost_usd_micros: None,
            turns_used: 0,
            tool_calls_used: 0,
            cost_usd_micros_used: 0,
            semantic_scopes: json!({}),
            memory_scope: json!({}),
            tool_scope: json!({}),
            connector_scope: json!({}),
            approval_policy: json!({"ontology_consumer_scope": scope}),
            external_effects: json!({}),
            context_packet_id: None,
            policy_revision_id: None,
            immutable_args_hash: None,
            audit_trace_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn consumer_projection_type_validation_is_fail_closed() {
        assert!(property_value_matches(&json!(42), "integer"));
        assert!(!property_value_matches(&json!("42"), "integer"));
        assert!(!property_value_matches(&json!("not-a-date"), "timestamp"));
        assert!(property_value_matches(&json!("2026-08-12"), "date"));
        assert!(!property_value_matches(&json!("not-an-array"), "array"));
        assert!(property_value_matches(&json!(Uuid::nil()), "uuid"));
        assert!(!property_value_matches(&json!("not-a-uuid"), "uuid"));
        assert!(property_value_matches(&json!("anything"), "unknown"));
    }

    #[test]
    fn consumer_task_grant_scope_requires_explicit_allowlist() {
        let missing = grant_with_scope(json!({}));
        assert!(!consumer_scope_contains(&missing, "objects", "Customer"));
        assert!(require_consumer_allowlist(Some(&missing), "objects", "Customer").is_err());
        let allowed = grant_with_scope(json!({"objects": ["Customer"]}));
        assert!(require_consumer_allowlist(Some(&allowed), "objects", "Customer").is_ok());
        assert!(require_consumer_allowlist(Some(&allowed), "objects", "Order").is_err());
    }
}
