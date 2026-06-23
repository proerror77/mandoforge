use std::sync::Arc;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    AppError, AppState, PolicyConfig, PolicyRevision, PolicyRuntime, PolicyRuntimeStatus,
    StagedPolicyRuntime,
};

pub(crate) fn runtime_policy(policy: PolicyConfig) -> Arc<RwLock<PolicyRuntime>> {
    Arc::new(RwLock::new(PolicyRuntime {
        active_revision_id: None,
        active: policy,
        staged: None,
    }))
}

fn session_rollout_bucket(session_id: Uuid) -> u8 {
    (session_id.as_u128() % 100) as u8
}

impl AppState {
    pub(crate) async fn active_policy(&self) -> PolicyConfig {
        self.policy.read().await.active.clone()
    }

    pub(crate) async fn policy_for_session(&self, session_id: Uuid) -> PolicyConfig {
        let runtime = self.policy.read().await;
        if let Some(staged) = runtime.staged.as_ref() {
            if session_rollout_bucket(session_id) < staged.rollout_percent {
                return staged.policy.clone();
            }
        }
        runtime.active.clone()
    }

    pub(crate) async fn activate_runtime_policy(
        &self,
        revision_id: Uuid,
        policy: PolicyConfig,
        rollout_percent: u8,
    ) {
        let mut runtime = self.policy.write().await;
        if rollout_percent >= 100 {
            runtime.active_revision_id = Some(revision_id);
            runtime.active = policy;
            runtime.staged = None;
        } else {
            runtime.staged = Some(StagedPolicyRuntime {
                revision_id,
                rollout_percent,
                policy,
            });
        }
    }

    pub(crate) async fn policy_runtime_status(&self) -> PolicyRuntimeStatus {
        let runtime = self.policy.read().await;
        PolicyRuntimeStatus {
            active_revision_id: runtime.active_revision_id,
            staged_revision_id: runtime.staged.as_ref().map(|staged| staged.revision_id),
            staged_rollout_percent: runtime.staged.as_ref().map(|staged| staged.rollout_percent),
            rollout_active: runtime.staged.is_some(),
        }
    }

    pub(crate) async fn cancel_staged_policy_rollout(
        &self,
    ) -> Result<PolicyRuntimeStatus, AppError> {
        let mut runtime = self.policy.write().await;
        if runtime.staged.take().is_none() {
            return Err(AppError::bad_request("no staged policy rollout is active"));
        }
        Ok(PolicyRuntimeStatus {
            active_revision_id: runtime.active_revision_id,
            staged_revision_id: None,
            staged_rollout_percent: None,
            rollout_active: false,
        })
    }

    pub(crate) async fn rollback_runtime_policy(
        &self,
        revision: &PolicyRevision,
    ) -> Result<(), AppError> {
        let policy = serde_json::from_value::<PolicyConfig>(revision.body.clone())
            .map_err(|error| AppError::bad_request(format!("invalid rollback policy: {error}")))?;
        let mut runtime = self.policy.write().await;
        runtime.active_revision_id = Some(revision.id);
        runtime.active = policy;
        runtime.staged = None;
        Ok(())
    }
}
