#[cfg(test)]
use std::path::Path as FsPath;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use async_trait::async_trait;
#[cfg(test)]
use axum::extract::Query;
#[cfg(test)]
use axum::http::header;
#[cfg(test)]
use axum::routing::{get, post};
use axum::{Json, Router, http::HeaderMap, middleware};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
#[cfg(test)]
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Executor, PgPool, postgres::PgPoolOptions};
#[cfg(test)]
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::info;
use uuid::Uuid;

const DEFAULT_TENANT_ID: &str = "00000000-0000-4000-8000-000000000001";
const CONSOLE_CONTENT_SECURITY_POLICY: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval' 'sha256-w1lKnuwwmhE0Xrkx/vuamFpvJ0MhJzm3MkSKnpQOQFQ='; connect-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'";
const CONSOLE_DEV_CONTENT_SECURITY_POLICY: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval' 'sha256-w1lKnuwwmhE0Xrkx/vuamFpvJ0MhJzm3MkSKnpQOQFQ='; connect-src 'self' http://127.0.0.1:* http://localhost:*; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'";

mod agent_release_automation;
mod agent_runtime_profile_release;
mod approval_runtime;
mod artifact_files;
mod authorization;
mod codex_app_server;
mod codex_app_server_ops;
mod context_packet_runtime;
mod db_bootstrap;
mod deployment_version;
mod dynamic_workflow_runtime;
mod enterprise_product_readiness;
mod enterprise_security_readiness;
mod error;
mod eval_judge;
mod eval_runtime;
mod execution;
mod execution_queue;
mod execution_queue_broker;
mod handlers;
mod http_shell;
mod mcp_gateway;
mod native_connectors;
mod observability;
mod observability_runtime;
mod ontology_action_profile;
mod ontology_dataset_profile;
mod ontology_engine;
mod ontology_onboarding_engine;
mod ontology_review;
mod ontology_review_graph_helpers;
mod ontology_seed_builders;
mod ontology_source_adapters;
mod policy;
mod policy_rollout_runtime;
mod policy_runtime;
mod provider;
mod provider_governance_runtime;
mod provider_mcp_runtime;
mod remote_computer_events;
mod remote_computer_execution_transport;
mod remote_computer_production;
mod remote_computer_readiness;
mod remote_computer_runner;
mod remote_computer_runtime;
mod remote_computer_sidecars;
mod remote_computer_state_sync;
mod remote_computer_supervision_runtime;
mod request_auth;
mod runtime_config;
mod runtime_support;
mod sandbox_runtime_protocol;
mod scheduler_runtime;
mod secrets;
mod semantic_memory_governance;
mod semantic_synthesis_runtime;
mod semantic_synthesis_schedules;
mod session_handoff_runtime;
mod session_loop_runtime;
mod shell_runner;
mod stage2_readiness;
mod state;
mod store_agent_handoffs;
mod store_approval_groups;
mod store_approval_notification_channels;
mod store_approvals;
mod store_artifacts;
mod store_audit;
mod store_backend;
mod store_codex_app_server;
mod store_context_packets;
mod store_cost_alert_routes;
mod store_dynamic_workflow_plans;
mod store_entities;
mod store_environments;
mod store_eval;
mod store_events;
mod store_github_bindings;
mod store_governance;
mod store_manager_plans;
mod store_memory_writeback;
mod store_ontology_release_workflow_triggers;
mod store_ontology_releases;
mod store_policy_revisions;
mod store_releases;
mod store_remote_computers;
mod store_rows;
mod store_runtime_profiles;
mod store_secret_records;
mod store_seed;
mod store_semantic_kernel;
mod store_session_loop_jobs;
mod store_session_threads;
mod store_tool_calls;
mod store_usage_rollups;
mod store_workflow_packs;
mod store_workflows;
mod telemetry_events;
mod tenant_isolation_runtime;
mod tenant_runtime_context;
mod types;
mod usage_finance_runtime;
mod usage_summary_runtime;
mod vault_kms_runtime;
mod vault_readiness_runtime;
mod worker_execution_runtime;
mod worker_load_validation;
mod worker_readiness;
mod workflow_graph_advancement;
mod workflow_graph_core;
mod workflow_pack;
mod workflow_pack_runtime;
mod workflow_step_execution;
mod workflow_task_grants;

pub(crate) use agent_release_automation::*;
pub(crate) use agent_runtime_profile_release::evaluate_agent_runtime_profile_release_gate;
pub(crate) use approval_runtime::*;
pub(crate) use artifact_files::{
    artifact_type_from_path, discover_artifact_files, normalize_codex_artifact_path,
    normalize_remote_computer_artifact_dir,
};
use authorization::{AuthorizationRequest, Permission, Principal, Role, RoleBasedAuthorizer};
use codex_app_server::{
    CodexAppServerClient, CodexAppServerConfig, CodexThreadRequest, CodexTurnRequest,
    HttpCodexAppServerClient, ReservedCodexAppServerClient, WsCodexAppServerClient,
};
#[cfg(test)]
use codex_app_server::{
    CodexCommandRequest, CodexCommandResponse, CodexInterruptResponse, CodexThreadResponse,
    CodexTurnResponse,
};
pub(crate) use codex_app_server_ops::*;
pub(crate) use context_packet_runtime::*;
#[cfg(test)]
pub(crate) use db_bootstrap::migration_paths;
pub(crate) use db_bootstrap::{run_migrations, seed_demo_tenant};
#[cfg(test)]
pub(crate) use deployment_version::deployment_version_from_lookup;
pub(crate) use deployment_version::{
    deployment_expected_value_matches, deployment_version_from_env,
};
pub(crate) use dynamic_workflow_runtime::*;
pub(crate) use enterprise_product_readiness::build_enterprise_product_completion_readiness;
pub(crate) use enterprise_security_readiness::build_enterprise_security_admin_readiness;
pub(crate) use error::AppError;
use eval_judge::{EvalJudgeClient, EvalJudgeConfig, HttpEvalJudgeClient};
#[cfg(test)]
use eval_judge::{EvalJudgeRequest, EvalJudgeResponse, ReservedEvalJudgeClient};
pub(crate) use eval_runtime::*;
use execution::{
    AgentCliRequest, ExecutionWorker, ExecutionWorkerOutcome, InlineExecutionWorker,
    QueueBackedExecutionWorker, run_agent_cli, run_execution_job, truncate_output,
};
#[cfg(test)]
use execution::{codex_jsonl_event_type, parse_codex_jsonl};
#[cfg(test)]
use execution_queue::{ExecutionJobRequest, ExecutionQueueBackend};
use execution_queue::{ExecutionJobStatus, ExecutionQueue};
use execution_queue_broker::{BrokerExecutionQueue, BrokerQueueConfig, BrokerQueueKind};
pub(crate) use http_shell::{api_cors_layer, security_headers_middleware};
use mcp_gateway::{
    HttpMcpGatewayClient, McpCallRequest, McpGatewayClient, McpGatewayConfig,
    ReservedMcpGatewayClient,
};
use observability::{
    HttpTelemetryExporter, ObservabilityConfig, ReservedTelemetryExporter, TelemetryEvent,
    TelemetryExporter,
};
pub(crate) use observability_runtime::*;
pub(crate) use ontology_action_profile::{
    ontology_action_executor_is_cross_system, ontology_action_has_effects,
    ontology_default_action_transaction_profile,
};
pub(crate) use ontology_dataset_profile::{ontology_is_pii_field, ontology_profile_demo_datasets};
pub(crate) use ontology_engine::*;
pub(crate) use ontology_onboarding_engine::*;
pub(crate) use ontology_review::normalize_ontology_review_decision;
pub(crate) use ontology_review_graph_helpers::{
    ontology_graph_action_id, ontology_graph_dataset_id, ontology_graph_logic_id,
    ontology_graph_merge_candidate_id, ontology_graph_metric_id,
    ontology_graph_node_id_for_subgraph_member, ontology_graph_object_id,
    ontology_graph_subgraph_id, ontology_graph_tool_id, ontology_proposal_risk,
};
pub(crate) use ontology_seed_builders::{
    ontology_demo_dataset, ontology_seed_action, ontology_seed_metric, ontology_seed_object,
    ontology_seed_relation,
};
#[cfg(test)]
use policy::ensure_read_only_sql;
use policy::{PolicyConfig, ensure_read_only_sql_with_policy, load_policy_config};
pub(crate) use policy_rollout_runtime::*;
pub(crate) use policy_runtime::runtime_policy;
#[cfg(test)]
use provider::parse_openai_compatible_provider_response;
use provider::{
    HarnessContext, MockProviderClient, OpenAiCompatibleProviderClient, ProviderClient,
    ProviderResponse, default_provider_tool_names,
};
pub(crate) use provider_governance_runtime::*;
pub(crate) use provider_mcp_runtime::*;
pub(crate) use remote_computer_events::{
    record_remote_computer_attachment_event, record_remote_computer_job_assignment_event,
    record_remote_computer_lease_event, record_remote_computer_sidecar_heartbeat_event,
    record_remote_computer_state_lock_event,
};
pub(crate) use remote_computer_execution_transport::build_remote_computer_execution_transport_readiness;
#[cfg(test)]
pub(crate) use remote_computer_execution_transport::remote_computer_execution_transport_state;
pub(crate) use remote_computer_production::build_remote_computer_production_path_payload;
pub(crate) use remote_computer_readiness::build_remote_computer_readiness;
use remote_computer_runner::{
    RemoteComputerRunnerConfig, RemoteComputerRunnerDryRunRequest,
    RemoteComputerRunnerDryRunResponse, RemoteComputerRunnerReadiness,
    remote_computer_runner_for_config,
};
pub(crate) use remote_computer_runtime::{
    RemoteComputerRuntimeIdentity, RemoteComputerSubstrate, build_remote_computer_runner_readiness,
    cleanup_remote_computer_lease_runtime, cleanup_remote_computer_session_runtimes,
    delete_remote_computer_runtime_resource, metadata_with_remote_computer_runtime_identity,
    remote_computer_runner_request_is_exec, remote_computer_runner_response_for_audit,
    remote_computer_runtime_identity, required_remote_computer_runtime_identity,
};
pub(crate) use remote_computer_sidecars::{
    build_remote_computer_sidecar_recovery_readiness, build_remote_computer_sidecar_supervision,
    execute_remote_computer_sidecar_recovery, remote_computer_sidecar_recovery_targets,
};
#[cfg(test)]
pub(crate) use remote_computer_sidecars::{
    build_remote_computer_sidecar_recovery_readiness_with_lookup,
    execute_remote_computer_sidecar_validation_controller,
};
pub(crate) use remote_computer_state_sync::{
    build_remote_computer_production_state_sync_readiness,
    execute_remote_computer_state_sync_controller, remote_computer_state_sync_base_issues,
    remote_computer_state_sync_controller_configured,
    remote_computer_state_sync_controller_required,
};
pub(crate) use remote_computer_supervision_runtime::*;
pub(crate) use request_auth::*;
#[cfg(test)]
pub(crate) use runtime_config::{
    ExecutionQueueBackendSelection, runtime_tenant_id_from_lookup, select_execution_queue_backend,
    tenant_runtime_mode_from_lookup,
};
pub(crate) use runtime_config::{
    approval_email_relay_url_from_env, approval_slack_webhook_url_from_env,
    approval_webhook_url_from_env, codex_app_server_client_from_env,
    codex_app_server_config_from_env, cost_alert_email_relay_url_from_env,
    cost_alert_smtp_config_from_env, cost_alert_webhook_url_from_env, eval_judge_client_from_env,
    eval_judge_config_from_env, execution_queue_from_env, execution_worker_from_env,
    mcp_gateway_client_from_env, mcp_gateway_config_from_env, runtime_tenant_id_from_env,
    telemetry_exporter_from_env, tenant_runtime_mode_from_env,
};
pub(crate) use runtime_support::*;
pub(crate) use sandbox_runtime_protocol::{
    SANDBOX_RUNTIME_EXECUTABLE, SANDBOX_RUNTIME_SUBCOMMAND, SandboxRuntimeOperation,
    SandboxRuntimeRequest, normalize_agent_cli_executable,
};
pub(crate) use scheduler_runtime::*;
use secrets::{
    SecretProvider, SecretProviderConfig, SecretProviderKind, SecretRef, SecretValue,
    VaultSecretProvider, secret_provider_from_env,
};
pub(crate) use semantic_memory_governance::*;
pub(crate) use semantic_synthesis_runtime::*;
pub(crate) use semantic_synthesis_schedules::*;
pub(crate) use session_handoff_runtime::*;
pub(crate) use session_loop_runtime::*;
#[cfg(test)]
use shell_runner::docker_shell_args;
use shell_runner::{shell_command, shell_runner};
pub(crate) use stage2_readiness::build_stage2_completion_readiness;
#[cfg(test)]
pub(crate) use stage2_readiness::parse_stage2_open_gaps;
pub(crate) use state::AppState;
use store_backend::{MemoryStore, StoreBackend};
pub(crate) use telemetry_events::telemetry_status_for_event;
pub(crate) use tenant_isolation_runtime::*;
pub(crate) use types::agent::{
    Agent, AgentRelease, AgentReleaseAttentionItem, AgentReleaseAutomationRun,
    AgentReleaseAutomationRunAttentionItem, AgentReleaseAutomationRunRecord,
    AgentReleaseAutomationRunSummary, AgentReleaseDeploymentReadiness,
    AgentReleaseDeploymentValidationRun, AgentReleaseLatestPromotion,
    AgentReleaseOrchestrationValidationRun, AgentReleaseProductionOpsReadiness,
    AgentReleaseProductionOrchestrationReadiness, AgentReleaseRolloutSummary, AgentRuntimeProfile,
    AgentRuntimeProfileReleaseGate, AgentVersion, CreateAgent, CreateAgentRelease,
    CreateAgentRuntimeProfile, CreateAgentVersion, CreateEnvironment, Environment,
    RejectAgentReleasePromotion, RequestAgentReleasePromotion, UpdateAgentRuntimeProfile,
    UpdateEnvironment,
};
pub(crate) use types::agent_handoff::{
    AgentHandoffAssignment, AgentHandoffEvent, AttachAgentHandoffRemoteComputerAssignment,
    CreateAgentHandoffAssignment, CreateAgentHandoffEvent, CreateManagerAgentPlan,
    EscalateAgentHandoffEvent, ManagerAgentPlan, ReviewManagerAgentPlan,
    TransitionAgentHandoffEvent,
};
pub(crate) use types::approval::{
    Approval, ApprovalCommitBinding, ApprovalCommitToken, ApprovalEscalationDueRun,
    ApprovalEscalationRule, ApprovalGroup, ApprovalNotificationChannelDelivery,
    ApprovalNotificationChannelPolicy, ApprovalNotificationDelivery,
    ApprovalNotificationDeliveryFailure, ApprovalNotificationDeliveryRun,
    ApprovalNotificationDeliveryRunAttentionItem, ApprovalNotificationDeliveryRunRecord,
    ApprovalNotificationDeliveryRunSummary, ApprovalNotificationDeploymentReadiness,
    ApprovalNotificationDeploymentValidationRun, ApprovalNotificationOpsValidationRun,
    ApprovalNotificationProductionOpsReadiness, ApprovalNotificationRoutingAttention,
    ApprovalNotificationRoutingSummary, CreateApprovalEscalationRule, CreateApprovalGroup,
    CreateApprovalNotificationChannelPolicy, EscalateApproval, ModifyApproval,
};
pub(crate) use types::artifact::{
    Artifact, CodexArtifactSyncRequest, CodexArtifactSyncResponse,
    RemoteComputerArtifactDiscoverRequest, RemoteComputerArtifactSyncRequest,
    RemoteComputerArtifactSyncResponse,
};
pub(crate) use types::audit::AuditLog;
pub(crate) use types::codex_app_server::{
    CodexAppServerControlPlaneAttentionItem, CodexAppServerControlPlaneSummary,
    CodexAppServerDeploymentReadiness, CodexAppServerOpsValidationRun, CodexAppServerPollRequest,
    CodexAppServerPollResponse, CodexAppServerProductionOpsReadiness, CodexAppServerRun,
    CodexAppServerStalePollRequest, CodexAppServerStalePollRun, CodexAppServerStatusPoint,
    CodexAppServerTraceDetail, CodexAppServerTraceSummary, CodexTraceArtifactLineage,
    CodexTraceDashboard, CodexTraceEvidence, CodexTurnTrace, default_codex_stale_after_seconds,
};
pub(crate) use types::collaboration::{
    AgentTeammate, CreateAgentTeammate, CreateSquad, CreateSquadMember, CreateWorkItem,
    CreateWorkItemAssignment, CreateWorkItemReview, Squad, SquadMember, WorkItem,
    WorkItemActivityEntry, WorkItemAssignment, WorkItemReview,
};
pub(crate) use types::context_packet::{
    ContextPacket, ContextPacketAgent, ContextPacketRuntimeProfile, ContextPacketSourceRef,
    RenderContextPacketRequest, RenderedContextBudget, RenderedContextOmissions,
    RenderedExecutionContext,
};
pub(crate) use types::defaults::*;
pub(crate) use types::deployment::{
    DeploymentVersion, EnterpriseEvidenceArchiveMetadata, EnterpriseProductCompletionLane,
    EnterpriseProductCompletionReadiness, EnterpriseSecurityAdminCheck,
    EnterpriseSecurityAdminReadiness, ProductionAutoDeployRequest,
    ProductionDeploymentVerifyRequest, Stage2CompletionReadiness, Stage2EvidenceRequirement,
};
pub(crate) use types::eval::{
    BootstrapEvalSuite, CreateEvalCase, CreateEvalDataset, CreateEvalJudgeProfile, CreateEvalRun,
    EvalCase, EvalDataset, EvalDriftDecision, EvalGateDecision, EvalGateRequest, EvalRun,
    EvalSuiteBootstrap,
};
pub(crate) use types::github::{ProjectGitHubBinding, UpsertProjectGitHubBinding};
pub(crate) use types::mcp::{
    CreateMcpServerRecord, McpServerDeploymentReadiness, McpServerDeploymentValidationRun,
    McpServerHealth, McpServerHealthRun, McpServerLatestRollout, McpServerRecord,
    McpServerRolloutAttentionItem, McpServerRolloutDueRun, McpServerRolloutProductionOpsReadiness,
    McpServerRolloutProductionOrchestrationReadiness, McpServerRolloutResponse,
    McpServerRolloutRunAttentionItem, McpServerRolloutRunRecord, McpServerRolloutRunSummary,
    McpServerRolloutSummary, McpServerScheduledHealthRun, RequestMcpServerRollout,
    UpdateMcpServerRecord, UpdateMcpServerStatus,
};
pub(crate) use types::observability::{
    ObservabilityBackpressure, ObservabilityCollectorAttentionItem,
    ObservabilityCollectorClusterRolloutReadiness,
    ObservabilityCollectorClusterRolloutValidationRun, ObservabilityCollectorDeploymentReadiness,
    ObservabilityCollectorHealthCheck, ObservabilityCollectorProductionOpsReadiness,
    ObservabilityCollectorReadiness, ObservabilityCollectorSignalPath, ObservabilityErrorEvent,
    ObservabilityRemediationPlan, ObservabilityRemediationPlanAction, ObservabilityRemediationRun,
    ObservabilityRemediationSupervisionReadiness, ObservabilitySummary,
    ObservabilityTelemetryStatus,
};
pub(crate) use types::ontology::{
    BuildSemanticOntologyRequest, ConfidenceCalibrationBucket, ConfidenceCalibrationRecord,
    ConfidenceCalibrationResponse, CreateOntologyOnboardingRunRequest,
    CreateOntologyReleaseCandidateRequest, CuratedDatasetDraft, EntityResolutionCandidate,
    EntityResolutionDecisionDraft, EntityResolutionRequest, EntityResolutionResponse,
    EntityResolutionRetrievalHit, ExpandSemanticOntologyRequest, ONTOLOGY_RELEASE_STATUS_ACTIVE,
    ONTOLOGY_RELEASE_STATUS_ACTIVE_TRIGGER_FAILED, ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_FAILED,
    ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_PENDING,
    ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_SKIPPED,
    ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_TRIGGERED, OntologyActionTransactionProfile,
    OntologyBuilderDag, OntologyBuilderEdge, OntologyBuilderExecutionLevel, OntologyBuilderNode,
    OntologyDatasetProfile, OntologyEngineReadiness, OntologyEngineReadinessCheck,
    OntologyForeignKeyCandidate, OntologyObjectType, OntologyOnboardingDataset,
    OntologyOnboardingField, OntologyOnboardingMaterializationResult,
    OntologyOnboardingProposalDraft, OntologyOnboardingRun, OntologyOnboardingToolSpec,
    OntologyOnboardingToolSpecResponse, OntologyPromptPacket, OntologyRegistry,
    OntologyRelationType, OntologyRelease, OntologyReleaseListQuery,
    OntologyReleaseWorkflowTrigger, OntologyReleaseWorkflowTriggerDrain, OntologyReviewGraph,
    OntologyReviewGraphEdge, OntologyReviewGraphNode, OntologySeedActionMapping,
    OntologySeedMetricMapping, OntologySeedObjectMapping, OntologySeedPack,
    OntologySeedPackSummary, OntologySeedRelationMapping, OntologySourceBundle,
    PropertyUnderstandingCandidate, ReviewOntologyCuratedDatasetRequest,
    ReviewOntologyOnboardingProposalRequest, ReviewOntologyProposalRequest,
    SchemaUnderstandingCandidate, SchemaUnderstandingRequest, SchemaUnderstandingResponse,
    SubgraphProposalDraft, SubgraphProposalMember, SubgraphProposalRequest,
    SubgraphProposalResponse, TaxonomyLayerCandidate, ontology_release_current_status,
    ontology_release_workflow_trigger_status_allowed,
};
pub(crate) use types::policy::{
    CreatePolicyRevision, PolicyActivationWindow, PolicyDiffChange, PolicyGateCaseInput,
    PolicyGateCaseResult, PolicyRevision, PolicyRevisionDiff, PolicyRevisionGate,
    PolicyRevisionGateRequest, PolicyRollbackResult, PolicyRolloutControllerBinding,
    PolicyRolloutOrchestrationReadiness, PolicyRolloutOrchestrationValidationRun, PolicyRuntime,
    PolicyRuntimeStatus, PolicyScheduledRolloutRun, PolicyScheduledRolloutScanDetail,
    PolicyTestResult, SimulatePolicy, StagedPolicyRuntime, TestPolicyRequest,
};
pub(crate) use types::provider::{
    CreateProviderAccess, CreateProviderRecord, DecideProviderStatusApproval, ProviderAccess,
    ProviderDeploymentReadiness, ProviderDeploymentValidationRun, ProviderGovernanceAttentionItem,
    ProviderGovernanceSummary, ProviderHealth, ProviderPolicyGateCheck,
    ProviderPolicyGateEnforcement, ProviderPolicyGateReport, ProviderPolicyGateRun,
    ProviderPolicyGateRunAttentionItem, ProviderPolicyGateRunResponse,
    ProviderPolicyGateRunSummary, ProviderProductionRollbackRun, ProviderProductionRolloutRun,
    ProviderRecord, ProviderRuntimeStatus, ProviderStatusApprovalResponse,
    RequestProviderStatusApproval, RotateProviderApiKeyRef, RunProviderProductionRollback,
    RunProviderProductionRollout, UpdateProviderAccess, UpdateProviderStatus,
};
pub(crate) use types::remote_computer::{
    CreateRemoteComputer, CreateRemoteComputerAttachment, CreateRemoteComputerJobAssignment,
    CreateRemoteComputerLease, CreateRemoteComputerSidecarHeartbeat, CreateRemoteComputerStateLock,
    ReleaseRemoteComputerStateLock, RemoteComputer,
    RemoteComputerAgentSandboxLiveEvidenceReadiness, RemoteComputerAgentSandboxReadiness,
    RemoteComputerArtifactDiscoverySidecarConfigReadiness, RemoteComputerAttachment,
    RemoteComputerAttentionItem, RemoteComputerAutoscalingReadiness,
    RemoteComputerExecutionTransportReadiness, RemoteComputerJobAssignment, RemoteComputerLease,
    RemoteComputerManifestReadiness, RemoteComputerProductionStateSyncReadiness,
    RemoteComputerReadinessReport, RemoteComputerReclaimRun, RemoteComputerSidecarHeartbeat,
    RemoteComputerSidecarRecoveryReadiness, RemoteComputerSidecarRecoveryRun,
    RemoteComputerSidecarRecoveryTarget, RemoteComputerSidecarSupervisionReadiness,
    RemoteComputerSidecarSupervisionRun, RemoteComputerStateFilesystemReadiness,
    RemoteComputerStateLock, RemoteComputerStateSyncValidationRun, RemoteComputerWarmPoolReadiness,
    UpdateRemoteComputerAttachment, UpdateRemoteComputerLease,
};
pub(crate) use types::scheduler::{
    SchedulerAttentionItem, SchedulerDeploymentReadiness, SchedulerDeploymentValidationRun,
    SchedulerDuePlan, SchedulerDuePlanItem, SchedulerDueRun, SchedulerOrchestrationSummary,
    SchedulerRetryPolicy, SchedulerRunDueRequest, SchedulerRunHistoryItem, SchedulerTaskError,
};
pub(crate) use types::semantic::{
    ContextPacketSemanticObject, CreateMemoryWritebackCandidates, CreateSemanticIngestionBatch,
    CreateSemanticLink, CreateSemanticObject, CreateSemanticSource, CreateSemanticSynthesisRun,
    ExpandSemanticLinksRequest, ExpandSemanticLinksResponse, FetchSemanticObjectRequest,
    FetchSemanticObjectResponse, FetchableSemanticObject, MemoryGovernanceAttentionItem,
    MemoryGovernanceObjectRef, MemoryGovernancePartition, MemoryGovernancePartitionDetail,
    MemoryGovernancePartitionQuery, MemoryGovernanceSummary, MemoryGovernanceWritebackQuery,
    MemoryGovernanceWritebackQueue, MemoryGovernanceWritebackRef, MemoryGovernanceWritebackSummary,
    MemoryWritebackCandidate, RenderedSemanticObject, ResolveSemanticConflictRequest,
    ReviewMemoryWritebackCandidate, RunSemanticDreamingRequest, SearchSemanticObjectsRequest,
    SearchSemanticObjectsResponse, SemanticAgingPolicySweep, SemanticGovernanceRunRequest,
    SemanticGovernanceRunResult, SemanticGraphConflict, SemanticGraphEdge, SemanticGraphNode,
    SemanticGraphPartition, SemanticGraphSnapshot, SemanticIngestionBatchResult,
    SemanticIngestionObjectRef, SemanticLink, SemanticObject, SemanticProductQuery,
    SemanticRetrievalBackendRegistry, SemanticRetrievalBackendStatus, SemanticSearchResponse,
    SemanticSearchResult, SemanticSource, SemanticSynthesisMemoryCandidateInput,
    SemanticSynthesisRunResult, SemanticSynthesisScheduleSweep, SemanticSynthesisScheduledRun,
    UpdateSemanticLink, UpdateSemanticObject, UpdateSemanticSource,
};
pub(crate) use types::session::{
    AddMessage, CreateSession, IncomingSessionEvent, SendSessionEvents, Session, SessionEvent,
    SessionLoopJob, SessionLoopJobStatus, SessionStatus, SessionThread, StreamEventsQuery,
};
pub(crate) use types::tenant::{
    AcceptTenantInvitation, AcceptedTenantInvitation, BootstrapTenantProvisioning,
    CreateMembership, CreateOrganization, CreateProject, CreateTeam, CreateTenantInvitation,
    Membership, Organization, Project, Team, TenantInvitation, TenantIsolationAttentionItem,
    TenantIsolationReadinessReport, TenantIsolationRlsReadiness, TenantIsolationScopedCounts,
    TenantIsolationTableCoverage, TenantProductionRoutingReadiness, TenantProvisioningResult,
    TenantRuntimeMode, TransferOrganizationOwnership,
};
pub(crate) use types::tool_call::ToolCall;
use types::tools::{
    ApprovalRequestTool, ArtifactCreateTool, ExecuteTool, FileReadTool, McpCallTool,
    OntologyTypeLookupTool, SemanticLinkExpandTool, SemanticObjectFetchTool,
    SemanticObjectSearchTool, ShellExecTool, SqlQueryTool, SqlSchemaTool, ToolDescriptor,
};
pub(crate) use types::usage::{
    AcknowledgeCostAlertRequest, CostAlert, CostAlertAcknowledgement, CostAlertDelivery,
    CostAlertRoute, CostAlertRouteDelivery, CostAlertSmtpConfig, CostAlertSummary,
    CreateCostAlertRoute, CreateUsageRollup, ProviderBudgetExhaustionForecast,
    ProviderBudgetStatus, ProviderUsageSummary, ToolUsageSummary, UsageBudgetPressure,
    UsageFinanceAttentionItem, UsageFinanceDashboardSummary, UsageFinanceExportDelivery,
    UsageFinanceOperationAudit, UsageFinanceOperationsRun, UsageFinanceOperationsSummary,
    UsageFinanceProductionCloseReadiness, UsageForecastHorizon, UsageForecastSummary, UsageRollup,
    UsageSummary, UsageTrendPeriod, UsageTrendProvider, UsageTrendSummary,
};
pub(crate) use types::vault::{
    CreateSecretRecord, RotateSecretRecord, SecretProviderHealth, SecretRecord, VaultKmsReadiness,
    VaultKmsRecoveryReadiness, VaultKmsRecoveryValidationRun, VaultKmsRotationDetail,
    VaultKmsRotationRun, VaultProductionRotationReadiness, VaultReadinessAttentionItem,
    VaultReadinessCheck, VaultReadinessReport,
};
pub(crate) use types::worker::{
    K8sAutoscalingManifest, WorkerAutoscalingReadiness, WorkerJobSummary, WorkerK8sReadiness,
    WorkerLeaseSummary, WorkerLoadValidationEvidence, WorkerLoadValidationRun, WorkerModeReadiness,
    WorkerProductionOpsReadiness, WorkerQueueBackendReadiness, WorkerReadinessAttentionItem,
    WorkerReadinessReport,
};
pub(crate) use types::workflow::{
    AgentInboxEntry, AgentInboxSnapshot, ClaimWorkflowStepRun, ClaimWorkflowStepRunResponse,
    CompileDynamicWorkflowPlan, CreateDynamicWorkflowPlan, CreateTaskGrant,
    CreateWorkflowDefinition, CreateWorkflowRun, CreateWorkflowStepRun,
    DynamicWorkflowAdjudicationRequest, DynamicWorkflowAdjudicationResponse, DynamicWorkflowPlan,
    DynamicWorkflowPlanCompilationResponse, DynamicWorkflowPlanMaterializationResponse,
    DynamicWorkflowPressureTestRequest, DynamicWorkflowPressureTestResponse,
    MaterializeDynamicWorkflowPlan, ReviewDynamicWorkflowPlan, RunDueWorkflowSteps,
    RunWorkflowStepRun, RunWorkflowStepRunResponse, SessionRuntimeRefs, TaskBoardItem,
    TaskBoardSnapshot, TaskGrant, UpdateWorkflowDefinition, UpdateWorkflowStepRun,
    WorkflowDefinition, WorkflowGraphConditionEvaluation, WorkflowGraphConsoleEdge,
    WorkflowGraphConsoleNode, WorkflowGraphFanInReadiness, WorkflowGraphNumericComparator,
    WorkflowGraphReadyStep, WorkflowGraphRetryPolicy, WorkflowGraphTimeComparator, WorkflowRun,
    WorkflowRunGraphConsole, WorkflowScheduledStepActivationRun,
    WorkflowScheduledStepActivationSweep, WorkflowStepRun, WorkflowTransition,
    WorkflowTransitionFilter, WorkflowTransitionQuery,
};
pub(crate) use types::workflow_pack::{
    InstallWorkflowPack, ValidateWorkflowPack, WorkflowPackActionProjection,
    WorkflowPackArchiveRequest, WorkflowPackBinding, WorkflowPackConfigWizardPlanRequest,
    WorkflowPackConnectorAssessment, WorkflowPackConnectorLaneImpact,
    WorkflowPackConnectorLaneRequirement, WorkflowPackConnectorOperationContract,
    WorkflowPackConnectorOperationStatus, WorkflowPackConnectorOperationStatusInput,
    WorkflowPackConnectorQualityAssessment, WorkflowPackConnectorQualityAssessmentRequest,
    WorkflowPackConnectorQualityResult, WorkflowPackConnectorSecretRefStatus,
    WorkflowPackConnectorTenantBindingInput, WorkflowPackInstallation,
    WorkflowPackOnboardingAssessment, WorkflowPackOnboardingAssessmentRequest,
    WorkflowPackOnboardingProfileInput, WorkflowPackOntologyProjection, WorkflowPackProfileAsset,
    WorkflowPackProfileAssetSaveRequest, WorkflowPackReleaseRequest, WorkflowPackRollbackRequest,
    WorkflowPackRuntimeObject, WorkflowPackStageRequest, WorkflowPackUpdateRequest,
    WorkflowPackWorkflowFile,
};
pub(crate) use usage_finance_runtime::*;
pub(crate) use usage_summary_runtime::*;
pub(crate) use vault_kms_runtime::*;
pub(crate) use vault_readiness_runtime::*;
pub(crate) use worker_execution_runtime::*;
pub(crate) use worker_load_validation::{
    execute_worker_load_validation, worker_load_validation_evidence,
};
#[cfg(test)]
pub(crate) use worker_load_validation::{
    execute_worker_load_validation_controller, worker_load_validation_evidence_from_audit_logs,
};
pub(crate) use worker_readiness::{
    build_worker_readiness, worker_autoscaling_readiness_from_manifests,
    worker_isolated_pool_configured_from_manifests, worker_k8s_readiness_from_manifests,
    worker_queue_backend_readiness,
};
pub(crate) use workflow_graph_advancement::*;
pub(crate) use workflow_graph_core::*;
pub(crate) use workflow_pack_runtime::*;
pub(crate) use workflow_step_execution::*;
pub(crate) use workflow_task_grants::*;

#[async_trait]
trait ToolExecutor: Send + Sync {
    fn descriptor(&self) -> ToolDescriptor;

    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        tool_call: &ToolCall,
    ) -> Result<Value, AppError>;
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let workspace_root = std::env::var("MANDOFORGE_WORKSPACE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".mandoforge/workspaces"));
    tokio::fs::create_dir_all(&workspace_root).await?;
    let policy = load_policy_config("config/policy.stage1.yaml").await?;

    let tenant_id = runtime_tenant_id_from_env()?;
    let tenant_runtime_mode = tenant_runtime_mode_from_env()?;
    let store = match std::env::var("DATABASE_URL") {
        Ok(database_url) if !database_url.trim().is_empty() => {
            let tenant_setting = format!("SET mandoforge.tenant_id = '{}'", tenant_id);
            let default_tenant_id = tenant_id;
            let pool = PgPoolOptions::new()
                .max_connections(
                    std::env::var("MANDOFORGE_DB_MAX_CONNECTIONS")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(20),
                )
                .after_connect(move |conn, _meta| {
                    let tenant_setting = tenant_setting.clone();
                    Box::pin(async move {
                        conn.execute(tenant_setting.as_str()).await?;
                        Ok(())
                    })
                })
                .before_acquire(move |conn, _meta| {
                    Box::pin(async move {
                        let tenant_id = current_request_tenant_id(default_tenant_id);
                        let tenant_setting = format!("SET mandoforge.tenant_id = '{}'", tenant_id);
                        conn.execute(tenant_setting.as_str()).await?;
                        Ok(true)
                    })
                })
                .connect(&database_url)
                .await
                .context("failed to connect to Postgres")?;
            run_migrations(&pool).await?;
            seed_demo_tenant(&pool, tenant_id).await?;
            StoreBackend::Postgres(pool)
        }
        _ => StoreBackend::Memory(Arc::new(RwLock::new(MemoryStore::default()))),
    };

    let execution_queue = execution_queue_from_env(&store, tenant_id)?;

    let state = AppState {
        store,
        execution_queue,
        execution_worker: execution_worker_from_env(),
        authorizer: Arc::new(RoleBasedAuthorizer),
        observability_config: ObservabilityConfig::from_env()
            .map_err(|error| anyhow::anyhow!(error.message))?,
        telemetry_exporter: telemetry_exporter_from_env()?,
        mcp_gateway_config: mcp_gateway_config_from_env()?,
        mcp_gateway_client: mcp_gateway_client_from_env()?,
        codex_app_server_config: codex_app_server_config_from_env()?,
        codex_app_server_client: codex_app_server_client_from_env()?,
        eval_judge_config: eval_judge_config_from_env()?,
        eval_judge_client: eval_judge_client_from_env()?,
        cost_alert_webhook_url: cost_alert_webhook_url_from_env(),
        cost_alert_email_relay_url: cost_alert_email_relay_url_from_env(),
        cost_alert_smtp_config: cost_alert_smtp_config_from_env(),
        approval_webhook_url: approval_webhook_url_from_env(),
        workspace_root,
        tenant_id,
        tenant_runtime_mode,
        policy: runtime_policy(policy),
    };
    state
        .seed_demo_agent()
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;

    let app = build_router(state);

    let addr: SocketAddr = std::env::var("MANDOFORGE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8787".to_string())
        .parse()
        .context("invalid MANDOFORGE_ADDR")?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "mandoforge api listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(state: AppState) -> Router {
    let tenant_context_state = state.clone();
    Router::new()
        .merge(handlers::deployment::router())
        .merge(handlers::agents::router())
        .merge(handlers::agent_releases::router())
        .merge(handlers::agent_handoffs::router())
        .merge(handlers::semantic::router())
        .merge(handlers::ontology_onboarding::router())
        .merge(handlers::ontology_releases::router())
        .merge(handlers::ontology_intelligence::router())
        .merge(handlers::ontology::router())
        .merge(handlers::memory_governance::router())
        .merge(handlers::sessions::router())
        .merge(handlers::manager_plans::router())
        .merge(handlers::dynamic_workflow_plans::router())
        .merge(handlers::workflows::router())
        .merge(handlers::tools::router())
        .merge(handlers::tenant::router())
        .merge(handlers::collaboration::router())
        .merge(handlers::providers::router())
        .merge(handlers::mcp::router())
        .merge(handlers::policy::router())
        .merge(handlers::vault::router())
        .merge(handlers::codex_app_server::router())
        .merge(handlers::packs::router())
        .merge(handlers::github::router())
        .merge(handlers::eval::router())
        .merge(handlers::usage::router())
        .merge(handlers::scheduler::router())
        .merge(handlers::observability::router())
        .merge(handlers::approvals::router())
        .merge(handlers::approval_notifications::router())
        .merge(handlers::execution_jobs::router())
        .merge(handlers::remote_computers::router())
        .merge(handlers::audit_logs::router())
        .fallback_service(ServeDir::new("web"))
        .route_layer(middleware::from_fn_with_state(
            tenant_context_state,
            tenant_context_middleware,
        ))
        .layer(api_cors_layer())
        .layer(middleware::from_fn(security_headers_middleware))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
