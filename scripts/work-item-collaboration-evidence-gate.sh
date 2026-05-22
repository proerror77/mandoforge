#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/work-item-collaboration-evidence}"
SUBJECT="${MANDOFORGE_WORK_ITEM_VERIFY_SUBJECT:-admin-1}"
ROLES="${MANDOFORGE_WORK_ITEM_VERIFY_ROLES:-admin}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)-$$}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "work item collaboration evidence gate requires $1" >&2
    exit 1
  fi
}

api_request() {
  local method="$1"
  local path="$2"
  local payload=""
  if [[ $# -ge 3 ]]; then
    payload="$3"
  fi
  local response_file
  local status
  response_file="$(mktemp)"
  if [[ "$method" == "POST" ]]; then
    status="$(
      curl -sS -o "$response_file" -w "%{http_code}" -X POST "$BASE_URL$path" \
        "${auth_headers[@]}" \
        -H 'content-type: application/json' \
        -d "$payload"
    )"
  else
    status="$(
      curl -sS -o "$response_file" -w "%{http_code}" "$BASE_URL$path" \
        "${auth_headers[@]}"
    )"
  fi
  if [[ "$status" != 2* ]]; then
    echo "work item collaboration evidence API request failed: $method $path returned HTTP $status" >&2
    sed -n '1,80p' "$response_file" >&2
    rm -f "$response_file"
    exit 1
  fi
  cat "$response_file"
  rm -f "$response_file"
}

api_get() {
  local path="$1"
  api_request GET "$path"
}

api_post() {
  local path="$1"
  local payload="{}"
  if [[ $# -ge 2 ]]; then
    payload="$2"
  fi
  api_request POST "$path" "$payload"
}

require_cmd jq
require_cmd curl

mkdir -p "$EVIDENCE_DIR"
curl -fsS "$BASE_URL/healthz" >/dev/null

created_file="$EVIDENCE_DIR/work-item-created.json"
assignment_file="$EVIDENCE_DIR/work-item-assignment-created.json"
review_file="$EVIDENCE_DIR/work-item-review-created.json"
manager_file="$EVIDENCE_DIR/manager-agent-created.json"
specialist_file="$EVIDENCE_DIR/specialist-agent-created.json"
teammate_file="$EVIDENCE_DIR/agent-teammate-created.json"
squad_file="$EVIDENCE_DIR/squad-created.json"
squad_member_file="$EVIDENCE_DIR/squad-member-created.json"
squad_assignment_file="$EVIDENCE_DIR/work-item-squad-assignment-created.json"
manager_session_file="$EVIDENCE_DIR/manager-session-created.json"
manager_plan_file="$EVIDENCE_DIR/manager-plan-created.json"
manager_plan_review_file="$EVIDENCE_DIR/manager-plan-reviewed.json"
list_file="$EVIDENCE_DIR/work-items.json"
assignment_list_file="$EVIDENCE_DIR/work-item-assignments.json"
review_list_file="$EVIDENCE_DIR/work-item-reviews.json"
activity_file="$EVIDENCE_DIR/work-item-activity.json"
teammate_list_file="$EVIDENCE_DIR/agent-teammates.json"
squad_list_file="$EVIDENCE_DIR/squads.json"
squad_member_list_file="$EVIDENCE_DIR/squad-members.json"
manager_plan_list_file="$EVIDENCE_DIR/work-item-manager-plans.json"
handoff_list_file="$EVIDENCE_DIR/manager-session-handoffs.json"
audit_file="$EVIDENCE_DIR/audit-logs.json"
summary_file="$EVIDENCE_DIR/summary.txt"

api_post /api/work-items "$(
  jq -nc --arg run_id "$RUN_ID" '{
    title: ("Agent OS collaboration intake " + $run_id),
    description: "Evidence that external work can enter the Agent OS as a tracked WorkItem.",
    source: "manual",
    source_url: ("mandoforge://work-items/" + $run_id),
    priority: "high",
    metadata: {
      gate: "work-item-collaboration-evidence",
      layer: "collaboration",
      runtime_evidence_required: true
    }
  }'
)" >"$created_file"

work_item_id="$(jq -r '.id' "$created_file")"
api_post "/api/work-items/$work_item_id/assignments" "$(
  jq -nc --arg run_id "$RUN_ID" '{
    assignee_kind: "agent",
    assignee_id: "runtime-specialist",
    role: "owner",
    metadata: {
      gate: "work-item-collaboration-evidence",
      layer: "collaboration",
      routing_reason: ("runtime evidence follow-up " + $run_id),
      requires_runtime_evidence: true
    }
  }'
)" >"$assignment_file"

assignment_id="$(jq -r '.id' "$assignment_file")"
api_post "/api/work-items/$work_item_id/reviews" "$(
  jq -nc --arg run_id "$RUN_ID" '{
    reviewer_kind: "agent",
    reviewer_id: "runtime-reviewer",
    status: "completed",
    decision: "approved",
    summary: ("Collaboration routing reviewed for " + $run_id),
    metadata: {
      gate: "work-item-collaboration-evidence",
      layer: "collaboration",
      review_reason: "runtime evidence checkpoint",
      runtime_evidence_checked: true
    }
  }'
)" >"$review_file"

review_id="$(jq -r '.id' "$review_file")"

api_post /api/agents "$(
  jq -nc --arg run_id "$RUN_ID" '{
    name: ("WorkItem Manager " + $run_id),
    kind: "manager",
    agent_role: "manager",
    provider: "openai-compatible",
    model: "gpt-5.4-mini",
    tools: ["approval.request"],
    semantic_scopes: {
      project_scope: "mandoforge",
      workflow_scope: "work-item-manager-plan"
    }
  }'
)" >"$manager_file"
manager_agent_id="$(jq -r '.id' "$manager_file")"

api_post /api/agents "$(
  jq -nc --arg run_id "$RUN_ID" '{
    name: ("WorkItem Specialist " + $run_id),
    kind: "specialist",
    agent_role: "specialist",
    provider: "openai-compatible",
    model: "gpt-5.4-mini",
    tools: ["agent_cli.exec"],
    semantic_scopes: {
      project_scope: "mandoforge",
      workflow_scope: "work-item-manager-plan"
    }
  }'
)" >"$specialist_file"
specialist_agent_id="$(jq -r '.id' "$specialist_file")"

api_post /api/sessions "$(
  jq -nc --arg agent_id "$manager_agent_id" --arg run_id "$RUN_ID" '{
    agent_id: $agent_id,
    title: ("WorkItem manager planning " + $run_id),
    message: "Create a plan record without starting specialist runtime execution."
  }'
)" >"$manager_session_file"
manager_session_id="$(jq -r '.id' "$manager_session_file")"

api_post "/api/sessions/$manager_session_id/manager-plans" "$(
  jq -nc --arg work_item_id "$work_item_id" --arg specialist_agent_id "$specialist_agent_id" --arg run_id "$RUN_ID" '{
    work_item_id: $work_item_id,
    specialist_agent_id: $specialist_agent_id,
    task_intake: {
      goal: ("Bind manager plan to WorkItem " + $run_id),
      source: "work_item"
    },
    decomposition: {
      steps: ["record plan", "review plan", "assign later"]
    },
    specialist_selection: {
      selected_agent_id: $specialist_agent_id,
      reason: "runtime evidence scope"
    },
    risk_classification: "medium",
    review: {
      required: true,
      status: "pending"
    }
  }'
)" >"$manager_plan_file"
manager_plan_id="$(jq -r '.id' "$manager_plan_file")"

api_post "/api/manager-plans/$manager_plan_id/review" "$(
  jq -nc --arg run_id "$RUN_ID" '{
    status: "approved",
    review: {
      status: "approved",
      summary: ("Manager plan reviewed for " + $run_id)
    }
  }'
)" >"$manager_plan_review_file"

api_post /api/agent-teammates "$(
  jq -nc --arg agent_id "$specialist_agent_id" --arg run_id "$RUN_ID" '{
    agent_id: $agent_id,
    display_name: ("Runtime Teammate " + $run_id),
    handle: ("runtime-teammate-" + $run_id),
    role: "specialist",
    metadata: {
      gate: "work-item-collaboration-evidence",
      layer: "collaboration"
    }
  }'
)" >"$teammate_file"
teammate_id="$(jq -r '.id' "$teammate_file")"

api_post /api/squads "$(
  jq -nc --arg run_id "$RUN_ID" '{
    name: ("Runtime Squad " + $run_id),
    purpose: "Route managed runtime work without becoming a runtime orchestrator.",
    metadata: {
      gate: "work-item-collaboration-evidence",
      layer: "collaboration"
    }
  }'
)" >"$squad_file"
squad_id="$(jq -r '.id' "$squad_file")"

api_post "/api/squads/$squad_id/members" "$(
  jq -nc --arg teammate_id "$teammate_id" '{
    teammate_id: $teammate_id,
    role: "owner",
    metadata: {
      gate: "work-item-collaboration-evidence",
      routing: "runtime"
    }
  }'
)" >"$squad_member_file"
squad_member_id="$(jq -r '.id' "$squad_member_file")"

api_post "/api/work-items/$work_item_id/assignments" "$(
  jq -nc --arg squad_id "$squad_id" --arg run_id "$RUN_ID" '{
    assignee_kind: "squad",
    assignee_id: $squad_id,
    role: "contributor",
    metadata: {
      gate: "work-item-collaboration-evidence",
      layer: "collaboration",
      routing_reason: ("squad routing evidence " + $run_id),
      squad_id: $squad_id
    }
  }'
)" >"$squad_assignment_file"
squad_assignment_id="$(jq -r '.id' "$squad_assignment_file")"

api_get /api/work-items >"$list_file"
api_get "/api/work-items/$work_item_id/assignments" >"$assignment_list_file"
api_get "/api/work-items/$work_item_id/reviews" >"$review_list_file"
api_get "/api/work-items/$work_item_id/activity" >"$activity_file"
api_get /api/agent-teammates >"$teammate_list_file"
api_get /api/squads >"$squad_list_file"
api_get "/api/squads/$squad_id/members" >"$squad_member_list_file"
api_get "/api/work-items/$work_item_id/manager-plans" >"$manager_plan_list_file"
api_get "/api/sessions/$manager_session_id/agent-handoffs" >"$handoff_list_file"
api_get /api/audit-logs >"$audit_file"

jq -e --arg id "$work_item_id" --arg run_id "$RUN_ID" '
  .id == $id
  and .title == ("Agent OS collaboration intake " + $run_id)
  and .status == "open"
  and .priority == "high"
  and .metadata.runtime_evidence_required == true
' "$created_file" >/dev/null

jq -e --arg id "$work_item_id" '
  any(.[]; .id == $id and .metadata.layer == "collaboration")
' "$list_file" >/dev/null

jq -e --arg id "$assignment_id" --arg work_item_id "$work_item_id" '
  .id == $id
  and .work_item_id == $work_item_id
  and .assignee_kind == "agent"
  and .assignee_id == "runtime-specialist"
  and .status == "assigned"
  and .metadata.requires_runtime_evidence == true
' "$assignment_file" >/dev/null

jq -e --arg id "$assignment_id" '
  any(.[]; .id == $id and .metadata.layer == "collaboration")
' "$assignment_list_file" >/dev/null

jq -e --arg id "$squad_assignment_id" --arg work_item_id "$work_item_id" --arg squad_id "$squad_id" '
  .id == $id
  and .work_item_id == $work_item_id
  and .assignee_kind == "squad"
  and .assignee_id == $squad_id
  and .role == "contributor"
  and .metadata.squad_id == $squad_id
' "$squad_assignment_file" >/dev/null

jq -e --arg id "$squad_assignment_id" --arg squad_id "$squad_id" '
  any(.[]; .id == $id and .assignee_kind == "squad" and .assignee_id == $squad_id)
' "$assignment_list_file" >/dev/null

jq -e --arg id "$review_id" --arg work_item_id "$work_item_id" '
  .id == $id
  and .work_item_id == $work_item_id
  and .reviewer_kind == "agent"
  and .reviewer_id == "runtime-reviewer"
  and .status == "completed"
  and .decision == "approved"
  and .metadata.runtime_evidence_checked == true
' "$review_file" >/dev/null

jq -e --arg id "$review_id" '
  any(.[]; .id == $id and .metadata.layer == "collaboration")
' "$review_list_file" >/dev/null

jq -e --arg teammate_id "$teammate_id" --arg specialist_agent_id "$specialist_agent_id" '
  .id == $teammate_id
  and .agent_id == $specialist_agent_id
  and .role == "specialist"
  and .status == "active"
' "$teammate_file" >/dev/null

jq -e --arg id "$teammate_id" '
  any(.[]; .id == $id and .metadata.layer == "collaboration")
' "$teammate_list_file" >/dev/null

jq -e --arg squad_id "$squad_id" '
  .id == $squad_id
  and .status == "active"
  and .metadata.layer == "collaboration"
' "$squad_file" >/dev/null

jq -e --arg id "$squad_id" '
  any(.[]; .id == $id and .metadata.layer == "collaboration")
' "$squad_list_file" >/dev/null

jq -e --arg id "$squad_member_id" --arg squad_id "$squad_id" --arg teammate_id "$teammate_id" '
  .id == $id
  and .squad_id == $squad_id
  and .teammate_id == $teammate_id
  and .role == "owner"
  and .status == "active"
' "$squad_member_file" >/dev/null

jq -e --arg id "$squad_member_id" --arg teammate_id "$teammate_id" '
  length == 1
  and .[0].id == $id
  and .[0].teammate_id == $teammate_id
' "$squad_member_list_file" >/dev/null

jq -e --arg work_item_id "$work_item_id" --arg assignment_id "$assignment_id" --arg review_id "$review_id" --arg manager_plan_id "$manager_plan_id" --arg squad_assignment_id "$squad_assignment_id" --arg squad_id "$squad_id" --arg subject "$SUBJECT" '
  length == 6
  and .[0].work_item_id == $work_item_id
  and .[0].event_type == "work_item.created"
  and .[0].actor_subject == $subject
  and .[1].event_type == "work_item.assignment_created"
  and .[1].subject_type == "work_item_assignment"
  and .[1].subject_id == $assignment_id
  and .[1].metadata.assignee_id == "runtime-specialist"
  and .[2].event_type == "work_item.review_created"
  and .[2].subject_type == "work_item_review"
  and .[2].subject_id == $review_id
  and .[2].metadata.decision == "approved"
  and .[3].event_type == "manager_plan.created"
  and .[3].subject_type == "manager_agent_plan"
  and .[3].subject_id == $manager_plan_id
  and .[4].event_type == "manager_plan.reviewed"
  and .[4].subject_type == "manager_agent_plan"
  and .[4].subject_id == $manager_plan_id
  and .[4].metadata.status == "approved"
  and .[5].event_type == "work_item.assignment_created"
  and .[5].subject_type == "work_item_assignment"
  and .[5].subject_id == $squad_assignment_id
  and .[5].metadata.assignee_kind == "squad"
  and .[5].metadata.assignee_id == $squad_id
' "$activity_file" >/dev/null

jq -e --arg id "$manager_plan_id" --arg work_item_id "$work_item_id" --arg manager_session_id "$manager_session_id" --arg manager_agent_id "$manager_agent_id" --arg specialist_agent_id "$specialist_agent_id" '
  .id == $id
  and .work_item_id == $work_item_id
  and .session_id == $manager_session_id
  and .manager_agent_id == $manager_agent_id
  and .specialist_agent_id == $specialist_agent_id
  and .status == "planned"
  and .risk_classification == "medium"
' "$manager_plan_file" >/dev/null

jq -e --arg id "$manager_plan_id" --arg work_item_id "$work_item_id" '
  .id == $id
  and .work_item_id == $work_item_id
  and .status == "approved"
' "$manager_plan_review_file" >/dev/null

jq -e --arg id "$manager_plan_id" --arg work_item_id "$work_item_id" '
  length == 1
  and .[0].id == $id
  and .[0].work_item_id == $work_item_id
' "$manager_plan_list_file" >/dev/null

jq -e 'length == 0' "$handoff_list_file" >/dev/null

jq -e --arg id "$work_item_id" --arg subject "$SUBJECT" '
  any(.[]; .action == "work_item.created"
    and .resource_type == "work_item"
    and .resource_id == $id
    and .details.subject == $subject
    and .details.source == "manual")
' "$audit_file" >/dev/null

jq -e --arg id "$assignment_id" --arg work_item_id "$work_item_id" '
  any(.[]; .action == "work_item.assignment_created"
    and .resource_type == "work_item_assignment"
    and .resource_id == $id
    and .details.work_item_id == $work_item_id
    and .details.assignee_kind == "agent"
    and .details.assignee_id == "runtime-specialist")
' "$audit_file" >/dev/null

jq -e --arg id "$review_id" --arg work_item_id "$work_item_id" '
  any(.[]; .action == "work_item.review_created"
    and .resource_type == "work_item_review"
    and .resource_id == $id
    and .details.work_item_id == $work_item_id
    and .details.reviewer_kind == "agent"
    and .details.decision == "approved")
' "$audit_file" >/dev/null

jq -e --arg id "$manager_plan_id" --arg work_item_id "$work_item_id" '
  any(.[]; .action == "manager_plan.created"
    and .resource_type == "manager_agent_plan"
    and .resource_id == $id
    and .details.work_item_id == $work_item_id)
' "$audit_file" >/dev/null

jq -e --arg id "$manager_plan_id" --arg work_item_id "$work_item_id" '
  any(.[]; .action == "manager_plan.reviewed"
    and .resource_type == "manager_agent_plan"
    and .resource_id == $id
    and .details.work_item_id == $work_item_id
    and .details.status == "approved")
' "$audit_file" >/dev/null

jq -e --arg id "$teammate_id" --arg agent_id "$specialist_agent_id" '
  any(.[]; .action == "agent_teammate.created"
    and .resource_type == "agent_teammate"
    and .resource_id == $id
    and .details.agent_id == $agent_id)
' "$audit_file" >/dev/null

jq -e --arg id "$squad_id" '
  any(.[]; .action == "squad.created"
    and .resource_type == "squad"
    and .resource_id == $id)
' "$audit_file" >/dev/null

jq -e --arg id "$squad_member_id" --arg squad_id "$squad_id" --arg teammate_id "$teammate_id" '
  any(.[]; .action == "squad.member_added"
    and .resource_type == "squad_member"
    and .resource_id == $id
    and .details.squad_id == $squad_id
    and .details.teammate_id == $teammate_id)
' "$audit_file" >/dev/null

{
  echo "work_item_collaboration_status=validated"
  echo "work_item_id=$work_item_id"
  echo "assignment_id=$assignment_id"
  echo "squad_assignment_id=$squad_assignment_id"
  echo "review_id=$review_id"
  echo "manager_agent_id=$manager_agent_id"
  echo "specialist_agent_id=$specialist_agent_id"
  echo "teammate_id=$teammate_id"
  echo "squad_id=$squad_id"
  echo "squad_member_id=$squad_member_id"
  echo "manager_session_id=$manager_session_id"
  echo "manager_plan_id=$manager_plan_id"
  echo "created_file=$created_file"
  echo "assignment_file=$assignment_file"
  echo "squad_assignment_file=$squad_assignment_file"
  echo "review_file=$review_file"
  echo "manager_file=$manager_file"
  echo "specialist_file=$specialist_file"
  echo "teammate_file=$teammate_file"
  echo "squad_file=$squad_file"
  echo "squad_member_file=$squad_member_file"
  echo "manager_session_file=$manager_session_file"
  echo "manager_plan_file=$manager_plan_file"
  echo "manager_plan_review_file=$manager_plan_review_file"
  echo "list_file=$list_file"
  echo "assignment_list_file=$assignment_list_file"
  echo "review_list_file=$review_list_file"
  echo "activity_file=$activity_file"
  echo "teammate_list_file=$teammate_list_file"
  echo "squad_list_file=$squad_list_file"
  echo "squad_member_list_file=$squad_member_list_file"
  echo "manager_plan_list_file=$manager_plan_list_file"
  echo "handoff_list_file=$handoff_list_file"
  echo "audit_file=$audit_file"
} | tee "$summary_file"

echo "work item collaboration evidence gate ok"
