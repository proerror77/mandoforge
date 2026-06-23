use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TenantRuntimeMode {
    SingleRuntimeTenant,
    TenantRouted,
}

impl TenantRuntimeMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SingleRuntimeTenant => "single_runtime_tenant",
            Self::TenantRouted => "tenant_routed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Organization {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) slug: String,
    pub(crate) owner_subject: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    #[serde(default)]
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateOrganization {
    pub(crate) name: String,
    pub(crate) slug: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TransferOrganizationOwnership {
    pub(crate) owner_subject: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BootstrapTenantProvisioning {
    pub(crate) organization_name: String,
    pub(crate) organization_slug: String,
    pub(crate) owner_subject: String,
    #[serde(default)]
    pub(crate) team_name: Option<String>,
    #[serde(default)]
    pub(crate) team_slug: Option<String>,
    #[serde(default)]
    pub(crate) project_name: Option<String>,
    #[serde(default)]
    pub(crate) project_slug: Option<String>,
    #[serde(default = "crate::default_bootstrap_owner_role")]
    pub(crate) owner_role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TenantProvisioningResult {
    pub(crate) organization: Organization,
    pub(crate) team: Option<Team>,
    pub(crate) project: Option<Project>,
    pub(crate) owner_membership: Membership,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TenantIsolationReadinessReport {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) readiness_score: i64,
    pub(crate) runtime_tenant_id: Uuid,
    pub(crate) runtime_tenant_mode: String,
    pub(crate) header_fail_closed: bool,
    pub(crate) membership_scope_enforced: bool,
    pub(crate) production_routing: TenantProductionRoutingReadiness,
    pub(crate) scoped_counts: TenantIsolationScopedCounts,
    pub(crate) table_coverage: Vec<TenantIsolationTableCoverage>,
    pub(crate) rls: TenantIsolationRlsReadiness,
    pub(crate) attention_items: Vec<TenantIsolationAttentionItem>,
    pub(crate) runbook_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TenantProductionRoutingReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) cross_tenant_routing_supported: bool,
    pub(crate) runtime_tenant_mode: String,
    pub(crate) header_fail_closed: bool,
    pub(crate) membership_scope_enforced: bool,
    pub(crate) rls_ready: bool,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) latest_controller_status: Option<String>,
    pub(crate) latest_controller_age_hours: Option<i64>,
    pub(crate) controller_evidence_fresh: bool,
    pub(crate) latest_controller_validated: bool,
    pub(crate) message: String,
    pub(crate) blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TenantIsolationScopedCounts {
    pub(crate) organizations: usize,
    pub(crate) teams: usize,
    pub(crate) projects: usize,
    pub(crate) memberships: usize,
    pub(crate) invitations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TenantIsolationTableCoverage {
    pub(crate) table: String,
    pub(crate) tenant_id_required: bool,
    pub(crate) store_filters_tenant: bool,
    pub(crate) rls_required_for_production: bool,
    pub(crate) rls_enabled: bool,
    pub(crate) rls_forced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TenantIsolationRlsReadiness {
    pub(crate) required_for_production: bool,
    pub(crate) enabled: bool,
    pub(crate) forced: bool,
    pub(crate) migration_asset_present: bool,
    pub(crate) tenant_context_configured: bool,
    pub(crate) enabled_table_count: usize,
    pub(crate) forced_table_count: usize,
    pub(crate) tracked_table_count: usize,
    pub(crate) status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TenantIsolationAttentionItem {
    pub(crate) kind: String,
    pub(crate) severity: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Team {
    pub(crate) id: Uuid,
    pub(crate) organization_id: Uuid,
    pub(crate) name: String,
    pub(crate) slug: String,
    pub(crate) created_at: DateTime<Utc>,
    #[serde(default)]
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateTeam {
    pub(crate) name: String,
    pub(crate) slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Project {
    pub(crate) id: Uuid,
    pub(crate) team_id: Uuid,
    pub(crate) name: String,
    pub(crate) slug: String,
    pub(crate) created_at: DateTime<Utc>,
    #[serde(default)]
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateProject {
    pub(crate) name: String,
    pub(crate) slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Membership {
    pub(crate) id: Uuid,
    pub(crate) user_id: String,
    pub(crate) organization_id: Option<Uuid>,
    pub(crate) team_id: Option<Uuid>,
    pub(crate) project_id: Option<Uuid>,
    pub(crate) role: String,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateMembership {
    pub(crate) user_id: String,
    #[serde(default)]
    pub(crate) team_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) project_id: Option<Uuid>,
    pub(crate) role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TenantInvitation {
    pub(crate) id: Uuid,
    pub(crate) organization_id: Uuid,
    pub(crate) team_id: Option<Uuid>,
    pub(crate) project_id: Option<Uuid>,
    pub(crate) email: String,
    pub(crate) role: String,
    pub(crate) status: String,
    pub(crate) token: String,
    pub(crate) invited_by: Option<String>,
    pub(crate) accepted_by: Option<String>,
    pub(crate) expires_at: DateTime<Utc>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) decided_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateTenantInvitation {
    pub(crate) email: String,
    pub(crate) role: String,
    #[serde(default)]
    pub(crate) team_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) project_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) expires_in_hours: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AcceptTenantInvitation {
    pub(crate) token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AcceptedTenantInvitation {
    pub(crate) invitation: TenantInvitation,
    pub(crate) membership: Membership,
}
