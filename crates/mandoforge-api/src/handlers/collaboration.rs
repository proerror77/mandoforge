use std::collections::BTreeMap;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    AgentTeammate, AppError, AppState, AuthorizationRequest, CreateAgentTeammate, CreateSquad,
    CreateSquadMember, CreateWorkItem, CreateWorkItemAssignment, CreateWorkItemReview,
    IngestWorkSurfaceEvent, Permission, Squad, SquadMember, WorkItem, WorkItemActivityEntry,
    WorkItemAssignment, WorkItemReview, WorkSurfaceIngestion, authorize_request,
    capability_failure_modes, capability_primary_action, capability_sample_tasks, header_value,
    new_audit_log, principal_from_request, project_work_item_semantic_object,
    validate_work_item_semantic_scopes,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/work-items",
            get(list_work_items).post(create_work_item),
        )
        .route("/api/work-surface-events", post(ingest_work_surface_event))
        .route(
            "/api/work-surface-events/capability-readback",
            get(get_work_surface_capability_readback),
        )
        .route(
            "/api/agent-teammates",
            get(list_agent_teammates).post(create_agent_teammate),
        )
        .route("/api/squads", get(list_squads).post(create_squad))
        .route(
            "/api/squads/{id}/members",
            get(list_squad_members).post(add_squad_member),
        )
        .route(
            "/api/work-items/{id}/assignments",
            get(list_work_item_assignments).post(create_work_item_assignment),
        )
        .route(
            "/api/work-items/{id}/reviews",
            get(list_work_item_reviews).post(create_work_item_review),
        )
        .route(
            "/api/work-items/{id}/activity",
            get(list_work_item_activity),
        )
        .route("/api/capability-discovery", get(get_capability_discovery))
}

async fn list_work_items(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkItem>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "work_items",
        None,
    )
    .await?;
    Ok(Json(state.list_work_items().await?))
}

async fn create_work_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkItem>,
) -> Result<Json<WorkItem>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsWrite,
        resource_type: "work_item".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    validate_work_item_semantic_scopes(&input.metadata)?;
    let work_item = state.create_work_item(input).await?;
    state
        .append_work_item_activity_entry(
            work_item.id,
            "work_item.created",
            Some(principal.subject_id.clone()),
            Some("work_item"),
            Some(work_item.id),
            format!("Created WorkItem: {}", work_item.title),
            json!({
                "title": work_item.title.clone(),
                "source": work_item.source.clone(),
                "status": work_item.status.clone(),
                "priority": work_item.priority.clone()
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "work_item.created",
            "work_item",
            Some(work_item.id),
            json!({
                "subject": principal.subject_id,
                "organization_id": work_item.organization_id,
                "team_id": work_item.team_id,
                "project_id": work_item.project_id,
                "title": work_item.title,
                "source": work_item.source,
                "status": work_item.status,
                "priority": work_item.priority
            }),
        ))
        .await?;
    project_work_item_semantic_object(&state, &work_item).await?;
    Ok(Json(work_item))
}

async fn ingest_work_surface_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<WorkSurfaceIngestion>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsWrite,
        resource_type: "work_surface_event".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    let input: IngestWorkSurfaceEvent =
        serde_json::from_slice(&body).map_err(|_| AppError::bad_request("invalid JSON body"))?;
    let adapter = work_surface_adapter(input.surface)?;
    let surface = adapter.surface.clone();
    let event_type = required_work_surface_text(input.event_type, "event_type")?;
    let auth = verify_work_surface_webhook_auth(&headers, &body, &surface)?;
    let cursor = normalize_work_surface_optional_text(input.cursor.or_else(|| {
        header_value(&headers, "x-mandoforge-work-surface-cursor").map(str::to_string)
    }));
    let delivery_id = normalize_work_surface_optional_text(input.delivery_id.or_else(|| {
        header_value(&headers, "x-mandoforge-work-surface-delivery-id")
            .or_else(|| header_value(&headers, "x-github-delivery"))
            .or_else(|| header_value(&headers, "linear-delivery"))
            .or_else(|| header_value(&headers, "x-atlassian-webhook-identifier"))
            .map(str::to_string)
            .or_else(|| work_surface_delivery_id(&surface, &input.metadata))
    }));
    let source_url = if adapter.known {
        input
            .source_url
            .or_else(|| work_surface_source_url(&input.metadata))
    } else {
        input.source_url
    };
    let metadata = work_surface_metadata(
        input.metadata,
        &headers,
        &adapter,
        &event_type,
        input.external_id,
        cursor.clone(),
        delivery_id.clone(),
        auth.clone(),
        input.occurred_at,
        input.actor.clone(),
        input.assignee.as_deref(),
    );
    validate_work_item_semantic_scopes(&metadata)?;
    if let Some(existing) = find_work_surface_replay(
        &state,
        &surface,
        &event_type,
        cursor.as_deref(),
        delivery_id.as_deref(),
    )
    .await?
    {
        let replay_metadata = work_surface_activity_metadata(
            &surface,
            &event_type,
            existing.source_url.clone(),
            existing
                .metadata
                .get("work_surface")
                .cloned()
                .unwrap_or(Value::Null),
            true,
            Some(existing.id),
        );
        let activity = state
            .append_work_item_activity_entry(
                existing.id,
                "work_surface.replayed",
                Some(principal.subject_id.clone()),
                Some("work_surface"),
                None,
                format!("Replayed {} event: {}", surface, existing.title),
                replay_metadata.clone(),
            )
            .await?;
        state
            .append_audit_log(new_audit_log(
                None,
                "work_surface",
                None,
                "work_surface.replayed",
                "work_item",
                Some(existing.id),
                json!({
                    "subject": principal.subject_id,
                    "surface": surface,
                    "event_type": event_type,
                    "work_item_id": existing.id,
                    "cursor": cursor,
                    "delivery_id": delivery_id,
                    "metadata": replay_metadata
                }),
            ))
            .await?;
        return Ok(Json(WorkSurfaceIngestion {
            replay_of_work_item_id: Some(existing.id),
            work_item: existing,
            activity,
            replayed: true,
        }));
    }
    let work_item = state
        .create_work_item(CreateWorkItem {
            organization_id: None,
            team_id: None,
            project_id: None,
            title: input.title,
            description: input.description,
            source: surface.clone(),
            source_url,
            status: "open".to_string(),
            priority: input.priority,
            assignee: input.assignee,
            metadata,
        })
        .await?;
    let activity_metadata = work_surface_activity_metadata(
        &surface,
        &event_type,
        work_item.source_url.clone(),
        work_item
            .metadata
            .get("work_surface")
            .cloned()
            .unwrap_or(Value::Null),
        false,
        None,
    );
    let activity = state
        .append_work_item_activity_entry(
            work_item.id,
            "work_surface.ingested",
            Some(principal.subject_id.clone()),
            Some("work_surface"),
            None,
            format!("Ingested {} event: {}", surface, work_item.title),
            activity_metadata.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "work_surface",
            None,
            "work_surface.ingested",
            "work_item",
            Some(work_item.id),
            json!({
                "subject": principal.subject_id,
                "surface": surface,
                "event_type": event_type,
                "actor": input.actor,
                "work_item_id": work_item.id,
                "source_url": work_item.source_url,
                "metadata": activity_metadata
            }),
        ))
        .await?;
    project_work_item_semantic_object(&state, &work_item).await?;
    Ok(Json(WorkSurfaceIngestion {
        work_item,
        activity,
        replayed: false,
        replay_of_work_item_id: None,
    }))
}

async fn get_work_surface_capability_readback(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "work_surface_event",
        None,
    )
    .await?;
    let work_items = state.list_work_items().await?;
    let work_surfaces = work_items
        .iter()
        .filter_map(|item| item.metadata.get("work_surface"))
        .collect::<Vec<_>>();
    let mut surfaces = BTreeMap::new();
    for work_surface in &work_surfaces {
        if let Some(surface) = work_surface.get("surface").and_then(Value::as_str) {
            *surfaces.entry(surface.to_string()).or_insert(0usize) += 1;
        }
    }
    let observed_count = |field: &str| {
        work_surfaces
            .iter()
            .filter(|work_surface| {
                work_surface
                    .get(field)
                    .and_then(|value| value.get("observed"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            .count()
    };
    Ok(Json(json!({
        "product_object": "WorkSurfaceConnector",
        "summary": {
            "ingested_work_surface_count": work_surfaces.len(),
            "surfaces": surfaces,
            "verified_webhook_count": work_surfaces
                .iter()
                .filter(|work_surface| {
                    work_surface
                        .get("authentication")
                        .and_then(|value| value.get("verified"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                })
                .count(),
            "rate_limit_evidence_count": observed_count("rate_limit"),
            "live_readback_evidence_count": observed_count("live_readback"),
            "email_authentication_evidence_count": observed_count("email_authentication")
        },
        "evidence_sources": [
            "work_items.metadata.work_surface",
            "work_surface.ingested_activity",
            "work_surface.ingested_audit"
        ],
        "implemented_boundaries": [
            "canonical WorkItem intake",
            "configured webhook signature verification",
            "cursor_or_delivery_replay_detection",
            "observed rate-limit evidence preservation",
            "observed live-readback evidence preservation",
            "observed email authentication evidence preservation"
        ],
        "open_boundaries": [
            "platform OAuth/token lifecycle",
            "active live API fetch/readback",
            "MandoForge-enforced rate-limit scheduling"
        ],
        "authority_boundary": "read-only Work Surface capability readback; connector evidence can create WorkItems but cannot start runtime execution or bypass Manager Runtime, Policy, Approval, or Audit"
    })))
}

fn required_work_surface_text(value: String, field: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppError::bad_request(format!(
            "work surface event {field} is required"
        )))
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_work_surface_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn work_surface_activity_metadata(
    surface: &str,
    event_type: &str,
    source_url: Option<String>,
    work_surface: Value,
    replayed: bool,
    replay_of_work_item_id: Option<Uuid>,
) -> Value {
    json!({
        "surface": surface,
        "event_type": event_type,
        "source_url": source_url,
        "work_surface": work_surface,
        "replayed": replayed,
        "replay_of_work_item_id": replay_of_work_item_id
    })
}

struct WorkSurfaceAdapter {
    surface: String,
    name: &'static str,
    known: bool,
}

fn work_surface_adapter(value: String) -> Result<WorkSurfaceAdapter, AppError> {
    let surface = required_work_surface_text(value, "surface")?;
    let normalized = surface.to_ascii_lowercase();
    let canonical = match normalized.as_str() {
        "lark" | "feishu" => ("feishu", "feishu.work_surface"),
        "slack" => ("slack", "slack.work_surface"),
        "gh" | "github" => ("github", "github.work_surface"),
        "jira" => ("jira", "jira.work_surface"),
        "linear" => ("linear", "linear.work_surface"),
        "mail" | "email" => ("email", "email.work_surface"),
        _ => {
            return Ok(WorkSurfaceAdapter {
                surface,
                name: "generic.work_surface",
                known: false,
            });
        }
    };
    Ok(WorkSurfaceAdapter {
        surface: canonical.0.to_string(),
        name: canonical.1,
        known: true,
    })
}

fn work_surface_metadata(
    metadata: Value,
    headers: &HeaderMap,
    adapter: &WorkSurfaceAdapter,
    event_type: &str,
    external_id: Option<String>,
    cursor: Option<String>,
    delivery_id: Option<String>,
    auth: Value,
    occurred_at: Option<String>,
    actor: Option<String>,
    assignee: Option<&str>,
) -> Value {
    let mut work_surface = Map::new();
    work_surface.insert("surface".to_string(), json!(&adapter.surface));
    work_surface.insert("adapter".to_string(), json!(adapter.name));
    work_surface.insert("event_type".to_string(), json!(event_type));
    work_surface.insert("external_id".to_string(), json!(external_id));
    work_surface.insert("cursor".to_string(), json!(cursor));
    work_surface.insert("delivery_id".to_string(), json!(delivery_id));
    work_surface.insert("authentication".to_string(), auth);
    work_surface.insert(
        "rate_limit".to_string(),
        work_surface_rate_limit_evidence(headers, &metadata),
    );
    work_surface.insert(
        "live_readback".to_string(),
        work_surface_live_readback_evidence(headers, &metadata),
    );
    if adapter.surface == "email" {
        work_surface.insert(
            "email_authentication".to_string(),
            work_surface_email_authentication_evidence(headers, &metadata),
        );
    }
    work_surface.insert("occurred_at".to_string(), json!(occurred_at));
    work_surface.insert("actor".to_string(), json!(actor));
    work_surface.insert(
        "extracted".to_string(),
        if adapter.known {
            work_surface_extracted_fields(&adapter.surface, &metadata)
        } else {
            json!({})
        },
    );
    work_surface.insert(
        "human_state".to_string(),
        if adapter.known {
            work_surface_human_state(&metadata, assignee)
        } else {
            json!({})
        },
    );
    let work_surface = Value::Object(work_surface);
    match metadata {
        Value::Object(mut object) => {
            object.insert("work_surface".to_string(), work_surface);
            Value::Object(object)
        }
        value => {
            let mut object = Map::new();
            object.insert("work_surface".to_string(), work_surface);
            object.insert("surface_payload".to_string(), value);
            Value::Object(object)
        }
    }
}

fn work_surface_rate_limit_evidence(headers: &HeaderMap, metadata: &Value) -> Value {
    let header_evidence = work_surface_header_evidence(
        headers,
        &[
            ("limit", "x-ratelimit-limit"),
            ("remaining", "x-ratelimit-remaining"),
            ("reset", "x-ratelimit-reset"),
            ("retry_after", "retry-after"),
        ],
    );
    let payload = work_surface_first_nested_value(
        metadata,
        &["rate_limit", "rateLimit", "rate_limit_evidence"],
    );
    work_surface_observed_evidence(header_evidence, payload)
}

fn work_surface_live_readback_evidence(headers: &HeaderMap, metadata: &Value) -> Value {
    let header_evidence = work_surface_header_evidence(
        headers,
        &[
            ("status", "x-mandoforge-work-surface-readback-status"),
            ("source", "x-mandoforge-work-surface-readback-source"),
            (
                "external_object_id",
                "x-mandoforge-work-surface-readback-object-id",
            ),
            ("checked_at", "x-mandoforge-work-surface-readback-at"),
        ],
    );
    let payload = work_surface_first_nested_value(
        metadata,
        &[
            "live_readback",
            "liveReadback",
            "live_readback_evidence",
            "api_readback",
        ],
    );
    work_surface_observed_evidence(header_evidence, payload)
}

fn work_surface_email_authentication_evidence(headers: &HeaderMap, metadata: &Value) -> Value {
    let header_evidence = work_surface_header_evidence(
        headers,
        &[
            ("authentication_results", "authentication-results"),
            ("arc_authentication_results", "arc-authentication-results"),
            ("received_spf", "received-spf"),
            ("dkim_signature", "dkim-signature"),
            ("arc_seal", "arc-seal"),
            ("arc_message_signature", "arc-message-signature"),
            ("message_id", "message-id"),
            ("return_path", "return-path"),
            ("provider", "x-mandoforge-email-provider"),
            ("spf_verdict", "x-mandoforge-email-spf"),
            ("dkim_verdict", "x-mandoforge-email-dkim"),
            ("dmarc_verdict", "x-mandoforge-email-dmarc"),
        ],
    );
    let payload = work_surface_first_nested_value(
        metadata,
        &[
            "email_authentication",
            "emailAuthentication",
            "authentication_results",
            "authenticationResults",
            "mail/authentication",
            "envelope/authentication",
            "headers/authentication-results",
        ],
    );
    work_surface_observed_evidence(header_evidence, payload)
}

fn work_surface_header_evidence(
    headers: &HeaderMap,
    fields: &[(&str, &str)],
) -> Map<String, Value> {
    let mut evidence = Map::new();
    for (field, header) in fields {
        if let Some(value) = header_value(headers, header) {
            evidence.insert((*field).to_string(), json!(value));
        }
    }
    evidence
}

fn work_surface_observed_evidence(
    header_evidence: Map<String, Value>,
    payload: Option<Value>,
) -> Value {
    let has_headers = !header_evidence.is_empty();
    let has_payload = payload.is_some();
    let mut sources = Vec::new();
    if has_headers {
        sources.push("headers");
    }
    if has_payload {
        sources.push("payload.metadata");
    }
    json!({
        "observed": has_headers || has_payload,
        "source": sources,
        "headers": header_evidence,
        "payload": payload.unwrap_or(Value::Null),
        "mandoforge_enforced": false,
    })
}

fn work_surface_source_url(metadata: &Value) -> Option<String> {
    work_surface_first_string(
        metadata,
        &["source_url", "url", "html_url", "permalink", "web_url"],
    )
}

fn work_surface_delivery_id(surface: &str, metadata: &Value) -> Option<String> {
    let candidates: &[&str] = match surface {
        "slack" => &[
            "delivery_id",
            "event_id",
            "event/event_id",
            "event/client_msg_id",
        ],
        "feishu" => &[
            "delivery_id",
            "event_id",
            "header/event_id",
            "event/message/message_id",
        ],
        "linear" => &["delivery_id", "webhook_id", "notification/id", "event/id"],
        "jira" => &["delivery_id", "webhook_event_id", "issue/id"],
        "github" => &["delivery_id"],
        "email" => &[
            "delivery_id",
            "message_id",
            "messageId",
            "message/id",
            "mail/message_id",
            "mail/messageId",
            "envelope/message_id",
        ],
        _ => &["delivery_id", "event_id"],
    };
    work_surface_first_nested_string(metadata, candidates)
}

fn work_surface_human_state(metadata: &Value, assignee: Option<&str>) -> Value {
    let mut state = Map::new();
    for (field, keys) in [
        ("owner", &["owner", "author", "sender", "user"][..]),
        ("reviewer", &["reviewer", "requested_reviewer"][..]),
        (
            "assignee",
            &["assignee", "assignee_login", "responsible"][..],
        ),
        ("blocker", &["blocker", "blocked_by"][..]),
        ("due_date", &["due_date", "due", "deadline"][..]),
        ("status", &["status", "state"][..]),
    ] {
        if let Some(value) = work_surface_first_value(metadata, keys) {
            state.insert(field.to_string(), value);
        }
    }
    if !state.contains_key("assignee")
        && let Some(assignee) = assignee
    {
        state.insert("assignee".to_string(), json!(assignee));
    }
    Value::Object(state)
}

fn work_surface_extracted_fields(surface: &str, metadata: &Value) -> Value {
    let mut extracted = Map::new();
    for (field, candidates) in [
        (
            "object_type",
            &[
                "object_type",
                "type",
                "issue/type",
                "pull_request/type",
                "event/type",
            ][..],
        ),
        (
            "object_id",
            &[
                "object_id",
                "id",
                "issue/number",
                "issue/id",
                "pull_request/id",
                "pull_request/number",
                "message/message_id",
                "message_id",
                "messageId",
                "mail/message_id",
                "mail/messageId",
                "envelope/message_id",
                "event/client_msg_id",
                "event/ts",
                "ticket/id",
            ][..],
        ),
        (
            "repository",
            &["repository", "repo", "repository/full_name", "project/repo"][..],
        ),
        (
            "channel",
            &["channel", "chat_id", "message/chat_id", "event/channel"][..],
        ),
        (
            "thread",
            &[
                "thread",
                "thread_id",
                "message/thread_id",
                "event/thread_ts",
            ][..],
        ),
        (
            "project",
            &["project", "project/key", "issue/project", "team/key"][..],
        ),
        (
            "labels",
            &["labels", "tags", "issue/labels", "pull_request/labels"][..],
        ),
        (
            "mentions",
            &["mentions", "mentioned_users", "event/mentions"][..],
        ),
    ] {
        if let Some(value) = work_surface_first_nested_value(metadata, candidates) {
            extracted.insert(field.to_string(), value);
        }
    }
    extracted.insert("surface".to_string(), json!(surface));
    Value::Object(extracted)
}

fn work_surface_first_string(metadata: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| metadata.get(*key)?.as_str().map(str::to_string))
}

fn work_surface_first_value(metadata: &Value, keys: &[&str]) -> Option<Value> {
    keys.iter().find_map(|key| metadata.get(*key).cloned())
}

fn work_surface_first_nested_string(metadata: &Value, keys: &[&str]) -> Option<String> {
    work_surface_first_nested_value(metadata, keys)
        .and_then(|value| value.as_str().map(str::to_string))
}

fn work_surface_first_nested_value(metadata: &Value, keys: &[&str]) -> Option<Value> {
    keys.iter().find_map(|key| {
        if let Some(value) = metadata.get(*key) {
            return Some(value.clone());
        }
        let pointer = format!("/{key}");
        metadata.pointer(&pointer).cloned()
    })
}

async fn find_work_surface_replay(
    state: &AppState,
    surface: &str,
    event_type: &str,
    cursor: Option<&str>,
    delivery_id: Option<&str>,
) -> Result<Option<WorkItem>, AppError> {
    if cursor.is_none() && delivery_id.is_none() {
        return Ok(None);
    }
    let work_items = state.list_work_items().await?;
    Ok(work_items.into_iter().find(|work_item| {
        let Some(work_surface) = work_item.metadata.get("work_surface") else {
            return false;
        };
        work_surface.get("surface").and_then(Value::as_str) == Some(surface)
            && work_surface.get("event_type").and_then(Value::as_str) == Some(event_type)
            && ((cursor.is_some() && work_surface.get("cursor").and_then(Value::as_str) == cursor)
                || (delivery_id.is_some()
                    && work_surface.get("delivery_id").and_then(Value::as_str) == delivery_id))
    }))
}

fn verify_work_surface_webhook_auth(
    headers: &HeaderMap,
    body: &[u8],
    surface: &str,
) -> Result<Value, AppError> {
    let signature_present = work_surface_signature_present(headers, surface);
    let Some((secret_ref, secret)) = work_surface_webhook_secret(surface) else {
        return Ok(json!({
            "mode": "mandoforge_rbac",
            "configured": false,
            "verified": false,
            "signature_present": signature_present
        }));
    };
    let verification = verify_work_surface_signature(headers, body, &secret, surface)?;
    Ok(json!({
        "mode": verification.mode,
        "configured": true,
        "verified": true,
        "signature_present": true,
        "signature_header": verification.header,
        "secret_ref": secret_ref
    }))
}

fn work_surface_webhook_secret(surface: &str) -> Option<(String, String)> {
    let surface_key = surface
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    [
        format!("MANDOFORGE_WORK_SURFACE_{surface_key}_WEBHOOK_SECRET"),
        "MANDOFORGE_WORK_SURFACE_WEBHOOK_SECRET".to_string(),
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(&key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(|value| (key, value))
    })
}

struct WorkSurfaceSignatureVerification {
    mode: &'static str,
    header: &'static str,
}

fn work_surface_signature_present(headers: &HeaderMap, surface: &str) -> bool {
    header_value(headers, "x-mandoforge-work-surface-signature").is_some()
        || (surface == "github" && header_value(headers, "x-hub-signature-256").is_some())
        || (surface == "slack" && header_value(headers, "x-slack-signature").is_some())
        || (surface == "linear" && header_value(headers, "linear-signature").is_some())
        || (surface == "jira" && header_value(headers, "x-hub-signature").is_some())
        || (surface == "feishu" && header_value(headers, "x-lark-signature").is_some())
}

fn verify_work_surface_signature(
    headers: &HeaderMap,
    body: &[u8],
    secret: &str,
    surface: &str,
) -> Result<WorkSurfaceSignatureVerification, AppError> {
    if let Some(signature) = header_value(headers, "x-mandoforge-work-surface-signature") {
        verify_hmac_sha256_signature(signature, body, secret, "sha256=")?;
        return Ok(WorkSurfaceSignatureVerification {
            mode: "hmac_sha256",
            header: "x-mandoforge-work-surface-signature",
        });
    }
    if surface == "github"
        && let Some(signature) = header_value(headers, "x-hub-signature-256")
    {
        verify_hmac_sha256_signature(signature, body, secret, "sha256=")?;
        return Ok(WorkSurfaceSignatureVerification {
            mode: "github_hmac_sha256",
            header: "x-hub-signature-256",
        });
    }
    if surface == "slack"
        && let Some(signature) = header_value(headers, "x-slack-signature")
    {
        verify_slack_signature(headers, body, secret, signature)?;
        return Ok(WorkSurfaceSignatureVerification {
            mode: "slack_hmac_sha256",
            header: "x-slack-signature",
        });
    }
    if surface == "linear"
        && let Some(signature) = header_value(headers, "linear-signature")
    {
        verify_hmac_sha256_signature(signature, body, secret, "")?;
        return Ok(WorkSurfaceSignatureVerification {
            mode: "linear_hmac_sha256",
            header: "linear-signature",
        });
    }
    if surface == "jira"
        && let Some(signature) = header_value(headers, "x-hub-signature")
    {
        verify_hmac_sha256_signature(signature, body, secret, "sha256=")?;
        return Ok(WorkSurfaceSignatureVerification {
            mode: "jira_websub_hmac_sha256",
            header: "x-hub-signature",
        });
    }
    if surface == "feishu"
        && let Some(signature) = header_value(headers, "x-lark-signature")
    {
        verify_feishu_signature(headers, body, secret, signature)?;
        return Ok(WorkSurfaceSignatureVerification {
            mode: "feishu_sha256",
            header: "x-lark-signature",
        });
    }
    Err(AppError::unauthorized(
        "missing work surface webhook signature",
    ))
}

fn verify_hmac_sha256_signature(
    signature: &str,
    body: &[u8],
    secret: &str,
    prefix: &str,
) -> Result<(), AppError> {
    let signature = signature.strip_prefix(prefix).unwrap_or(signature).trim();
    let signature_bytes =
        hex::decode(signature).map_err(|_| AppError::unauthorized("non-hex signature"))?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| AppError::internal("hmac key error"))?;
    mac.update(body);
    mac.verify_slice(&signature_bytes)
        .map_err(|_| AppError::unauthorized("signature mismatch"))
}

fn verify_feishu_signature(
    headers: &HeaderMap,
    body: &[u8],
    secret: &str,
    signature: &str,
) -> Result<(), AppError> {
    let timestamp = header_value(headers, "x-lark-request-timestamp")
        .ok_or_else(|| AppError::unauthorized("missing x-lark-request-timestamp"))?;
    let nonce = header_value(headers, "x-lark-request-nonce")
        .ok_or_else(|| AppError::unauthorized("missing x-lark-request-nonce"))?;
    let mut hasher = Sha256::new();
    hasher.update(timestamp.as_bytes());
    hasher.update(nonce.as_bytes());
    hasher.update(secret.as_bytes());
    hasher.update(body);
    let expected = hex::encode(hasher.finalize());
    if constant_time_str_eq(expected.as_str(), signature.trim()) {
        Ok(())
    } else {
        Err(AppError::unauthorized("signature mismatch"))
    }
}

fn constant_time_str_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right.iter())
        .fold(0u8, |diff, (left, right)| diff | (left ^ right))
        == 0
}

fn verify_slack_signature(
    headers: &HeaderMap,
    body: &[u8],
    secret: &str,
    signature: &str,
) -> Result<(), AppError> {
    let timestamp = header_value(headers, "x-slack-request-timestamp")
        .ok_or_else(|| AppError::unauthorized("missing x-slack-request-timestamp"))?;
    let request_timestamp = timestamp
        .parse::<i64>()
        .map_err(|_| AppError::unauthorized("invalid x-slack-request-timestamp"))?;
    let now = Utc::now().timestamp();
    if (now - request_timestamp).abs() > 60 * 5 {
        return Err(AppError::unauthorized("stale x-slack-request-timestamp"));
    }
    let signature = signature
        .strip_prefix("v0=")
        .ok_or_else(|| AppError::unauthorized("malformed x-slack-signature"))?
        .trim();
    let signature_bytes =
        hex::decode(signature).map_err(|_| AppError::unauthorized("non-hex signature"))?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| AppError::internal("hmac key error"))?;
    mac.update(b"v0:");
    mac.update(timestamp.as_bytes());
    mac.update(b":");
    mac.update(body);
    mac.verify_slice(&signature_bytes)
        .map_err(|_| AppError::unauthorized("signature mismatch"))
}

async fn list_agent_teammates(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentTeammate>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "agent_teammates",
        None,
    )
    .await?;
    Ok(Json(state.list_agent_teammates().await?))
}

async fn create_agent_teammate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateAgentTeammate>,
) -> Result<Json<AgentTeammate>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsWrite,
        resource_type: "agent_teammate".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    let teammate = state.create_agent_teammate(input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "agent_teammate.created",
            "agent_teammate",
            Some(teammate.id),
            json!({
                "subject": principal.subject_id,
                "agent_id": teammate.agent_id,
                "display_name": teammate.display_name,
                "handle": teammate.handle,
                "role": teammate.role,
                "status": teammate.status
            }),
        ))
        .await?;
    Ok(Json(teammate))
}

async fn list_squads(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Squad>>, AppError> {
    authorize_request(&state, &headers, Permission::SessionsRead, "squads", None).await?;
    Ok(Json(state.list_squads().await?))
}

async fn create_squad(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateSquad>,
) -> Result<Json<Squad>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsWrite,
        resource_type: "squad".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    let squad = state.create_squad(input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "squad.created",
            "squad",
            Some(squad.id),
            json!({
                "subject": principal.subject_id,
                "name": squad.name,
                "status": squad.status
            }),
        ))
        .await?;
    Ok(Json(squad))
}

async fn list_squad_members(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<SquadMember>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "squad",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_squad_members(id).await?))
}

async fn add_squad_member(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateSquadMember>,
) -> Result<Json<SquadMember>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsWrite,
        resource_type: "squad".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    let member = state.add_squad_member(id, input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "squad.member_added",
            "squad_member",
            Some(member.id),
            json!({
                "subject": principal.subject_id,
                "squad_id": member.squad_id,
                "teammate_id": member.teammate_id,
                "role": member.role,
                "status": member.status
            }),
        ))
        .await?;
    Ok(Json(member))
}

async fn list_work_item_assignments(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkItemAssignment>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "work_item",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_work_item_assignments(id).await?))
}

async fn create_work_item_assignment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkItemAssignment>,
) -> Result<Json<WorkItemAssignment>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsWrite,
        resource_type: "work_item".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    let assignment = state
        .create_work_item_assignment(id, input, Some(principal.subject_id.clone()))
        .await?;
    state
        .append_work_item_activity_entry(
            assignment.work_item_id,
            "work_item.assignment_created",
            Some(principal.subject_id.clone()),
            Some("work_item_assignment"),
            Some(assignment.id),
            format!(
                "Assigned {} {} as {}",
                assignment.assignee_kind, assignment.assignee_id, assignment.role
            ),
            json!({
                "assignee_kind": assignment.assignee_kind.clone(),
                "assignee_id": assignment.assignee_id.clone(),
                "role": assignment.role.clone(),
                "status": assignment.status.clone()
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "work_item.assignment_created",
            "work_item_assignment",
            Some(assignment.id),
            json!({
                "subject": principal.subject_id,
                "work_item_id": assignment.work_item_id,
                "assignee_kind": assignment.assignee_kind,
                "assignee_id": assignment.assignee_id,
                "role": assignment.role,
                "status": assignment.status,
                "assigned_by": assignment.assigned_by
            }),
        ))
        .await?;
    Ok(Json(assignment))
}

async fn list_work_item_reviews(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkItemReview>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "work_item",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_work_item_reviews(id).await?))
}

async fn create_work_item_review(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkItemReview>,
) -> Result<Json<WorkItemReview>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsWrite,
        resource_type: "work_item".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    let review = state.create_work_item_review(id, input).await?;
    state
        .append_work_item_activity_entry(
            review.work_item_id,
            "work_item.review_created",
            Some(principal.subject_id.clone()),
            Some("work_item_review"),
            Some(review.id),
            match &review.decision {
                Some(decision) => format!("Review completed with decision: {decision}"),
                None => "Review requested".to_string(),
            },
            json!({
                "reviewer_kind": review.reviewer_kind.clone(),
                "reviewer_id": review.reviewer_id.clone(),
                "status": review.status.clone(),
                "decision": review.decision.clone(),
                "summary": review.summary.clone()
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "work_item.review_created",
            "work_item_review",
            Some(review.id),
            json!({
                "subject": principal.subject_id,
                "work_item_id": review.work_item_id,
                "reviewer_kind": review.reviewer_kind,
                "reviewer_id": review.reviewer_id,
                "status": review.status,
                "decision": review.decision
            }),
        ))
        .await?;
    Ok(Json(review))
}

async fn list_work_item_activity(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkItemActivityEntry>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "work_item",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_work_item_activity(id).await?))
}

async fn get_capability_discovery(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "capability_discovery",
        None,
    )
    .await?;
    let agents = state.list_agents().await?;
    let work_items = state.list_work_items().await?;
    let pending_memory = state
        .list_memory_writeback_candidates(None)
        .await?
        .into_iter()
        .filter(|candidate| candidate.status == "pending")
        .count();
    let agent_cards = agents
        .iter()
        .map(|agent| {
            json!({
                "agent_id": agent.id,
                "name": agent.name,
                "kind": agent.kind,
                "agent_role": agent.agent_role,
                "provider": agent.provider,
                "model": agent.model,
                "release_state": agent.release_state,
                "tools": agent.tools,
                "skill_ids": agent.skill_ids,
                "workflow_pack_ids": agent.workflow_pack_ids,
                "semantic_scopes": agent.semantic_scopes,
                "primary_action": capability_primary_action(agent),
                "failure_modes": capability_failure_modes(agent),
                "sample_tasks": capability_sample_tasks(agent),
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "status": "ready",
        "generated_at": Utc::now(),
        "summary": {
            "agent_count": agents.len(),
            "open_work_item_count": work_items.iter().filter(|item| !matches!(item.status.as_str(), "done" | "canceled")).count(),
            "pending_memory_review_count": pending_memory,
        },
        "agent_cards": agent_cards,
        "product_capabilities": agent_os_product_capabilities(),
        "suggested_prompts": [
            {
                "target_view": "manager",
                "title": "拆解并派发一个工作项",
                "prompt": "请把这个目标拆解成 WorkItem、选择合适 Agent、设置 SLA，并建立复审点。",
                "action": "create_work_item_then_start_pack_workflow"
            },
            {
                "target_view": "semantic",
                "title": "整理领域记忆",
                "prompt": "请扫描这个 domain 的冲突、过期记忆与 ontology 缺口，建立需要复审的队列。",
                "action": "open_semantic_workbench"
            },
            {
                "target_view": "board",
                "title": "查看队列风险",
                "prompt": "请找出 blocked、overdue、无人领取的任务，并给出下一步处理建议。",
                "action": "inspect_task_board"
            }
        ],
        "onboarding_steps": [
            {
                "key": "create_work_item",
                "title": "建立 WorkItem",
                "description": "先把业务目标变成可审计、可派工、可复审的任务。"
            },
            {
                "key": "start_pack_manager_workflow",
                "title": "启动 Pack Manager Workflow",
                "description": "通过 Workflow Pack 定义的 manager workflow 做 intake、拆解、派工、SLA 检查和复审。"
            },
            {
                "key": "review_memory",
                "title": "复审 Memory Queue",
                "description": "批准或拒绝 reflection / dreaming 产生的记忆候选。"
            },
            {
                "key": "install_pack",
                "title": "安装 Workflow Pack",
                "description": "把领域流程、Agent 技能和运行对象绑定成可复用模板。"
            }
        ],
        "empty_states": [
            {
                "view": "manager",
                "title": "还没有 Pack Manager Workflow 结果",
                "action": "start_pack_manager_workflow"
            },
            {
                "view": "semantic",
                "title": "还没有可治理的语义对象",
                "action": "ingest_semantic_source"
            },
            {
                "view": "board",
                "title": "还没有 WorkItem",
                "action": "create_work_item"
            }
        ],
    })))
}

fn agent_os_product_capabilities() -> Vec<Value> {
    vec![
        json!({
            "key": "work_surface_connector",
            "product_object": "WorkSurfaceConnector",
            "status": "available",
            "api_routes": [
                "POST /api/work-surface-events",
                "GET /api/work-surface-events/capability-readback"
            ],
            "lifecycle_actions": ["ingest", "verify_webhook", "detect_replay", "preserve_observed_evidence", "readback"],
            "evidence_events": [
                "work_surface.ingested",
                "work_surface.replayed"
            ],
            "authority_boundary": "Work Surface connectors create governed WorkItems and evidence readbacks only; they do not start runtime execution or bypass Manager Runtime, Policy, Approval, or Audit."
        }),
        json!({
            "key": "workflow_pack",
            "product_object": "WorkflowPack",
            "status": "available",
            "api_routes": [
                "GET /api/workflow-packs/marketplace",
                "POST /api/workflow-packs/install",
                "POST /api/workflow-packs/installations/{id}/stage",
                "POST /api/workflow-packs/installations/{id}/release",
                "POST /api/workflow-packs/installations/{id}/rollback",
                "POST /api/workflow-packs/installations/{id}/archive"
            ],
            "lifecycle_actions": ["validate", "install", "configure", "stage", "release", "rollback", "archive"],
            "evidence_events": [
                "workflow_pack.installed",
                "workflow_pack.staged",
                "workflow_pack.bindings_materialized",
                "workflow_pack.released",
                "workflow_pack.rolled_back",
                "workflow_pack.archived"
            ],
            "authority_boundary": "Admin-gated product capability; release and rollback require explicit gate evidence."
        }),
        json!({
            "key": "domain_pack",
            "product_object": "DomainPack",
            "status": "available",
            "api_routes": [
                "GET /api/workflow-packs/marketplace",
                "POST /api/workflow-packs/install",
                "GET /api/workflow-packs/installations/{id}/runtime-objects"
            ],
            "lifecycle_actions": ["validate", "install", "configure", "stage", "release", "rollback", "archive"],
            "evidence_events": [
                "workflow_pack.installed",
                "workflow_pack.staged",
                "workflow_pack.released",
                "workflow_pack.rolled_back"
            ],
            "authority_boundary": "Domain behavior is installed through WorkflowPack governance and cannot bypass runtime policy, approval, event logging, or audit."
        }),
        json!({
            "key": "agent_version",
            "product_object": "AgentVersion",
            "status": "available",
            "api_routes": [
                "GET /api/agents/{id}/versions",
                "GET /api/agents/{id}/versions/{version}",
                "GET /api/agents/{id}/versions/{version}/capability-readback",
                "GET /api/agents/{id}/releases",
                "POST /api/agents/{id}/release-requests",
                "POST /api/agents/{id}/releases/{release_id}/approve",
                "POST /api/agents/{id}/releases/{release_id}/rollback"
            ],
            "lifecycle_actions": ["version", "request_release", "approve", "reject", "validate_deployment", "rollback"],
            "evidence_events": [
                "agent.created",
                "agent.release_promotion_requested",
                "agent.release_promotion_approved",
                "agent.release_promotion_rejected",
                "agent.release_rolled_back"
            ],
            "authority_boundary": "Agent releases remain governed by release policy, approval, deployment validation, and rollback controllers."
        }),
        json!({
            "key": "environment_profile",
            "product_object": "EnvironmentProfile",
            "status": "available",
            "api_routes": [
                "GET /api/agent-runtime-profiles",
                "POST /api/agent-runtime-profiles",
                "GET /api/agent-runtime-profile-release-gates",
                "GET /api/agent-runtime-profiles/{id}/release-gate",
                "GET /api/agent-runtime-profiles/{id}/capability-readback",
                "GET /api/environments"
            ],
            "lifecycle_actions": ["create", "update", "archive", "evaluate_release_gate", "bind_environment"],
            "evidence_events": [
                "agent_runtime_profile.created",
                "agent_runtime_profile.updated",
                "agent_runtime_profile.archived",
                "environment.created",
                "environment.updated"
            ],
            "authority_boundary": "Environment profiles select runtime bindings; execution still flows through Managed Runtime, Tool Router, Policy, Approval, and Audit."
        }),
        json!({
            "key": "ontology_action_contract",
            "product_object": "OntologyActionContract",
            "status": "available",
            "api_routes": [
                "GET /api/ontology/action-contracts",
                "GET /api/ontology/action-contracts/{id}",
                "POST /api/ontology/action-contracts/{id}/release-candidate"
            ],
            "lifecycle_actions": ["list", "inspect", "package_release_candidate", "promote_release", "rollback_release"],
            "evidence_events": [
                "ontology_release.candidate_created",
                "ontology_release.promoted",
                "ontology_release.rolled_back"
            ],
            "authority_boundary": "Ontology grants business-action validity only; TaskGrant, Policy, Approval, connector scope, and Tool Router remain execution authority."
        }),
        json!({
            "key": "tool_spec",
            "product_object": "ToolSpec",
            "status": "available",
            "api_routes": [
                "GET /api/ontology/onboarding/runs/{id}/tool-specs",
                "GET /api/ontology/onboarding/runs/{id}/tool-specs/capability-readback",
                "POST /api/workflow-packs/validate"
            ],
            "lifecycle_actions": ["generate", "review", "materialize", "validate_pack_contract"],
            "evidence_events": [
                "ontology_onboarding.run_materialized",
                "workflow_pack.installed",
                "workflow_pack.staged"
            ],
            "authority_boundary": "Tool specs describe governed tool bindings; tool execution still requires TaskGrant, Policy, Approval when needed, and Tool Router enforcement."
        }),
        json!({
            "key": "eval_gate",
            "product_object": "EvalGate",
            "status": "available",
            "api_routes": [
                "POST /api/eval/suites/stage2-regression",
                "GET /api/eval/runs",
                "POST /api/eval/runs/{id}/gate",
                "GET /api/eval/runs/{id}/capability-readback",
                "GET /api/eval/runs/{id}/drift"
            ],
            "lifecycle_actions": ["bootstrap_suite", "run_eval", "gate", "inspect_drift"],
            "evidence_events": [
                "eval.suite_bootstrapped",
                "eval.judge_profile_saved"
            ],
            "decision_surfaces": ["POST /api/eval/runs/{id}/gate"],
            "authority_boundary": "Eval gates produce release evidence and do not execute business actions."
        }),
        json!({
            "key": "release",
            "product_object": "Release",
            "status": "available",
            "api_routes": [
                "POST /api/workflow-packs/installations/{id}/release",
                "POST /api/ontology/releases/{id}/promote",
                "POST /api/agents/{id}/releases/{release_id}/approve",
                "POST /api/policy/rollout/run-due"
            ],
            "lifecycle_actions": ["request", "gate", "approve", "promote", "activate"],
            "evidence_events": [
                "workflow_pack.released",
                "ontology_release.promoted",
                "agent.release_promotion_approved",
                "policy.rollout_due_run"
            ],
            "authority_boundary": "Release is a governance transition with auditable gate evidence, not an execution shortcut."
        }),
        json!({
            "key": "rollback",
            "product_object": "Rollback",
            "status": "available",
            "api_routes": [
                "POST /api/workflow-packs/installations/{id}/rollback",
                "POST /api/ontology/releases/{id}/rollback",
                "POST /api/agents/{id}/releases/{release_id}/rollback",
                "POST /api/policy/rollout/rollback"
            ],
            "lifecycle_actions": ["validate_target", "rollback", "record_gate_evidence", "audit"],
            "evidence_events": [
                "workflow_pack.rolled_back",
                "ontology_release.rolled_back",
                "agent.release_rolled_back",
                "policy.rollback_completed"
            ],
            "authority_boundary": "Rollback requires explicit target/gate evidence and records audit; it does not erase historical release evidence."
        }),
    ]
}
