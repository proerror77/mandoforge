use serde_json::Value;

use crate::{
    OntologyOnboardingDataset, OntologyOnboardingField, OntologySeedActionMapping,
    OntologySeedMetricMapping, OntologySeedObjectMapping, OntologySeedRelationMapping,
    ontology_default_action_transaction_profile,
};

pub(crate) fn ontology_demo_dataset(
    table_name: &str,
    source_system: &str,
    source_object: &str,
    field_defs: Vec<(&str, &str)>,
    rows: Vec<Value>,
) -> OntologyOnboardingDataset {
    let fields = field_defs
        .into_iter()
        .map(|(name, field_type)| OntologyOnboardingField {
            name: name.to_string(),
            field_type: field_type.to_string(),
            sample_values: rows
                .iter()
                .filter_map(|row| row.get(name).cloned())
                .filter(|value| !value.is_null())
                .take(3)
                .collect(),
        })
        .collect();
    OntologyOnboardingDataset {
        table_name: table_name.to_string(),
        source_system: source_system.to_string(),
        source_object: source_object.to_string(),
        fields,
        rows,
    }
}

pub(crate) fn ontology_seed_object(
    table_name: &str,
    object_name: &str,
) -> OntologySeedObjectMapping {
    OntologySeedObjectMapping {
        table_name: table_name.to_string(),
        object_name: object_name.to_string(),
    }
}

pub(crate) fn ontology_seed_relation(
    name: &str,
    from_object: &str,
    relation: &str,
    to_object: &str,
    source_table: &str,
    source_field: &str,
    reference_table: &str,
) -> OntologySeedRelationMapping {
    OntologySeedRelationMapping {
        name: name.to_string(),
        from_object: from_object.to_string(),
        relation: relation.to_string(),
        to_object: to_object.to_string(),
        source_table: source_table.to_string(),
        source_field: source_field.to_string(),
        reference_table: reference_table.to_string(),
    }
}

pub(crate) fn ontology_seed_metric(
    name: &str,
    target_object: &str,
    expression: &str,
    evidence: Value,
) -> OntologySeedMetricMapping {
    OntologySeedMetricMapping {
        name: name.to_string(),
        target_object: target_object.to_string(),
        expression: expression.to_string(),
        evidence,
    }
}

pub(crate) fn ontology_seed_action(
    name: &str,
    target_object: &str,
    approval_required: bool,
    inputs: Value,
    reads: Value,
    effects: Value,
    executor: Value,
) -> OntologySeedActionMapping {
    let transaction_profile = ontology_default_action_transaction_profile(&effects, &executor);
    OntologySeedActionMapping {
        name: name.to_string(),
        target_object: target_object.to_string(),
        approval_required,
        inputs,
        reads,
        effects,
        executor,
        transaction_profile,
    }
}
