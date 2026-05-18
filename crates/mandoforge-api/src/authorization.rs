use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Role {
    Admin,
    Operator,
    Approver,
    Viewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Permission {
    AgentsRead,
    AgentsWrite,
    SessionsRead,
    SessionsRun,
    SessionsWrite,
    ToolsExecute,
    ApprovalsDecide,
    ExecutionJobsRun,
    AuditRead,
    Admin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct Principal {
    pub(crate) tenant_id: Uuid,
    pub(crate) subject_id: String,
    pub(crate) roles: Vec<Role>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct AuthorizationRequest {
    pub(crate) tenant_id: Uuid,
    pub(crate) permission: Permission,
    pub(crate) resource_type: String,
    pub(crate) resource_id: Option<Uuid>,
}

#[async_trait]
#[allow(dead_code)]
pub(crate) trait Authorizer: Send + Sync {
    async fn authorize(
        &self,
        principal: &Principal,
        request: &AuthorizationRequest,
    ) -> Result<(), AppError>;
}

#[allow(dead_code)]
pub(crate) struct RoleBasedAuthorizer;

#[async_trait]
impl Authorizer for RoleBasedAuthorizer {
    async fn authorize(
        &self,
        principal: &Principal,
        request: &AuthorizationRequest,
    ) -> Result<(), AppError> {
        if principal.tenant_id != request.tenant_id {
            return Err(AppError::forbidden(
                "principal tenant does not match requested tenant",
            ));
        }
        if principal
            .roles
            .iter()
            .any(|role| role_allows_permission(*role, request.permission))
        {
            return Ok(());
        }

        Err(AppError::forbidden(format!(
            "principal {} is not allowed to {:?} {}",
            principal.subject_id, request.permission, request.resource_type
        )))
    }
}

#[allow(dead_code)]
fn role_allows_permission(role: Role, permission: Permission) -> bool {
    match role {
        Role::Admin => true,
        Role::Operator => matches!(
            permission,
            Permission::AgentsRead
                | Permission::SessionsRead
                | Permission::SessionsRun
                | Permission::SessionsWrite
                | Permission::ToolsExecute
                | Permission::ExecutionJobsRun
                | Permission::AuditRead
        ),
        Role::Approver => matches!(
            permission,
            Permission::AgentsRead
                | Permission::SessionsRead
                | Permission::ApprovalsDecide
                | Permission::AuditRead
        ),
        Role::Viewer => matches!(
            permission,
            Permission::AgentsRead | Permission::SessionsRead | Permission::AuditRead
        ),
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{
        AuthorizationRequest, Authorizer, Permission, Principal, Role, RoleBasedAuthorizer,
        role_allows_permission,
    };

    #[test]
    fn role_permissions_are_explicit() {
        assert!(role_allows_permission(Role::Admin, Permission::Admin));
        assert!(!role_allows_permission(
            Role::Operator,
            Permission::ApprovalsDecide
        ));
        assert!(role_allows_permission(
            Role::Approver,
            Permission::ApprovalsDecide
        ));
        assert!(!role_allows_permission(
            Role::Approver,
            Permission::ToolsExecute
        ));
        assert!(role_allows_permission(
            Role::Operator,
            Permission::ExecutionJobsRun
        ));
        assert!(role_allows_permission(
            Role::Operator,
            Permission::SessionsRun
        ));
        assert!(role_allows_permission(
            Role::Viewer,
            Permission::SessionsRead
        ));
        assert!(!role_allows_permission(
            Role::Viewer,
            Permission::ToolsExecute
        ));
        assert!(!role_allows_permission(
            Role::Operator,
            Permission::AgentsWrite
        ));
    }

    #[tokio::test]
    async fn role_authorizer_allows_matching_tenant_and_permission() {
        let tenant_id = Uuid::new_v4();
        let authorizer = RoleBasedAuthorizer;
        let principal = Principal {
            tenant_id,
            subject_id: "operator-1".to_string(),
            roles: vec![Role::Operator],
        };
        let request = AuthorizationRequest {
            tenant_id,
            permission: Permission::ToolsExecute,
            resource_type: "tool_call".to_string(),
            resource_id: Some(Uuid::new_v4()),
        };

        authorizer
            .authorize(&principal, &request)
            .await
            .expect("operator can execute approved tools");
    }

    #[tokio::test]
    async fn role_authorizer_denies_missing_permission() {
        let tenant_id = Uuid::new_v4();
        let authorizer = RoleBasedAuthorizer;
        let principal = Principal {
            tenant_id,
            subject_id: "viewer-1".to_string(),
            roles: vec![Role::Viewer],
        };
        let request = AuthorizationRequest {
            tenant_id,
            permission: Permission::ApprovalsDecide,
            resource_type: "approval".to_string(),
            resource_id: Some(Uuid::new_v4()),
        };

        assert!(authorizer.authorize(&principal, &request).await.is_err());
    }

    #[tokio::test]
    async fn role_authorizer_denies_cross_tenant_access() {
        let authorizer = RoleBasedAuthorizer;
        let principal = Principal {
            tenant_id: Uuid::new_v4(),
            subject_id: "admin-1".to_string(),
            roles: vec![Role::Admin],
        };
        let request = AuthorizationRequest {
            tenant_id: Uuid::new_v4(),
            permission: Permission::Admin,
            resource_type: "tenant".to_string(),
            resource_id: None,
        };

        assert!(authorizer.authorize(&principal, &request).await.is_err());
    }
}
