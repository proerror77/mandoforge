use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::{Agent, AppError, AppState};

impl AppState {
    pub(crate) async fn seed_demo_agent(&self) -> Result<(), AppError> {
        let agent = Agent {
            id: Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid"),
            name: "Generic Orchestrator Agent".to_string(),
            kind: "orchestrator".to_string(),
            team_id: None,
            project_id: None,
            provider: "openai-compatible".to_string(),
            model: "gpt-5.4-mini".to_string(),
            system_prompt: "You are a general-purpose orchestrator. Use tools through the runtime only, request approval before risky actions, and preserve an auditable timeline.".to_string(),
            tools: vec![
                "file.read".to_string(),
                "file.write".to_string(),
                "sql.get_schema".to_string(),
                "sql.query".to_string(),
                "shell.exec".to_string(),
                "codex.exec".to_string(),
                "approval.request".to_string(),
                "artifact.create".to_string(),
            ],
            created_at: Utc::now(),
        };

        match &self.store {
            StoreBackend::Memory(inner) => {
                inner.write().await.agents.insert(agent.id, agent.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO agents (id, tenant_id, name, kind, provider, model, system_prompt, tools, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                     ON CONFLICT (id) DO NOTHING",
                )
                .bind(agent.id)
                .bind(self.tenant_id)
                .bind(&agent.name)
                .bind(&agent.kind)
                .bind(&agent.provider)
                .bind(&agent.model)
                .bind(&agent.system_prompt)
                .bind(json!(agent.tools))
                .bind(agent.created_at)
                .execute(pool)
                .await?;
            }
        }
        self.insert_agent_version(&agent, 1).await?;
        Ok(())
    }
}
