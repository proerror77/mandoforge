use serde_json::Value;

use crate::OntologyActionTransactionProfile;

pub(crate) fn ontology_default_action_transaction_profile(
    effects: &Value,
    executor: &Value,
) -> OntologyActionTransactionProfile {
    if ontology_action_has_effects(effects) && ontology_action_executor_is_cross_system(executor) {
        OntologyActionTransactionProfile::ProposalOnly
    } else {
        OntologyActionTransactionProfile::LocalSerializable
    }
}

pub(crate) fn ontology_action_has_effects(effects: &Value) -> bool {
    effects
        .as_array()
        .map(|values| !values.is_empty())
        .unwrap_or(false)
}

pub(crate) fn ontology_action_executor_is_cross_system(executor: &Value) -> bool {
    matches!(
        executor.get("type").and_then(Value::as_str),
        Some("http_api" | "external_api" | "webhook" | "mcp_connector")
    )
}
