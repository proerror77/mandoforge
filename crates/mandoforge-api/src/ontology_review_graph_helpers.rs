use serde_json::Value;
use uuid::Uuid;

use crate::{OntologyOnboardingProposalDraft, SubgraphProposalMember, ontology_slug};

pub(crate) fn ontology_graph_node_id_for_subgraph_member(
    member: &SubgraphProposalMember,
) -> Option<String> {
    match member.proposal_type.as_str() {
        "object" => Some(ontology_graph_object_id(&member.name)),
        "relation" => None,
        "metric" => Some(ontology_graph_metric_id(&member.name)),
        "logic" | "logic_rule" => Some(ontology_graph_logic_id(&member.name)),
        "action" => Some(ontology_graph_action_id(&member.name)),
        _ => None,
    }
}

pub(crate) fn ontology_graph_dataset_id(table_name: &str) -> String {
    format!("dataset:{}", ontology_slug(table_name))
}

pub(crate) fn ontology_graph_object_id(object_name: &str) -> String {
    format!("object:{}", ontology_slug(object_name))
}

pub(crate) fn ontology_graph_metric_id(metric_name: &str) -> String {
    format!("metric:{}", ontology_slug(metric_name))
}

pub(crate) fn ontology_graph_logic_id(logic_name: &str) -> String {
    format!("logic:{}", ontology_slug(logic_name))
}

pub(crate) fn ontology_graph_action_id(action_name: &str) -> String {
    format!("action:{}", ontology_slug(action_name))
}

pub(crate) fn ontology_graph_tool_id(tool_namespace: &str, action_name: &str) -> String {
    format!(
        "tool:{}:{}",
        ontology_slug(tool_namespace),
        ontology_slug(action_name)
    )
}

pub(crate) fn ontology_graph_subgraph_id(target_object: &str) -> String {
    format!("subgraph:{}", ontology_slug(target_object))
}

pub(crate) fn ontology_graph_merge_candidate_id(proposal_id: Uuid, object_id: Uuid) -> String {
    format!("merge:{proposal_id}:{object_id}")
}

pub(crate) fn ontology_proposal_risk(proposal: &OntologyOnboardingProposalDraft) -> String {
    if proposal.proposal_type == "action"
        && (proposal
            .content
            .get("transaction_profile")
            .and_then(Value::as_str)
            == Some("proposal_only")
            || proposal
                .content
                .get("policy")
                .and_then(|policy| policy.get("approval_required"))
                .and_then(Value::as_bool)
                .unwrap_or(true))
    {
        "approval_required".to_string()
    } else if proposal
        .evidence
        .get("pii_candidates")
        .and_then(Value::as_array)
        .map(|values| !values.is_empty())
        .unwrap_or(false)
    {
        "pii_review".to_string()
    } else if proposal.confidence < 0.90 {
        "needs_review".to_string()
    } else {
        "low".to_string()
    }
}
