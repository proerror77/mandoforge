use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::cost_alert_route_from_row;
use crate::{AppError, AppState, CostAlertRoute, CreateCostAlertRoute};

impl AppState {
    pub(crate) async fn list_cost_alert_routes(&self) -> Result<Vec<CostAlertRoute>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut routes: Vec<_> = inner
                    .read()
                    .await
                    .cost_alert_routes
                    .values()
                    .cloned()
                    .collect();
                routes.sort_by_key(|route| route.created_at);
                routes.reverse();
                Ok(routes)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, channel, target, severity_filter, status, created_at
                     FROM cost_alert_routes
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(cost_alert_route_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_cost_alert_route(
        &self,
        input: CreateCostAlertRoute,
    ) -> Result<CostAlertRoute, AppError> {
        let route = CostAlertRoute {
            id: Uuid::new_v4(),
            name: input.name,
            channel: input.channel,
            target: input.target,
            severity_filter: input.severity_filter,
            status: "active".to_string(),
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store
                    .cost_alert_routes
                    .values()
                    .any(|existing| existing.name == route.name)
                {
                    return Err(AppError::bad_request(
                        "cost alert route name already exists",
                    ));
                }
                store.cost_alert_routes.insert(route.id, route.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO cost_alert_routes (id, tenant_id, name, channel, target, severity_filter, status, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                )
                .bind(route.id)
                .bind(self.tenant_id)
                .bind(&route.name)
                .bind(&route.channel)
                .bind(&route.target)
                .bind(&route.severity_filter)
                .bind(&route.status)
                .bind(route.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(route)
    }
}
