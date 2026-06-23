use std::collections::HashSet;

use serde_json::{Value, json};

use crate::{OntologyDatasetProfile, OntologyForeignKeyCandidate, OntologyOnboardingDataset};

pub(crate) fn ontology_profile_demo_datasets(
    datasets: &[OntologyOnboardingDataset],
) -> Vec<OntologyDatasetProfile> {
    datasets
        .iter()
        .map(|dataset| ontology_profile_dataset(dataset, datasets))
        .collect()
}

fn ontology_profile_dataset(
    dataset: &OntologyOnboardingDataset,
    datasets: &[OntologyOnboardingDataset],
) -> OntologyDatasetProfile {
    let row_count = dataset.rows.len();
    let mut primary_key_candidates = Vec::new();
    let mut foreign_key_candidates = Vec::new();
    let mut enum_candidates = Vec::new();
    let mut time_dimensions = Vec::new();
    let mut currency_fields = Vec::new();
    let mut pii_candidates = Vec::new();
    let mut field_null_rates = serde_json::Map::new();
    let mut field_uniqueness = serde_json::Map::new();

    for field in &dataset.fields {
        let values = ontology_field_values(dataset, &field.name);
        let non_null_count = values.iter().filter(|value| !value.is_null()).count();
        let distinct_count = ontology_distinct_value_count(&values);
        let null_rate = if row_count == 0 {
            0.0
        } else {
            (row_count - non_null_count) as f64 / row_count as f64
        };
        let uniqueness = if non_null_count == 0 {
            0.0
        } else {
            distinct_count as f64 / non_null_count as f64
        };
        field_null_rates.insert(field.name.clone(), json!(null_rate));
        field_uniqueness.insert(field.name.clone(), json!(uniqueness));

        if row_count > 0 && non_null_count == row_count && distinct_count == row_count {
            primary_key_candidates.push(field.name.clone());
        }
        if distinct_count > 0 && distinct_count <= 8 && uniqueness < 1.0 {
            enum_candidates.push(field.name.clone());
        }
        if field.name.ends_with("_at") {
            time_dimensions.push(field.name.clone());
        }
        if ontology_is_currency_field(&field.name) {
            currency_fields.push(field.name.clone());
        }
        if ontology_is_pii_field(&field.name) {
            pii_candidates.push(field.name.clone());
        }
        if field.name.ends_with("_id") {
            foreign_key_candidates.extend(ontology_foreign_key_candidates(
                dataset,
                &field.name,
                datasets,
            ));
        }
    }

    OntologyDatasetProfile {
        table_name: dataset.table_name.clone(),
        row_count,
        primary_key_candidates,
        foreign_key_candidates,
        enum_candidates,
        time_dimensions,
        currency_fields,
        pii_candidates,
        field_null_rates: Value::Object(field_null_rates),
        field_uniqueness: Value::Object(field_uniqueness),
    }
}

fn ontology_field_values(dataset: &OntologyOnboardingDataset, field_name: &str) -> Vec<Value> {
    dataset
        .rows
        .iter()
        .map(|row| row.get(field_name).cloned().unwrap_or(Value::Null))
        .collect()
}

fn ontology_distinct_value_count(values: &[Value]) -> usize {
    values
        .iter()
        .filter(|value| !value.is_null())
        .map(ontology_normalized_value_key)
        .collect::<HashSet<_>>()
        .len()
}

fn ontology_normalized_value_key(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn ontology_is_currency_field(field_name: &str) -> bool {
    let lower = field_name.to_ascii_lowercase();
    lower.contains("price") || lower.contains("amount") || lower.contains("total")
}

pub(crate) fn ontology_is_pii_field(field_name: &str) -> bool {
    let lower = field_name.to_ascii_lowercase();
    lower.contains("email") || lower.contains("phone") || lower.contains("address")
}

fn ontology_foreign_key_candidates(
    dataset: &OntologyOnboardingDataset,
    field_name: &str,
    datasets: &[OntologyOnboardingDataset],
) -> Vec<OntologyForeignKeyCandidate> {
    let expected_table = ontology_expected_reference_table(field_name);
    datasets
        .iter()
        .filter(|candidate| candidate.table_name != dataset.table_name)
        .filter(|candidate| {
            expected_table
                .as_ref()
                .map(|expected| candidate.table_name == *expected)
                .unwrap_or_else(|| candidate.table_name == field_name.trim_end_matches("_id"))
        })
        .filter(|candidate| candidate.fields.iter().any(|field| field.name == "id"))
        .filter_map(|candidate| {
            let join_success_rate =
                ontology_join_success_rate(dataset, field_name, candidate, "id");
            (join_success_rate > 0.0).then(|| OntologyForeignKeyCandidate {
                field: field_name.to_string(),
                references_table: candidate.table_name.clone(),
                references_field: "id".to_string(),
                join_success_rate,
            })
        })
        .collect()
}

fn ontology_expected_reference_table(field_name: &str) -> Option<String> {
    let base = field_name.strip_suffix("_id")?;
    Some(
        match base {
            "customer" => "customers",
            "order" => "orders",
            "product" => "products",
            "sku" => "skus",
            "refund" => "refunds",
            "ticket" => "tickets",
            "inventory" => "inventory",
            "insured" => "insureds",
            "policy" => "policies",
            "claim" => "claims",
            "broker" => "brokers",
            value => value,
        }
        .to_string(),
    )
}

fn ontology_join_success_rate(
    dataset: &OntologyOnboardingDataset,
    field_name: &str,
    reference_dataset: &OntologyOnboardingDataset,
    reference_field: &str,
) -> f64 {
    let reference_values = reference_dataset
        .rows
        .iter()
        .filter_map(|row| row.get(reference_field))
        .filter(|value| !value.is_null())
        .map(ontology_normalized_value_key)
        .collect::<HashSet<_>>();
    let values = dataset
        .rows
        .iter()
        .filter_map(|row| row.get(field_name))
        .filter(|value| !value.is_null())
        .collect::<Vec<_>>();
    if values.is_empty() {
        return 0.0;
    }
    let matches = values
        .iter()
        .filter(|value| reference_values.contains(&ontology_normalized_value_key(value)))
        .count();
    matches as f64 / values.len() as f64
}
