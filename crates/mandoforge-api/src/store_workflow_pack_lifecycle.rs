use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_releases::{
    AGENT_RELEASE_COLUMNS, insert_or_get_promoted_agent_release_tx, lock_agent_release_target_tx,
    new_workflow_pack_agent_release, require_workflow_pack_agent_version_tx,
};
use crate::store_rows::{
    agent_release_from_row, workflow_definition_from_row, workflow_pack_binding_from_row,
    workflow_pack_installation_from_row, workflow_pack_runtime_object_from_row,
};
use crate::{AgentRelease, AppError, AppState, AuditLog, WorkflowPackInstallation, new_audit_log};

pub(crate) enum WorkflowPackAgentReleaseTransition {
    RequirePromoted {
        targets: Vec<(Uuid, Uuid)>,
        environment: String,
    },
    PromoteFromPack {
        targets: Vec<(Uuid, Uuid)>,
        environment: String,
        promoted_by: String,
        gate_evidence: Value,
    },
    RollbackPackPromotions,
}

pub(crate) struct WorkflowPackLifecycleTransitionRequest {
    pub(crate) installation_id: Uuid,
    pub(crate) expected_status: String,
    pub(crate) next_status: String,
    pub(crate) eval_gate_status: String,
    pub(crate) release_gate_status: String,
    pub(crate) gate_evidence: Value,
    pub(crate) staged_at: Option<DateTime<Utc>>,
    pub(crate) released_at: Option<DateTime<Utc>>,
    pub(crate) occurred_at: DateTime<Utc>,
    pub(crate) audit_action: String,
    pub(crate) audit_details: Value,
    pub(crate) agent_release_transition: WorkflowPackAgentReleaseTransition,
}

enum PreparedAgentReleaseTransition {
    RequirePromoted {
        targets: Vec<(Uuid, Uuid)>,
        environment: String,
    },
    PromoteFromPack {
        targets: Vec<(Uuid, Uuid)>,
        environment: String,
        promoted_by: String,
        gate_evidence: Value,
    },
    RollbackPackPromotions,
}

impl AppState {
    pub(crate) async fn transition_workflow_pack_lifecycle(
        &self,
        request: WorkflowPackLifecycleTransitionRequest,
    ) -> Result<WorkflowPackInstallation, AppError> {
        validate_workflow_pack_lifecycle_transition(
            &request.expected_status,
            &request.next_status,
        )?;
        if !request.audit_details.is_object() {
            return Err(AppError::bad_request(
                "workflow pack lifecycle audit details must be a JSON object",
            ));
        }
        let release_transition =
            prepare_agent_release_transition(request.agent_release_transition)?;
        if matches!(
            release_transition,
            PreparedAgentReleaseTransition::PromoteFromPack { .. }
        ) && crate::agent_release_enforcement_required()
        {
            return Err(AppError::forbidden(
                "production workflow pack releases require independently promoted AgentRelease records",
            ));
        }

        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let current = store
                    .workflow_pack_installations
                    .get(&request.installation_id)
                    .filter(|installation| installation.archived_at.is_none())
                    .cloned()
                    .ok_or_else(|| AppError::not_found("workflow pack installation not found"))?;
                if current.status != request.expected_status {
                    return Err(workflow_pack_status_conflict());
                }

                let required_releases = match &release_transition {
                    PreparedAgentReleaseTransition::RequirePromoted {
                        targets,
                        environment,
                    } => {
                        let mut releases = Vec::with_capacity(targets.len());
                        for (agent_id, agent_version_id) in targets {
                            let release = store
                                .agent_releases
                                .values()
                                .find(|release| {
                                    release.agent_id == *agent_id
                                        && release.agent_version_id == *agent_version_id
                                        && release.status == "promoted"
                                        && release.environment.eq_ignore_ascii_case(environment)
                                        && release.automation_policy["source"]
                                            != "workflow_pack_release"
                                })
                                .cloned()
                                .ok_or_else(|| {
                                    missing_independent_agent_release(
                                        *agent_version_id,
                                        environment,
                                    )
                                })?;
                            releases.push(release);
                        }
                        releases
                    }
                    PreparedAgentReleaseTransition::PromoteFromPack { targets, .. } => {
                        for (agent_id, agent_version_id) in targets {
                            let version_exists =
                                store.agent_versions.get(agent_id).is_some_and(|versions| {
                                    versions
                                        .iter()
                                        .any(|version| version.id == *agent_version_id)
                                });
                            if !version_exists {
                                return Err(AppError::bad_request(
                                    "workflow pack agent binding targets an unknown agent version",
                                ));
                            }
                        }
                        Vec::new()
                    }
                    PreparedAgentReleaseTransition::RollbackPackPromotions => Vec::new(),
                };

                let installation = store
                    .workflow_pack_installations
                    .get_mut(&request.installation_id)
                    .expect("workflow pack installation checked before lifecycle transition");
                installation.status = request.next_status.clone();
                installation.eval_gate_status = request.eval_gate_status.clone();
                installation.release_gate_status = request.release_gate_status.clone();
                installation.gate_evidence = request.gate_evidence.clone();
                installation.staged_at = request.staged_at;
                installation.released_at = request.released_at;
                if request.next_status == "archived" {
                    installation.archived_at = Some(request.occurred_at);
                }
                installation.updated_at = request.occurred_at;
                let installation = installation.clone();

                let mut definitions = Vec::new();
                for definition in store.workflow_definitions.values_mut() {
                    if definition.pack_installation_id == Some(request.installation_id)
                        && definition.archived_at.is_none()
                    {
                        definition.release_state = request.next_status.clone();
                        definition.updated_at = request.occurred_at;
                        if request.next_status == "archived" {
                            definition.archived_at = Some(request.occurred_at);
                        }
                        definitions.push(definition.clone());
                    }
                }
                definitions.sort_by_key(|definition| definition.created_at);

                let mut bindings = Vec::new();
                for binding in store.workflow_pack_bindings.values_mut() {
                    if binding.installation_id == request.installation_id
                        && binding.status != "superseded"
                    {
                        binding.status = request.next_status.clone();
                        binding.updated_at = request.occurred_at;
                        bindings.push(binding.clone());
                    }
                }
                bindings.sort_by(|left, right| {
                    left.binding_type
                        .cmp(&right.binding_type)
                        .then(left.binding_key.cmp(&right.binding_key))
                });

                let mut runtime_objects = Vec::new();
                for object in store.workflow_pack_runtime_objects.values_mut() {
                    if object.installation_id == request.installation_id
                        && object.status != "superseded"
                    {
                        object.status = request.next_status.clone();
                        object.updated_at = request.occurred_at;
                        runtime_objects.push(object.clone());
                    }
                }
                runtime_objects.sort_by(|left, right| {
                    left.object_type
                        .cmp(&right.object_type)
                        .then(left.object_key.cmp(&right.object_key))
                });

                if matches!(request.next_status.as_str(), "rolled_back" | "archived") {
                    let owned_version_ids = store
                        .workflow_pack_bindings
                        .values()
                        .filter(|binding| {
                            binding.installation_id == request.installation_id
                                && binding.binding_type == "agent"
                        })
                        .filter_map(|binding| binding.target_id)
                        .collect::<BTreeSet<_>>();
                    let owned_agent_ids = store
                        .agent_versions
                        .iter()
                        .filter(|(_, versions)| {
                            versions
                                .iter()
                                .any(|version| owned_version_ids.contains(&version.id))
                        })
                        .map(|(agent_id, _)| *agent_id)
                        .collect::<Vec<_>>();
                    for agent_id in owned_agent_ids {
                        if let Some(agent) = store.agents.get_mut(&agent_id) {
                            agent.release_state = "disabled".to_string();
                        }
                    }
                }

                let mut agent_releases = match &release_transition {
                    PreparedAgentReleaseTransition::RequirePromoted { .. } => required_releases,
                    PreparedAgentReleaseTransition::PromoteFromPack {
                        targets,
                        environment,
                        promoted_by,
                        gate_evidence,
                    } => {
                        let mut releases = Vec::with_capacity(targets.len());
                        for (agent_id, agent_version_id) in targets {
                            let existing_id = store
                                .agent_releases
                                .values()
                                .find(|release| {
                                    release.agent_id == *agent_id
                                        && release.agent_version_id == *agent_version_id
                                        && release.status == "promoted"
                                        && release.environment.eq_ignore_ascii_case(environment)
                                })
                                .map(|release| release.id);
                            if let Some(existing_id) = existing_id {
                                let release = store
                                    .agent_releases
                                    .get_mut(&existing_id)
                                    .expect("promoted AgentRelease selected from store");
                                add_workflow_pack_release_reference(
                                    release,
                                    request.installation_id,
                                );
                                releases.push(release.clone());
                                continue;
                            }
                            let release = new_workflow_pack_agent_release(
                                request.installation_id,
                                *agent_id,
                                *agent_version_id,
                                environment,
                                promoted_by,
                                gate_evidence,
                                request.occurred_at,
                            );
                            store.agent_releases.insert(release.id, release.clone());
                            releases.push(release);
                        }
                        releases
                    }
                    PreparedAgentReleaseTransition::RollbackPackPromotions => {
                        let mut releases = Vec::new();
                        for release in store.agent_releases.values_mut() {
                            if release.status == "promoted"
                                && release.automation_policy["source"] == "workflow_pack_release"
                                && workflow_pack_release_references(release)
                                    .contains(&request.installation_id)
                            {
                                let remaining = remove_workflow_pack_release_reference(
                                    release,
                                    request.installation_id,
                                );
                                if remaining == 0 {
                                    release.status = "rolled_back".to_string();
                                }
                                releases.push(release.clone());
                            }
                        }
                        releases
                    }
                };
                agent_releases.sort_by_key(|release| release.id);

                let audit_log = workflow_pack_lifecycle_audit(
                    &installation,
                    &request.audit_action,
                    request.audit_details,
                    definitions.len(),
                    bindings.len(),
                    runtime_objects.len(),
                    &agent_releases,
                );
                store.audit_logs.insert(audit_log.id, audit_log);
                Ok(installation)
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let required_releases = match &release_transition {
                    PreparedAgentReleaseTransition::RequirePromoted {
                        targets,
                        environment,
                    } => {
                        let mut releases = Vec::with_capacity(targets.len());
                        for (agent_id, agent_version_id) in targets {
                            let sql = format!(
                                "SELECT {AGENT_RELEASE_COLUMNS}
                                 FROM agent_releases
                                 WHERE tenant_id = $1
                                   AND agent_id = $2
                                   AND agent_version_id = $3
                                   AND lower(environment) = lower($4)
                                   AND status = 'promoted'
                                   AND COALESCE(automation_policy ->> 'source', '') <> 'workflow_pack_release'
                                 LIMIT 1
                                 FOR SHARE"
                            );
                            let row = sqlx::query(&sql)
                                .bind(self.current_tenant_id())
                                .bind(agent_id)
                                .bind(agent_version_id)
                                .bind(environment)
                                .fetch_optional(&mut *tx)
                                .await?
                                .ok_or_else(|| {
                                    missing_independent_agent_release(
                                        *agent_version_id,
                                        environment,
                                    )
                                })?;
                            releases.push(agent_release_from_row(row)?);
                        }
                        releases
                    }
                    PreparedAgentReleaseTransition::PromoteFromPack {
                        targets,
                        environment,
                        ..
                    } => {
                        for (agent_id, agent_version_id) in targets {
                            require_workflow_pack_agent_version_tx(
                                &mut tx,
                                self.current_tenant_id(),
                                *agent_id,
                                *agent_version_id,
                            )
                            .await?;
                        }
                        for (agent_id, agent_version_id) in targets {
                            lock_agent_release_target_tx(
                                &mut tx,
                                self.current_tenant_id(),
                                *agent_id,
                                *agent_version_id,
                                environment,
                            )
                            .await?;
                        }
                        Vec::new()
                    }
                    PreparedAgentReleaseTransition::RollbackPackPromotions => Vec::new(),
                };

                let installation_row = sqlx::query(
                    "UPDATE workflow_pack_installations
                     SET status = $4,
                         eval_gate_status = $5,
                         release_gate_status = $6,
                         gate_evidence = $7,
                         staged_at = $8,
                         released_at = $9,
                         archived_at = CASE WHEN $4 = 'archived' THEN $10 ELSE archived_at END,
                         updated_at = $10
                     WHERE tenant_id = $1
                       AND id = $2
                       AND archived_at IS NULL
                       AND status = $3
                     RETURNING id, pack_id, kind, version, manifest_path, manifest, validation_report, status, eval_gate_status, release_gate_status, gate_evidence, staged_at, released_at, archived_at, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(request.installation_id)
                .bind(&request.expected_status)
                .bind(&request.next_status)
                .bind(&request.eval_gate_status)
                .bind(&request.release_gate_status)
                .bind(&request.gate_evidence)
                .bind(request.staged_at)
                .bind(request.released_at)
                .bind(request.occurred_at)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(workflow_pack_status_conflict)?;
                let installation = workflow_pack_installation_from_row(installation_row)?;

                let definition_rows = sqlx::query(
                    "UPDATE workflow_definitions
                     SET release_state = $3,
                         updated_at = $4,
                         archived_at = CASE WHEN $3 = 'archived' THEN $4 ELSE archived_at END
                     WHERE tenant_id = $1
                       AND pack_installation_id = $2
                       AND archived_at IS NULL
                     RETURNING id, pack_installation_id, pack_id, pack_version, name, entrypoint, trigger_type, default_agent_id, default_environment_id, input_schema_ref, output_schema_ref, step_graph, handoff_rules, execution_strategy, runtime_adapter, runtime_mode, runtime_capability_contract, event_ingestion_policy, approval_policy_ref, eval_gate_refs, release_state, created_at, updated_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(request.installation_id)
                .bind(&request.next_status)
                .bind(request.occurred_at)
                .fetch_all(&mut *tx)
                .await?;
                let definitions = definition_rows
                    .into_iter()
                    .map(workflow_definition_from_row)
                    .collect::<Result<Vec<_>, _>>()?;

                let binding_rows = sqlx::query(
                    "UPDATE workflow_pack_bindings
                     SET status = $3, updated_at = $4
                     WHERE tenant_id = $1
                       AND installation_id = $2
                       AND status <> 'superseded'
                     RETURNING id, installation_id, pack_id, pack_version, binding_type, binding_key, source_path, target_kind, target_id, status, materialized_payload, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(request.installation_id)
                .bind(&request.next_status)
                .bind(request.occurred_at)
                .fetch_all(&mut *tx)
                .await?;
                let bindings = binding_rows
                    .into_iter()
                    .map(workflow_pack_binding_from_row)
                    .collect::<Result<Vec<_>, _>>()?;

                if matches!(request.next_status.as_str(), "rolled_back" | "archived") {
                    sqlx::query(
                        "UPDATE agents AS agents
                         SET release_state = 'disabled'
                         FROM workflow_pack_bindings AS bindings
                         INNER JOIN agent_versions AS versions ON versions.id = bindings.target_id
                         WHERE agents.tenant_id = $1
                           AND bindings.tenant_id = $1
                           AND bindings.installation_id = $2
                           AND bindings.binding_type = 'agent'
                           AND agents.id = versions.agent_id
                           AND agents.archived_at IS NULL",
                    )
                    .bind(self.current_tenant_id())
                    .bind(request.installation_id)
                    .execute(&mut *tx)
                    .await?;
                }

                let runtime_object_rows = sqlx::query(
                    "UPDATE workflow_pack_runtime_objects
                     SET status = $3, updated_at = $4
                     WHERE tenant_id = $1
                       AND installation_id = $2
                       AND status <> 'superseded'
                     RETURNING id, installation_id, binding_id, pack_id, pack_version, object_type, object_key, runtime_kind, status, spec, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(request.installation_id)
                .bind(&request.next_status)
                .bind(request.occurred_at)
                .fetch_all(&mut *tx)
                .await?;
                let runtime_objects = runtime_object_rows
                    .into_iter()
                    .map(workflow_pack_runtime_object_from_row)
                    .collect::<Result<Vec<_>, _>>()?;

                let agent_releases = match &release_transition {
                    PreparedAgentReleaseTransition::RequirePromoted { .. } => required_releases,
                    PreparedAgentReleaseTransition::PromoteFromPack {
                        targets,
                        environment,
                        promoted_by,
                        gate_evidence,
                    } => {
                        let mut releases = Vec::with_capacity(targets.len());
                        for (agent_id, agent_version_id) in targets {
                            let candidate = new_workflow_pack_agent_release(
                                request.installation_id,
                                *agent_id,
                                *agent_version_id,
                                environment,
                                promoted_by,
                                gate_evidence,
                                request.occurred_at,
                            );
                            let release = insert_or_get_promoted_agent_release_tx(
                                &mut tx,
                                self.current_tenant_id(),
                                &candidate,
                            )
                            .await?;
                            releases.push(
                                add_workflow_pack_release_reference_tx(
                                    &mut tx,
                                    self.current_tenant_id(),
                                    release,
                                    request.installation_id,
                                )
                                .await?,
                            );
                        }
                        releases
                    }
                    PreparedAgentReleaseTransition::RollbackPackPromotions => {
                        let sql = format!(
                            "SELECT {AGENT_RELEASE_COLUMNS}
                             FROM agent_releases
                             WHERE tenant_id = $1
                               AND status = 'promoted'
                               AND automation_policy ->> 'source' = 'workflow_pack_release'
                               AND (
                                   (
                                       jsonb_typeof(automation_policy -> 'workflow_pack_installation_ids') = 'array'
                                       AND (automation_policy -> 'workflow_pack_installation_ids') ? $2
                                   )
                                   OR (
                                       NOT (automation_policy ? 'workflow_pack_installation_ids')
                                       AND automation_policy ->> 'workflow_pack_installation_id' = $2
                                   )
                               )
                             FOR UPDATE"
                        );
                        let rows = sqlx::query(&sql)
                            .bind(self.current_tenant_id())
                            .bind(request.installation_id.to_string())
                            .fetch_all(&mut *tx)
                            .await?;
                        let releases = rows
                            .into_iter()
                            .map(agent_release_from_row)
                            .collect::<Result<Vec<_>, _>>()?;
                        let mut updated = Vec::with_capacity(releases.len());
                        for mut release in releases {
                            let remaining = remove_workflow_pack_release_reference(
                                &mut release,
                                request.installation_id,
                            );
                            if remaining == 0 {
                                release.status = "rolled_back".to_string();
                            }
                            let update_sql = format!(
                                "UPDATE agent_releases
                                 SET status = $3, automation_policy = $4
                                 WHERE tenant_id = $1 AND id = $2 AND status = 'promoted'
                                 RETURNING {AGENT_RELEASE_COLUMNS}"
                            );
                            let row = sqlx::query(&update_sql)
                                .bind(self.current_tenant_id())
                                .bind(release.id)
                                .bind(&release.status)
                                .bind(&release.automation_policy)
                                .fetch_optional(&mut *tx)
                                .await?
                                .ok_or_else(|| {
                                    AppError::conflict(
                                        "workflow pack AgentRelease changed during rollback",
                                    )
                                })?;
                            updated.push(agent_release_from_row(row)?);
                        }
                        updated
                    }
                };

                let audit_log = workflow_pack_lifecycle_audit(
                    &installation,
                    &request.audit_action,
                    request.audit_details,
                    definitions.len(),
                    bindings.len(),
                    runtime_objects.len(),
                    &agent_releases,
                );
                insert_audit_log_tx(&mut tx, self.current_tenant_id(), &audit_log).await?;
                tx.commit().await?;
                Ok(installation)
            }
        }
    }
}

fn workflow_pack_release_references(release: &AgentRelease) -> BTreeSet<Uuid> {
    if release.automation_policy["source"] != "workflow_pack_release" {
        return BTreeSet::new();
    }
    if let Some(values) = release
        .automation_policy
        .get("workflow_pack_installation_ids")
        .and_then(Value::as_array)
    {
        return values
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|value| Uuid::parse_str(value).ok())
            .collect();
    }
    release
        .automation_policy
        .get("workflow_pack_installation_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .into_iter()
        .collect()
}

fn set_workflow_pack_release_references(release: &mut AgentRelease, references: &BTreeSet<Uuid>) {
    let Some(policy) = release.automation_policy.as_object_mut() else {
        return;
    };
    policy.insert(
        "workflow_pack_installation_ids".to_string(),
        json!(references.iter().copied().collect::<Vec<_>>()),
    );
}

fn add_workflow_pack_release_reference(release: &mut AgentRelease, installation_id: Uuid) {
    if release.automation_policy["source"] != "workflow_pack_release" {
        return;
    }
    let mut references = workflow_pack_release_references(release);
    references.insert(installation_id);
    set_workflow_pack_release_references(release, &references);
}

fn remove_workflow_pack_release_reference(
    release: &mut AgentRelease,
    installation_id: Uuid,
) -> usize {
    let mut references = workflow_pack_release_references(release);
    references.remove(&installation_id);
    set_workflow_pack_release_references(release, &references);
    references.len()
}

async fn add_workflow_pack_release_reference_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    release: AgentRelease,
    installation_id: Uuid,
) -> Result<AgentRelease, AppError> {
    if release.automation_policy["source"] != "workflow_pack_release" {
        return Ok(release);
    }
    let select_sql = format!(
        "SELECT {AGENT_RELEASE_COLUMNS}
         FROM agent_releases
         WHERE tenant_id = $1 AND id = $2 AND status = 'promoted'
         FOR UPDATE"
    );
    let row = sqlx::query(&select_sql)
        .bind(tenant_id)
        .bind(release.id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| {
            AppError::conflict("workflow pack AgentRelease changed during release association")
        })?;
    let mut release = agent_release_from_row(row)?;
    let existing_references = workflow_pack_release_references(&release);
    if existing_references.contains(&installation_id) {
        return Ok(release);
    }
    add_workflow_pack_release_reference(&mut release, installation_id);
    let update_sql = format!(
        "UPDATE agent_releases
         SET automation_policy = $3
         WHERE tenant_id = $1 AND id = $2 AND status = 'promoted'
         RETURNING {AGENT_RELEASE_COLUMNS}"
    );
    let row = sqlx::query(&update_sql)
        .bind(tenant_id)
        .bind(release.id)
        .bind(&release.automation_policy)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| {
            AppError::conflict("workflow pack AgentRelease changed during release association")
        })?;
    agent_release_from_row(row)
}

fn prepare_agent_release_transition(
    transition: WorkflowPackAgentReleaseTransition,
) -> Result<PreparedAgentReleaseTransition, AppError> {
    match transition {
        WorkflowPackAgentReleaseTransition::RequirePromoted {
            targets,
            environment,
        } => Ok(PreparedAgentReleaseTransition::RequirePromoted {
            targets: normalize_agent_release_targets(targets)?,
            environment: normalize_agent_release_environment(&environment)?,
        }),
        WorkflowPackAgentReleaseTransition::PromoteFromPack {
            targets,
            environment,
            promoted_by,
            gate_evidence,
        } => Ok(PreparedAgentReleaseTransition::PromoteFromPack {
            targets: normalize_agent_release_targets(targets)?,
            environment: normalize_agent_release_environment(&environment)?,
            promoted_by,
            gate_evidence,
        }),
        WorkflowPackAgentReleaseTransition::RollbackPackPromotions => {
            Ok(PreparedAgentReleaseTransition::RollbackPackPromotions)
        }
    }
}

fn normalize_agent_release_targets(
    targets: Vec<(Uuid, Uuid)>,
) -> Result<Vec<(Uuid, Uuid)>, AppError> {
    let targets = targets.into_iter().collect::<BTreeSet<_>>();
    if targets.is_empty() {
        return Err(AppError::bad_request(
            "workflow pack release requires materialized agent versions",
        ));
    }
    Ok(targets.into_iter().collect())
}

fn normalize_agent_release_environment(environment: &str) -> Result<String, AppError> {
    let environment = environment.trim().to_ascii_lowercase();
    if environment.is_empty() {
        return Err(AppError::bad_request(
            "workflow pack release environment is required",
        ));
    }
    Ok(environment)
}

fn validate_workflow_pack_lifecycle_transition(
    expected_status: &str,
    next_status: &str,
) -> Result<(), AppError> {
    let valid = matches!(
        (expected_status, next_status),
        ("staged", "released")
            | ("released", "rolled_back")
            | (
                "installed" | "staged" | "released" | "rolled_back",
                "archived"
            )
    );
    if !valid {
        return Err(AppError::bad_request(
            "invalid workflow pack lifecycle transition",
        ));
    }
    Ok(())
}

fn workflow_pack_status_conflict() -> AppError {
    AppError::bad_request("workflow pack installation status conflict: concurrent update detected")
}

fn missing_independent_agent_release(agent_version_id: Uuid, environment: &str) -> AppError {
    AppError::bad_request(format!(
        "workflow pack production release requires independently promoted agent version {agent_version_id} for environment {environment}"
    ))
}

fn workflow_pack_lifecycle_audit(
    installation: &WorkflowPackInstallation,
    action: &str,
    mut details: Value,
    workflow_definition_count: usize,
    binding_count: usize,
    runtime_object_count: usize,
    agent_releases: &[AgentRelease],
) -> AuditLog {
    let detail_object = details
        .as_object_mut()
        .expect("workflow pack lifecycle audit details validated before mutation");
    detail_object.insert(
        "workflow_definition_count".to_string(),
        json!(workflow_definition_count),
    );
    detail_object.insert("binding_count".to_string(), json!(binding_count));
    detail_object.insert(
        "runtime_object_count".to_string(),
        json!(runtime_object_count),
    );
    detail_object.insert(
        "agent_release_count".to_string(),
        json!(agent_releases.len()),
    );
    detail_object.insert(
        "agent_release_ids".to_string(),
        json!(
            agent_releases
                .iter()
                .map(|release| release.id)
                .collect::<Vec<_>>()
        ),
    );
    new_audit_log(
        None,
        "user",
        None,
        action,
        "workflow_pack_installation",
        Some(installation.id),
        json!({
            "pack_id": installation.pack_id,
            "kind": installation.kind,
            "version": installation.version,
            "status": installation.status,
            "eval_gate_status": installation.eval_gate_status,
            "release_gate_status": installation.release_gate_status,
            "details": details,
        }),
    )
}

async fn insert_audit_log_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    audit_log: &AuditLog,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO audit_logs
            (id, tenant_id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(audit_log.id)
    .bind(tenant_id)
    .bind(audit_log.session_id)
    .bind(&audit_log.actor_type)
    .bind(audit_log.actor_id)
    .bind(&audit_log.action)
    .bind(&audit_log.resource_type)
    .bind(audit_log.resource_id)
    .bind(&audit_log.details)
    .bind(audit_log.created_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
