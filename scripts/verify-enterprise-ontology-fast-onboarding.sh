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
  and any(.nodes[]?; .node_type == "logic")
  and any(.nodes[]?; .node_type == "action")
  and any(.nodes[]?; .node_type == "tool")
  and any(.edges[]?; .edge_type == "maps_to" and .source_proposal_id != null)
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
jq -e 'any(.tool_specs[]?; .name == "commerce.refund_order" and .read_write_risk == "write_approval_required" and .source_refs.source_mapping != null)' "$tool_specs_file" >/dev/null

insurance_run_file="$EVIDENCE_DIR/insurance-run.json"
curl -sS -X POST "${headers[@]}" \
  -H "content-type: application/json" \
  -d '{"industry":"insurance","source_mode":"demo_insurance"}' \
  "$BASE_URL/api/ontology/onboarding/runs" \
  | tee "$insurance_run_file" >/dev/null
jq -e '.source_mode == "demo_insurance" and any(.proposals[]?; .name == "approve_claim")' \
  "$insurance_run_file" >/dev/null

summary_file="$EVIDENCE_DIR/summary.txt"
{
  echo "run_id=$run_id"
  echo "seed_packs_file=$seed_packs_file"
  echo "dataset_count=$(jq -r '.dataset_count' "$run_file")"
  echo "proposal_count=$(jq -r '.proposal_count' "$run_file")"
  echo "semantic_object_count=$(jq -r '.semantic_object_count' "$materialized_file")"
  echo "semantic_link_count=$(jq -r '.semantic_link_count' "$materialized_file")"
  echo "tool_spec_count=$(jq -r '.tool_spec_count' "$materialized_file")"
  echo "dag_file=$dag_file"
  echo "prompt_packet_file=$prompt_packet_file"
  echo "curated_review_file=$curated_review_file"
  echo "run_after_curated_review_file=$run_after_curated_review_file"
  echo "review_graph_file=$review_graph_file"
  echo "review_graph_after_file=$review_graph_after_file"
  echo "insurance_run_id=$(jq -r '.id' "$insurance_run_file")"
  echo "run_file=$run_file"
  echo "materialized_file=$materialized_file"
  echo "tool_specs_file=$tool_specs_file"
  echo "insurance_run_file=$insurance_run_file"
} >"$summary_file"

cat "$summary_file"
echo "enterprise ontology fast-onboarding gate ok"
