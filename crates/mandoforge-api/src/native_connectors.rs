use std::collections::BTreeMap;
use std::time::Duration;

use chrono::Utc;
use hmac::{Hmac, Mac};
use md5::{Digest as Md5Digest, Md5};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::Sha256;

use crate::AppError;

type HmacSha256 = Hmac<Sha256>;

const LIVE_ENABLED_ENV: &str = "MANDOFORGE_NATIVE_CONNECTOR_LIVE_ENABLED";
const ECOMMERCE_LIVE_ENABLED_ENV: &str = "MANDOFORGE_ECOMMERCE_LIVE_ADAPTERS_ENABLED";
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NativeConnectorProductionReadiness {
    pub generated_at: chrono::DateTime<Utc>,
    pub status: String,
    pub required_evidence_class: String,
    pub live_enabled: bool,
    pub connector_count: usize,
    pub ready_connector_count: usize,
    pub blocked_connector_count: usize,
    pub connectors: Vec<NativeConnectorProductionReadinessItem>,
    pub next_actions: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NativeConnectorProductionReadinessItem {
    pub connector_id: String,
    pub provider: String,
    pub manifest_path: String,
    pub status: String,
    pub current_evidence_class: String,
    pub required_evidence_class: String,
    pub required_secret_refs: Vec<String>,
    pub configured_secret_refs: Vec<String>,
    pub missing_secret_refs: Vec<String>,
    pub checks: Vec<NativeConnectorProductionReadinessCheck>,
    pub blockers: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NativeConnectorProductionReadinessCheck {
    pub id: String,
    pub title: String,
    pub status: String,
    pub evidence: Vec<String>,
    pub blockers: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone)]
struct NativeConnectorCall {
    connector_id: String,
    operation: String,
    api_name: String,
    payload: Map<String, Value>,
    dry_run: bool,
}

#[derive(Debug, Clone)]
struct LiveHttpRequest {
    adapter: &'static str,
    method: &'static str,
    url: String,
    headers: BTreeMap<String, String>,
    query: BTreeMap<String, String>,
    body: Option<Value>,
    secret_refs: Vec<String>,
}

pub(crate) fn is_supported_ecommerce_connector(connector_id: &str) -> bool {
    matches!(
        connector_id,
        "tmall-top"
            | "taobao-open-platform"
            | "tiktok-shop-open-api"
            | "xiaohongshu-shop"
            | "amazon-selling-partner-api"
    )
}

pub(crate) fn build_native_connector_production_readiness() -> NativeConnectorProductionReadiness {
    let specs = native_connector_production_specs();
    let live_enabled = env_bool(LIVE_ENABLED_ENV) || env_bool(ECOMMERCE_LIVE_ENABLED_ENV);
    let connectors = specs
        .iter()
        .map(|spec| build_native_connector_readiness_item(spec, live_enabled))
        .collect::<Vec<_>>();
    let connector_count = connectors.len();
    let ready_connector_count = connectors
        .iter()
        .filter(|connector| connector.status == "ready")
        .count();
    let blocked_connector_count = connectors
        .iter()
        .filter(|connector| connector.status == "blocked")
        .count();
    let status = if connector_count > 0 && ready_connector_count == connector_count {
        "ready"
    } else {
        "blocked"
    }
    .to_string();
    let next_actions = vec![
        "configure sandbox and live environment separation for every native ecommerce connector"
            .to_string(),
        "add token refresh, rate-limit, retry, reconciliation, and webhook ingestion evidence"
            .to_string(),
        "publish compensation or explicit non-compensable policy per write operation".to_string(),
        "archive immutable deployment evidence with target, version, logs, and owner".to_string(),
    ];
    let message = if status == "ready" {
        format!(
            "All {connector_count} native ecommerce connectors have customer-grade production evidence"
        )
    } else {
        format!(
            "Native connector production readiness is blocked: {ready_connector_count}/{connector_count} connectors are customer-grade ready and {blocked_connector_count} remain blocked"
        )
    };

    NativeConnectorProductionReadiness {
        generated_at: Utc::now(),
        status,
        required_evidence_class: "customer_grade".to_string(),
        live_enabled,
        connector_count,
        ready_connector_count,
        blocked_connector_count,
        connectors,
        next_actions,
        message,
    }
}

pub(crate) async fn execute_ecommerce_connector_call(args: &Value) -> Result<Value, AppError> {
    let call = NativeConnectorCall::from_args(args)?;
    if !is_supported_ecommerce_connector(&call.connector_id) {
        return Err(AppError::bad_request(format!(
            "unsupported ecommerce connector {}",
            call.connector_id
        )));
    }

    let live_enabled = env_bool(LIVE_ENABLED_ENV) || env_bool(ECOMMERCE_LIVE_ENABLED_ENV);
    let resolve_secrets = live_enabled && !call.dry_run;
    let request = build_live_request(&call, resolve_secrets)?;
    if call.dry_run || !live_enabled {
        return Ok(json!({
            "status": if call.dry_run { "dry_run_prepared" } else { "live_disabled" },
            "connector_id": call.connector_id,
            "operation": call.operation,
            "api_name": call.api_name,
            "live_enabled": live_enabled,
            "request": request.redacted(),
        }));
    }

    let response = execute_http_request(&request).await?;
    Ok(json!({
        "status": "live_called",
        "connector_id": call.connector_id,
        "operation": call.operation,
        "api_name": call.api_name,
        "request": request.redacted(),
        "response": response,
    }))
}

#[derive(Debug, Clone)]
struct NativeConnectorProductionSpec {
    connector_id: &'static str,
    provider: &'static str,
    manifest_path: &'static str,
    required_secret_refs: Vec<&'static str>,
    sandbox_base_url_env: &'static str,
    live_base_url_env: &'static str,
    token_refresh_controller_env: &'static str,
    rate_limit_policy_env: &'static str,
    reconciliation_controller_env: &'static str,
    webhook_ingestion_env: &'static str,
    compensation_policy_env: &'static str,
    deployment_evidence_archive_env: &'static str,
}

fn native_connector_production_specs() -> Vec<NativeConnectorProductionSpec> {
    vec![
        NativeConnectorProductionSpec {
            connector_id: "tmall-top",
            provider: "alibaba-top",
            manifest_path: "packs/ecommerce-tmall/connectors/tmall-top.yaml",
            required_secret_refs: vec![
                "TMALL_TOP_APP_KEY",
                "TMALL_TOP_APP_SECRET",
                "TMALL_TOP_SESSION",
            ],
            sandbox_base_url_env: "MANDOFORGE_TMALL_TOP_SANDBOX_BASE_URL",
            live_base_url_env: "MANDOFORGE_TMALL_TOP_LIVE_BASE_URL",
            token_refresh_controller_env: "MANDOFORGE_TMALL_TOP_TOKEN_REFRESH_CONTROLLER_URL",
            rate_limit_policy_env: "MANDOFORGE_TMALL_TOP_RATE_LIMIT_POLICY",
            reconciliation_controller_env: "MANDOFORGE_TMALL_TOP_RECONCILIATION_CONTROLLER_URL",
            webhook_ingestion_env: "MANDOFORGE_TMALL_TOP_WEBHOOK_INGESTION_URL",
            compensation_policy_env: "MANDOFORGE_TMALL_TOP_COMPENSATION_POLICY",
            deployment_evidence_archive_env: "MANDOFORGE_TMALL_TOP_DEPLOYMENT_EVIDENCE_ARCHIVE",
        },
        NativeConnectorProductionSpec {
            connector_id: "taobao-open-platform",
            provider: "alibaba-top",
            manifest_path: "packs/ecommerce-taobao/connectors/taobao-open-platform.yaml",
            required_secret_refs: vec![
                "TAOBAO_TOP_APP_KEY",
                "TAOBAO_TOP_APP_SECRET",
                "TAOBAO_TOP_SESSION",
            ],
            sandbox_base_url_env: "MANDOFORGE_TAOBAO_TOP_SANDBOX_BASE_URL",
            live_base_url_env: "MANDOFORGE_TAOBAO_TOP_LIVE_BASE_URL",
            token_refresh_controller_env: "MANDOFORGE_TAOBAO_TOP_TOKEN_REFRESH_CONTROLLER_URL",
            rate_limit_policy_env: "MANDOFORGE_TAOBAO_TOP_RATE_LIMIT_POLICY",
            reconciliation_controller_env: "MANDOFORGE_TAOBAO_TOP_RECONCILIATION_CONTROLLER_URL",
            webhook_ingestion_env: "MANDOFORGE_TAOBAO_TOP_WEBHOOK_INGESTION_URL",
            compensation_policy_env: "MANDOFORGE_TAOBAO_TOP_COMPENSATION_POLICY",
            deployment_evidence_archive_env: "MANDOFORGE_TAOBAO_TOP_DEPLOYMENT_EVIDENCE_ARCHIVE",
        },
        NativeConnectorProductionSpec {
            connector_id: "xiaohongshu-shop",
            provider: "xiaohongshu-open-platform",
            manifest_path: "packs/ecommerce-xiaohongshu/connectors/xiaohongshu-shop.yaml",
            required_secret_refs: vec!["XHS_APP_ID", "XHS_APP_SECRET", "XHS_ACCESS_TOKEN"],
            sandbox_base_url_env: "MANDOFORGE_XHS_SANDBOX_BASE_URL",
            live_base_url_env: "MANDOFORGE_XHS_LIVE_BASE_URL",
            token_refresh_controller_env: "MANDOFORGE_XHS_TOKEN_REFRESH_CONTROLLER_URL",
            rate_limit_policy_env: "MANDOFORGE_XHS_RATE_LIMIT_POLICY",
            reconciliation_controller_env: "MANDOFORGE_XHS_RECONCILIATION_CONTROLLER_URL",
            webhook_ingestion_env: "MANDOFORGE_XHS_WEBHOOK_INGESTION_URL",
            compensation_policy_env: "MANDOFORGE_XHS_COMPENSATION_POLICY",
            deployment_evidence_archive_env: "MANDOFORGE_XHS_DEPLOYMENT_EVIDENCE_ARCHIVE",
        },
        NativeConnectorProductionSpec {
            connector_id: "tiktok-shop-open-api",
            provider: "tiktok-shop-open-api",
            manifest_path: "packs/ecommerce-tiktok-shop/connectors/tiktok-shop-open-api.yaml",
            required_secret_refs: vec![
                "TIKTOK_SHOP_APP_KEY",
                "TIKTOK_SHOP_APP_SECRET",
                "TIKTOK_SHOP_ACCESS_TOKEN",
            ],
            sandbox_base_url_env: "MANDOFORGE_TIKTOK_SHOP_SANDBOX_BASE_URL",
            live_base_url_env: "MANDOFORGE_TIKTOK_SHOP_LIVE_BASE_URL",
            token_refresh_controller_env: "MANDOFORGE_TIKTOK_SHOP_TOKEN_REFRESH_CONTROLLER_URL",
            rate_limit_policy_env: "MANDOFORGE_TIKTOK_SHOP_RATE_LIMIT_POLICY",
            reconciliation_controller_env: "MANDOFORGE_TIKTOK_SHOP_RECONCILIATION_CONTROLLER_URL",
            webhook_ingestion_env: "MANDOFORGE_TIKTOK_SHOP_WEBHOOK_INGESTION_URL",
            compensation_policy_env: "MANDOFORGE_TIKTOK_SHOP_COMPENSATION_POLICY",
            deployment_evidence_archive_env: "MANDOFORGE_TIKTOK_SHOP_DEPLOYMENT_EVIDENCE_ARCHIVE",
        },
        NativeConnectorProductionSpec {
            connector_id: "amazon-selling-partner-api",
            provider: "amazon-sp-api",
            manifest_path: "packs/ecommerce-amazon/connectors/amazon-selling-partner-api.yaml",
            required_secret_refs: vec![
                "AMAZON_SPAPI_ACCESS_TOKEN",
                "AWS_ACCESS_KEY_ID",
                "AWS_SECRET_ACCESS_KEY",
            ],
            sandbox_base_url_env: "MANDOFORGE_AMAZON_SPAPI_SANDBOX_BASE_URL",
            live_base_url_env: "MANDOFORGE_AMAZON_SPAPI_LIVE_BASE_URL",
            token_refresh_controller_env: "MANDOFORGE_AMAZON_SPAPI_TOKEN_REFRESH_CONTROLLER_URL",
            rate_limit_policy_env: "MANDOFORGE_AMAZON_SPAPI_RATE_LIMIT_POLICY",
            reconciliation_controller_env: "MANDOFORGE_AMAZON_SPAPI_RECONCILIATION_CONTROLLER_URL",
            webhook_ingestion_env: "MANDOFORGE_AMAZON_SPAPI_WEBHOOK_INGESTION_URL",
            compensation_policy_env: "MANDOFORGE_AMAZON_SPAPI_COMPENSATION_POLICY",
            deployment_evidence_archive_env: "MANDOFORGE_AMAZON_SPAPI_DEPLOYMENT_EVIDENCE_ARCHIVE",
        },
    ]
}

fn build_native_connector_readiness_item(
    spec: &NativeConnectorProductionSpec,
    live_enabled: bool,
) -> NativeConnectorProductionReadinessItem {
    let required_secret_refs = spec
        .required_secret_refs
        .iter()
        .map(|secret| (*secret).to_string())
        .collect::<Vec<_>>();
    let configured_secret_refs = spec
        .required_secret_refs
        .iter()
        .filter(|secret| env_nonempty(secret))
        .map(|secret| (*secret).to_string())
        .collect::<Vec<_>>();
    let missing_secret_refs = spec
        .required_secret_refs
        .iter()
        .filter(|secret| !env_nonempty(secret))
        .map(|secret| (*secret).to_string())
        .collect::<Vec<_>>();
    let checks = vec![
        readiness_check(
            "live-runtime-gate",
            "Live runtime gate",
            live_enabled,
            vec![format!(
                "{LIVE_ENABLED_ENV} or {ECOMMERCE_LIVE_ENABLED_ENV} enables live HTTP execution"
            )],
            vec![format!(
                "set {LIVE_ENABLED_ENV}=true only after connector-specific readiness passes"
            )],
        ),
        readiness_check(
            "credential-binding",
            "Credential binding",
            missing_secret_refs.is_empty(),
            configured_secret_refs
                .iter()
                .map(|secret| format!("{secret} configured"))
                .collect(),
            missing_secret_refs
                .iter()
                .map(|secret| format!("{secret} missing"))
                .collect(),
        ),
        readiness_check(
            "sandbox-live-separation",
            "Sandbox and live separation",
            env_nonempty(spec.sandbox_base_url_env) && env_nonempty(spec.live_base_url_env),
            configured_env_evidence(&[spec.sandbox_base_url_env, spec.live_base_url_env]),
            vec![format!(
                "{} and {} must both be configured",
                spec.sandbox_base_url_env, spec.live_base_url_env
            )],
        ),
        readiness_check(
            "token-lifecycle",
            "Token refresh and expiry handling",
            env_nonempty(spec.token_refresh_controller_env),
            configured_env_evidence(&[spec.token_refresh_controller_env]),
            vec![format!(
                "{} must point to a controller that proves refresh, expiry, and rotation behavior",
                spec.token_refresh_controller_env
            )],
        ),
        readiness_check(
            "rate-limit-retry",
            "Rate limits, retry, and timeout taxonomy",
            env_nonempty(spec.rate_limit_policy_env),
            configured_env_evidence(&[spec.rate_limit_policy_env]),
            vec![format!(
                "{} must declare the platform-specific rate-limit and retry policy",
                spec.rate_limit_policy_env
            )],
        ),
        readiness_check(
            "idempotency-reconciliation",
            "Idempotency and external reconciliation",
            env_nonempty(spec.reconciliation_controller_env),
            configured_env_evidence(&[spec.reconciliation_controller_env]),
            vec![format!(
                "{} must validate idempotent writes against external platform state",
                spec.reconciliation_controller_env
            )],
        ),
        readiness_check(
            "webhook-ingestion",
            "Webhook or polling ingestion",
            env_nonempty(spec.webhook_ingestion_env),
            configured_env_evidence(&[spec.webhook_ingestion_env]),
            vec![format!(
                "{} must capture external state changes with provenance",
                spec.webhook_ingestion_env
            )],
        ),
        readiness_check(
            "compensation-policy",
            "Compensation or non-compensable policy",
            env_nonempty(spec.compensation_policy_env),
            configured_env_evidence(&[spec.compensation_policy_env]),
            vec![format!(
                "{} must define compensation behavior or explicit non-compensable operations",
                spec.compensation_policy_env
            )],
        ),
        readiness_check(
            "archived-deployment-evidence",
            "Archived deployment evidence",
            env_nonempty(spec.deployment_evidence_archive_env),
            configured_env_evidence(&[spec.deployment_evidence_archive_env]),
            vec![format!(
                "{} must point to an immutable customer-grade evidence archive with deployment target, connector version, logs, and owner",
                spec.deployment_evidence_archive_env
            )],
        ),
        NativeConnectorProductionReadinessCheck {
            id: "approval-commit-boundary".to_string(),
            title: "Approval commit boundary".to_string(),
            status: "ready".to_string(),
            evidence: vec![
                "native.connector.call executes writes only through approval commit flow"
                    .to_string(),
                "adapter returns redacted request plans for dry-run and disabled-live paths"
                    .to_string(),
            ],
            blockers: Vec::new(),
            next_actions: Vec::new(),
        },
        NativeConnectorProductionReadinessCheck {
            id: "secret-redaction".to_string(),
            title: "Secret redaction".to_string(),
            status: "ready".to_string(),
            evidence: vec![
                "LiveHttpRequest::redacted exposes header/query/body keys and secret refs only"
                    .to_string(),
            ],
            blockers: Vec::new(),
            next_actions: Vec::new(),
        },
    ];
    let blockers = checks
        .iter()
        .flat_map(|check| check.blockers.clone())
        .collect::<Vec<_>>();
    let status = if blockers.is_empty() {
        "ready"
    } else {
        "blocked"
    }
    .to_string();
    let current_evidence_class = if status == "ready" {
        "customer_grade"
    } else {
        "repo_controlled"
    }
    .to_string();
    let next_actions = checks
        .iter()
        .flat_map(|check| check.next_actions.clone())
        .collect::<Vec<_>>();

    NativeConnectorProductionReadinessItem {
        connector_id: spec.connector_id.to_string(),
        provider: spec.provider.to_string(),
        manifest_path: spec.manifest_path.to_string(),
        status,
        current_evidence_class,
        required_evidence_class: "customer_grade".to_string(),
        required_secret_refs,
        configured_secret_refs,
        missing_secret_refs,
        checks,
        blockers,
        next_actions,
    }
}

fn readiness_check(
    id: &str,
    title: &str,
    ready: bool,
    evidence: Vec<String>,
    blocked_next_actions: Vec<String>,
) -> NativeConnectorProductionReadinessCheck {
    let blockers = if ready {
        Vec::new()
    } else {
        blocked_next_actions.clone()
    };
    NativeConnectorProductionReadinessCheck {
        id: id.to_string(),
        title: title.to_string(),
        status: if ready { "ready" } else { "blocked" }.to_string(),
        evidence,
        blockers,
        next_actions: if ready {
            Vec::new()
        } else {
            blocked_next_actions
        },
    }
}

fn configured_env_evidence(keys: &[&str]) -> Vec<String> {
    keys.iter()
        .filter(|key| env_nonempty(key))
        .map(|key| format!("{key} configured"))
        .collect()
}

fn env_nonempty(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

impl NativeConnectorCall {
    fn from_args(args: &Value) -> Result<Self, AppError> {
        let connector_id = string_field(args, "connector_id")?;
        let operation =
            string_field(args, "operation").or_else(|_| string_field(args, "operation_id"))?;
        let payload = args
            .get("payload")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let api_name = args
            .get("api_name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| operation_api_name(&connector_id, &operation).to_string());
        let dry_run = args
            .get("dry_run")
            .and_then(Value::as_bool)
            .or_else(|| payload.get("dry_run").and_then(Value::as_bool))
            .unwrap_or(false);
        Ok(Self {
            connector_id,
            operation,
            api_name,
            payload,
            dry_run,
        })
    }
}

impl LiveHttpRequest {
    fn redacted(&self) -> Value {
        json!({
            "adapter": self.adapter,
            "method": self.method,
            "url": self.url,
            "header_keys": self.headers.keys().collect::<Vec<_>>(),
            "query_keys": self.query.keys().collect::<Vec<_>>(),
            "body_keys": self.body.as_ref().and_then(Value::as_object).map(|body| body.keys().collect::<Vec<_>>()).unwrap_or_default(),
            "secret_refs": self.secret_refs,
        })
    }
}

fn build_live_request(
    call: &NativeConnectorCall,
    resolve_secrets: bool,
) -> Result<LiveHttpRequest, AppError> {
    match call.connector_id.as_str() {
        "tmall-top" | "taobao-open-platform" => build_alibaba_top_request(call, resolve_secrets),
        "tiktok-shop-open-api" => build_tiktok_shop_request(call, resolve_secrets),
        "amazon-selling-partner-api" => build_amazon_sp_api_request(call, resolve_secrets),
        "xiaohongshu-shop" => build_xiaohongshu_request(call, resolve_secrets),
        _ => Err(AppError::bad_request(
            "unsupported native ecommerce connector",
        )),
    }
}

fn build_alibaba_top_request(
    call: &NativeConnectorCall,
    resolve_secrets: bool,
) -> Result<LiveHttpRequest, AppError> {
    let secret_prefix = if call.connector_id == "tmall-top" {
        "TMALL_TOP"
    } else {
        "TAOBAO_TOP"
    };
    let app_key_name = format!("{secret_prefix}_APP_KEY");
    let app_secret_name = format!("{secret_prefix}_APP_SECRET");
    let session_name = format!("{secret_prefix}_SESSION");
    let app_key = connector_secret(&app_key_name, resolve_secrets)?;
    let app_secret = connector_secret(&app_secret_name, resolve_secrets)?;
    let session = connector_secret(&session_name, resolve_secrets)?;

    let mut params = BTreeMap::new();
    params.insert("app_key".to_string(), app_key);
    params.insert("format".to_string(), "json".to_string());
    params.insert("method".to_string(), call.api_name.clone());
    params.insert("session".to_string(), session);
    params.insert("sign_method".to_string(), "md5".to_string());
    params.insert(
        "timestamp".to_string(),
        Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    );
    params.insert("v".to_string(), "2.0".to_string());
    for (key, value) in &call.payload {
        if key.starts_with("__adapter") || key == "dry_run" {
            continue;
        }
        params.insert(key.clone(), scalar_to_string(value)?);
    }
    let sign = alibaba_top_md5_sign(&params, &app_secret);
    params.insert("sign".to_string(), sign);

    Ok(LiveHttpRequest {
        adapter: "alibaba_top",
        method: "POST",
        url: call
            .payload
            .get("__adapter_base_url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("https://eco.taobao.com/router/rest")
            .to_string(),
        headers: BTreeMap::new(),
        query: params,
        body: None,
        secret_refs: vec![app_key_name, app_secret_name, session_name],
    })
}

fn build_tiktok_shop_request(
    call: &NativeConnectorCall,
    resolve_secrets: bool,
) -> Result<LiveHttpRequest, AppError> {
    let app_key = connector_secret("TIKTOK_SHOP_APP_KEY", resolve_secrets)?;
    let app_secret = connector_secret("TIKTOK_SHOP_APP_SECRET", resolve_secrets)?;
    let access_token = connector_secret("TIKTOK_SHOP_ACCESS_TOKEN", resolve_secrets)?;
    let path = endpoint_path(call, "/api/mandoforge/connector/operation")?;
    let mut query = common_signed_query(call, app_key);
    query.insert("access_token".to_string(), access_token.clone());
    let body = Value::Object(call.payload.clone());
    let sign = tiktok_shop_hmac_sha256_sign(&path, &query, &body, &app_secret)?;
    query.insert("sign".to_string(), sign);
    let mut headers = BTreeMap::new();
    headers.insert(
        "Authorization".to_string(),
        format!("Bearer {access_token}"),
    );
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    Ok(LiveHttpRequest {
        adapter: "tiktok_shop_open_api",
        method: "POST",
        url: format!(
            "{}{}",
            base_url(
                call,
                "MANDOFORGE_TIKTOK_SHOP_BASE_URL",
                "https://open-api.tiktokglobalshop.com"
            ),
            path
        ),
        headers,
        query,
        body: Some(body),
        secret_refs: vec![
            "TIKTOK_SHOP_APP_KEY".to_string(),
            "TIKTOK_SHOP_APP_SECRET".to_string(),
            "TIKTOK_SHOP_ACCESS_TOKEN".to_string(),
        ],
    })
}

fn build_xiaohongshu_request(
    call: &NativeConnectorCall,
    resolve_secrets: bool,
) -> Result<LiveHttpRequest, AppError> {
    let app_id = connector_secret("XHS_APP_ID", resolve_secrets)?;
    let app_secret = connector_secret("XHS_APP_SECRET", resolve_secrets)?;
    let access_token = connector_secret("XHS_ACCESS_TOKEN", resolve_secrets)?;
    let path = endpoint_path(call, "/api/mandoforge/connector/operation")?;
    let body = Value::Object(call.payload.clone());
    let timestamp = Utc::now().timestamp().to_string();
    let signature_material = format!("{path}{timestamp}{body}");
    let sign = hmac_sha256_hex(&app_secret, &signature_material)?;
    let mut headers = BTreeMap::new();
    headers.insert(
        "Authorization".to_string(),
        format!("Bearer {access_token}"),
    );
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers.insert("X-App-Id".to_string(), app_id);
    headers.insert("X-Timestamp".to_string(), timestamp);
    headers.insert("X-Signature".to_string(), sign);

    Ok(LiveHttpRequest {
        adapter: "xiaohongshu_shop",
        method: "POST",
        url: format!(
            "{}{}",
            base_url(
                call,
                "MANDOFORGE_XHS_BASE_URL",
                "https://ark.xiaohongshu.com"
            ),
            path
        ),
        headers,
        query: BTreeMap::new(),
        body: Some(body),
        secret_refs: vec![
            "XHS_APP_ID".to_string(),
            "XHS_APP_SECRET".to_string(),
            "XHS_ACCESS_TOKEN".to_string(),
        ],
    })
}

fn build_amazon_sp_api_request(
    call: &NativeConnectorCall,
    resolve_secrets: bool,
) -> Result<LiveHttpRequest, AppError> {
    let access_token = connector_secret("AMAZON_SPAPI_ACCESS_TOKEN", resolve_secrets)
        .or_else(|_| connector_secret("AMAZON_LWA_ACCESS_TOKEN", resolve_secrets))?;
    let aws_access_key_id = connector_secret("AWS_ACCESS_KEY_ID", resolve_secrets)?;
    let aws_secret_access_key = connector_secret("AWS_SECRET_ACCESS_KEY", resolve_secrets)?;
    let env_region = std::env::var("MANDOFORGE_AMAZON_SPAPI_REGION").ok();
    let region = call
        .payload
        .get("__adapter_region")
        .or_else(|| call.payload.get("region"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| env_region.as_deref())
        .unwrap_or("us-east-1")
        .to_string();
    let path = endpoint_path(call, amazon_default_path(&call.operation))?;
    let method = if call.operation.contains("read") || call.api_name.contains("get") {
        "GET"
    } else {
        "POST"
    };
    let base = base_url(
        call,
        "MANDOFORGE_AMAZON_SPAPI_BASE_URL",
        "https://sellingpartnerapi-na.amazon.com",
    );
    let query = payload_query(&call.payload)?;
    let body = if method == "GET" {
        None
    } else {
        Some(Value::Object(call.payload.clone()))
    };
    let url = format!("{base}{path}");
    let host = base
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("sellingpartnerapi-na.amazon.com")
        .to_string();
    let mut headers = BTreeMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers.insert("host".to_string(), host);
    headers.insert("x-amz-access-token".to_string(), access_token);
    let signed_headers = amazon_sigv4_headers(
        method,
        &path,
        &query,
        body.as_ref(),
        &headers,
        &region,
        &aws_access_key_id,
        &aws_secret_access_key,
    )?;
    headers.extend(signed_headers);

    Ok(LiveHttpRequest {
        adapter: "amazon_sp_api",
        method,
        url,
        headers,
        query,
        body,
        secret_refs: vec![
            "AMAZON_SPAPI_ACCESS_TOKEN".to_string(),
            "AMAZON_LWA_ACCESS_TOKEN".to_string(),
            "AWS_ACCESS_KEY_ID".to_string(),
            "AWS_SECRET_ACCESS_KEY".to_string(),
        ],
    })
}

async fn execute_http_request(request: &LiveHttpRequest) -> Result<Value, AppError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
        .build()?;
    let mut headers = HeaderMap::new();
    for (key, value) in &request.headers {
        headers.insert(
            key.parse::<reqwest::header::HeaderName>()
                .map_err(|_| AppError::bad_request(format!("invalid connector header {key}")))?,
            HeaderValue::from_str(value)
                .map_err(|_| AppError::bad_request(format!("invalid connector header {key}")))?,
        );
    }
    let method = reqwest::Method::from_bytes(request.method.as_bytes())
        .map_err(|_| AppError::bad_request("invalid connector HTTP method"))?;
    let mut builder = client
        .request(method, &request.url)
        .headers(headers)
        .query(&request.query);
    if let Some(body) = &request.body {
        builder = builder.json(body);
    }
    let response = builder.send().await?;
    let status = response.status();
    let body = response.text().await?;
    let body_preview: String = body.chars().take(16_384).collect();
    Ok(json!({
        "http_status": status.as_u16(),
        "success": status.is_success(),
        "body_preview": body_preview,
        "body_truncated": body.chars().count() > body_preview.chars().count(),
    }))
}

fn operation_api_name(connector_id: &str, operation: &str) -> &'static str {
    match (connector_id, operation) {
        ("tmall-top", "order-trade-list-read") | ("taobao-open-platform", "order-list-read") => {
            "taobao.trades.sold.get"
        }
        ("tmall-top", "review-feed-read") => "tmall.traderate.feeds.get",
        ("taobao-open-platform", "review-feed-read") => "taobao.traderates.get",
        ("tmall-top", "refund-list-read") | ("taobao-open-platform", "refund-list-read") => {
            "taobao.refunds.receive.get"
        }
        ("tmall-top", "refund-detail-read") => "taobao.refund.get",
        ("tmall-top", "item-detail-read") | ("taobao-open-platform", "item-detail-read") => {
            "taobao.item.seller.get"
        }
        ("tmall-top", "qianniu-task-list-read") => "taobao.qianniu.tasks.get",
        ("tmall-top", "review-explanation-submit")
        | ("taobao-open-platform", "review-reply-submit") => "taobao.traderate.explain.add",
        ("tmall-top", "refund-agree") | ("taobao-open-platform", "refund-agree") => {
            "taobao.rp.refunds.agree"
        }
        ("tmall-top", "refund-refuse") => "taobao.refund.refuse",
        ("tmall-top", "returngoods-agree") => "taobao.rp.returngoods.agree",
        ("tmall-top", "returngoods-refuse") => "taobao.rp.returngoods.refuse",
        ("tmall-top", "qianniu-task-create") => "taobao.qianniu.task.create",
        ("tmall-top", "picture-upload") => "taobao.picture.upload",
        ("tmall-top", "content-media-secret-upload") => "taobao.content.media.upload.secret",
        ("tmall-top", "content-media-public-upload") => "taobao.content.media.upload.pub",
        ("tmall-top", "content-video-publish") => "taobao.content.video.publishx",
        ("tmall-top", "item-fast-update") => "alibaba.item.edit.fastupdate",
        ("tiktok-shop-open-api", "order-list-read") => "tiktok.shop.orders.search",
        ("tiktok-shop-open-api", "product-list-read") => "tiktok.shop.products.search",
        ("tiktok-shop-open-api", "review-list-read") => "tiktok.shop.reviews.search",
        ("tiktok-shop-open-api", "return-list-read") => "tiktok.shop.returns.search",
        ("tiktok-shop-open-api", "review-reply-submit") => "tiktok.shop.review.reply",
        ("tiktok-shop-open-api", "return-refund-approve") => "tiktok.shop.return.refund.approve",
        ("xiaohongshu-shop", "order-list-read") => "xhs.shop.order.list",
        ("xiaohongshu-shop", "product-list-read") => "xhs.shop.product.list",
        ("xiaohongshu-shop", "comment-list-read") => "xhs.shop.comment.list",
        ("xiaohongshu-shop", "after-sales-list-read") => "xhs.shop.aftersales.list",
        ("xiaohongshu-shop", "comment-reply-submit") => "xhs.shop.comment.reply",
        ("xiaohongshu-shop", "after-sales-refund-approve") => "xhs.shop.aftersales.refund.approve",
        ("amazon-selling-partner-api", "orders-read") => "spapi.orders.v0.getOrders",
        ("amazon-selling-partner-api", "listings-read") => "spapi.listings.items.getListingsItem",
        ("amazon-selling-partner-api", "returns-read") => "spapi.returns.getReturns",
        ("amazon-selling-partner-api", "reports-read") => "spapi.reports.getReports",
        ("amazon-selling-partner-api", "listing-content-update") => {
            "spapi.listings.items.patchListingsItem"
        }
        ("amazon-selling-partner-api", "return-case-approve") => "spapi.returns.approveReturn",
        _ => "native.connector.operation",
    }
}

fn amazon_default_path(operation: &str) -> &'static str {
    match operation {
        "orders-read" => "/orders/v0/orders",
        "reports-read" => "/reports/2021-06-30/reports",
        "listings-read" | "listing-content-update" => "/listings/2021-08-01/items",
        "returns-read" | "return-case-approve" => "/returns/2021-06-30/returns",
        _ => "/",
    }
}

fn endpoint_path(call: &NativeConnectorCall, default_path: &str) -> Result<String, AppError> {
    let path = call
        .payload
        .get("__adapter_endpoint_path")
        .or_else(|| call.payload.get("endpoint_path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_path);
    if !path.starts_with('/') || path.contains("..") {
        return Err(AppError::bad_request(
            "connector endpoint_path must be an absolute safe path",
        ));
    }
    Ok(path.to_string())
}

fn base_url(call: &NativeConnectorCall, env_key: &str, default_url: &str) -> String {
    call.payload
        .get("__adapter_base_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| std::env::var(env_key).ok())
        .unwrap_or_else(|| default_url.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn common_signed_query(call: &NativeConnectorCall, app_key: String) -> BTreeMap<String, String> {
    let mut query = BTreeMap::new();
    query.insert("app_key".to_string(), app_key);
    query.insert("timestamp".to_string(), Utc::now().timestamp().to_string());
    query.insert("operation".to_string(), call.operation.clone());
    query
}

fn payload_query(payload: &Map<String, Value>) -> Result<BTreeMap<String, String>, AppError> {
    let mut query = BTreeMap::new();
    for (key, value) in payload {
        if key.starts_with("__adapter") || value.is_object() || value.is_array() {
            continue;
        }
        query.insert(key.clone(), scalar_to_string(value)?);
    }
    Ok(query)
}

fn alibaba_top_md5_sign(params: &BTreeMap<String, String>, secret: &str) -> String {
    let mut text = String::with_capacity(secret.len() * 2 + params.len() * 16);
    text.push_str(secret);
    for (key, value) in params {
        text.push_str(key);
        text.push_str(value);
    }
    text.push_str(secret);
    let mut hasher = Md5::new();
    hasher.update(text.as_bytes());
    hex::encode_upper(hasher.finalize())
}

fn tiktok_shop_hmac_sha256_sign(
    path: &str,
    query: &BTreeMap<String, String>,
    body: &Value,
    secret: &str,
) -> Result<String, AppError> {
    let mut text = String::new();
    text.push_str(path);
    for (key, value) in query {
        if key != "sign" && key != "access_token" {
            text.push_str(key);
            text.push_str(value);
        }
    }
    text.push_str(&body.to_string());
    hmac_sha256_hex(secret, &text)
}

fn hmac_sha256_hex(secret: &str, text: &str) -> Result<String, AppError> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| AppError::bad_request("invalid HMAC secret"))?;
    mac.update(text.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn amazon_sigv4_headers(
    method: &str,
    path: &str,
    query: &BTreeMap<String, String>,
    body: Option<&Value>,
    base_headers: &BTreeMap<String, String>,
    region: &str,
    access_key_id: &str,
    secret_access_key: &str,
) -> Result<BTreeMap<String, String>, AppError> {
    let now = Utc::now();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = now.format("%Y%m%d").to_string();
    let payload = body.map(Value::to_string).unwrap_or_default();
    let payload_hash = sha256_hex(payload.as_bytes());
    let mut headers = BTreeMap::new();
    for (key, value) in base_headers {
        headers.insert(key.to_ascii_lowercase(), value.trim().to_string());
    }
    headers.insert("x-amz-date".to_string(), amz_date.clone());
    let signed_headers = headers.keys().cloned().collect::<Vec<_>>().join(";");
    let canonical_headers = headers
        .iter()
        .map(|(key, value)| format!("{key}:{value}\n"))
        .collect::<String>();
    let canonical_query = query
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    let canonical_request = format!(
        "{method}\n{path}\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );
    let credential_scope = format!("{date_stamp}/{region}/execute-api/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );
    let signing_key = amazon_sigv4_signing_key(secret_access_key, &date_stamp, region)?;
    let mut mac = HmacSha256::new_from_slice(&signing_key)
        .map_err(|_| AppError::bad_request("invalid AWS signing key"))?;
    mac.update(string_to_sign.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={access_key_id}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );
    Ok(BTreeMap::from([
        ("x-amz-date".to_string(), amz_date),
        ("Authorization".to_string(), authorization),
    ]))
}

fn amazon_sigv4_signing_key(
    secret_access_key: &str,
    date_stamp: &str,
    region: &str,
) -> Result<Vec<u8>, AppError> {
    let k_date = hmac_sha256_bytes(format!("AWS4{secret_access_key}").as_bytes(), date_stamp)?;
    let k_region = hmac_sha256_bytes(&k_date, region)?;
    let k_service = hmac_sha256_bytes(&k_region, "execute-api")?;
    hmac_sha256_bytes(&k_service, "aws4_request")
}

fn hmac_sha256_bytes(key: &[u8], text: &str) -> Result<Vec<u8>, AppError> {
    let mut mac =
        HmacSha256::new_from_slice(key).map_err(|_| AppError::bad_request("invalid HMAC key"))?;
    mac.update(text.as_bytes());
    Ok(mac.finalize().into_bytes().to_vec())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = <Sha256 as sha2::Digest>::digest(bytes);
    hex::encode(digest)
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn scalar_to_string(value: &Value) -> Result<String, AppError> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Null => Ok(String::new()),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).map_err(AppError::from),
    }
}

fn string_field(value: &Value, key: &str) -> Result<String, AppError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| AppError::bad_request(format!("{key} is required")))
}

fn connector_secret(name: &str, resolve: bool) -> Result<String, AppError> {
    if !resolve {
        return Ok(format!("__MANDOFORGE_SECRET_REF:{name}__"));
    }
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::forbidden(format!("connector secret {name} is not configured")))
}

fn env_bool(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe {
                if let Some(previous) = self.previous.as_ref() {
                    std::env::set_var(self.key, previous);
                } else {
                    std::env::remove_var(self.key);
                }
            }
        }
    }

    #[test]
    fn alibaba_top_request_signs_without_leaking_secrets() {
        let _lock = env_lock().lock().expect("env lock");
        let _app_key = EnvGuard::set("TMALL_TOP_APP_KEY", "app-key");
        let _app_secret = EnvGuard::set("TMALL_TOP_APP_SECRET", "app-secret");
        let _session = EnvGuard::set("TMALL_TOP_SESSION", "session-token");

        let call = NativeConnectorCall::from_args(&json!({
            "connector_id": "tmall-top",
            "operation": "refund-agree",
            "dry_run": true,
            "payload": {"refund_id": "R1", "tid": "T1", "oid": "O1"}
        }))
        .expect("call");
        let request = build_alibaba_top_request(&call, true).expect("request");
        assert_eq!(request.adapter, "alibaba_top");
        assert_eq!(
            request.query.get("method").map(String::as_str),
            Some("taobao.rp.refunds.agree")
        );
        assert!(request.query.contains_key("sign"));
        let redacted = request.redacted();
        assert!(!redacted.to_string().contains("app-secret"));
        assert!(!redacted.to_string().contains("session-token"));
    }

    #[tokio::test]
    async fn disabled_live_adapter_returns_redacted_prepared_request() {
        let _lock = env_lock().lock().expect("env lock");
        let _app_key = EnvGuard::set("TAOBAO_TOP_APP_KEY", "app-key");
        let _app_secret = EnvGuard::set("TAOBAO_TOP_APP_SECRET", "app-secret");
        let _session = EnvGuard::set("TAOBAO_TOP_SESSION", "session-token");

        let result = execute_ecommerce_connector_call(&json!({
            "connector_id": "taobao-open-platform",
            "operation": "review-reply-submit",
            "payload": {"oid": "O1", "tid": "T1", "reply": "thanks"}
        }))
        .await
        .expect("prepared request");
        assert_eq!(result["status"], json!("live_disabled"));
        assert_eq!(result["request"]["adapter"], json!("alibaba_top"));
        assert!(!result.to_string().contains("session-token"));
    }

    #[test]
    fn native_connector_production_readiness_fails_closed_without_customer_evidence() {
        let _lock = env_lock().lock().expect("env lock");
        let mut env_keys = BTreeSet::from([LIVE_ENABLED_ENV, ECOMMERCE_LIVE_ENABLED_ENV]);
        for spec in native_connector_production_specs() {
            for key in spec.required_secret_refs {
                env_keys.insert(key);
            }
            env_keys.insert(spec.sandbox_base_url_env);
            env_keys.insert(spec.live_base_url_env);
            env_keys.insert(spec.token_refresh_controller_env);
            env_keys.insert(spec.rate_limit_policy_env);
            env_keys.insert(spec.reconciliation_controller_env);
            env_keys.insert(spec.webhook_ingestion_env);
            env_keys.insert(spec.compensation_policy_env);
            env_keys.insert(spec.deployment_evidence_archive_env);
        }
        let _guards = env_keys
            .into_iter()
            .map(|key| EnvGuard::set(key, ""))
            .collect::<Vec<_>>();

        let readiness = build_native_connector_production_readiness();

        assert_eq!(readiness.status, "blocked");
        assert!(!readiness.live_enabled);
        assert_eq!(readiness.connector_count, 5);
        assert_eq!(readiness.ready_connector_count, 0);
        assert_eq!(readiness.blocked_connector_count, 5);
        assert!(
            readiness
                .connectors
                .iter()
                .any(|connector| connector.connector_id == "tmall-top")
        );
        for connector in &readiness.connectors {
            assert_eq!(connector.status, "blocked");
            assert_eq!(connector.required_evidence_class, "customer_grade");
            assert!(
                connector
                    .checks
                    .iter()
                    .any(|check| check.id == "secret-redaction" && check.status == "ready")
            );
            assert!(
                connector
                    .checks
                    .iter()
                    .any(|check| check.id == "idempotency-reconciliation"
                        && check.status == "blocked")
            );
            assert!(connector.checks.iter().any(|check| check.id
                == "archived-deployment-evidence"
                && check.status == "blocked"));
        }
    }

    #[test]
    fn native_connector_production_readiness_requires_archived_customer_grade_evidence() {
        let _lock = env_lock().lock().expect("env lock");
        let mut env_keys = BTreeSet::from([LIVE_ENABLED_ENV, ECOMMERCE_LIVE_ENABLED_ENV]);
        for spec in native_connector_production_specs() {
            for key in spec.required_secret_refs {
                env_keys.insert(key);
            }
            env_keys.insert(spec.sandbox_base_url_env);
            env_keys.insert(spec.live_base_url_env);
            env_keys.insert(spec.token_refresh_controller_env);
            env_keys.insert(spec.rate_limit_policy_env);
            env_keys.insert(spec.reconciliation_controller_env);
            env_keys.insert(spec.webhook_ingestion_env);
            env_keys.insert(spec.compensation_policy_env);
            env_keys.insert(spec.deployment_evidence_archive_env);
        }
        let _guards = env_keys
            .into_iter()
            .map(|key| EnvGuard::set(key, "configured"))
            .collect::<Vec<_>>();
        let _live_gate = EnvGuard::set(LIVE_ENABLED_ENV, "true");

        let readiness = build_native_connector_production_readiness();

        assert_eq!(readiness.status, "ready");
        assert!(readiness.live_enabled);
        assert_eq!(readiness.ready_connector_count, readiness.connector_count);
        assert_eq!(readiness.blocked_connector_count, 0);
        for connector in &readiness.connectors {
            assert_eq!(connector.status, "ready");
            assert_eq!(connector.current_evidence_class, "customer_grade");
            assert!(
                connector
                    .checks
                    .iter()
                    .any(|check| check.id == "archived-deployment-evidence"
                        && check.status == "ready")
            );
        }
    }
}
