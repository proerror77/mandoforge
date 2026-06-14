#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_ONTOLOGY_ONBOARDING_GATE_SUBJECT:-ontology-onboarding-gate}"
ROLES="${MANDOFORGE_ONTOLOGY_ONBOARDING_GATE_ROLES:-admin}"
AUTH_TOKEN="${MANDOFORGE_ONTOLOGY_ONBOARDING_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-${MANDOFORGE_WORKER_TOKEN:-}}}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/enterprise-ontology-fast-onboarding}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "enterprise ontology fast-onboarding gate requires $1" >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq

mkdir -p "$EVIDENCE_DIR"

headers=()
if [[ -n "$AUTH_TOKEN" ]]; then
  headers+=(-H "authorization: Bearer $AUTH_TOKEN")
else
  headers+=(
    -H "x-mandoforge-subject: $SUBJECT"
    -H "x-mandoforge-roles: $ROLES"
  )
fi

seed_packs_file="$EVIDENCE_DIR/seed-packs.json"
curl -sS "${headers[@]}" "$BASE_URL/api/ontology/onboarding/seed-packs" \
  | tee "$seed_packs_file" >/dev/null
jq -e 'any(.[]?; .industry == "ecommerce") and any(.[]?; .industry == "insurance")' \
  "$seed_packs_file" >/dev/null

run_file="$EVIDENCE_DIR/demo-run.json"
curl -sS -X POST "${headers[@]}" "$BASE_URL/api/ontology/onboarding/demo-runs" \
  | tee "$run_file" >/dev/null
run_id="$(jq -r '.id // empty' "$run_file")"
if [[ -z "$run_id" ]]; then
  echo "demo onboarding run did not return id" >&2
  exit 1
fi

jq -e '.dataset_count == 8 and .proposal_count >= 32' "$run_file" >/dev/null

schema_understanding_file="$EVIDENCE_DIR/schema-understanding.json"
curl -sS -X POST "${headers[@]}" \
  -H "content-type: application/json" \
  -d "{\"run_id\":\"$run_id\",\"max_sample_rows\":5}" \
  "$BASE_URL/api/ontology/intelligence/schema-understanding" \
  | tee "$schema_understanding_file" >/dev/null
jq -e '
  .industry == "ecommerce"
  and .source_mode == "demo_ecommerce"
  and .candidate_count == 8
  and any(.candidates[]?; .table_name == "orders" and .object_type_candidate == "Order" and .recommendation == "draft_ready")
  and any(.candidates[]?; .table_name == "orders" and any(.properties[]?; .field_name == "customer_id" and .semantic_role == "foreign_key"))
  and any(.candidates[]?; .table_name == "orders" and .evidence.llm_status == "not_invoked")
' "$schema_understanding_file" >/dev/null

subgraph_file="$EVIDENCE_DIR/subgraph-order.json"
curl -sS -X POST "${headers[@]}" \
  -H "content-type: application/json" \
  -d "{\"run_id\":\"$run_id\",\"target_object\":\"Order\"}" \
  "$BASE_URL/api/ontology/intelligence/subgraph-proposals" \
  | tee "$subgraph_file" >/dev/null
jq -e '
  .industry == "ecommerce"
  and .source_mode == "demo_ecommerce"
  and .subgraph_count == 1
  and any(.subgraphs[]?; .target_object == "Order" and .review_status == "pending")
  and any(.subgraphs[]?.members[]?; .name == "Customer")
  and any(.subgraphs[]?.members[]?; .name == "Order")
  and any(.subgraphs[]?.members[]?; .name == "OrderLine")
  and any(.subgraphs[]?.members[]?; .name == "SKU")
  and any(.subgraphs[]?.members[]?; .name == "Customer places Order")
  and any(.subgraphs[]?.members[]?; .name == "Order contains OrderLine")
  and any(.subgraphs[]?.members[]?; .name == "GMV")
  and any(.subgraphs[]?.members[]?; .name == "refund_order")
  and all(.subgraphs[]?.members[]?; .proposal_id != null)
' "$subgraph_file" >/dev/null

client_object_file="$EVIDENCE_DIR/client-semantic-object.json"
curl -sS -X POST "${headers[@]}" \
  -H "content-type: application/json" \
  -d '{
    "object_type":"business_object",
    "object_key":"commerce.client",
    "title":"Client",
    "summary":"Gate fixture for entity resolution.",
    "content":{"object_type":"Client","domain_scope":"commerce"},
    "semantic_scopes":{"domain_scope":"commerce","workflow_scope":"entity-resolution-gate","memory_scope":"ontology","share_policy":"test"},
    "source_uri":"mandoforge://gate/entity-resolution/client",
    "provenance":{"source":"verify-enterprise-ontology-fast-onboarding"},
    "trust_level":"source_attested",
    "freshness":"current",
    "status":"active"
  }' \
  "$BASE_URL/api/semantic-objects" \
  | tee "$client_object_file" >/dev/null

entity_resolution_file="$EVIDENCE_DIR/entity-resolution-customer.json"
curl -sS -X POST "${headers[@]}" \
  -H "content-type: application/json" \
  -d '{"candidate_name":"Customer","candidate_object_type":"Customer","domain_scope":"commerce"}' \
  "$BASE_URL/api/ontology/intelligence/entity-resolution" \
  | tee "$entity_resolution_file" >/dev/null
jq -e '
  .candidate_count == 1
  and .candidates[0].decision.is_duplicate == true
  and .candidates[0].decision.decision == "merge_into_existing"
  and .candidates[0].decision.review_required == true
  and any(.candidates[0].retrieval_hits[]?; .title == "Client" and (.match_reasons | index("alias_synonym")))
' "$entity_resolution_file" >/dev/null

dag_file="$EVIDENCE_DIR/dag.json"
curl -sS "${headers[@]}" "$BASE_URL/api/ontology/onboarding/runs/$run_id/dag" \
  | tee "$dag_file" >/dev/null
jq -e '
  .mode == "pipeline_mapping_v2"
  and any(.execution_levels[]?; (.node_ids | index("metadata_scan")) and (.node_ids | index("schema_profile")))
  and any(.nodes[]?; .id == "review_graph" and .node_type == "visual_review")
' "$dag_file" >/dev/null

prompt_packet_file="$EVIDENCE_DIR/prompt-packet.json"
curl -sS "${headers[@]}" "$BASE_URL/api/ontology/onboarding/runs/$run_id/prompt-packet" \
  | tee "$prompt_packet_file" >/dev/null
jq -e '
  .industry == "ecommerce"
  and (.curated_datasets | length) == 8
  and any(.allowed_ontology_triples[]?; .from_object == "Customer" and .to_object == "Order")
  and any(.policy_reminders[]?; contains("approval-gated"))
' "$prompt_packet_file" >/dev/null

orders_curated_id="$(jq -r '.curated_datasets[] | select(.table_name == "orders") | .id' "$prompt_packet_file")"
curated_review_file="$EVIDENCE_DIR/curated-orders-review.json"
curl -sS -X POST "${headers[@]}" \
  -H "content-type: application/json" \
  -d '{"decision":"reject","reason":"gate exercises curated dataset review boundary"}' \
  "$BASE_URL/api/ontology/onboarding/curated-datasets/$orders_curated_id/review" \
  | tee "$curated_review_file" >/dev/null
jq -e '.review_status == "rejected" and .reviewer_metadata.reviewer != null' \
  "$curated_review_file" >/dev/null

run_after_curated_review_file="$EVIDENCE_DIR/demo-run-after-curated-review.json"
curl -sS "${headers[@]}" "$BASE_URL/api/ontology/onboarding/runs/$run_id" \
  | tee "$run_after_curated_review_file" >/dev/null
jq -e 'any(.proposals[]?; .name == "Order" and .recommendation == "needs_more_evidence")' \
  "$run_after_curated_review_file" >/dev/null

review_graph_file="$EVIDENCE_DIR/review-graph.json"
curl -sS "${headers[@]}" "$BASE_URL/api/ontology/onboarding/runs/$run_id/review-graph" \
  | tee "$review_graph_file" >/dev/null
jq -e '
  any(.nodes[]?; .node_type == "object" and .label == "Customer" and .source_proposal_id != null)
  and any(.nodes[]?; .node_type == "subgraph" and .label == "Order business subgraph" and .source_proposal_id != null)
  and any(.nodes[]?; .node_type == "merge_candidate" and .label == "Client" and .source_proposal_id != null)
  and any(.nodes[]?; .node_type == "logic")
  and any(.nodes[]?; .node_type == "action")
  and any(.nodes[]?; .node_type == "tool")
  and any(.edges[]?; .edge_type == "maps_to" and .source_proposal_id != null)
  and any(.edges[]?; .edge_type == "groups" and .source_proposal_id != null)
  and any(.edges[]?; .edge_type == "merge_suggests" and .source_proposal_id != null)
  and any(.edges[]?; .edge_type == "relates_to")
  and any(.edges[]?; .edge_type == "depends_on")
  and any(.edges[]?; .edge_type == "validates")
  and any(.edges[]?; .edge_type == "compiles_to")
' "$review_graph_file" >/dev/null

proposal_ids=()
while IFS= read -r proposal_id; do
  proposal_ids+=("$proposal_id")
done < <(
  jq -r '
    .proposals[]
    | select(.proposal_type == "object" or .proposal_type == "relation" or .proposal_type == "action" or .proposal_type == "logic")
    | .id
  ' "$run_file"
)

if [[ "${#proposal_ids[@]}" -lt 1 ]]; then
  echo "demo onboarding run did not return approvable proposals" >&2
  exit 1
fi

for proposal_id in "${proposal_ids[@]}"; do
  curl -sS -X POST "${headers[@]}" \
    -H "content-type: application/json" \
    -d '{"decision":"approve","reason":"gate-approved demo proposal"}' \
    "$BASE_URL/api/ontology/onboarding/proposals/$proposal_id/review" \
    >"$EVIDENCE_DIR/review-$proposal_id.json"
done

calibration_file="$EVIDENCE_DIR/calibration.json"
curl -sS "${headers[@]}" "$BASE_URL/api/ontology/intelligence/runs/$run_id/calibration" \
  | tee "$calibration_file" >/dev/null
jq -e --arg run_id "$run_id" --argjson expected_count "${#proposal_ids[@]}" '
  .run_id == $run_id
  and .record_count >= $expected_count
  and any(.records[]?; .proposal_type == "object" and .reviewer_status == "approved")
  and any(.records[]?; .proposal_type == "relation" and .reviewer_status == "approved")
  and any(.records[]?; .proposal_type == "action" and .reviewer_status == "approved")
  and any(.buckets[]?; .proposal_type == "object" and .reviewer_status == "approved" and .count >= 1)
  and .threshold_policy.customer_tunable == true
  and .threshold_policy.not_a_global_production_benchmark == true
  and .threshold_policy.configuration_surface.scope == "customer_or_domain_policy"
' "$calibration_file" >/dev/null

materialized_file="$EVIDENCE_DIR/materialized.json"
curl -sS -X POST "${headers[@]}" "$BASE_URL/api/ontology/onboarding/runs/$run_id/materialize" \
  | tee "$materialized_file" >/dev/null
jq -e '.semantic_object_count >= 1 and .semantic_link_count >= 1 and .tool_spec_count >= 1' \
  "$materialized_file" >/dev/null

review_graph_after_file="$EVIDENCE_DIR/review-graph-after-materialize.json"
curl -sS "${headers[@]}" "$BASE_URL/api/ontology/onboarding/runs/$run_id/review-graph" \
  | tee "$review_graph_after_file" >/dev/null
jq -e 'any(.nodes[]?; .node_type == "tool" and .status == "compiled")' \
  "$review_graph_after_file" >/dev/null

tool_specs_file="$EVIDENCE_DIR/tool-specs.json"
curl -sS "${headers[@]}" "$BASE_URL/api/ontology/onboarding/runs/$run_id/tool-specs" \
  | tee "$tool_specs_file" >/dev/null
jq -e 'any(.tool_specs[]?; .name == "commerce.refund_order")' "$tool_specs_file" >/dev/null
jq -e 'any(.tool_specs[]?; .name == "commerce.refund_order" and .read_write_risk == "write_approval_required" and .transaction_profile == "proposal_only" and .execution_mode == "proposal_only" and .source_refs.source_mapping != null)' "$tool_specs_file" >/dev/null

insurance_run_file="$EVIDENCE_DIR/insurance-run.json"
curl -sS -X POST "${headers[@]}" \
  -H "content-type: application/json" \
  -d '{"industry":"insurance","source_mode":"demo_insurance"}' \
  "$BASE_URL/api/ontology/onboarding/runs" \
  | tee "$insurance_run_file" >/dev/null
jq -e '.source_mode == "demo_insurance" and any(.proposals[]?; .name == "approve_claim")' \
  "$insurance_run_file" >/dev/null

insurance_schema_understanding_file="$EVIDENCE_DIR/insurance-schema-understanding.json"
curl -sS -X POST "${headers[@]}" \
  -H "content-type: application/json" \
  -d '{"industry":"insurance","source_mode":"demo_insurance"}' \
  "$BASE_URL/api/ontology/intelligence/schema-understanding" \
  | tee "$insurance_schema_understanding_file" >/dev/null
jq -e '
  .industry == "insurance"
  and .source_mode == "demo_insurance"
  and any(.candidates[]?; .table_name == "claims" and .object_type_candidate == "Claim")
' "$insurance_schema_understanding_file" >/dev/null

summary_file="$EVIDENCE_DIR/summary.txt"
{
  echo "run_id=$run_id"
  echo "seed_packs_file=$seed_packs_file"
  echo "dataset_count=$(jq -r '.dataset_count' "$run_file")"
  echo "proposal_count=$(jq -r '.proposal_count' "$run_file")"
  echo "semantic_object_count=$(jq -r '.semantic_object_count' "$materialized_file")"
  echo "semantic_link_count=$(jq -r '.semantic_link_count' "$materialized_file")"
  echo "tool_spec_count=$(jq -r '.tool_spec_count' "$materialized_file")"
  echo "schema_understanding_file=$schema_understanding_file"
  echo "subgraph_file=$subgraph_file"
  echo "entity_resolution_file=$entity_resolution_file"
  echo "dag_file=$dag_file"
  echo "prompt_packet_file=$prompt_packet_file"
  echo "curated_review_file=$curated_review_file"
  echo "run_after_curated_review_file=$run_after_curated_review_file"
  echo "review_graph_file=$review_graph_file"
  echo "review_graph_after_file=$review_graph_after_file"
  echo "calibration_file=$calibration_file"
  echo "insurance_run_id=$(jq -r '.id' "$insurance_run_file")"
  echo "run_file=$run_file"
  echo "materialized_file=$materialized_file"
  echo "tool_specs_file=$tool_specs_file"
  echo "insurance_run_file=$insurance_run_file"
  echo "insurance_schema_understanding_file=$insurance_schema_understanding_file"
} >"$summary_file"

cat "$summary_file"
echo "enterprise ontology fast-onboarding gate ok"
