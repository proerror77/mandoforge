#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/ecommerce-platform-closed-loop}"

if ! command -v ruby >/dev/null 2>&1; then
  echo "ecommerce platform closed-loop verification requires ruby" >&2
  exit 1
fi

mkdir -p "$EVIDENCE_DIR"

EVIDENCE_DIR="$EVIDENCE_DIR" ruby <<'RUBY'
require "json"
require "set"
require "yaml"

SPECS = [
  {
    pack: "ecommerce-tmall",
    connector_id: "tmall-top",
    connector_path: "packs/ecommerce-tmall/connectors/tmall-top.yaml",
    sandbox_env: "MANDOFORGE_TMALL_TOP_SANDBOX_BASE_URL",
    live_env: "MANDOFORGE_TMALL_TOP_LIVE_BASE_URL",
    token_env: "MANDOFORGE_TMALL_TOP_TOKEN_REFRESH_CONTROLLER_URL",
    rate_env: "MANDOFORGE_TMALL_TOP_RATE_LIMIT_POLICY",
    reconciliation_env: "MANDOFORGE_TMALL_TOP_RECONCILIATION_CONTROLLER_URL",
    webhook_env: "MANDOFORGE_TMALL_TOP_WEBHOOK_INGESTION_URL",
    compensation_env: "MANDOFORGE_TMALL_TOP_COMPENSATION_POLICY"
  },
  {
    pack: "ecommerce-taobao",
    connector_id: "taobao-open-platform",
    connector_path: "packs/ecommerce-taobao/connectors/taobao-open-platform.yaml",
    sandbox_env: "MANDOFORGE_TAOBAO_TOP_SANDBOX_BASE_URL",
    live_env: "MANDOFORGE_TAOBAO_TOP_LIVE_BASE_URL",
    token_env: "MANDOFORGE_TAOBAO_TOP_TOKEN_REFRESH_CONTROLLER_URL",
    rate_env: "MANDOFORGE_TAOBAO_TOP_RATE_LIMIT_POLICY",
    reconciliation_env: "MANDOFORGE_TAOBAO_TOP_RECONCILIATION_CONTROLLER_URL",
    webhook_env: "MANDOFORGE_TAOBAO_TOP_WEBHOOK_INGESTION_URL",
    compensation_env: "MANDOFORGE_TAOBAO_TOP_COMPENSATION_POLICY"
  },
  {
    pack: "ecommerce-xiaohongshu",
    connector_id: "xiaohongshu-shop",
    connector_path: "packs/ecommerce-xiaohongshu/connectors/xiaohongshu-shop.yaml",
    sandbox_env: "MANDOFORGE_XHS_SANDBOX_BASE_URL",
    live_env: "MANDOFORGE_XHS_LIVE_BASE_URL",
    token_env: "MANDOFORGE_XHS_TOKEN_REFRESH_CONTROLLER_URL",
    rate_env: "MANDOFORGE_XHS_RATE_LIMIT_POLICY",
    reconciliation_env: "MANDOFORGE_XHS_RECONCILIATION_CONTROLLER_URL",
    webhook_env: "MANDOFORGE_XHS_WEBHOOK_INGESTION_URL",
    compensation_env: "MANDOFORGE_XHS_COMPENSATION_POLICY"
  },
  {
    pack: "ecommerce-tiktok-shop",
    connector_id: "tiktok-shop-open-api",
    connector_path: "packs/ecommerce-tiktok-shop/connectors/tiktok-shop-open-api.yaml",
    sandbox_env: "MANDOFORGE_TIKTOK_SHOP_SANDBOX_BASE_URL",
    live_env: "MANDOFORGE_TIKTOK_SHOP_LIVE_BASE_URL",
    token_env: "MANDOFORGE_TIKTOK_SHOP_TOKEN_REFRESH_CONTROLLER_URL",
    rate_env: "MANDOFORGE_TIKTOK_SHOP_RATE_LIMIT_POLICY",
    reconciliation_env: "MANDOFORGE_TIKTOK_SHOP_RECONCILIATION_CONTROLLER_URL",
    webhook_env: "MANDOFORGE_TIKTOK_SHOP_WEBHOOK_INGESTION_URL",
    compensation_env: "MANDOFORGE_TIKTOK_SHOP_COMPENSATION_POLICY"
  },
  {
    pack: "ecommerce-amazon",
    connector_id: "amazon-selling-partner-api",
    connector_path: "packs/ecommerce-amazon/connectors/amazon-selling-partner-api.yaml",
    sandbox_env: "MANDOFORGE_AMAZON_SPAPI_SANDBOX_BASE_URL",
    live_env: "MANDOFORGE_AMAZON_SPAPI_LIVE_BASE_URL",
    token_env: "MANDOFORGE_AMAZON_SPAPI_TOKEN_REFRESH_CONTROLLER_URL",
    rate_env: "MANDOFORGE_AMAZON_SPAPI_RATE_LIMIT_POLICY",
    reconciliation_env: "MANDOFORGE_AMAZON_SPAPI_RECONCILIATION_CONTROLLER_URL",
    webhook_env: "MANDOFORGE_AMAZON_SPAPI_WEBHOOK_INGESTION_URL",
    compensation_env: "MANDOFORGE_AMAZON_SPAPI_COMPENSATION_POLICY"
  }
].freeze

REQUIRED_BIND_FIELDS = %w[
  tenant_id
  workspace_id
  connector_id
  api_name
  operation_id
  object_id
  payload_digest
  approval_commit_token
].freeze

def require_file(path, failures)
  failures << "#{path} missing" unless File.file?(path) && File.size(path).positive?
end

def assert(failures, condition, message)
  failures << message unless condition
end

def yaml_file(path)
  YAML.load_file(path)
rescue Psych::Exception => error
  raise "#{path} is not valid YAML: #{error.message}"
end

failures = []
summary = {
  source: "verify-ecommerce-platform-closed-loop",
  required_evidence_class: "customer_grade",
  platform_count: SPECS.length,
  platforms: []
}

require_file("packs/ecommerce-core/package.yaml", failures)
core = yaml_file("packs/ecommerce-core/package.yaml")
assert(failures, core["kind"] == "DomainPack", "ecommerce-core must be a DomainPack")
assert(failures, core.dig("semantic_scopes", "domain_scope") == "ecommerce", "ecommerce-core domain_scope must be ecommerce")

SPECS.each do |spec|
  pack_path = "packs/#{spec[:pack]}/package.yaml"
  require_file(pack_path, failures)
  require_file(spec[:connector_path], failures)

  pack = yaml_file(pack_path)
  connector_doc = yaml_file(spec[:connector_path])
  connector = Array(connector_doc["connectors"]).find { |item| item["id"] == spec[:connector_id] }
  assert(failures, !connector.nil?, "#{spec[:connector_path]} must declare #{spec[:connector_id]}")
  next unless connector

  platform = {
    pack: spec[:pack],
    connector_id: spec[:connector_id],
    connector_path: spec[:connector_path],
    read_operation_count: Array(connector["read_operations"]).length,
    write_operation_count: Array(connector["write_operations"]).length,
    action_count: Array(pack["actions"]).length
  }
  summary[:platforms] << platform

  assert(failures, pack["kind"] == "DomainPack", "#{spec[:pack]} must be a DomainPack")
  assert(failures, pack.dig("semantic_scopes", "domain_scope") == "ecommerce", "#{spec[:pack]} domain_scope must be ecommerce")
  assert(failures, Array(pack["extends"]).any? { |ext| ext["id"] == "ecommerce-core" && ext["required"] == true }, "#{spec[:pack]} must extend ecommerce-core")
  assert(failures, Array(pack["release_gates"]).any? { |gate| gate["id"] == "connector-readiness" && gate["required"] == true }, "#{spec[:pack]} must require connector-readiness release gate")
  assert(failures, Array(pack["release_gates"]).any? { |gate| gate["id"] == "approval-policy" && gate["required"] == true }, "#{spec[:pack]} must require approval-policy release gate")

  adapter = connector["adapter_contract"] || {}
  assert(failures, adapter["runtime"] == "native.connector.call", "#{spec[:connector_id]} adapter runtime must be native.connector.call")
  assert(failures, adapter["live_execution"] == "approval_commit_only", "#{spec[:connector_id]} live execution must be approval_commit_only")
  assert(failures, adapter["dry_run_supported"] == true, "#{spec[:connector_id]} must support dry_run")
  assert(failures, adapter.dig("live_runtime", "enable_env") == "MANDOFORGE_NATIVE_CONNECTOR_LIVE_ENABLED", "#{spec[:connector_id]} must use the shared live runtime gate")
  assert(failures, Array(adapter.dig("live_runtime", "required_env")).any?, "#{spec[:connector_id]} live runtime must declare required_env")

  read_operations = Array(connector["read_operations"])
  write_operations = Array(connector["write_operations"])
  assert(failures, read_operations.any?, "#{spec[:connector_id]} must declare read_operations")
  assert(failures, write_operations.any?, "#{spec[:connector_id]} must declare write_operations")

  read_operations.each do |operation|
    assert(failures, operation["api_name"].to_s != "", "#{spec[:connector_id]} read operation #{operation["id"]} missing api_name")
    assert(failures, Array(operation.dig("request_contract", "required_fields")).any?, "#{spec[:connector_id]} read operation #{operation["id"]} missing request required_fields")
    assert(failures, Array(operation.dig("response_contract", "required_fields")).any?, "#{spec[:connector_id]} read operation #{operation["id"]} missing response required_fields")
    assert(failures, operation.dig("response_contract", "evidence_id_field").to_s != "", "#{spec[:connector_id]} read operation #{operation["id"]} missing evidence_id_field")
  end

  write_ids = write_operations.map { |operation| operation["id"] }.to_set
  write_operations.each do |operation|
    assert(failures, operation["approval_required"] == true, "#{spec[:connector_id]} write operation #{operation["id"]} must require approval")
    assert(failures, operation["api_name"].to_s != "", "#{spec[:connector_id]} write operation #{operation["id"]} missing api_name")
    assert(failures, Array(operation.dig("request_contract", "required_fields")).any?, "#{spec[:connector_id]} write operation #{operation["id"]} missing request required_fields")
    assert(failures, Array(operation.dig("request_contract", "forbidden_fields")).any?, "#{spec[:connector_id]} write operation #{operation["id"]} missing forbidden_fields")
    assert(failures, Array(operation.dig("response_contract", "required_fields")).any?, "#{spec[:connector_id]} write operation #{operation["id"]} missing response required_fields")
  end

  Array(pack["actions"]).each do |action_ref|
    action_path = "packs/#{spec[:pack]}/#{action_ref["path"]}"
    require_file(action_path, failures)
    action = yaml_file(action_path)
    assert(failures, action["connector_id"] == spec[:connector_id], "#{action_path} must bind #{spec[:connector_id]}")
    assert(failures, write_ids.include?(action["operation_id"]), "#{action_path} operation_id must reference a write operation")
    assert(failures, action.dig("effects", "native_connector_call") == "native.connector.call", "#{action_path} must execute through native.connector.call")
    assert(failures, action.dig("approval", "approval_commit_token_required") == true, "#{action_path} must require approval_commit_token")
    assert(failures, action.dig("approval", "payload_digest_required") == true, "#{action_path} must require payload_digest")
  end

  readiness = connector["readiness_probes"] || {}
  assert(failures, Array(readiness.dig("credential_probe", "required_secrets")).any?, "#{spec[:connector_id]} must declare credential_probe.required_secrets")
  assert(failures, Array(readiness.dig("tenant_scope_probe", "required_fields")).include?("tenant_id"), "#{spec[:connector_id]} tenant scope must require tenant_id")
  assert(failures, Array(readiness.dig("tenant_scope_probe", "required_fields")).include?("workspace_id"), "#{spec[:connector_id]} tenant scope must require workspace_id")
  assert(failures, Array(readiness.dig("permission_probe", "sample_read_operations")).any?, "#{spec[:connector_id]} must declare sample_read_operations")

  approval = connector["approval_commit_binding"] || {}
  bind_fields = Array(approval["bind_fields"])
  assert(failures, approval["required_for_write_operations"] == true, "#{spec[:connector_id]} must require approval commit binding")
  missing_bind_fields = REQUIRED_BIND_FIELDS - bind_fields
  assert(failures, missing_bind_fields.empty?, "#{spec[:connector_id]} approval binding missing #{missing_bind_fields.join(", ")}")

  production = connector["production_readiness"] || {}
  assert(failures, production["required_evidence_class"] == "customer_grade", "#{spec[:connector_id]} production_readiness must require customer_grade evidence")
  assert(failures, production["fail_closed_without_evidence"] == true, "#{spec[:connector_id]} production_readiness must fail closed")
  assert(failures, production.dig("environment_separation", "sandbox_base_url_env") == spec[:sandbox_env], "#{spec[:connector_id]} sandbox env mismatch")
  assert(failures, production.dig("environment_separation", "live_base_url_env") == spec[:live_env], "#{spec[:connector_id]} live env mismatch")
  assert(failures, production.dig("token_lifecycle", "required") == true, "#{spec[:connector_id]} token lifecycle must be required")
  assert(failures, production.dig("token_lifecycle", "controller_env") == spec[:token_env], "#{spec[:connector_id]} token lifecycle env mismatch")
  assert(failures, production.dig("rate_limit_retry", "required") == true, "#{spec[:connector_id]} rate limit retry must be required")
  assert(failures, production.dig("rate_limit_retry", "policy_env") == spec[:rate_env], "#{spec[:connector_id]} rate limit env mismatch")
  assert(failures, Array(production.dig("rate_limit_retry", "error_taxonomy")).any?, "#{spec[:connector_id]} rate limit retry must declare error taxonomy")
  assert(failures, production.dig("idempotency_reconciliation", "required") == true, "#{spec[:connector_id]} reconciliation must be required")
  assert(failures, production.dig("idempotency_reconciliation", "controller_env") == spec[:reconciliation_env], "#{spec[:connector_id]} reconciliation env mismatch")
  assert(failures, Array(production.dig("idempotency_reconciliation", "idempotency_fields")).include?("payload_digest"), "#{spec[:connector_id]} reconciliation must include payload_digest idempotency")
  assert(failures, production.dig("webhook_ingestion", "required") == true, "#{spec[:connector_id]} webhook ingestion must be required")
  assert(failures, production.dig("webhook_ingestion", "endpoint_env") == spec[:webhook_env], "#{spec[:connector_id]} webhook env mismatch")
  assert(failures, production.dig("compensation", "required") == true, "#{spec[:connector_id]} compensation policy must be required")
  assert(failures, production.dig("compensation", "policy_env") == spec[:compensation_env], "#{spec[:connector_id]} compensation env mismatch")
  assert(failures, production.dig("approval_commit_boundary", "required") == true, "#{spec[:connector_id]} production readiness must require approval commit boundary")
  assert(failures, production.dig("secret_redaction", "required") == true, "#{spec[:connector_id]} production readiness must require secret redaction")
  assert(failures, connector.dig("prompt_injection_boundary", "treat_results_as_data") == true, "#{spec[:connector_id]} must treat connector results as data")
end

summary[:failure_count] = failures.length
summary[:status] = failures.empty? ? "ready" : "failed"

evidence_dir = ENV.fetch("EVIDENCE_DIR")
File.write(File.join(evidence_dir, "closed-loop-summary.json"), JSON.pretty_generate(summary) + "\n")
File.write(
  File.join(evidence_dir, "summary.txt"),
  [
    "ecommerce_platform_closed_loop_status=#{summary[:status]}",
    "required_evidence_class=#{summary[:required_evidence_class]}",
    "platform_count=#{summary[:platform_count]}",
    "failure_count=#{summary[:failure_count]}",
    "summary_json=#{File.join(evidence_dir, "closed-loop-summary.json")}"
  ].join("\n") + "\n"
)

puts File.read(File.join(evidence_dir, "summary.txt"))
if failures.any?
  warn "ecommerce platform closed-loop verification failed:"
  failures.each { |failure| warn "- #{failure}" }
  exit 1
end
RUBY

echo "ecommerce platform closed-loop contract ok"
