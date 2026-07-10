use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::{Agent, AppError, AppState, Environment};

impl AppState {
    pub(crate) async fn seed_demo_agent(&self) -> Result<(), AppError> {
        let agent = Agent {
            id: Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid"),
            name: "Generic Orchestrator Agent".to_string(),
            kind: "orchestrator".to_string(),
            team_id: None,
            project_id: None,
            runtime_profile_id: None,
            agent_role: "manager".to_string(),
            provider: "openai-compatible".to_string(),
            model: "gpt-5.5-mini".to_string(),
            system_prompt: "You are a general-purpose orchestrator. Use tools through the runtime only, request approval before risky actions, and preserve an auditable timeline.".to_string(),
            tools: vec![
                "file.read".to_string(),
                "file.write".to_string(),
                "sql.get_schema".to_string(),
                "sql.query".to_string(),
                "shell.exec".to_string(),
                "codex.exec".to_string(),
                "agent_cli.exec".to_string(),
                "approval.request".to_string(),
                "artifact.create".to_string(),
                "mcp.call".to_string(),
            ],
            tool_policy: json!({}),
            mcp_server_ids: Vec::new(),
            skill_ids: Vec::new(),
            workflow_pack_ids: Vec::new(),
            remote_computer_profile: json!({}),
            semantic_scopes: json!({}),
            release_state: "active".to_string(),
            created_at: Utc::now(),
        };

        match &self.store {
            StoreBackend::Memory(inner) => {
                inner.write().await.agents.insert(agent.id, agent.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO agents
                        (id, tenant_id, name, kind, provider, model, system_prompt, tools, runtime_profile_id, agent_role, tool_policy, mcp_server_ids, skill_ids, workflow_pack_ids, remote_computer_profile, semantic_scopes, release_state, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                     ON CONFLICT (id) DO NOTHING",
                )
                .bind(agent.id)
                .bind(self.current_tenant_id())
                .bind(&agent.name)
                .bind(&agent.kind)
                .bind(&agent.provider)
                .bind(&agent.model)
                .bind(&agent.system_prompt)
                .bind(json!(agent.tools))
                .bind(agent.runtime_profile_id)
                .bind(&agent.agent_role)
                .bind(&agent.tool_policy)
                .bind(json!(agent.mcp_server_ids))
                .bind(json!(agent.skill_ids))
                .bind(json!(agent.workflow_pack_ids))
                .bind(&agent.remote_computer_profile)
                .bind(&agent.semantic_scopes)
                .bind(&agent.release_state)
                .bind(agent.created_at)
                .execute(pool)
                .await?;
            }
        }
        self.insert_agent_version(&agent, 1, serde_json::json!({}))
            .await?;
        self.seed_default_environment().await?;
        Ok(())
    }

    async fn seed_default_environment(&self) -> Result<(), AppError> {
        let now = Utc::now();
        let release_environment = crate::store_entities::configured_agent_release_environment()
            .unwrap_or_else(|| "development".to_string());
        let environment_id = match &self.store {
            StoreBackend::Memory(_) => {
                Uuid::parse_str("22222222-2222-4222-8222-222222222222").expect("valid uuid")
            }
            StoreBackend::Postgres(_) => Uuid::new_v4(),
        };
        let environment = Environment {
            id: environment_id,
            name: "Local Worker".to_string(),
            environment_type: "local".to_string(),
            runtime_profile_id: None,
            remote_computer_profile: json!({}),
            codex_app_server_profile: json!({}),
            worker_queue_binding: json!({
                "queue": "managed-agent",
                "release_environment": release_environment.clone(),
            }),
            state_mounts: json!({}),
            network_policy: json!({}),
            vault_requirements: json!({}),
            mcp_requirements: json!({}),
            release_state: "active".to_string(),
            status: "enabled".to_string(),
            created_at: now,
            updated_at: now,
            archived_at: None,
        };

        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .environments
                    .insert(environment.id, environment);
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "UPDATE environments
                     SET worker_queue_binding = jsonb_set(
                             worker_queue_binding,
                             '{release_environment}',
                             to_jsonb($2::text),
                             true
                         ),
                         updated_at = $3
                     WHERE tenant_id = $1
                       AND archived_at IS NULL
                       AND (
                           worker_queue_binding->>'release_environment' IS NULL
                           OR btrim(worker_queue_binding->>'release_environment') = ''
                       )",
                )
                .bind(self.current_tenant_id())
                .bind(&release_environment)
                .bind(now)
                .execute(pool)
                .await?;
                sqlx::query(
                    "INSERT INTO environments
                        (id, tenant_id, name, environment_type, runtime_profile_id,
                         remote_computer_profile, codex_app_server_profile, worker_queue_binding,
                         state_mounts, network_policy, vault_requirements, mcp_requirements,
                         release_state, status, created_at, updated_at, archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, NULL)
                     ON CONFLICT DO NOTHING",
                )
                .bind(environment.id)
                .bind(self.current_tenant_id())
                .bind(&environment.name)
                .bind(&environment.environment_type)
                .bind(environment.runtime_profile_id)
                .bind(&environment.remote_computer_profile)
                .bind(&environment.codex_app_server_profile)
                .bind(&environment.worker_queue_binding)
                .bind(&environment.state_mounts)
                .bind(&environment.network_policy)
                .bind(&environment.vault_requirements)
                .bind(&environment.mcp_requirements)
                .bind(&environment.release_state)
                .bind(&environment.status)
                .bind(environment.created_at)
                .bind(environment.updated_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }
}
