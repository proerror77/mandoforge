use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

#[cfg(test)]
pub(crate) fn ontology_demo_datasets() -> Vec<OntologyOnboardingDataset> {
    ontology_demo_source_bundle().datasets
}

pub(crate) fn ontology_demo_source_bundle() -> OntologySourceBundle {
    OntologySourceBundle {
        industry: "ecommerce".to_string(),
        source_mode: "demo_ecommerce".to_string(),
        tool_namespace: "commerce".to_string(),
        datasets: ontology_demo_ecommerce_datasets(),
    }
}

pub(crate) fn ontology_demo_ecommerce_datasets() -> Vec<OntologyOnboardingDataset> {
    vec![
        ontology_demo_dataset(
            "customers",
            "demo_commerce",
            "customers",
            vec![
                ("id", "string"),
                ("email", "string"),
                ("name", "string"),
                ("created_at", "timestamp"),
            ],
            vec![
                json!({"id":"cus_1","email":"a@example.com","name":"Ada","created_at":"2026-06-01T00:00:00Z"}),
                json!({"id":"cus_2","email":"b@example.com","name":"Ben","created_at":"2026-06-02T00:00:00Z"}),
                json!({"id":"cus_3","email":"c@example.com","name":"Cy","created_at":"2026-06-03T00:00:00Z"}),
                json!({"id":"cus_4","email":"d@example.com","name":"Dee","created_at":"2026-06-04T00:00:00Z"}),
            ],
        ),
        ontology_demo_dataset(
            "orders",
            "demo_commerce",
            "orders",
            vec![
                ("id", "string"),
                ("customer_id", "string"),
                ("status", "string"),
                ("total_price", "decimal"),
                ("created_at", "timestamp"),
            ],
            vec![
                json!({"id":"ord_1","customer_id":"cus_1","status":"paid","total_price":120.0,"created_at":"2026-06-08T00:00:00Z"}),
                json!({"id":"ord_2","customer_id":"cus_1","status":"refunded","total_price":80.0,"created_at":"2026-06-09T00:00:00Z"}),
                json!({"id":"ord_3","customer_id":"cus_2","status":"paid","total_price":210.0,"created_at":"2026-06-10T00:00:00Z"}),
                json!({"id":"ord_4","customer_id":"cus_3","status":"fulfilled","total_price":45.0,"created_at":"2026-06-11T00:00:00Z"}),
            ],
        ),
        ontology_demo_dataset(
            "order_items",
            "demo_commerce",
            "order_items",
            vec![
                ("id", "string"),
                ("order_id", "string"),
                ("sku_id", "string"),
                ("quantity", "integer"),
                ("line_total", "decimal"),
            ],
            vec![
                json!({"id":"oli_1","order_id":"ord_1","sku_id":"sku_1","quantity":1,"line_total":120.0}),
                json!({"id":"oli_2","order_id":"ord_2","sku_id":"sku_2","quantity":2,"line_total":80.0}),
                json!({"id":"oli_3","order_id":"ord_3","sku_id":"sku_3","quantity":1,"line_total":210.0}),
                json!({"id":"oli_4","order_id":"ord_4","sku_id":"sku_1","quantity":1,"line_total":45.0}),
            ],
        ),
        ontology_demo_dataset(
            "products",
            "demo_commerce",
            "products",
            vec![
                ("id", "string"),
                ("title", "string"),
                ("category", "string"),
            ],
            vec![
                json!({"id":"prd_1","title":"Running Shoe","category":"footwear"}),
                json!({"id":"prd_2","title":"Trail Jacket","category":"apparel"}),
                json!({"id":"prd_3","title":"Water Bottle","category":"accessory"}),
                json!({"id":"prd_4","title":"Yoga Mat","category":"fitness"}),
            ],
        ),
        ontology_demo_dataset(
            "skus",
            "demo_commerce",
            "skus",
            vec![
                ("id", "string"),
                ("product_id", "string"),
                ("sku_code", "string"),
                ("price", "decimal"),
            ],
            vec![
                json!({"id":"sku_1","product_id":"prd_1","sku_code":"SHOE-8","price":120.0}),
                json!({"id":"sku_2","product_id":"prd_2","sku_code":"JACKET-M","price":40.0}),
                json!({"id":"sku_3","product_id":"prd_3","sku_code":"BOTTLE-1L","price":210.0}),
                json!({"id":"sku_4","product_id":"prd_4","sku_code":"MAT-BLUE","price":45.0}),
            ],
        ),
        ontology_demo_dataset(
            "inventory",
            "demo_commerce",
            "inventory",
            vec![
                ("id", "string"),
                ("sku_id", "string"),
                ("warehouse_id", "string"),
                ("available_quantity", "integer"),
            ],
            vec![
                json!({"id":"inv_1","sku_id":"sku_1","warehouse_id":"wh_1","available_quantity":5}),
                json!({"id":"inv_2","sku_id":"sku_2","warehouse_id":"wh_1","available_quantity":2}),
                json!({"id":"inv_3","sku_id":"sku_3","warehouse_id":"wh_2","available_quantity":50}),
                json!({"id":"inv_4","sku_id":"sku_4","warehouse_id":"wh_2","available_quantity":1}),
            ],
        ),
        ontology_demo_dataset(
            "refunds",
            "demo_commerce",
            "refunds",
            vec![
                ("id", "string"),
                ("order_id", "string"),
                ("amount", "decimal"),
                ("reason", "string"),
                ("status", "string"),
            ],
            vec![
                json!({"id":"ref_1","order_id":"ord_2","amount":80.0,"reason":"size_issue","status":"approved"}),
                json!({"id":"ref_2","order_id":"ord_3","amount":20.0,"reason":"late_delivery","status":"requested"}),
                json!({"id":"ref_3","order_id":"ord_1","amount":10.0,"reason":"coupon_adjustment","status":"closed"}),
                json!({"id":"ref_4","order_id":"ord_4","amount":5.0,"reason":"minor_defect","status":"requested"}),
            ],
        ),
        ontology_demo_dataset(
            "tickets",
            "demo_commerce",
            "tickets",
            vec![
                ("id", "string"),
                ("customer_id", "string"),
                ("order_id", "string"),
                ("status", "string"),
                ("topic", "string"),
            ],
            vec![
                json!({"id":"tic_1","customer_id":"cus_1","order_id":"ord_1","status":"open","topic":"shipping"}),
                json!({"id":"tic_2","customer_id":"cus_2","order_id":"ord_3","status":"escalated","topic":"refund"}),
                json!({"id":"tic_3","customer_id":"cus_3","order_id":"ord_4","status":"closed","topic":"coupon"}),
                json!({"id":"tic_4","customer_id":"cus_4","order_id":null,"status":"open","topic":"product_question"}),
            ],
        ),
    ]
}

pub(crate) fn ontology_ecommerce_seed_pack() -> OntologySeedPack {
    OntologySeedPack {
        industry: "ecommerce".to_string(),
        domain_scope: "commerce".to_string(),
        source_mode: "demo_ecommerce".to_string(),
        tool_namespace: "commerce".to_string(),
        objects: vec![
            ontology_seed_object("customers", "Customer"),
            ontology_seed_object("orders", "Order"),
            ontology_seed_object("order_items", "OrderLine"),
            ontology_seed_object("products", "Product"),
            ontology_seed_object("skus", "SKU"),
            ontology_seed_object("inventory", "InventoryItem"),
            ontology_seed_object("refunds", "Refund"),
            ontology_seed_object("tickets", "SupportTicket"),
        ],
        relations: vec![
            ontology_seed_relation(
                "Customer places Order",
                "Customer",
                "places",
                "Order",
                "orders",
                "customer_id",
                "customers",
            ),
            ontology_seed_relation(
                "Order contains OrderLine",
                "Order",
                "contains",
                "OrderLine",
                "order_items",
                "order_id",
                "orders",
            ),
            ontology_seed_relation(
                "OrderLine references SKU",
                "OrderLine",
                "references",
                "SKU",
                "order_items",
                "sku_id",
                "skus",
            ),
            ontology_seed_relation(
                "SKU represents Product",
                "SKU",
                "represents",
                "Product",
                "skus",
                "product_id",
                "products",
            ),
            ontology_seed_relation(
                "SKU has InventoryItem",
                "SKU",
                "has",
                "InventoryItem",
                "inventory",
                "sku_id",
                "skus",
            ),
            ontology_seed_relation(
                "Order may_have Refund",
                "Order",
                "may_have",
                "Refund",
                "refunds",
                "order_id",
                "orders",
            ),
            ontology_seed_relation(
                "Customer creates SupportTicket",
                "Customer",
                "creates",
                "SupportTicket",
                "tickets",
                "customer_id",
                "customers",
            ),
            ontology_seed_relation(
                "Order has SupportTicket",
                "Order",
                "has",
                "SupportTicket",
                "tickets",
                "order_id",
                "orders",
            ),
        ],
        metrics: vec![
            ontology_seed_metric(
                "GMV",
                "Order",
                "sum(orders.total_price) where orders.status in ('paid','fulfilled')",
                json!({
                    "unit": "currency",
                    "base_table": "orders",
                    "measure_field": "total_price",
                    "time_dimension": "created_at"
                }),
            ),
            ontology_seed_metric(
                "AOV",
                "Order",
                "GMV / count(distinct orders.id)",
                json!({
                    "unit": "currency",
                    "depends_on": ["GMV"],
                    "base_table": "orders"
                }),
            ),
            ontology_seed_metric(
                "Refund Rate",
                "Refund",
                "count(distinct refunds.order_id) / count(distinct orders.id)",
                json!({
                    "unit": "ratio",
                    "base_table": "refunds",
                    "join": "refunds.order_id = orders.id"
                }),
            ),
            ontology_seed_metric(
                "Repeat Purchase Rate",
                "Customer",
                "customers with count(orders.id) > 1 / active customers",
                json!({
                    "unit": "ratio",
                    "base_table": "orders",
                    "join": "orders.customer_id = customers.id"
                }),
            ),
            ontology_seed_metric(
                "Inventory Turnover",
                "InventoryItem",
                "units_sold / average_inventory",
                json!({
                    "unit": "ratio",
                    "base_table": "inventory",
                    "join": "inventory.sku_id = skus.id"
                }),
            ),
        ],
        actions: vec![
            ontology_seed_action(
                "refund_order",
                "Order",
                true,
                json!({
                    "order_id": "string",
                    "amount": "decimal",
                    "reason": "string"
                }),
                json!(["Order", "Payment", "Customer"]),
                json!([
                    {"type": "create_object", "object": "Refund"},
                    {"type": "update_attribute", "target": "Order.status"},
                    {"type": "create_relation", "relation": "Order may_have Refund"}
                ]),
                json!({"type": "http_api", "ref": "POST /orders/{order_id}/refund"}),
            ),
            ontology_seed_action(
                "issue_coupon",
                "Customer",
                true,
                json!({
                    "customer_id": "string",
                    "coupon_code": "string",
                    "reason": "string"
                }),
                json!(["Customer", "Order"]),
                json!([
                    {"type": "create_object", "object": "Coupon"},
                    {"type": "create_relation", "relation": "Customer receives Coupon"}
                ]),
                json!({"type": "http_api", "ref": "POST /customers/{customer_id}/coupons"}),
            ),
            ontology_seed_action(
                "adjust_inventory",
                "InventoryItem",
                true,
                json!({
                    "inventory_item_id": "string",
                    "delta_quantity": "integer",
                    "reason": "string"
                }),
                json!(["InventoryItem", "SKU"]),
                json!([
                    {"type": "update_attribute", "target": "InventoryItem.available_quantity"}
                ]),
                json!({"type": "http_api", "ref": "POST /inventory/{inventory_item_id}/adjust"}),
            ),
            ontology_seed_action(
                "escalate_ticket",
                "SupportTicket",
                false,
                json!({
                    "ticket_id": "string",
                    "reason": "string"
                }),
                json!(["SupportTicket", "Customer", "Order"]),
                json!([
                    {"type": "update_attribute", "target": "SupportTicket.status"}
                ]),
                json!({"type": "http_api", "ref": "POST /tickets/{ticket_id}/escalate"}),
            ),
        ],
    }
}

pub(crate) fn ontology_insurance_demo_source_bundle() -> OntologySourceBundle {
    OntologySourceBundle {
        industry: "insurance".to_string(),
        source_mode: "demo_insurance".to_string(),
        tool_namespace: "insurance".to_string(),
        datasets: vec![
            ontology_demo_dataset(
                "insureds",
                "demo_insurance",
                "insureds",
                vec![
                    ("id", "string"),
                    ("email", "string"),
                    ("name", "string"),
                    ("created_at", "timestamp"),
                ],
                vec![
                    json!({"id":"ins_1","email":"a@example.com","name":"Ada","created_at":"2026-06-01T00:00:00Z"}),
                    json!({"id":"ins_2","email":"b@example.com","name":"Ben","created_at":"2026-06-02T00:00:00Z"}),
                    json!({"id":"ins_3","email":"c@example.com","name":"Cy","created_at":"2026-06-03T00:00:00Z"}),
                    json!({"id":"ins_4","email":"d@example.com","name":"Dee","created_at":"2026-06-04T00:00:00Z"}),
                ],
            ),
            ontology_demo_dataset(
                "policies",
                "demo_insurance",
                "policies",
                vec![
                    ("id", "string"),
                    ("insured_id", "string"),
                    ("policy_number", "string"),
                    ("premium_amount", "decimal"),
                    ("effective_at", "timestamp"),
                ],
                vec![
                    json!({"id":"pol_1","insured_id":"ins_1","policy_number":"P-100","premium_amount":1200.0,"effective_at":"2026-06-01T00:00:00Z"}),
                    json!({"id":"pol_2","insured_id":"ins_2","policy_number":"P-200","premium_amount":950.0,"effective_at":"2026-06-02T00:00:00Z"}),
                    json!({"id":"pol_3","insured_id":"ins_3","policy_number":"P-300","premium_amount":1800.0,"effective_at":"2026-06-03T00:00:00Z"}),
                    json!({"id":"pol_4","insured_id":"ins_4","policy_number":"P-400","premium_amount":700.0,"effective_at":"2026-06-04T00:00:00Z"}),
                ],
            ),
            ontology_demo_dataset(
                "claims",
                "demo_insurance",
                "claims",
                vec![
                    ("id", "string"),
                    ("policy_id", "string"),
                    ("insured_id", "string"),
                    ("claim_amount", "decimal"),
                    ("status", "string"),
                    ("opened_at", "timestamp"),
                ],
                vec![
                    json!({"id":"clm_1","policy_id":"pol_1","insured_id":"ins_1","claim_amount":200.0,"status":"open","opened_at":"2026-06-08T00:00:00Z"}),
                    json!({"id":"clm_2","policy_id":"pol_2","insured_id":"ins_2","claim_amount":450.0,"status":"approved","opened_at":"2026-06-09T00:00:00Z"}),
                    json!({"id":"clm_3","policy_id":"pol_3","insured_id":"ins_3","claim_amount":1200.0,"status":"review","opened_at":"2026-06-10T00:00:00Z"}),
                    json!({"id":"clm_4","policy_id":"pol_4","insured_id":"ins_4","claim_amount":80.0,"status":"closed","opened_at":"2026-06-11T00:00:00Z"}),
                ],
            ),
            ontology_demo_dataset(
                "brokers",
                "demo_insurance",
                "brokers",
                vec![("id", "string"), ("name", "string"), ("email", "string")],
                vec![
                    json!({"id":"bro_1","name":"North Agency","email":"north@example.com"}),
                    json!({"id":"bro_2","name":"South Agency","email":"south@example.com"}),
                    json!({"id":"bro_3","name":"East Agency","email":"east@example.com"}),
                    json!({"id":"bro_4","name":"West Agency","email":"west@example.com"}),
                ],
            ),
        ],
    }
}

pub(crate) fn ontology_insurance_seed_pack() -> OntologySeedPack {
    OntologySeedPack {
        industry: "insurance".to_string(),
        domain_scope: "insurance".to_string(),
        source_mode: "demo_insurance".to_string(),
        tool_namespace: "insurance".to_string(),
        objects: vec![
            ontology_seed_object("insureds", "Insured"),
            ontology_seed_object("policies", "Policy"),
            ontology_seed_object("claims", "Claim"),
            ontology_seed_object("brokers", "Broker"),
        ],
        relations: vec![
            ontology_seed_relation(
                "Policy covers Insured",
                "Policy",
                "covers",
                "Insured",
                "policies",
                "insured_id",
                "insureds",
            ),
            ontology_seed_relation(
                "Insured files Claim",
                "Insured",
                "files",
                "Claim",
                "claims",
                "insured_id",
                "insureds",
            ),
            ontology_seed_relation(
                "Claim belongs_to Policy",
                "Claim",
                "belongs_to",
                "Policy",
                "claims",
                "policy_id",
                "policies",
            ),
        ],
        metrics: vec![
            ontology_seed_metric(
                "Loss Ratio",
                "Claim",
                "sum(claims.claim_amount) / sum(policies.premium_amount)",
                json!({
                    "unit": "ratio",
                    "base_table": "claims",
                    "join": "claims.policy_id = policies.id"
                }),
            ),
            ontology_seed_metric(
                "Claim Cycle Time",
                "Claim",
                "avg(claims.closed_at - claims.opened_at)",
                json!({
                    "unit": "duration",
                    "base_table": "claims",
                    "time_dimension": "opened_at"
                }),
            ),
        ],
        actions: vec![
            ontology_seed_action(
                "approve_claim",
                "Claim",
                true,
                json!({
                    "claim_id": "string",
                    "approved_amount": "decimal",
                    "reason": "string"
                }),
                json!(["Claim", "Policy", "Insured"]),
                json!([
                    {"type": "update_attribute", "target": "Claim.status"},
                    {"type": "create_audit_event", "event": "Claim approved"}
                ]),
                json!({"type": "http_api", "ref": "POST /claims/{claim_id}/approve"}),
            ),
            ontology_seed_action(
                "request_documents",
                "Claim",
                false,
                json!({
                    "claim_id": "string",
                    "document_types": "array"
                }),
                json!(["Claim", "Insured"]),
                json!([
                    {"type": "create_object", "object": "DocumentRequest"}
                ]),
                json!({"type": "http_api", "ref": "POST /claims/{claim_id}/document-requests"}),
            ),
        ],
    }
}

#[cfg(test)]
pub(crate) fn ontology_generate_demo_proposals(
    datasets: &[OntologyOnboardingDataset],
    profiles: &[OntologyDatasetProfile],
) -> Vec<OntologyOnboardingProposalDraft> {
    ontology_generate_demo_proposals_for_run(Uuid::new_v4(), datasets, profiles)
}

#[cfg(test)]
pub(crate) fn ontology_generate_demo_proposals_for_run(
    run_id: Uuid,
    datasets: &[OntologyOnboardingDataset],
    profiles: &[OntologyDatasetProfile],
) -> Vec<OntologyOnboardingProposalDraft> {
    ontology_generate_seed_proposals_for_run(
        run_id,
        &ontology_ecommerce_seed_pack(),
        datasets,
        profiles,
    )
}

pub(crate) fn ontology_generate_seed_proposals_for_run(
    run_id: Uuid,
    seed: &OntologySeedPack,
    datasets: &[OntologyOnboardingDataset],
    profiles: &[OntologyDatasetProfile],
) -> Vec<OntologyOnboardingProposalDraft> {
    let mut proposals = Vec::new();
    for mapping in &seed.objects {
        if let (Some(dataset), Some(profile)) = (
            datasets
                .iter()
                .find(|dataset| dataset.table_name == mapping.table_name),
            ontology_profile_by_table(profiles, &mapping.table_name),
        ) {
            proposals.push(ontology_object_proposal(
                run_id,
                seed,
                dataset,
                profile,
                &mapping.object_name,
            ));
            proposals.push(ontology_logic_rule_proposal(
                run_id,
                seed,
                dataset,
                profile,
                &mapping.object_name,
            ));
        }
    }

    for mapping in &seed.relations {
        if let Some(profile) = ontology_profile_by_table(profiles, &mapping.source_table)
            && let Some(candidate) = ontology_profile_fk(
                profile,
                &mapping.source_field,
                &mapping.reference_table,
                "id",
            )
        {
            proposals.push(ontology_relation_proposal(
                run_id,
                seed,
                mapping,
                candidate.join_success_rate,
            ));
        }
    }

    for mapping in &seed.metrics {
        proposals.push(ontology_metric_proposal(run_id, seed, mapping));
    }

    for mapping in &seed.actions {
        proposals.push(ontology_action_proposal(run_id, seed, mapping));
    }

    proposals
}

pub(crate) fn ontology_object_proposal(
    run_id: Uuid,
    seed: &OntologySeedPack,
    dataset: &OntologyOnboardingDataset,
    profile: &OntologyDatasetProfile,
    object_name: &str,
) -> OntologyOnboardingProposalDraft {
    ontology_proposal(
        run_id,
        "object",
        object_name,
        format!(
            "{}.{} -> {object_name}",
            dataset.source_system, dataset.table_name
        ),
        0.94,
        json!({
            "table": dataset.table_name,
            "row_count": profile.row_count,
            "industry": seed.industry,
            "source_mode": seed.source_mode,
            "domain_scope": seed.domain_scope,
            "tool_namespace": seed.tool_namespace,
            "primary_key_candidates": profile.primary_key_candidates,
            "time_dimensions": profile.time_dimensions,
            "currency_fields": profile.currency_fields,
            "pii_candidates": profile.pii_candidates,
            "seed_ontology_match": object_name,
        }),
        json!({
            "object_type": object_name,
            "industry": seed.industry,
            "source_mode": seed.source_mode,
            "domain_scope": seed.domain_scope,
            "tool_namespace": seed.tool_namespace,
            "source_table": dataset.table_name,
            "source_system": dataset.source_system,
            "primary_key": profile.primary_key_candidates.first().cloned().unwrap_or_else(|| "id".to_string()),
            "properties": dataset.fields.iter().map(|field| {
                json!({
                    "name": field.name,
                    "type": field.field_type,
                    "sample_values": field.sample_values,
                })
            }).collect::<Vec<_>>(),
        }),
    )
}

pub(crate) fn ontology_relation_proposal(
    run_id: Uuid,
    seed: &OntologySeedPack,
    mapping: &OntologySeedRelationMapping,
    join_success_rate: f64,
) -> OntologyOnboardingProposalDraft {
    ontology_proposal(
        run_id,
        "relation",
        &mapping.name,
        format!(
            "{}.{} = {}.id",
            mapping.source_table, mapping.source_field, mapping.reference_table
        ),
        if join_success_rate >= 0.99 {
            0.96
        } else {
            0.88
        },
        json!({
            "industry": seed.industry,
            "source_mode": seed.source_mode,
            "domain_scope": seed.domain_scope,
            "tool_namespace": seed.tool_namespace,
            "source_table": mapping.source_table,
            "source_field": mapping.source_field,
            "references_table": mapping.reference_table,
            "references_field": "id",
            "join_success_rate": join_success_rate,
            "seed_relation_match": mapping.name,
        }),
        json!({
            "industry": seed.industry,
            "source_mode": seed.source_mode,
            "domain_scope": seed.domain_scope,
            "tool_namespace": seed.tool_namespace,
            "from_object": mapping.from_object,
            "relation": mapping.relation,
            "to_object": mapping.to_object,
            "link_type": mapping.name,
            "source_mapping": format!("{}.{} = {}.id", mapping.source_table, mapping.source_field, mapping.reference_table),
        }),
    )
}

pub(crate) fn ontology_metric_proposal(
    run_id: Uuid,
    seed: &OntologySeedPack,
    mapping: &OntologySeedMetricMapping,
) -> OntologyOnboardingProposalDraft {
    ontology_proposal(
        run_id,
        "metric",
        &mapping.name,
        mapping.expression.clone(),
        0.86,
        json!({
            "semantic_model": seed.domain_scope,
            "industry": seed.industry,
            "source_mode": seed.source_mode,
            "domain_scope": seed.domain_scope,
            "tool_namespace": seed.tool_namespace,
            "target_object": mapping.target_object,
            "expression": mapping.expression,
            "definition_evidence": mapping.evidence,
        }),
        json!({
            "metric_name": mapping.name,
            "industry": seed.industry,
            "source_mode": seed.source_mode,
            "domain_scope": seed.domain_scope,
            "tool_namespace": seed.tool_namespace,
            "target_object": mapping.target_object,
            "expression": mapping.expression,
            "governance": {
                "requires_owner_review": true,
                "downstream_tools_use_canonical_definition": true
            }
        }),
    )
}

pub(crate) fn ontology_action_proposal(
    run_id: Uuid,
    seed: &OntologySeedPack,
    mapping: &OntologySeedActionMapping,
) -> OntologyOnboardingProposalDraft {
    let has_effects = ontology_action_has_effects(&mapping.effects);
    let cross_system_write =
        has_effects && ontology_action_executor_is_cross_system(&mapping.executor);
    let approval_required = mapping.approval_required
        || (cross_system_write
            && mapping.transaction_profile == OntologyActionTransactionProfile::ProposalOnly);
    ontology_proposal(
        run_id,
        "action",
        &mapping.name,
        format!("{} -> {}", mapping.name, mapping.target_object),
        0.82,
        json!({
            "industry": seed.industry,
            "source_mode": seed.source_mode,
            "domain_scope": seed.domain_scope,
            "tool_namespace": seed.tool_namespace,
            "target_object": mapping.target_object,
            "contract_source": "demo_openapi_and_sop_seed",
            "approval_required": approval_required,
            "effect_count": mapping.effects.as_array().map(Vec::len).unwrap_or_default(),
            "transaction_profile": mapping.transaction_profile,
            "execution_mode": ontology_action_execution_mode(mapping.transaction_profile, !has_effects),
            "cross_system_write": cross_system_write,
        }),
        json!({
            "action": mapping.name,
            "industry": seed.industry,
            "source_mode": seed.source_mode,
            "domain_scope": seed.domain_scope,
            "tool_namespace": seed.tool_namespace,
            "target_object": mapping.target_object,
            "inputs": mapping.inputs,
            "reads": mapping.reads,
            "effects": mapping.effects,
            "policy": {
                "approval_required": approval_required,
                "approval_required_if": if approval_required { Value::String("external_write_or_financial_impact".to_string()) } else { Value::Null },
                "transaction_profile": mapping.transaction_profile,
                "cross_system_write": cross_system_write
            },
            "transaction_profile": mapping.transaction_profile,
            "transaction_policy": {
                "profile": mapping.transaction_profile,
                "execution_mode": ontology_action_execution_mode(mapping.transaction_profile, !has_effects),
                "requires_human_approval": approval_required,
                "compensation_required": mapping.transaction_profile == OntologyActionTransactionProfile::Saga,
                "cross_system_write": cross_system_write
            },
            "executor": mapping.executor,
            "audit_event": format!("{}.{}", seed.tool_namespace, mapping.name),
        }),
    )
}

pub(crate) fn ontology_logic_rule_proposal(
    run_id: Uuid,
    seed: &OntologySeedPack,
    dataset: &OntologyOnboardingDataset,
    profile: &OntologyDatasetProfile,
    object_name: &str,
) -> OntologyOnboardingProposalDraft {
    let primary_key = profile
        .primary_key_candidates
        .first()
        .cloned()
        .unwrap_or_else(|| "id".to_string());
    let rule_name = format!("{object_name} identity rule");
    ontology_proposal(
        run_id,
        "logic",
        &rule_name,
        format!(
            "validate {object_name}.{primary_key} from {}",
            dataset.table_name
        ),
        if profile.primary_key_candidates.is_empty() {
            0.72
        } else {
            0.91
        },
        json!({
            "industry": seed.industry,
            "source_mode": seed.source_mode,
            "domain_scope": seed.domain_scope,
            "tool_namespace": seed.tool_namespace,
            "source_table": dataset.table_name,
            "target_object": object_name,
            "primary_key": primary_key,
            "primary_key_candidates": profile.primary_key_candidates,
            "enum_candidates": profile.enum_candidates,
            "pii_candidates": profile.pii_candidates,
            "field_null_rates": profile.field_null_rates,
        }),
        json!({
            "logic_rule": rule_name,
            "industry": seed.industry,
            "source_mode": seed.source_mode,
            "domain_scope": seed.domain_scope,
            "tool_namespace": seed.tool_namespace,
            "target_object": object_name,
            "source_table": dataset.table_name,
            "rule_kind": "validation",
            "enabled": false,
            "expression": {
                "type": "primary_key_unique",
                "field": primary_key,
                "requires_non_null": true
            },
            "policy": {
                "requires_owner_review": true,
                "disabled_until_publish_policy": true
            }
        }),
    )
}

pub(crate) fn ontology_proposal(
    run_id: Uuid,
    proposal_type: &str,
    name: &str,
    source_mapping: String,
    confidence: f64,
    evidence: Value,
    content: Value,
) -> OntologyOnboardingProposalDraft {
    OntologyOnboardingProposalDraft {
        id: Uuid::new_v4(),
        run_id,
        proposal_type: proposal_type.to_string(),
        name: name.to_string(),
        source_mapping,
        confidence,
        evidence,
        recommendation: "approve".to_string(),
        review_status: "pending".to_string(),
        content,
    }
}

pub(crate) fn ontology_profile_by_table<'a>(
    profiles: &'a [OntologyDatasetProfile],
    table_name: &str,
) -> Option<&'a OntologyDatasetProfile> {
    profiles
        .iter()
        .find(|profile| profile.table_name == table_name)
}

pub(crate) fn ontology_profile_fk<'a>(
    profile: &'a OntologyDatasetProfile,
    field: &str,
    references_table: &str,
    references_field: &str,
) -> Option<&'a OntologyForeignKeyCandidate> {
    profile.foreign_key_candidates.iter().find(|candidate| {
        candidate.field == field
            && candidate.references_table == references_table
            && candidate.references_field == references_field
    })
}

#[cfg(test)]
pub(crate) async fn create_demo_ontology_onboarding_run_for_test(
    state: &AppState,
) -> Result<OntologyOnboardingRun, AppError> {
    create_ontology_onboarding_run_with_actor(state, "ecommerce", "demo_ecommerce", "test").await
}

#[cfg(test)]
pub(crate) async fn review_ontology_onboarding_proposal_for_test(
    state: &AppState,
    proposal_id: Uuid,
    decision: &str,
    reason: Option<&str>,
) -> Result<OntologyOnboardingProposalDraft, AppError> {
    review_ontology_onboarding_proposal_with_actor(state, proposal_id, decision, reason, "test")
        .await
}

#[cfg(test)]
pub(crate) async fn materialize_ontology_onboarding_run_for_test(
    state: &AppState,
    run_id: Uuid,
) -> Result<OntologyOnboardingMaterializationResult, AppError> {
    materialize_ontology_onboarding_run_with_actor(state, run_id, "test").await
}

pub(crate) async fn ontology_onboarding_tool_specs_for_run(
    state: &AppState,
    run_id: Uuid,
) -> Result<Vec<OntologyOnboardingToolSpec>, AppError> {
    let mut specs = ontology_onboarding_proposal_objects(state)
        .await?
        .into_iter()
        .filter(|object| ontology_onboarding_object_run_id(object) == Some(run_id))
        .filter(ontology_onboarding_object_materialized)
        .map(|object| ontology_onboarding_object_proposal(&object))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|proposal| {
            proposal.proposal_type == "action" && proposal.review_status == "approved"
        })
        .map(|proposal| ontology_tool_spec_from_action_proposal(run_id, &proposal))
        .collect::<Result<Vec<_>, _>>()?;
    specs.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(specs)
}

pub(crate) fn ontology_tool_spec_from_action_proposal(
    run_id: Uuid,
    proposal: &OntologyOnboardingProposalDraft,
) -> Result<OntologyOnboardingToolSpec, AppError> {
    let target_object = proposal
        .content
        .get("target_object")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::bad_request("action proposal missing target_object"))?;
    let mut policy = proposal
        .content
        .get("policy")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let approval_required = policy
        .get("approval_required")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let effects = proposal
        .content
        .get("effects")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let read_only = !ontology_action_has_effects(&effects);
    let transaction_profile =
        ontology_action_transaction_profile_for_proposal(proposal, read_only)?;
    let approval_required = approval_required
        || (!read_only && transaction_profile == OntologyActionTransactionProfile::ProposalOnly);
    let execution_mode = ontology_action_execution_mode(transaction_profile, read_only);
    if let Some(policy) = policy.as_object_mut() {
        policy.insert("approval_required".to_string(), json!(approval_required));
        policy.insert(
            "transaction_profile".to_string(),
            json!(transaction_profile),
        );
        policy.insert("execution_mode".to_string(), json!(execution_mode));
    }
    let audit_event = proposal
        .content
        .get("audit_event")
        .and_then(Value::as_str)
        .unwrap_or("commerce.action")
        .to_string();
    Ok(OntologyOnboardingToolSpec {
        id: proposal.id,
        run_id,
        name: format!(
            "{}.{}",
            ontology_proposal_tool_namespace(proposal),
            proposal.name
        ),
        description: format!(
            "Compiled ontology action tool for {target_object}: {}.",
            proposal.name
        ),
        tool_kind: "ontology_action".to_string(),
        target_object: target_object.to_string(),
        read_only,
        approval_required,
        input_schema: proposal
            .content
            .get("inputs")
            .cloned()
            .unwrap_or_else(|| json!({})),
        effects,
        policy,
        transaction_profile,
        execution_mode: execution_mode.to_string(),
        read_write_risk: if approval_required {
            "write_approval_required".to_string()
        } else if read_only {
            "read_only".to_string()
        } else {
            "write_low_risk_update".to_string()
        },
        source_refs: json!({
            "proposal_id": proposal.id,
            "proposal_type": proposal.proposal_type,
            "source_mapping": proposal.source_mapping,
            "target_object": target_object,
            "transaction_profile": transaction_profile,
            "execution_mode": execution_mode,
            "evidence": proposal.evidence,
        }),
        audit_event,
        source_proposal_id: proposal.id,
    })
}

pub(crate) fn ontology_action_transaction_profile_for_proposal(
    proposal: &OntologyOnboardingProposalDraft,
    read_only: bool,
) -> Result<OntologyActionTransactionProfile, AppError> {
    if read_only {
        return Ok(OntologyActionTransactionProfile::LocalSerializable);
    }
    let Some(value) = proposal.content.get("transaction_profile") else {
        return Err(AppError::bad_request(
            "action proposal missing transaction_profile",
        ));
    };
    ontology_action_transaction_profile_from_value(value)
}

pub(crate) fn ontology_action_transaction_profile_from_value(
    value: &Value,
) -> Result<OntologyActionTransactionProfile, AppError> {
    match value.as_str() {
        Some("proposal_only") => Ok(OntologyActionTransactionProfile::ProposalOnly),
        Some("local_serializable") => Ok(OntologyActionTransactionProfile::LocalSerializable),
        Some("event_sourced") => Ok(OntologyActionTransactionProfile::EventSourced),
        Some("saga") => Ok(OntologyActionTransactionProfile::Saga),
        _ => Err(AppError::bad_request(
            "transaction_profile must be proposal_only, local_serializable, event_sourced, or saga",
        )),
    }
}

pub(crate) fn ontology_action_execution_mode(
    transaction_profile: OntologyActionTransactionProfile,
    read_only: bool,
) -> &'static str {
    if read_only {
        "read_only"
    } else if transaction_profile == OntologyActionTransactionProfile::ProposalOnly {
        "proposal_only"
    } else {
        "executable_after_approval"
    }
}

pub(crate) async fn review_ontology_curated_dataset_with_actor(
    state: &AppState,
    draft_id: &str,
    decision: &str,
    reason: Option<&str>,
    actor_subject: &str,
) -> Result<CuratedDatasetDraft, AppError> {
    let (_decision, review_status) = normalize_ontology_onboarding_review_decision(decision)?;
    let (run_id, table_name) = ontology_parse_curated_dataset_id(draft_id)?;
    let run = get_ontology_onboarding_run_for_state(state, run_id).await?;
    let seed =
        ontology_seed_and_source_for_request(&ontology_run_industry(&run), &run.source_mode)?.0;
    let mut draft = ontology_curated_dataset_drafts_for_run(&run, &seed)
        .into_iter()
        .find(|draft| draft.table_name == table_name)
        .ok_or_else(|| AppError::not_found("ontology curated dataset draft not found"))?;
    let review = json!({
        "decision": decision,
        "status": review_status,
        "reason": reason,
        "reviewer": actor_subject,
        "reviewed_at": Utc::now(),
    });
    draft.review_status = review_status.clone();
    draft.reviewer_metadata = review.clone();
    ontology_upsert_curated_dataset_review_object(state, run_id, &draft, review.clone()).await?;
    ontology_apply_curated_dataset_review_to_proposals(
        state,
        run_id,
        &table_name,
        &review_status,
        actor_subject,
    )
    .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "ontology_onboarding.curated_dataset_reviewed",
            "ontology_curated_dataset",
            Some(run_id),
            json!({
                "subject": actor_subject,
                "run_id": run_id,
                "draft_id": draft.id,
                "table_name": table_name,
                "review": review,
            }),
        ))
        .await?;
    Ok(draft)
}

pub(crate) fn ontology_parse_curated_dataset_id(
    draft_id: &str,
) -> Result<(Uuid, String), AppError> {
    let mut parts = draft_id.splitn(3, ':');
    let Some(prefix) = parts.next() else {
        return Err(AppError::bad_request("invalid curated dataset draft id"));
    };
    let Some(run_id) = parts.next() else {
        return Err(AppError::bad_request("invalid curated dataset draft id"));
    };
    let Some(table_name) = parts.next() else {
        return Err(AppError::bad_request("invalid curated dataset draft id"));
    };
    if prefix != "curated" || table_name.trim().is_empty() {
        return Err(AppError::bad_request(
            "curated dataset draft id must be curated:{run_id}:{table_name}",
        ));
    }
    let run_id = Uuid::parse_str(run_id)
        .map_err(|_| AppError::bad_request("curated dataset draft id has invalid run_id"))?;
    Ok((run_id, table_name.to_string()))
}

pub(crate) async fn ontology_upsert_curated_dataset_review_object(
    state: &AppState,
    run_id: Uuid,
    draft: &CuratedDatasetDraft,
    review: Value,
) -> Result<(), AppError> {
    let object_key = ontology_curated_dataset_review_object_key(run_id, &draft.table_name);
    let content = json!({
        "run_id": run_id,
        "draft": draft,
        "review": review,
    });
    if let Some(existing) = state
        .list_semantic_objects()
        .await?
        .into_iter()
        .find(|object| object.object_key == object_key && object.archived_at.is_none())
    {
        state
            .update_semantic_object(
                existing.id,
                UpdateSemanticObject {
                    title: None,
                    summary: None,
                    content: Some(content),
                    semantic_scopes: None,
                    source_uri: None,
                    provenance: None,
                    trust_level: None,
                    freshness: None,
                    status: None,
                },
            )
            .await?;
        return Ok(());
    }
    state
        .create_semantic_object(CreateSemanticObject {
            source_id: None,
            object_type: "ontology_curated_dataset_review".to_string(),
            object_key,
            title: format!("Curated dataset review: {}", draft.table_name),
            summary: format!(
                "Review boundary for ontology curated dataset draft {}.",
                draft.table_name
            ),
            content,
            semantic_scopes: json!({
                "domain_scope": "ontology",
                "workflow_scope": "enterprise-ontology-fast-onboarding",
                "memory_scope": "ontology",
                "share_policy": "review_required",
            }),
            source_uri: Some(format!(
                "mandoforge://ontology/onboarding/curated-datasets/{}",
                draft.id
            )),
            provenance: json!({
                "source": "ontology_onboarding.curated_dataset_review",
                "run_id": run_id,
                "table_name": draft.table_name,
                "reviewed_at": Utc::now(),
            }),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "active".to_string(),
        })
        .await?;
    Ok(())
}

pub(crate) async fn ontology_apply_curated_dataset_review_to_proposals(
    state: &AppState,
    run_id: Uuid,
    table_name: &str,
    review_status: &str,
    actor_subject: &str,
) -> Result<(), AppError> {
    let recommendation = match review_status {
        "approved" => "approve",
        "rejected" | "changes_requested" | "needs_more_evidence" => "needs_more_evidence",
        _ => return Ok(()),
    };
    for object in ontology_onboarding_proposal_objects(state)
        .await?
        .into_iter()
        .filter(|object| ontology_onboarding_object_run_id(object) == Some(run_id))
    {
        let mut proposal = ontology_onboarding_object_proposal(&object)?;
        if !ontology_proposal_references_table(&proposal, table_name) {
            continue;
        }
        proposal.recommendation = recommendation.to_string();
        state
            .update_semantic_object(
                object.id,
                UpdateSemanticObject {
                    title: None,
                    summary: None,
                    content: Some(ontology_onboarding_proposal_content(
                        run_id,
                        &proposal,
                        ontology_onboarding_object_materialized(&object),
                        object.content.get("review").cloned(),
                    )?),
                    semantic_scopes: None,
                    source_uri: None,
                    provenance: None,
                    trust_level: None,
                    freshness: None,
                    status: None,
                },
            )
            .await?;
    }
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "ontology_onboarding.proposal_recommendations_refreshed",
            "ontology_onboarding_run",
            Some(run_id),
            json!({
                "subject": actor_subject,
                "run_id": run_id,
                "table_name": table_name,
                "curated_dataset_review_status": review_status,
                "recommendation": recommendation,
            }),
        ))
        .await?;
    Ok(())
}

pub(crate) fn ontology_proposal_references_table(
    proposal: &OntologyOnboardingProposalDraft,
    table_name: &str,
) -> bool {
    let table_matches = |value: Option<&Value>| {
        value
            .and_then(Value::as_str)
            .map(|value| value == table_name)
            .unwrap_or(false)
    };
    table_matches(proposal.content.get("source_table"))
        || table_matches(proposal.evidence.get("source_table"))
        || table_matches(proposal.evidence.get("table"))
        || table_matches(proposal.evidence.get("references_table"))
}

pub(crate) fn ontology_curated_dataset_review_object_key(run_id: Uuid, table_name: &str) -> String {
    format!(
        "ontology:curated-dataset-review:{run_id}:{}",
        ontology_slug(table_name)
    )
}

pub(crate) fn ontology_builder_dag_for_mode(
    mode: &str,
    run_id: Option<Uuid>,
    changed_node_id: Option<&str>,
) -> Result<OntologyBuilderDag, AppError> {
    let mode = mode.trim().to_ascii_lowercase().replace('-', "_");
    let (nodes, edges) = match mode.as_str() {
        "pipeline_mapping_v2" => ontology_pipeline_mapping_v2_dag_parts(),
        "simple_llm_extraction_v1" => ontology_simple_llm_extraction_v1_dag_parts(),
        _ => {
            return Err(AppError::bad_request(
                "ontology builder DAG mode must be pipeline_mapping_v2 or simple_llm_extraction_v1",
            ));
        }
    };
    let execution_levels = ontology_topological_execution_levels(&nodes, &edges)?;
    let stale_node_ids = changed_node_id
        .map(|node_id| ontology_downstream_node_ids(node_id, &edges))
        .unwrap_or_default();
    Ok(OntologyBuilderDag {
        run_id,
        mode,
        nodes,
        edges,
        execution_levels,
        stale_node_ids,
    })
}

pub(crate) fn ontology_pipeline_mapping_v2_dag_parts()
-> (Vec<OntologyBuilderNode>, Vec<OntologyBuilderEdge>) {
    let nodes = vec![
        ontology_builder_node(
            "connector_sync",
            "Connector sync",
            "ingestion",
            "stable",
            "Replicate source objects into raw storage.",
        ),
        ontology_builder_node(
            "raw_snapshot",
            "Raw snapshot",
            "raw",
            "stable",
            "Preserve source payload, batch, and lineage fields.",
        ),
        ontology_builder_node(
            "metadata_scan",
            "Metadata scan",
            "catalog",
            "stable",
            "Collect table, field, owner, lineage, and usage metadata.",
        ),
        ontology_builder_node(
            "schema_profile",
            "Schema profile",
            "profile",
            "stable",
            "Measure keys, null rates, joins, enums, time, currency, and PII.",
        ),
        ontology_builder_node(
            "curated_dataset_draft",
            "Curated dataset draft",
            "curation",
            "review_required",
            "Create reviewable curated dataset candidates.",
        ),
        ontology_builder_node(
            "prompt_packet",
            "Ontology prompt packet",
            "ai_context",
            "stable",
            "Bundle seed ontology, catalog, profiles, samples, policies, and allowed triples.",
        ),
        ontology_builder_node(
            "proposal_engine",
            "Proposal engine",
            "ai_inference",
            "review_required",
            "Propose objects, relations, metrics, logic rules, actions, and mappings.",
        ),
        ontology_builder_node(
            "review_graph",
            "Review graph",
            "visual_review",
            "review_required",
            "Project proposals into an operator graph for business-logic validation.",
        ),
        ontology_builder_node(
            "human_review",
            "Human review",
            "approval",
            "review_required",
            "Accept, reject, modify, merge, or request evidence.",
        ),
        ontology_builder_node(
            "semantic_materialize",
            "Semantic materialization",
            "ontology_store",
            "approval_gated",
            "Write approved ontology objects, links, metrics, and disabled logic rules.",
        ),
        ontology_builder_node(
            "tool_compile",
            "Semantic tool compile",
            "agent_tools",
            "approval_gated",
            "Compile approved action types into governed agent tools.",
        ),
    ];
    let edges = vec![
        ontology_builder_edge(
            "connector_sync",
            "raw_snapshot",
            "produces",
            "Raw data depends on connector sync.",
        ),
        ontology_builder_edge(
            "raw_snapshot",
            "metadata_scan",
            "catalogs",
            "Catalog scans raw tables.",
        ),
        ontology_builder_edge(
            "raw_snapshot",
            "schema_profile",
            "profiles",
            "Profiler samples raw tables.",
        ),
        ontology_builder_edge(
            "metadata_scan",
            "curated_dataset_draft",
            "informs",
            "Curated drafts need catalog metadata.",
        ),
        ontology_builder_edge(
            "schema_profile",
            "curated_dataset_draft",
            "informs",
            "Curated drafts need data quality evidence.",
        ),
        ontology_builder_edge(
            "curated_dataset_draft",
            "prompt_packet",
            "feeds",
            "Prompt packet uses reviewed dataset candidates.",
        ),
        ontology_builder_edge(
            "metadata_scan",
            "prompt_packet",
            "feeds",
            "Prompt packet includes catalog context.",
        ),
        ontology_builder_edge(
            "schema_profile",
            "prompt_packet",
            "feeds",
            "Prompt packet includes profile evidence.",
        ),
        ontology_builder_edge(
            "prompt_packet",
            "proposal_engine",
            "constrains",
            "AI proposals must cite packet evidence.",
        ),
        ontology_builder_edge(
            "proposal_engine",
            "review_graph",
            "projects",
            "Graph visualizes proposal dependencies.",
        ),
        ontology_builder_edge(
            "review_graph",
            "human_review",
            "supports",
            "Operators review graph evidence before approval.",
        ),
        ontology_builder_edge(
            "human_review",
            "semantic_materialize",
            "gates",
            "Only approved changes materialize.",
        ),
        ontology_builder_edge(
            "semantic_materialize",
            "tool_compile",
            "feeds",
            "Tools compile from approved ontology actions.",
        ),
    ];
    (nodes, edges)
}

pub(crate) fn ontology_simple_llm_extraction_v1_dag_parts()
-> (Vec<OntologyBuilderNode>, Vec<OntologyBuilderEdge>) {
    let nodes = vec![
        ontology_builder_node(
            "document_upload",
            "Document upload",
            "document",
            "stable",
            "Receive uploaded SOP, contract, or manual.",
        ),
        ontology_builder_node(
            "prompt_select",
            "Prompt select",
            "configuration",
            "stable",
            "Select extraction prompt and model.",
        ),
        ontology_builder_node(
            "llm_extract",
            "LLM extract",
            "ai_inference",
            "review_required",
            "Extract draft graph facts from bounded document text.",
        ),
        ontology_builder_node(
            "schema_validate",
            "Schema validate",
            "validation",
            "stable",
            "Validate extracted facts against ontology IR schema.",
        ),
        ontology_builder_node(
            "review_graph",
            "Review graph",
            "visual_review",
            "review_required",
            "Show extracted objects and relations for confirmation.",
        ),
        ontology_builder_node(
            "human_review",
            "Human review",
            "approval",
            "review_required",
            "Approve or reject extracted graph changes.",
        ),
        ontology_builder_node(
            "semantic_materialize",
            "Semantic materialization",
            "ontology_store",
            "approval_gated",
            "Store approved graph facts.",
        ),
    ];
    let edges = vec![
        ontology_builder_edge(
            "document_upload",
            "llm_extract",
            "feeds",
            "Extraction reads uploaded document text.",
        ),
        ontology_builder_edge(
            "prompt_select",
            "llm_extract",
            "configures",
            "Extraction depends on prompt/model choice.",
        ),
        ontology_builder_edge(
            "llm_extract",
            "schema_validate",
            "validates",
            "Schema validation checks LLM output.",
        ),
        ontology_builder_edge(
            "schema_validate",
            "review_graph",
            "projects",
            "Only valid facts are visualized.",
        ),
        ontology_builder_edge(
            "review_graph",
            "human_review",
            "supports",
            "Operators inspect graph before approval.",
        ),
        ontology_builder_edge(
            "human_review",
            "semantic_materialize",
            "gates",
            "Only approved graph facts materialize.",
        ),
    ];
    (nodes, edges)
}

pub(crate) fn ontology_builder_node(
    id: &str,
    label: &str,
    node_type: &str,
    status: &str,
    summary: &str,
) -> OntologyBuilderNode {
    OntologyBuilderNode {
        id: id.to_string(),
        label: label.to_string(),
        node_type: node_type.to_string(),
        status: status.to_string(),
        summary: summary.to_string(),
    }
}

pub(crate) fn ontology_builder_edge(
    from: &str,
    to: &str,
    edge_type: &str,
    reason: &str,
) -> OntologyBuilderEdge {
    OntologyBuilderEdge {
        from: from.to_string(),
        to: to.to_string(),
        edge_type: edge_type.to_string(),
        reason: reason.to_string(),
    }
}

pub(crate) fn ontology_topological_execution_levels(
    nodes: &[OntologyBuilderNode],
    edges: &[OntologyBuilderEdge],
) -> Result<Vec<OntologyBuilderExecutionLevel>, AppError> {
    let node_ids = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    let mut in_degree = node_ids
        .iter()
        .map(|node_id| (node_id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<String, Vec<String>>::new();
    for edge in edges {
        if !node_ids.contains(&edge.from) || !node_ids.contains(&edge.to) {
            return Err(AppError::bad_request(
                "ontology builder DAG edge references an unknown node",
            ));
        }
        *in_degree.entry(edge.to.clone()).or_default() += 1;
        outgoing
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
    }
    let mut ready = in_degree
        .iter()
        .filter(|&(_, degree)| *degree == 0)
        .map(|(node_id, _)| node_id.clone())
        .collect::<Vec<_>>();
    ready.sort();
    let mut visited = 0usize;
    let mut levels = Vec::new();
    while !ready.is_empty() {
        let current = ready;
        visited += current.len();
        levels.push(OntologyBuilderExecutionLevel {
            level: levels.len(),
            node_ids: current.clone(),
        });
        let mut next_ready = Vec::new();
        for node_id in current {
            if let Some(children) = outgoing.get(&node_id) {
                for child in children {
                    if let Some(degree) = in_degree.get_mut(child) {
                        *degree = degree.saturating_sub(1);
                        if *degree == 0 {
                            next_ready.push(child.clone());
                        }
                    }
                }
            }
        }
        next_ready.sort();
        next_ready.dedup();
        ready = next_ready;
    }
    if visited != nodes.len() {
        return Err(AppError::bad_request(
            "ontology builder DAG contains a cycle; execution rejected before start",
        ));
    }
    Ok(levels)
}

pub(crate) fn ontology_downstream_node_ids(
    node_id: &str,
    edges: &[OntologyBuilderEdge],
) -> Vec<String> {
    let mut visited = BTreeSet::new();
    let mut stack = vec![node_id.to_string()];
    while let Some(current) = stack.pop() {
        for edge in edges.iter().filter(|edge| edge.from == current) {
            if visited.insert(edge.to.clone()) {
                stack.push(edge.to.clone());
            }
        }
    }
    visited.into_iter().collect()
}

pub(crate) fn ontology_prompt_packet_for_run(
    run: &OntologyOnboardingRun,
) -> Result<OntologyPromptPacket, AppError> {
    let (seed, _) =
        ontology_seed_and_source_for_request(&ontology_run_industry(run), &run.source_mode)?;
    let curated_datasets = ontology_curated_dataset_drafts_for_run(run, &seed);
    Ok(OntologyPromptPacket {
        run_id: run.id,
        industry: seed.industry.clone(),
        source_mode: run.source_mode.clone(),
        domain_scope: seed.domain_scope.clone(),
        tool_namespace: seed.tool_namespace.clone(),
        seed_pack: seed.clone(),
        curated_datasets,
        profiles: run.profiles.clone(),
        allowed_ontology_triples: seed
            .relations
            .iter()
            .map(|relation| {
                json!({
                    "from_object": relation.from_object,
                    "relation": relation.relation,
                    "to_object": relation.to_object,
                    "link_type": relation.name,
                })
            })
            .collect(),
        evidence_rules: vec![
            "Every object mapping must cite source table, primary key evidence, and field coverage.".to_string(),
            "Every relation must cite join success rate and null-rate evidence.".to_string(),
            "Metrics must use canonical semantic definitions and owner review.".to_string(),
            "Actions must declare reads, effects, executor, policy, and audit event.".to_string(),
        ],
        policy_reminders: vec![
            "Do not materialize unapproved ontology proposals.".to_string(),
            "PII-bearing fields require owner review and least-privilege access.".to_string(),
            "Write-like actions stay approval-gated until production policy enables execution.".to_string(),
            "Logic rules are created disabled until a publish policy enables enforcement.".to_string(),
        ],
        proposal_count: run.proposals.len(),
    })
}

pub(crate) fn ontology_curated_dataset_drafts_for_run(
    run: &OntologyOnboardingRun,
    seed: &OntologySeedPack,
) -> Vec<CuratedDatasetDraft> {
    run.datasets
        .iter()
        .filter_map(|dataset| {
            let profile = run
                .profiles
                .iter()
                .find(|profile| profile.table_name == dataset.table_name)?;
            let object_candidate = seed
                .objects
                .iter()
                .find(|mapping| mapping.table_name == dataset.table_name)
                .map(|mapping| mapping.object_name.clone());
            let null_issue_count = profile
                .field_null_rates
                .as_object()
                .map(|rates| {
                    rates
                        .values()
                        .filter(|rate| rate.as_f64().unwrap_or_default() > 0.20)
                        .count()
                })
                .unwrap_or_default();
            let quality_score = (0.98 - (null_issue_count as f64 * 0.05)).clamp(0.50, 0.99);
            let mut issues = Vec::new();
            if profile.primary_key_candidates.is_empty() {
                issues.push("no_primary_key_candidate".to_string());
            }
            if !profile.pii_candidates.is_empty() {
                issues.push("pii_review_required".to_string());
            }
            Some(CuratedDatasetDraft {
                id: format!("curated:{}:{}", run.id, dataset.table_name),
                table_name: dataset.table_name.clone(),
                source_system: dataset.source_system.clone(),
                object_candidate,
                quality_score,
                review_status: if issues
                    .iter()
                    .any(|issue| issue == "no_primary_key_candidate")
                {
                    "needs_more_evidence".to_string()
                } else {
                    "pending_review".to_string()
                },
                issues,
                schema_version: "draft.v1".to_string(),
                reviewer_metadata: json!({}),
                sample_rows: dataset.rows.iter().take(3).cloned().collect(),
                profile: profile.clone(),
            })
        })
        .collect()
}

pub(crate) async fn ontology_schema_understanding_for_request(
    state: &AppState,
    input: &SchemaUnderstandingRequest,
) -> Result<SchemaUnderstandingResponse, AppError> {
    let max_sample_rows = input.max_sample_rows.unwrap_or(5).clamp(1, 10);
    if let Some(run_id) = input.run_id {
        let run = get_ontology_onboarding_run_for_state(state, run_id).await?;
        let (seed, _) =
            ontology_seed_and_source_for_request(&ontology_run_industry(&run), &run.source_mode)?;
        return Ok(ontology_schema_understanding_for_datasets(
            Some(run.id),
            &seed,
            &run.datasets,
            &run.profiles,
            max_sample_rows,
        ));
    }
    let industry = input.industry.as_deref().unwrap_or("ecommerce");
    let source_mode = input.source_mode.as_deref().unwrap_or("demo_ecommerce");
    let (seed, source) = ontology_seed_and_source_for_request(industry, source_mode)?;
    let profiles = ontology_profile_demo_datasets(&source.datasets);
    Ok(ontology_schema_understanding_for_datasets(
        None,
        &seed,
        &source.datasets,
        &profiles,
        max_sample_rows,
    ))
}

pub(crate) fn ontology_schema_understanding_for_datasets(
    run_id: Option<Uuid>,
    seed: &OntologySeedPack,
    datasets: &[OntologyOnboardingDataset],
    profiles: &[OntologyDatasetProfile],
    max_sample_rows: usize,
) -> SchemaUnderstandingResponse {
    let candidates = datasets
        .iter()
        .filter_map(|dataset| {
            let profile = profiles
                .iter()
                .find(|profile| profile.table_name == dataset.table_name)?;
            Some(ontology_schema_understanding_candidate(
                seed,
                dataset,
                profile,
                max_sample_rows,
            ))
        })
        .collect::<Vec<_>>();
    SchemaUnderstandingResponse {
        run_id,
        industry: seed.industry.clone(),
        source_mode: seed.source_mode.clone(),
        domain_scope: seed.domain_scope.clone(),
        tool_namespace: seed.tool_namespace.clone(),
        candidate_count: candidates.len(),
        candidates,
    }
}

pub(crate) fn ontology_schema_understanding_candidate(
    seed: &OntologySeedPack,
    dataset: &OntologyOnboardingDataset,
    profile: &OntologyDatasetProfile,
    max_sample_rows: usize,
) -> SchemaUnderstandingCandidate {
    let seed_match = seed
        .objects
        .iter()
        .find(|mapping| mapping.table_name == dataset.table_name)
        .map(|mapping| mapping.object_name.clone());
    let object_type_candidate = seed_match
        .clone()
        .unwrap_or_else(|| ontology_infer_object_type_from_table(&dataset.table_name));
    let profile_score =
        ontology_schema_understanding_profile_score(seed_match.is_some(), profile, dataset);
    let confidence =
        ontology_schema_understanding_confidence(seed_match.is_some(), profile, dataset);
    let recommendation = ontology_schema_understanding_recommendation(confidence);
    let sample_row_refs = dataset
        .rows
        .iter()
        .take(max_sample_rows)
        .enumerate()
        .map(|(index, _)| {
            format!(
                "mandoforge://ontology/sources/{}/{}/rows/{index}",
                dataset.source_system, dataset.table_name
            )
        })
        .collect::<Vec<_>>();
    SchemaUnderstandingCandidate {
        table_name: dataset.table_name.clone(),
        source_system: dataset.source_system.clone(),
        source_object: dataset.source_object.clone(),
        object_type_candidate: object_type_candidate.clone(),
        confidence,
        recommendation,
        properties: dataset
            .fields
            .iter()
            .map(|field| ontology_property_understanding_candidate(field, profile))
            .collect(),
        taxonomy_layers: ontology_taxonomy_layers_for_candidate(
            seed,
            &object_type_candidate,
            seed_match.is_some(),
            confidence,
        ),
        evidence: json!({
            "engine": "deterministic_schema_understanding_v1",
            "llm_status": "not_invoked",
            "profile_score": profile_score,
            "seed_ontology_match": seed_match,
            "primary_key_candidates": profile.primary_key_candidates,
            "foreign_key_candidates": profile.foreign_key_candidates,
            "pii_candidates": profile.pii_candidates,
            "currency_fields": profile.currency_fields,
            "time_dimensions": profile.time_dimensions,
            "row_count": profile.row_count,
            "field_count": dataset.fields.len(),
            "sample_row_refs": sample_row_refs,
            "source_mode": seed.source_mode,
            "industry": seed.industry,
            "domain_scope": seed.domain_scope,
            "tool_namespace": seed.tool_namespace,
        }),
    }
}

pub(crate) fn ontology_schema_understanding_profile_score(
    has_seed_match: bool,
    profile: &OntologyDatasetProfile,
    dataset: &OntologyOnboardingDataset,
) -> f64 {
    let mut score: f64 = 0.20;
    if profile.row_count > 0 {
        score += 0.15;
    }
    if has_seed_match {
        score += 0.25;
    }
    if !profile.primary_key_candidates.is_empty() {
        score += 0.15;
    }
    if dataset.fields.len() >= 3 {
        score += 0.10;
    }
    if !profile.foreign_key_candidates.is_empty() {
        score += 0.08;
    }
    if !profile.time_dimensions.is_empty() || !profile.currency_fields.is_empty() {
        score += 0.04;
    }
    if ontology_profile_has_low_null_pressure(profile) {
        score += 0.03;
    }
    score.clamp(0.0, 1.0)
}

pub(crate) fn ontology_schema_understanding_confidence(
    has_seed_match: bool,
    profile: &OntologyDatasetProfile,
    dataset: &OntologyOnboardingDataset,
) -> f64 {
    let profile_score =
        ontology_schema_understanding_profile_score(has_seed_match, profile, dataset);
    let confidence = 0.45 + (profile_score * 0.50);
    confidence.clamp(0.40, 0.96)
}

pub(crate) fn ontology_schema_understanding_recommendation(confidence: f64) -> String {
    if confidence >= 0.90 {
        "draft_ready".to_string()
    } else if confidence >= 0.70 {
        "quick_review".to_string()
    } else {
        "needs_review".to_string()
    }
}

pub(crate) fn ontology_profile_has_low_null_pressure(profile: &OntologyDatasetProfile) -> bool {
    profile
        .field_null_rates
        .as_object()
        .map(|rates| {
            rates
                .values()
                .all(|rate| rate.as_f64().unwrap_or_default() <= 0.20)
        })
        .unwrap_or(false)
}

pub(crate) fn ontology_property_understanding_candidate(
    field: &OntologyOnboardingField,
    profile: &OntologyDatasetProfile,
) -> PropertyUnderstandingCandidate {
    let semantic_role = ontology_property_semantic_role(&field.name, profile);
    let null_rate = profile
        .field_null_rates
        .get(&field.name)
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let uniqueness = profile
        .field_uniqueness
        .get(&field.name)
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let confidence = match semantic_role.as_str() {
        "primary_key" => 0.96,
        "foreign_key" => 0.92,
        "pii" | "currency" | "time_dimension" => 0.88,
        "enum" => 0.82,
        _ => 0.74,
    };
    PropertyUnderstandingCandidate {
        field_name: field.name.clone(),
        field_type: field.field_type.clone(),
        semantic_role,
        confidence,
        evidence: json!({
            "null_rate": null_rate,
            "uniqueness": uniqueness,
            "sample_values": field.sample_values,
            "is_primary_key_candidate": profile.primary_key_candidates.contains(&field.name),
            "foreign_key_candidates": profile.foreign_key_candidates
                .iter()
                .filter(|candidate| candidate.field == field.name)
                .collect::<Vec<_>>(),
        }),
    }
}

pub(crate) fn ontology_property_semantic_role(
    field_name: &str,
    profile: &OntologyDatasetProfile,
) -> String {
    if profile
        .primary_key_candidates
        .iter()
        .any(|field| field == field_name)
        && field_name == "id"
    {
        "primary_key".to_string()
    } else if profile
        .foreign_key_candidates
        .iter()
        .any(|candidate| candidate.field == field_name)
    {
        "foreign_key".to_string()
    } else if profile
        .pii_candidates
        .iter()
        .any(|field| field == field_name)
    {
        "pii".to_string()
    } else if profile
        .currency_fields
        .iter()
        .any(|field| field == field_name)
    {
        "currency".to_string()
    } else if profile
        .time_dimensions
        .iter()
        .any(|field| field == field_name)
    {
        "time_dimension".to_string()
    } else if profile
        .enum_candidates
        .iter()
        .any(|field| field == field_name)
    {
        "enum".to_string()
    } else {
        "attribute".to_string()
    }
}

pub(crate) fn ontology_taxonomy_layers_for_candidate(
    seed: &OntologySeedPack,
    object_type_candidate: &str,
    has_seed_match: bool,
    confidence: f64,
) -> Vec<TaxonomyLayerCandidate> {
    vec![
        TaxonomyLayerCandidate {
            layer: 1,
            label: "Business Entity".to_string(),
            confidence: 0.90,
            rationale: "All schema-understanding candidates are business-facing ontology drafts."
                .to_string(),
        },
        TaxonomyLayerCandidate {
            layer: 2,
            label: ontology_title_case(&seed.domain_scope),
            confidence: if has_seed_match { 0.92 } else { 0.70 },
            rationale: format!(
                "Candidate belongs to the {} domain scope.",
                seed.domain_scope
            ),
        },
        TaxonomyLayerCandidate {
            layer: 3,
            label: object_type_candidate.to_string(),
            confidence,
            rationale: if has_seed_match {
                "Matched a seed ontology object mapping with profile evidence.".to_string()
            } else {
                "Inferred from table naming and profile evidence; requires human review."
                    .to_string()
            },
        },
    ]
}

pub(crate) fn ontology_infer_object_type_from_table(table_name: &str) -> String {
    let trimmed = table_name
        .trim_start_matches("raw_")
        .trim_start_matches("stg_")
        .trim_start_matches("curated_");
    let singular = trimmed.strip_suffix('s').unwrap_or(trimmed);
    ontology_title_case(singular)
}

pub(crate) fn ontology_title_case(value: &str) -> String {
    value
        .split(['_', '-', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<String>()
}

pub(crate) async fn ontology_subgraph_proposals_for_request(
    state: &AppState,
    input: &SubgraphProposalRequest,
) -> Result<SubgraphProposalResponse, AppError> {
    let run = if let Some(run_id) = input.run_id {
        get_ontology_onboarding_run_for_state(state, run_id).await?
    } else {
        let industry = input.industry.as_deref().unwrap_or("ecommerce");
        let source_mode = input.source_mode.as_deref().unwrap_or("demo_ecommerce");
        let (seed, source) = ontology_seed_and_source_for_request(industry, source_mode)?;
        let profiles = ontology_profile_demo_datasets(&source.datasets);
        let run_id = Uuid::new_v4();
        let proposals =
            ontology_generate_seed_proposals_for_run(run_id, &seed, &source.datasets, &profiles);
        OntologyOnboardingRun {
            id: run_id,
            status: "draft_preview".to_string(),
            source_mode: source.source_mode,
            dataset_count: source.datasets.len(),
            profile_count: profiles.len(),
            proposal_count: proposals.len(),
            approved_count: 0,
            materialized_count: 0,
            datasets: source.datasets,
            profiles,
            proposals,
            generated_at: Utc::now(),
        }
    };
    let (seed, _) =
        ontology_seed_and_source_for_request(&ontology_run_industry(&run), &run.source_mode)?;
    let review_status = input
        .review_decision
        .as_deref()
        .map(ontology_normalize_subgraph_review_status)
        .transpose()?
        .unwrap_or_else(|| "pending".to_string());
    let target_filter = input.target_object.as_deref();
    let subgraphs = ontology_subgraph_proposals_for_run(&run, &seed, target_filter, &review_status);
    Ok(SubgraphProposalResponse {
        run_id: Some(run.id),
        industry: seed.industry.clone(),
        source_mode: run.source_mode.clone(),
        domain_scope: seed.domain_scope.clone(),
        tool_namespace: seed.tool_namespace.clone(),
        subgraph_count: subgraphs.len(),
        subgraphs,
    })
}

pub(crate) fn ontology_subgraph_proposals_for_run(
    run: &OntologyOnboardingRun,
    seed: &OntologySeedPack,
    target_filter: Option<&str>,
    review_status: &str,
) -> Vec<SubgraphProposalDraft> {
    let mut target_objects = run
        .proposals
        .iter()
        .filter(|proposal| proposal.proposal_type == "object")
        .filter_map(|proposal| {
            proposal
                .content
                .get("object_type")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect::<BTreeSet<_>>();
    for action in &seed.actions {
        target_objects.insert(action.target_object.clone());
    }
    let mut subgraphs = target_objects
        .into_iter()
        .filter(|target| {
            target_filter
                .map(|filter| ontology_slug(filter) == ontology_slug(target))
                .unwrap_or(true)
        })
        .map(|target| ontology_subgraph_proposal_for_object(run, seed, &target, review_status))
        .filter(|subgraph| !subgraph.members.is_empty())
        .collect::<Vec<_>>();
    subgraphs.sort_by(|left, right| left.target_object.cmp(&right.target_object));
    subgraphs
}

pub(crate) fn ontology_subgraph_proposal_for_object(
    run: &OntologyOnboardingRun,
    seed: &OntologySeedPack,
    target_object: &str,
    review_status: &str,
) -> SubgraphProposalDraft {
    let mut members = BTreeMap::<Uuid, SubgraphProposalMember>::new();
    let related_objects = ontology_subgraph_related_objects(run, target_object, 2);
    for proposal in &run.proposals {
        if ontology_proposal_belongs_to_subgraph(proposal, seed, target_object, &related_objects) {
            members.insert(
                proposal.id,
                SubgraphProposalMember {
                    proposal_id: proposal.id,
                    proposal_type: proposal.proposal_type.clone(),
                    name: proposal.name.clone(),
                    role: ontology_subgraph_member_role(proposal, target_object),
                    confidence: proposal.confidence,
                    review_status: proposal.review_status.clone(),
                },
            );
        }
    }
    let members = members.into_values().collect::<Vec<_>>();
    let confidence = if members.is_empty() {
        0.0
    } else {
        members.iter().map(|member| member.confidence).sum::<f64>() / members.len() as f64
    };
    let recommendation = if review_status == "rejected" {
        "do_not_materialize_children".to_string()
    } else if confidence >= 0.90 {
        "quick_review".to_string()
    } else {
        "needs_review".to_string()
    };
    SubgraphProposalDraft {
        id: ontology_subgraph_id(run.id, target_object),
        run_id: Some(run.id),
        name: format!("{target_object} business subgraph"),
        target_object: target_object.to_string(),
        review_status: review_status.to_string(),
        confidence,
        recommendation,
        evidence: json!({
            "engine": "deterministic_subgraph_proposal_v1",
            "authority": "proposal_only",
            "target_object": target_object,
            "related_objects": related_objects,
            "member_count": members.len(),
            "member_proposal_ids": members.iter().map(|member| member.proposal_id).collect::<Vec<_>>(),
            "materialization_policy": "subgraph review does not materialize child proposals",
            "industry": seed.industry,
            "source_mode": seed.source_mode,
            "domain_scope": seed.domain_scope,
            "tool_namespace": seed.tool_namespace,
        }),
        members,
    }
}

pub(crate) fn ontology_subgraph_related_objects(
    run: &OntologyOnboardingRun,
    target_object: &str,
    max_depth: usize,
) -> BTreeSet<String> {
    let mut related = BTreeSet::from([ontology_slug(target_object)]);
    for _ in 0..max_depth {
        let before = related.len();
        for proposal in run
            .proposals
            .iter()
            .filter(|proposal| proposal.proposal_type == "relation")
        {
            let Some(from_object) = proposal.content.get("from_object").and_then(Value::as_str)
            else {
                continue;
            };
            let Some(to_object) = proposal.content.get("to_object").and_then(Value::as_str) else {
                continue;
            };
            let from_slug = ontology_slug(from_object);
            let to_slug = ontology_slug(to_object);
            if related.contains(&from_slug) || related.contains(&to_slug) {
                related.insert(from_slug);
                related.insert(to_slug);
            }
        }
        if related.len() == before {
            break;
        }
    }
    related
}

pub(crate) fn ontology_proposal_belongs_to_subgraph(
    proposal: &OntologyOnboardingProposalDraft,
    seed: &OntologySeedPack,
    target_object: &str,
    related_objects: &BTreeSet<String>,
) -> bool {
    let target_slug = ontology_slug(target_object);
    match proposal.proposal_type.as_str() {
        "object" => proposal
            .content
            .get("object_type")
            .and_then(Value::as_str)
            .map(|object| related_objects.contains(&ontology_slug(object)))
            .unwrap_or(false),
        "logic" | "logic_rule" => proposal
            .content
            .get("target_object")
            .and_then(Value::as_str)
            .map(|object| related_objects.contains(&ontology_slug(object)))
            .unwrap_or(false),
        "relation" => {
            let from_matches = proposal
                .content
                .get("from_object")
                .and_then(Value::as_str)
                .map(|object| related_objects.contains(&ontology_slug(object)))
                .unwrap_or(false);
            let to_matches = proposal
                .content
                .get("to_object")
                .and_then(Value::as_str)
                .map(|object| related_objects.contains(&ontology_slug(object)))
                .unwrap_or(false);
            from_matches || to_matches
        }
        "metric" => {
            proposal
                .content
                .get("target_object")
                .and_then(Value::as_str)
                .map(|object| {
                    ontology_slug(object) == target_slug
                        || related_objects.contains(&ontology_slug(object))
                })
                .unwrap_or(false)
                || seed
                    .metrics
                    .iter()
                    .find(|metric| metric.name == proposal.name)
                    .map(|metric| ontology_metric_expression_mentions_object(metric, target_object))
                    .unwrap_or(false)
        }
        "action" => proposal
            .content
            .get("target_object")
            .and_then(Value::as_str)
            .map(|object| ontology_slug(object) == target_slug)
            .unwrap_or(false),
        _ => false,
    }
}

pub(crate) fn ontology_metric_expression_mentions_object(
    metric: &OntologySeedMetricMapping,
    target_object: &str,
) -> bool {
    let object_slug = ontology_slug(target_object);
    let expression = metric.expression.to_ascii_lowercase();
    let evidence = metric.evidence.to_string().to_ascii_lowercase();
    object_slug
        .split('_')
        .any(|part| !part.is_empty() && (expression.contains(part) || evidence.contains(part)))
}

pub(crate) fn ontology_subgraph_member_role(
    proposal: &OntologyOnboardingProposalDraft,
    target_object: &str,
) -> String {
    match proposal.proposal_type.as_str() {
        "object" => "anchor_object".to_string(),
        "relation" => {
            let from_matches = proposal
                .content
                .get("from_object")
                .and_then(Value::as_str)
                .map(|object| ontology_slug(object) == ontology_slug(target_object))
                .unwrap_or(false);
            if from_matches {
                "outbound_relation".to_string()
            } else {
                "inbound_relation".to_string()
            }
        }
        "metric" => "metric".to_string(),
        "logic" | "logic_rule" => "validation_logic".to_string(),
        "action" => "action".to_string(),
        _ => "supporting_proposal".to_string(),
    }
}

pub(crate) fn ontology_normalize_subgraph_review_status(
    decision: &str,
) -> Result<String, AppError> {
    let decision = decision.trim().to_ascii_lowercase().replace('-', "_");
    match decision.as_str() {
        "pending" => Ok("pending".to_string()),
        "approve" | "approved" => Ok("approved".to_string()),
        "reject" | "rejected" => Ok("rejected".to_string()),
        "request_changes" | "changes_requested" => Ok("changes_requested".to_string()),
        "needs_more_evidence" => Ok("needs_more_evidence".to_string()),
        _ => Err(AppError::bad_request(
            "subgraph review decision must be pending, approve, reject, request_changes, or needs_more_evidence",
        )),
    }
}

pub(crate) fn ontology_subgraph_id(run_id: Uuid, target_object: &str) -> String {
    format!("subgraph:{run_id}:{}", ontology_slug(target_object))
}

pub(crate) async fn ontology_entity_resolution_for_request(
    state: &AppState,
    input: &EntityResolutionRequest,
) -> Result<EntityResolutionResponse, AppError> {
    let min_score = input.min_score.unwrap_or(0.50).clamp(0.0, 1.0);
    let candidates = if let Some(run_id) = input.run_id {
        let run = get_ontology_onboarding_run_for_state(state, run_id).await?;
        run.proposals
            .iter()
            .filter(|proposal| proposal.proposal_type == "object")
            .map(|proposal| {
                let candidate_name = proposal
                    .content
                    .get("object_type")
                    .and_then(Value::as_str)
                    .unwrap_or(&proposal.name)
                    .to_string();
                let domain_scope = ontology_proposal_domain_scope(proposal);
                (candidate_name.clone(), candidate_name, domain_scope)
            })
            .collect::<Vec<_>>()
    } else {
        vec![(
            input.candidate_name.clone().ok_or_else(|| {
                AppError::bad_request("entity resolution requires candidate_name or run_id")
            })?,
            input
                .candidate_object_type
                .clone()
                .or_else(|| input.candidate_name.clone())
                .ok_or_else(|| {
                    AppError::bad_request(
                        "entity resolution requires candidate_object_type or candidate_name",
                    )
                })?,
            input
                .domain_scope
                .clone()
                .unwrap_or_else(|| "commerce".to_string()),
        )]
    };
    let semantic_objects = state.list_semantic_objects().await?;
    let resolved = candidates
        .into_iter()
        .map(|(candidate_name, candidate_object_type, domain_scope)| {
            ontology_entity_resolution_candidate(
                &candidate_name,
                &candidate_object_type,
                &domain_scope,
                &semantic_objects,
                min_score,
            )
        })
        .collect::<Vec<_>>();
    Ok(EntityResolutionResponse {
        run_id: input.run_id,
        candidate_count: resolved.len(),
        candidates: resolved,
    })
}

pub(crate) fn ontology_entity_resolution_candidate(
    candidate_name: &str,
    candidate_object_type: &str,
    domain_scope: &str,
    semantic_objects: &[SemanticObject],
    min_score: f64,
) -> EntityResolutionCandidate {
    let normalized_name = ontology_resolution_normalized_name(candidate_name);
    let mut retrieval_hits = semantic_objects
        .iter()
        .filter(|object| object.status == "active" && object.archived_at.is_none())
        .filter_map(|object| {
            ontology_entity_resolution_hit(
                candidate_name,
                candidate_object_type,
                domain_scope,
                &normalized_name,
                object,
            )
        })
        .filter(|hit| hit.score >= min_score)
        .collect::<Vec<_>>();
    retrieval_hits.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.title.cmp(&right.title))
    });
    retrieval_hits.truncate(5);
    let decision = ontology_entity_resolution_decision(candidate_name, &retrieval_hits);
    EntityResolutionCandidate {
        candidate_name: candidate_name.to_string(),
        candidate_object_type: candidate_object_type.to_string(),
        domain_scope: domain_scope.to_string(),
        normalized_name,
        retrieval_hits,
        decision,
        evidence: json!({
            "engine": "deterministic_entity_resolution_v1",
            "retrieval": ["exact_normalized_name", "alias_synonym", "token_overlap", "object_type_compatibility"],
            "vector_status": "not_configured",
            "bm25_status": "deterministic_token_overlap",
            "llm_status": "not_invoked",
        }),
    }
}

pub(crate) fn ontology_entity_resolution_hit(
    candidate_name: &str,
    candidate_object_type: &str,
    domain_scope: &str,
    normalized_name: &str,
    object: &SemanticObject,
) -> Option<EntityResolutionRetrievalHit> {
    let object_domain = ontology_semantic_object_domain_scope(object);
    let object_business_type = ontology_semantic_object_business_type(object);
    let object_label = object_business_type
        .clone()
        .unwrap_or_else(|| object.title.clone());
    let object_normalized = ontology_resolution_normalized_name(&object_label);
    let candidate_business_type = ontology_resolution_normalized_name(candidate_object_type);
    let mut score: f64 = 0.0;
    let mut match_reasons = Vec::new();

    if object_domain == domain_scope {
        score += 0.10;
        match_reasons.push("same_domain_scope".to_string());
    }
    if object_normalized == normalized_name {
        score += 0.55;
        match_reasons.push("exact_normalized_name".to_string());
    }
    if ontology_resolution_alias_match(normalized_name, &object_normalized) {
        score += 0.50;
        match_reasons.push("alias_synonym".to_string());
    }
    let token_overlap = ontology_resolution_token_overlap(normalized_name, &object_normalized);
    if token_overlap > 0.0 {
        score += token_overlap * 0.25;
        match_reasons.push("token_overlap".to_string());
    }
    let compatible_type = object.object_type == "business_object"
        || object.object_type == "ontology_object_type"
        || object.object_type == "ontology_onboarding_proposal";
    if compatible_type {
        score += 0.10;
        match_reasons.push("object_type_compatible".to_string());
    }
    if object_normalized == candidate_business_type
        || ontology_resolution_alias_match(&candidate_business_type, &object_normalized)
    {
        score += 0.15;
        match_reasons.push("business_type_compatible".to_string());
    }
    if match_reasons.is_empty() {
        return None;
    }
    Some(EntityResolutionRetrievalHit {
        object_id: object.id,
        object_key: object.object_key.clone(),
        title: object.title.clone(),
        object_type: object.object_type.clone(),
        domain_scope: object_domain,
        normalized_name: object_normalized,
        score: score.clamp(0.0, 1.0),
        match_reasons,
        evidence: json!({
            "candidate_name": candidate_name,
            "candidate_object_type": candidate_object_type,
            "semantic_object_type": object.object_type,
            "semantic_object_title": object.title,
            "semantic_object_key": object.object_key,
            "business_type": object_business_type,
        }),
    })
}

pub(crate) fn ontology_entity_resolution_decision(
    candidate_name: &str,
    retrieval_hits: &[EntityResolutionRetrievalHit],
) -> EntityResolutionDecisionDraft {
    let Some(best_hit) = retrieval_hits.first() else {
        return EntityResolutionDecisionDraft {
            is_duplicate: false,
            canonical_name: candidate_name.to_string(),
            existing_node_uuid: None,
            confidence: 0.0,
            decision: "new_entity".to_string(),
            review_required: false,
            rationale: "No compatible existing ontology object passed retrieval threshold."
                .to_string(),
        };
    };
    let is_duplicate = best_hit.score >= 0.80;
    EntityResolutionDecisionDraft {
        is_duplicate,
        canonical_name: if is_duplicate {
            best_hit.title.clone()
        } else {
            candidate_name.to_string()
        },
        existing_node_uuid: is_duplicate.then_some(best_hit.object_id),
        confidence: best_hit.score,
        decision: if is_duplicate {
            "merge_into_existing".to_string()
        } else if best_hit.score >= 0.60 {
            "possible_match_needs_review".to_string()
        } else {
            "new_entity".to_string()
        },
        review_required: best_hit.score >= 0.60,
        rationale: if is_duplicate {
            "High-confidence deterministic retrieval suggests this candidate duplicates an existing ontology object.".to_string()
        } else {
            "Retrieval evidence is not strong enough to merge automatically.".to_string()
        },
    }
}

pub(crate) fn ontology_semantic_object_domain_scope(object: &SemanticObject) -> String {
    object
        .content
        .get("domain_scope")
        .or_else(|| object.semantic_scopes.get("domain_scope"))
        .and_then(Value::as_str)
        .unwrap_or("global")
        .to_string()
}

pub(crate) fn ontology_semantic_object_business_type(object: &SemanticObject) -> Option<String> {
    object
        .content
        .get("object_type")
        .or_else(|| object.content.get("business_object"))
        .or_else(|| object.content.get("target_object"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(crate) fn ontology_resolution_normalized_name(value: &str) -> String {
    ontology_slug(value)
}

pub(crate) fn ontology_resolution_alias_match(left: &str, right: &str) -> bool {
    left == right
        || ontology_resolution_aliases(left).contains(right)
        || ontology_resolution_aliases(right).contains(left)
}

pub(crate) fn ontology_resolution_aliases(value: &str) -> BTreeSet<&'static str> {
    match value {
        "customer" | "client" | "account" | "buyer" | "insured" => {
            BTreeSet::from(["customer", "client", "account", "buyer", "insured"])
        }
        "order" | "trade" | "sales_order" => BTreeSet::from(["order", "trade", "sales_order"]),
        "ticket" | "support_ticket" | "case" => {
            BTreeSet::from(["ticket", "support_ticket", "case"])
        }
        "claim" | "insurance_claim" => BTreeSet::from(["claim", "insurance_claim"]),
        _ => BTreeSet::new(),
    }
}

pub(crate) fn ontology_resolution_token_overlap(left: &str, right: &str) -> f64 {
    let left_tokens = left
        .split('_')
        .filter(|token| !token.is_empty())
        .collect::<BTreeSet<_>>();
    let right_tokens = right
        .split('_')
        .filter(|token| !token.is_empty())
        .collect::<BTreeSet<_>>();
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }
    let intersection = left_tokens.intersection(&right_tokens).count();
    let union = left_tokens.union(&right_tokens).count();
    intersection as f64 / union as f64
}

pub(crate) async fn ontology_review_graph_for_run(
    state: &AppState,
    run: &OntologyOnboardingRun,
) -> Result<OntologyReviewGraph, AppError> {
    const NODE_LIMIT: usize = 96;
    const EDGE_LIMIT: usize = 160;
    let mut nodes = BTreeMap::<String, OntologyReviewGraphNode>::new();
    let mut edges = BTreeMap::<String, OntologyReviewGraphEdge>::new();
    for dataset in &run.datasets {
        ontology_review_graph_insert_node(
            &mut nodes,
            OntologyReviewGraphNode {
                id: ontology_graph_dataset_id(&dataset.table_name),
                node_type: "dataset".to_string(),
                label: dataset.table_name.clone(),
                status: "profiled".to_string(),
                confidence: 1.0,
                risk: if dataset
                    .fields
                    .iter()
                    .any(|field| ontology_is_pii_field(&field.name))
                {
                    "pii_review".to_string()
                } else {
                    "low".to_string()
                },
                evidence: json!({
                    "source_system": dataset.source_system,
                    "source_object": dataset.source_object,
                    "field_count": dataset.fields.len(),
                    "sample_row_count": dataset.rows.len(),
                }),
                source_proposal_id: None,
            },
        );
    }
    for proposal in &run.proposals {
        match proposal.proposal_type.as_str() {
            "object" => ontology_review_graph_project_object(&mut nodes, &mut edges, proposal),
            "relation" => ontology_review_graph_project_relation(&mut nodes, &mut edges, proposal),
            "metric" => ontology_review_graph_project_metric(&mut nodes, &mut edges, proposal),
            "logic" | "logic_rule" => {
                ontology_review_graph_project_logic(&mut nodes, &mut edges, proposal)
            }
            "action" => ontology_review_graph_project_action(&mut nodes, &mut edges, proposal),
            _ => {}
        }
    }
    let (seed, _) =
        ontology_seed_and_source_for_request(&ontology_run_industry(run), &run.source_mode)?;
    for subgraph in ontology_subgraph_proposals_for_run(run, &seed, None, "pending") {
        ontology_review_graph_project_subgraph(&mut nodes, &mut edges, &subgraph);
    }
    let semantic_objects = state.list_semantic_objects().await?;
    ontology_review_graph_project_entity_resolution(&mut nodes, &mut edges, run, &semantic_objects);
    let materialized_specs = ontology_onboarding_tool_specs_for_run(state, run.id).await?;
    let materialized_tool_ids = materialized_specs
        .iter()
        .map(|spec| spec.source_proposal_id)
        .collect::<BTreeSet<_>>();
    for proposal in run
        .proposals
        .iter()
        .filter(|proposal| proposal.proposal_type == "action")
    {
        let tool_node_id =
            ontology_graph_tool_id(&ontology_proposal_tool_namespace(proposal), &proposal.name);
        ontology_review_graph_insert_node(
            &mut nodes,
            OntologyReviewGraphNode {
                id: tool_node_id.clone(),
                node_type: "tool".to_string(),
                label: format!(
                    "{}.{}",
                    ontology_proposal_tool_namespace(proposal),
                    proposal.name
                ),
                status: if materialized_tool_ids.contains(&proposal.id) {
                    "compiled".to_string()
                } else {
                    "proposed".to_string()
                },
                confidence: proposal.confidence,
                risk: ontology_proposal_risk(proposal),
                evidence: json!({
                    "approval_required": proposal.content["policy"]["approval_required"],
                    "audit_event": proposal.content["audit_event"],
                    "transaction_profile": proposal.content["transaction_profile"],
                    "execution_mode": proposal.content["transaction_policy"]["execution_mode"],
                    "source_mapping": proposal.source_mapping,
                }),
                source_proposal_id: Some(proposal.id),
            },
        );
        let action_node_id = ontology_graph_action_id(&proposal.name);
        ontology_review_graph_insert_edge(
            &mut edges,
            OntologyReviewGraphEdge {
                id: format!("{action_node_id}->{tool_node_id}:compiles_to"),
                from: action_node_id,
                to: tool_node_id,
                edge_type: "compiles_to".to_string(),
                status: proposal.review_status.clone(),
                confidence: proposal.confidence,
                risk: ontology_proposal_risk(proposal),
                evidence: json!({
                    "approval_required": proposal.content["policy"]["approval_required"],
                    "transaction_profile": proposal.content["transaction_profile"],
                    "execution_mode": proposal.content["transaction_policy"]["execution_mode"],
                    "read_only": proposal.content["effects"].as_array().map(|effects| effects.is_empty()).unwrap_or(false),
                }),
                source_proposal_id: Some(proposal.id),
            },
        );
    }
    let omitted_node_count = nodes.len().saturating_sub(NODE_LIMIT);
    let omitted_edge_count = edges.len().saturating_sub(EDGE_LIMIT);
    Ok(OntologyReviewGraph {
        run_id: run.id,
        nodes: nodes.into_values().take(NODE_LIMIT).collect(),
        edges: edges.into_values().take(EDGE_LIMIT).collect(),
        truncated: omitted_node_count > 0 || omitted_edge_count > 0,
        omitted_node_count,
        omitted_edge_count,
    })
}

pub(crate) fn ontology_review_graph_project_object(
    nodes: &mut BTreeMap<String, OntologyReviewGraphNode>,
    edges: &mut BTreeMap<String, OntologyReviewGraphEdge>,
    proposal: &OntologyOnboardingProposalDraft,
) {
    let object_name = proposal
        .content
        .get("object_type")
        .and_then(Value::as_str)
        .unwrap_or(&proposal.name);
    let object_node_id = ontology_graph_object_id(object_name);
    ontology_review_graph_insert_node(
        nodes,
        OntologyReviewGraphNode {
            id: object_node_id.clone(),
            node_type: "object".to_string(),
            label: object_name.to_string(),
            status: proposal.review_status.clone(),
            confidence: proposal.confidence,
            risk: ontology_proposal_risk(proposal),
            evidence: proposal.evidence.clone(),
            source_proposal_id: Some(proposal.id),
        },
    );
    if let Some(source_table) = proposal.content.get("source_table").and_then(Value::as_str) {
        let dataset_node_id = ontology_graph_dataset_id(source_table);
        ontology_review_graph_insert_edge(
            edges,
            OntologyReviewGraphEdge {
                id: format!("{dataset_node_id}->{object_node_id}:maps_to"),
                from: dataset_node_id,
                to: object_node_id,
                edge_type: "maps_to".to_string(),
                status: proposal.review_status.clone(),
                confidence: proposal.confidence,
                risk: ontology_proposal_risk(proposal),
                evidence: json!({
                    "source_mapping": proposal.source_mapping,
                    "primary_key": proposal.content["primary_key"],
                }),
                source_proposal_id: Some(proposal.id),
            },
        );
    }
}

pub(crate) fn ontology_review_graph_project_relation(
    _nodes: &mut BTreeMap<String, OntologyReviewGraphNode>,
    edges: &mut BTreeMap<String, OntologyReviewGraphEdge>,
    proposal: &OntologyOnboardingProposalDraft,
) {
    let from_object = proposal.content.get("from_object").and_then(Value::as_str);
    let to_object = proposal.content.get("to_object").and_then(Value::as_str);
    if let (Some(from_object), Some(to_object)) = (from_object, to_object) {
        let from_node = ontology_graph_object_id(from_object);
        let to_node = ontology_graph_object_id(to_object);
        ontology_review_graph_insert_edge(
            edges,
            OntologyReviewGraphEdge {
                id: format!("{from_node}->{to_node}:{}", ontology_slug(&proposal.name)),
                from: from_node,
                to: to_node,
                edge_type: "relates_to".to_string(),
                status: proposal.review_status.clone(),
                confidence: proposal.confidence,
                risk: ontology_proposal_risk(proposal),
                evidence: proposal.evidence.clone(),
                source_proposal_id: Some(proposal.id),
            },
        );
    }
}

pub(crate) fn ontology_review_graph_project_metric(
    nodes: &mut BTreeMap<String, OntologyReviewGraphNode>,
    edges: &mut BTreeMap<String, OntologyReviewGraphEdge>,
    proposal: &OntologyOnboardingProposalDraft,
) {
    let metric_node_id = ontology_graph_metric_id(&proposal.name);
    ontology_review_graph_insert_node(
        nodes,
        OntologyReviewGraphNode {
            id: metric_node_id.clone(),
            node_type: "metric".to_string(),
            label: proposal.name.clone(),
            status: proposal.review_status.clone(),
            confidence: proposal.confidence,
            risk: ontology_proposal_risk(proposal),
            evidence: proposal.evidence.clone(),
            source_proposal_id: Some(proposal.id),
        },
    );
    if let Some(target_object) = proposal
        .content
        .get("target_object")
        .and_then(Value::as_str)
    {
        let object_node_id = ontology_graph_object_id(target_object);
        ontology_review_graph_insert_edge(
            edges,
            OntologyReviewGraphEdge {
                id: format!("{metric_node_id}->{object_node_id}:uses_metric"),
                from: metric_node_id.clone(),
                to: object_node_id,
                edge_type: "uses_metric".to_string(),
                status: proposal.review_status.clone(),
                confidence: proposal.confidence,
                risk: ontology_proposal_risk(proposal),
                evidence: json!({"expression": proposal.content["expression"]}),
                source_proposal_id: Some(proposal.id),
            },
        );
    }
    if let Some(depends_on) = proposal
        .evidence
        .get("definition_evidence")
        .and_then(|value| value.get("depends_on"))
        .and_then(Value::as_array)
    {
        for dependency in depends_on.iter().filter_map(Value::as_str) {
            let dependency_node_id = ontology_graph_metric_id(dependency);
            ontology_review_graph_insert_node(
                nodes,
                OntologyReviewGraphNode {
                    id: dependency_node_id.clone(),
                    node_type: "metric".to_string(),
                    label: dependency.to_string(),
                    status: "referenced".to_string(),
                    confidence: proposal.confidence,
                    risk: "needs_review".to_string(),
                    evidence: json!({"referenced_by": proposal.name}),
                    source_proposal_id: Some(proposal.id),
                },
            );
            ontology_review_graph_insert_edge(
                edges,
                OntologyReviewGraphEdge {
                    id: format!("{metric_node_id}->{dependency_node_id}:depends_on"),
                    from: metric_node_id.clone(),
                    to: dependency_node_id,
                    edge_type: "depends_on".to_string(),
                    status: proposal.review_status.clone(),
                    confidence: proposal.confidence,
                    risk: ontology_proposal_risk(proposal),
                    evidence: json!({"source_mapping": proposal.source_mapping}),
                    source_proposal_id: Some(proposal.id),
                },
            );
        }
    }
}

pub(crate) fn ontology_review_graph_project_logic(
    nodes: &mut BTreeMap<String, OntologyReviewGraphNode>,
    edges: &mut BTreeMap<String, OntologyReviewGraphEdge>,
    proposal: &OntologyOnboardingProposalDraft,
) {
    let logic_node_id = ontology_graph_logic_id(&proposal.name);
    ontology_review_graph_insert_node(
        nodes,
        OntologyReviewGraphNode {
            id: logic_node_id.clone(),
            node_type: "logic".to_string(),
            label: proposal.name.clone(),
            status: proposal.review_status.clone(),
            confidence: proposal.confidence,
            risk: ontology_proposal_risk(proposal),
            evidence: proposal.evidence.clone(),
            source_proposal_id: Some(proposal.id),
        },
    );
    if let Some(target_object) = proposal
        .content
        .get("target_object")
        .and_then(Value::as_str)
    {
        let object_node_id = ontology_graph_object_id(target_object);
        ontology_review_graph_insert_edge(
            edges,
            OntologyReviewGraphEdge {
                id: format!("{logic_node_id}->{object_node_id}:validates"),
                from: logic_node_id,
                to: object_node_id,
                edge_type: "validates".to_string(),
                status: proposal.review_status.clone(),
                confidence: proposal.confidence,
                risk: ontology_proposal_risk(proposal),
                evidence: proposal.content.clone(),
                source_proposal_id: Some(proposal.id),
            },
        );
    }
}

pub(crate) fn ontology_review_graph_project_action(
    nodes: &mut BTreeMap<String, OntologyReviewGraphNode>,
    edges: &mut BTreeMap<String, OntologyReviewGraphEdge>,
    proposal: &OntologyOnboardingProposalDraft,
) {
    let action_node_id = ontology_graph_action_id(&proposal.name);
    ontology_review_graph_insert_node(
        nodes,
        OntologyReviewGraphNode {
            id: action_node_id.clone(),
            node_type: "action".to_string(),
            label: proposal.name.clone(),
            status: proposal.review_status.clone(),
            confidence: proposal.confidence,
            risk: ontology_proposal_risk(proposal),
            evidence: proposal.evidence.clone(),
            source_proposal_id: Some(proposal.id),
        },
    );
    if let Some(target_object) = proposal
        .content
        .get("target_object")
        .and_then(Value::as_str)
    {
        let object_node_id = ontology_graph_object_id(target_object);
        ontology_review_graph_insert_edge(
            edges,
            OntologyReviewGraphEdge {
                id: format!("{action_node_id}->{object_node_id}:acts_on"),
                from: action_node_id,
                to: object_node_id,
                edge_type: "acts_on".to_string(),
                status: proposal.review_status.clone(),
                confidence: proposal.confidence,
                risk: ontology_proposal_risk(proposal),
                evidence: proposal.content.clone(),
                source_proposal_id: Some(proposal.id),
            },
        );
    }
}

pub(crate) fn ontology_review_graph_project_subgraph(
    nodes: &mut BTreeMap<String, OntologyReviewGraphNode>,
    edges: &mut BTreeMap<String, OntologyReviewGraphEdge>,
    subgraph: &SubgraphProposalDraft,
) {
    let subgraph_node_id = ontology_graph_subgraph_id(&subgraph.target_object);
    let member_ids = subgraph
        .members
        .iter()
        .map(|member| member.proposal_id)
        .collect::<Vec<_>>();
    ontology_review_graph_insert_node(
        nodes,
        OntologyReviewGraphNode {
            id: subgraph_node_id.clone(),
            node_type: "subgraph".to_string(),
            label: subgraph.name.clone(),
            status: subgraph.review_status.clone(),
            confidence: subgraph.confidence,
            risk: if subgraph.review_status == "rejected" {
                "blocked".to_string()
            } else if subgraph.confidence < 0.90 {
                "needs_review".to_string()
            } else {
                "low".to_string()
            },
            evidence: json!({
                "target_object": subgraph.target_object,
                "member_count": subgraph.members.len(),
                "member_proposal_ids": member_ids,
                "recommendation": subgraph.recommendation,
                "authority": "proposal_only",
            }),
            source_proposal_id: subgraph.members.first().map(|member| member.proposal_id),
        },
    );
    for member in &subgraph.members {
        if let Some(member_node_id) = ontology_graph_node_id_for_subgraph_member(member) {
            ontology_review_graph_insert_edge(
                edges,
                OntologyReviewGraphEdge {
                    id: format!("{subgraph_node_id}->{member_node_id}:groups"),
                    from: subgraph_node_id.clone(),
                    to: member_node_id,
                    edge_type: "groups".to_string(),
                    status: subgraph.review_status.clone(),
                    confidence: member.confidence,
                    risk: if subgraph.review_status == "rejected" {
                        "blocked".to_string()
                    } else if member.confidence < 0.90 {
                        "needs_review".to_string()
                    } else {
                        "low".to_string()
                    },
                    evidence: json!({
                        "subgraph_id": subgraph.id,
                        "target_object": subgraph.target_object,
                        "member_role": member.role,
                        "member_proposal_type": member.proposal_type,
                        "member_review_status": member.review_status,
                        "materialization_policy": "subgraph review does not materialize child proposals",
                    }),
                    source_proposal_id: Some(member.proposal_id),
                },
            );
        }
    }
}

pub(crate) fn ontology_review_graph_project_entity_resolution(
    nodes: &mut BTreeMap<String, OntologyReviewGraphNode>,
    edges: &mut BTreeMap<String, OntologyReviewGraphEdge>,
    run: &OntologyOnboardingRun,
    semantic_objects: &[SemanticObject],
) {
    for proposal in run
        .proposals
        .iter()
        .filter(|proposal| proposal.proposal_type == "object")
    {
        let candidate_name = proposal
            .content
            .get("object_type")
            .and_then(Value::as_str)
            .unwrap_or(&proposal.name);
        let domain_scope = ontology_proposal_domain_scope(proposal);
        let candidate = ontology_entity_resolution_candidate(
            candidate_name,
            candidate_name,
            &domain_scope,
            semantic_objects,
            0.60,
        );
        let Some(best_hit) = candidate.retrieval_hits.first() else {
            continue;
        };
        if !candidate.decision.review_required {
            continue;
        }
        let object_node_id = ontology_graph_object_id(candidate_name);
        let merge_node_id = ontology_graph_merge_candidate_id(proposal.id, best_hit.object_id);
        ontology_review_graph_insert_node(
            nodes,
            OntologyReviewGraphNode {
                id: merge_node_id.clone(),
                node_type: "merge_candidate".to_string(),
                label: best_hit.title.clone(),
                status: candidate.decision.decision.clone(),
                confidence: candidate.decision.confidence,
                risk: if candidate.decision.is_duplicate {
                    "merge_review_required".to_string()
                } else {
                    "possible_match".to_string()
                },
                evidence: json!({
                    "candidate_name": candidate.candidate_name,
                    "canonical_name": candidate.decision.canonical_name,
                    "existing_node_uuid": candidate.decision.existing_node_uuid,
                    "best_hit": best_hit,
                    "decision": candidate.decision.clone(),
                    "authority": "proposal_only",
                }),
                source_proposal_id: Some(proposal.id),
            },
        );
        ontology_review_graph_insert_edge(
            edges,
            OntologyReviewGraphEdge {
                id: format!("{object_node_id}->{merge_node_id}:merge_suggests"),
                from: object_node_id,
                to: merge_node_id,
                edge_type: "merge_suggests".to_string(),
                status: "needs_review".to_string(),
                confidence: candidate.decision.confidence,
                risk: "merge_review_required".to_string(),
                evidence: json!({
                    "match_reasons": best_hit.match_reasons.clone(),
                    "review_required": true,
                    "materialization_policy": "merge suggestions never materialize without human review",
                }),
                source_proposal_id: Some(proposal.id),
            },
        );
    }
}

pub(crate) fn ontology_review_graph_insert_node(
    nodes: &mut BTreeMap<String, OntologyReviewGraphNode>,
    node: OntologyReviewGraphNode,
) {
    nodes.entry(node.id.clone()).or_insert(node);
}

pub(crate) fn ontology_review_graph_insert_edge(
    edges: &mut BTreeMap<String, OntologyReviewGraphEdge>,
    edge: OntologyReviewGraphEdge,
) {
    edges.entry(edge.id.clone()).or_insert(edge);
}

pub(crate) async fn create_ontology_onboarding_run_with_actor(
    state: &AppState,
    industry: &str,
    source_mode: &str,
    actor_subject: &str,
) -> Result<OntologyOnboardingRun, AppError> {
    let run_id = Uuid::new_v4();
    let (seed, source) = ontology_seed_and_source_for_request(industry, source_mode)?;
    let datasets = source.datasets.clone();
    let profiles = ontology_profile_demo_datasets(&datasets);
    let proposals = ontology_generate_seed_proposals_for_run(run_id, &seed, &datasets, &profiles);
    for proposal in &proposals {
        state
            .create_semantic_object(CreateSemanticObject {
                source_id: None,
                object_type: "ontology_onboarding_proposal".to_string(),
                object_key: ontology_onboarding_proposal_object_key(run_id, proposal.id),
                title: format!("Ontology onboarding proposal: {}", proposal.name),
                summary: format!(
                    "{} proposal for {} ontology fast onboarding; review required before materialization.",
                    proposal.proposal_type, seed.industry
                ),
                content: ontology_onboarding_proposal_content(run_id, proposal, false, None)?,
                semantic_scopes: json!({
                    "domain_scope": seed.domain_scope,
                    "workflow_scope": "enterprise-ontology-fast-onboarding",
                    "memory_scope": "ontology",
                    "share_policy": "review_required",
                }),
                source_uri: Some(format!(
                    "mandoforge://ontology/onboarding/runs/{run_id}/proposals/{}",
                    proposal.id
                )),
                provenance: json!({
                    "source": "ontology_onboarding.demo_run",
                    "industry": seed.industry,
                    "source_mode": source.source_mode,
                    "tool_namespace": source.tool_namespace,
                    "authority": "proposal_only",
                    "generated_at": Utc::now(),
                }),
                trust_level: "source_attested".to_string(),
                freshness: "current".to_string(),
                status: "active".to_string(),
            })
            .await?;
    }
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "ontology_onboarding.demo_run_created",
            "ontology_onboarding_run",
            Some(run_id),
            json!({
                "subject": actor_subject,
                "run_id": run_id,
                "dataset_count": datasets.len(),
                "profile_count": profiles.len(),
                "proposal_count": proposals.len(),
                "industry": seed.industry,
                "source_mode": source.source_mode,
            }),
        ))
        .await?;
    Ok(OntologyOnboardingRun {
        id: run_id,
        status: "pending_review".to_string(),
        source_mode: source.source_mode,
        dataset_count: datasets.len(),
        profile_count: profiles.len(),
        proposal_count: proposals.len(),
        approved_count: 0,
        materialized_count: 0,
        datasets,
        profiles,
        proposals,
        generated_at: Utc::now(),
    })
}

pub(crate) async fn create_ontology_onboarding_run_from_adapter(
    state: &AppState,
    adapted: ontology_source_adapters::OntologySourceAdapterOutput,
    actor_subject: &str,
) -> Result<OntologyOnboardingRun, AppError> {
    let run_id = Uuid::new_v4();
    let datasets = adapted.bundle.datasets.clone();
    let profiles = ontology_profile_demo_datasets(&datasets);
    let seed = adapted.seed.clone();
    let proposals = ontology_generate_seed_proposals_for_run(run_id, &seed, &datasets, &profiles);
    for proposal in &proposals {
        state
            .create_semantic_object(CreateSemanticObject {
                source_id: None,
                object_type: "ontology_onboarding_proposal".to_string(),
                object_key: ontology_onboarding_proposal_object_key(run_id, proposal.id),
                title: format!("Ontology onboarding proposal: {}", proposal.name),
                summary: format!(
                    "{} proposal from {} adapter; review required before materialization.",
                    proposal.proposal_type, adapted.adapter_type
                ),
                content: ontology_onboarding_proposal_content(run_id, proposal, false, None)?,
                semantic_scopes: json!({
                    "domain_scope": seed.domain_scope,
                    "workflow_scope": "enterprise-ontology-fast-onboarding",
                    "memory_scope": "ontology",
                    "share_policy": "review_required",
                }),
                source_uri: Some(format!(
                    "mandoforge://ontology/onboarding/runs/{run_id}/proposals/{}",
                    proposal.id
                )),
                provenance: json!({
                    "source": "ontology_onboarding.adapter_run",
                    "adapter_type": adapted.adapter_type,
                    "source_label": adapted.source_label,
                    "schema_only": adapted.schema_only,
                    "authority": "proposal_only",
                    "generated_at": Utc::now(),
                }),
                trust_level: "source_attested".to_string(),
                freshness: "current".to_string(),
                status: "active".to_string(),
            })
            .await?;
    }
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "ontology_onboarding.adapter_run_created",
            "ontology_onboarding_run",
            Some(run_id),
            json!({
                "subject": actor_subject,
                "run_id": run_id,
                "adapter_type": adapted.adapter_type,
                "source_label": adapted.source_label,
                "schema_only": adapted.schema_only,
                "dataset_count": datasets.len(),
                "proposal_count": proposals.len(),
                "warnings": adapted.warnings,
            }),
        ))
        .await?;
    Ok(OntologyOnboardingRun {
        id: run_id,
        status: "pending_review".to_string(),
        source_mode: adapted.bundle.source_mode.clone(),
        dataset_count: datasets.len(),
        profile_count: profiles.len(),
        proposal_count: proposals.len(),
        approved_count: 0,
        materialized_count: 0,
        datasets,
        profiles,
        proposals,
        generated_at: Utc::now(),
    })
}

pub(crate) fn ontology_available_seed_packs() -> Vec<OntologySeedPack> {
    vec![
        ontology_ecommerce_seed_pack(),
        ontology_insurance_seed_pack(),
    ]
}

pub(crate) fn ontology_seed_and_source_for_request(
    industry: &str,
    source_mode: &str,
) -> Result<(OntologySeedPack, OntologySourceBundle), AppError> {
    let industry = industry.trim().to_ascii_lowercase().replace('-', "_");
    let source_mode = source_mode.trim().to_ascii_lowercase().replace('-', "_");
    match (industry.as_str(), source_mode.as_str()) {
        ("ecommerce" | "commerce", "demo_ecommerce" | "demo") => Ok((
            ontology_ecommerce_seed_pack(),
            ontology_demo_source_bundle(),
        )),
        ("insurance", "demo_insurance" | "demo") => Ok((
            ontology_insurance_seed_pack(),
            ontology_insurance_demo_source_bundle(),
        )),
        _ => Ok(ontology_generic_seed_and_source(&industry, &source_mode)),
    }
}

pub(crate) fn ontology_generic_seed_and_source(
    industry: &str,
    source_mode: &str,
) -> (OntologySeedPack, OntologySourceBundle) {
    let domain_scope = format!("domain_{}", industry.replace(' ', "_"));
    let tool_namespace = format!("tools.{}", industry.replace(' ', "_"));
    let seed = OntologySeedPack {
        industry: industry.to_string(),
        domain_scope: domain_scope.clone(),
        source_mode: source_mode.to_string(),
        tool_namespace: tool_namespace.clone(),
        objects: vec![OntologySeedObjectMapping {
            table_name: "entities".to_string(),
            object_name: "Entity".to_string(),
        }],
        relations: vec![],
        metrics: vec![],
        actions: vec![],
    };
    let source = OntologySourceBundle {
        industry: industry.to_string(),
        source_mode: source_mode.to_string(),
        tool_namespace,
        datasets: vec![OntologyOnboardingDataset {
            table_name: "entities".to_string(),
            source_system: industry.to_string(),
            source_object: "entities".to_string(),
            fields: vec![],
            rows: vec![],
        }],
    };
    (seed, source)
}

pub(crate) async fn list_ontology_onboarding_runs_for_state(
    state: &AppState,
) -> Result<Vec<OntologyOnboardingRun>, AppError> {
    let mut grouped = BTreeMap::<Uuid, Vec<SemanticObject>>::new();
    for object in ontology_onboarding_proposal_objects(state).await? {
        if let Some(run_id) = ontology_onboarding_object_run_id(&object) {
            grouped.entry(run_id).or_default().push(object);
        }
    }
    let mut runs = grouped
        .into_iter()
        .map(|(run_id, objects)| ontology_onboarding_run_from_objects(run_id, &objects))
        .collect::<Result<Vec<_>, _>>()?;
    runs.sort_by_key(|run| std::cmp::Reverse(run.generated_at));
    Ok(runs)
}

pub(crate) async fn get_ontology_onboarding_run_for_state(
    state: &AppState,
    run_id: Uuid,
) -> Result<OntologyOnboardingRun, AppError> {
    let objects = ontology_onboarding_proposal_objects(state)
        .await?
        .into_iter()
        .filter(|object| ontology_onboarding_object_run_id(object) == Some(run_id))
        .collect::<Vec<_>>();
    if objects.is_empty() {
        return Err(AppError::not_found("ontology onboarding run not found"));
    }
    ontology_onboarding_run_from_objects(run_id, &objects)
}

pub(crate) async fn review_ontology_onboarding_proposal_with_actor(
    state: &AppState,
    proposal_id: Uuid,
    decision: &str,
    reason: Option<&str>,
    actor_subject: &str,
) -> Result<OntologyOnboardingProposalDraft, AppError> {
    let (decision, review_status) = normalize_ontology_onboarding_review_decision(decision)?;
    let object = ontology_onboarding_find_proposal_object(state, proposal_id).await?;
    let run_id = ontology_onboarding_object_run_id(&object)
        .ok_or_else(|| AppError::bad_request("ontology onboarding proposal missing run_id"))?;
    let mut proposal = ontology_onboarding_object_proposal(&object)?;
    proposal.review_status = review_status.clone();
    let review = json!({
        "decision": decision,
        "status": review_status,
        "reason": reason,
        "reviewer": actor_subject,
        "reviewed_at": Utc::now(),
    });
    state
        .update_semantic_object(
            object.id,
            UpdateSemanticObject {
                title: None,
                summary: None,
                content: Some(ontology_onboarding_proposal_content(
                    run_id,
                    &proposal,
                    ontology_onboarding_object_materialized(&object),
                    Some(review.clone()),
                )?),
                semantic_scopes: None,
                source_uri: None,
                provenance: None,
                trust_level: None,
                freshness: None,
                status: None,
            },
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "ontology_onboarding.proposal_reviewed",
            "ontology_onboarding_proposal",
            Some(proposal_id),
            json!({
                "subject": actor_subject,
                "run_id": run_id,
                "proposal_id": proposal_id,
                "proposal_type": proposal.proposal_type,
                "proposal_name": proposal.name,
                "review": review,
            }),
        ))
        .await?;
    ontology_record_confidence_calibration(
        state,
        run_id,
        &proposal,
        &decision,
        &review_status,
        actor_subject,
    )
    .await?;
    Ok(proposal)
}

pub(crate) async fn materialize_ontology_onboarding_run_with_actor(
    state: &AppState,
    run_id: Uuid,
    actor_subject: &str,
) -> Result<OntologyOnboardingMaterializationResult, AppError> {
    let objects = ontology_onboarding_proposal_objects(state)
        .await?
        .into_iter()
        .filter(|object| ontology_onboarding_object_run_id(object) == Some(run_id))
        .collect::<Vec<_>>();
    if objects.is_empty() {
        return Err(AppError::not_found("ontology onboarding run not found"));
    }
    let mut semantic_object_ids = Vec::new();
    let mut semantic_link_ids = Vec::new();
    let mut tool_spec_count = 0usize;
    let mut materialized_proposal_count = 0usize;
    for object in objects {
        if ontology_onboarding_object_materialized(&object) {
            continue;
        }
        let proposal = ontology_onboarding_object_proposal(&object)?;
        if proposal.review_status != "approved" {
            continue;
        }
        if proposal.proposal_type == "object"
            && ontology_object_proposal_has_unreviewed_merge_risk(state, &proposal).await?
        {
            continue;
        }
        match proposal.proposal_type.as_str() {
            "object" => {
                let semantic_object =
                    ontology_materialize_business_object(state, &proposal).await?;
                semantic_object_ids.push(semantic_object.id);
            }
            "metric" => {
                let semantic_object = ontology_materialize_metric(state, &proposal).await?;
                semantic_object_ids.push(semantic_object.id);
            }
            "action" => {
                let semantic_object = ontology_materialize_action(state, &proposal).await?;
                semantic_object_ids.push(semantic_object.id);
                tool_spec_count += 1;
            }
            "logic" | "logic_rule" => {
                let semantic_object = ontology_materialize_logic_rule(state, &proposal).await?;
                semantic_object_ids.push(semantic_object.id);
            }
            "relation" => {
                let link = ontology_materialize_relation(state, &proposal).await?;
                semantic_link_ids.push(link.id);
            }
            _ => {}
        }
        let review = object.content.get("review").cloned();
        state
            .update_semantic_object(
                object.id,
                UpdateSemanticObject {
                    title: None,
                    summary: None,
                    content: Some(ontology_onboarding_proposal_content(
                        run_id, &proposal, true, review,
                    )?),
                    semantic_scopes: None,
                    source_uri: None,
                    provenance: None,
                    trust_level: None,
                    freshness: None,
                    status: None,
                },
            )
            .await?;
        materialized_proposal_count += 1;
    }
    let result = OntologyOnboardingMaterializationResult {
        run_id,
        status: if materialized_proposal_count == 0 {
            "no_approved_changes".to_string()
        } else {
            "materialized".to_string()
        },
        semantic_object_count: semantic_object_ids.len(),
        semantic_link_count: semantic_link_ids.len(),
        tool_spec_count,
        semantic_object_ids,
        semantic_link_ids,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "ontology_onboarding.run_materialized",
            "ontology_onboarding_run",
            Some(run_id),
            json!({
                "subject": actor_subject,
                "run_id": run_id,
                "status": result.status,
                "semantic_object_count": result.semantic_object_count,
                "semantic_link_count": result.semantic_link_count,
                "tool_spec_count": result.tool_spec_count,
            }),
        ))
        .await?;
    Ok(result)
}

pub(crate) fn ontology_onboarding_proposal_object_key(run_id: Uuid, proposal_id: Uuid) -> String {
    format!("ontology:onboarding:{run_id}:{proposal_id}")
}

pub(crate) fn ontology_onboarding_proposal_content(
    run_id: Uuid,
    proposal: &OntologyOnboardingProposalDraft,
    materialized: bool,
    review: Option<Value>,
) -> Result<Value, AppError> {
    let mut content = serde_json::Map::new();
    content.insert("run_id".to_string(), json!(run_id));
    content.insert(
        "proposal".to_string(),
        serde_json::to_value(proposal).map_err(|error| {
            AppError::bad_request(format!("invalid ontology onboarding proposal: {error}"))
        })?,
    );
    content.insert("review_status".to_string(), json!(proposal.review_status));
    content.insert("materialized".to_string(), json!(materialized));
    if let Some(review) = review {
        content.insert("review".to_string(), review);
    }
    Ok(Value::Object(content))
}

pub(crate) async fn ontology_onboarding_proposal_objects(
    state: &AppState,
) -> Result<Vec<SemanticObject>, AppError> {
    Ok(state
        .list_semantic_objects()
        .await?
        .into_iter()
        .filter(|object| {
            object.object_type == "ontology_onboarding_proposal" && object.status == "active"
        })
        .collect())
}

pub(crate) async fn ontology_onboarding_find_proposal_object(
    state: &AppState,
    proposal_id: Uuid,
) -> Result<SemanticObject, AppError> {
    ontology_onboarding_proposal_objects(state)
        .await?
        .into_iter()
        .find(|object| {
            ontology_onboarding_object_proposal(object)
                .map(|proposal| proposal.id == proposal_id)
                .unwrap_or(false)
        })
        .ok_or_else(|| AppError::not_found("ontology onboarding proposal not found"))
}

pub(crate) fn ontology_onboarding_object_run_id(object: &SemanticObject) -> Option<Uuid> {
    object
        .content
        .get("run_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

pub(crate) fn ontology_onboarding_object_proposal(
    object: &SemanticObject,
) -> Result<OntologyOnboardingProposalDraft, AppError> {
    serde_json::from_value(
        object
            .content
            .get("proposal")
            .cloned()
            .ok_or_else(|| AppError::bad_request("ontology onboarding proposal missing content"))?,
    )
    .map_err(|error| {
        AppError::bad_request(format!("invalid ontology onboarding proposal: {error}"))
    })
}

pub(crate) fn ontology_onboarding_object_materialized(object: &SemanticObject) -> bool {
    object
        .content
        .get("materialized")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(crate) async fn create_ontology_release_candidate_with_actor(
    state: &AppState,
    run_id: Uuid,
    input: CreateOntologyReleaseCandidateRequest,
    actor_subject: &str,
) -> Result<OntologyRelease, AppError> {
    let run = get_ontology_onboarding_run_for_state(state, run_id).await?;
    let candidate_objects = ontology_onboarding_proposal_objects(state)
        .await?
        .into_iter()
        .filter(|object| ontology_onboarding_object_run_id(object) == Some(run_id))
        .filter(ontology_onboarding_object_materialized)
        .collect::<Vec<_>>();
    let mut materialized_objects = Vec::new();
    let mut proposals = Vec::new();
    for object in candidate_objects {
        let proposal = ontology_onboarding_object_proposal(&object)?;
        if proposal.review_status == "approved" {
            materialized_objects.push(object);
            proposals.push(proposal);
        }
    }
    if materialized_objects.is_empty() {
        return Err(AppError::bad_request(
            "ontology release candidate requires approved materialized proposals",
        ));
    }
    let domain_scope = proposals
        .first()
        .map(ontology_proposal_domain_scope)
        .unwrap_or_else(|| "commerce".to_string());
    let active_release = state
        .active_ontology_release_for_domain(&domain_scope)
        .await?;
    let materialized_object_ids =
        ontology_release_materialized_semantic_object_ids(state, &proposals).await?;
    let materialized_link_ids =
        ontology_release_materialized_semantic_link_ids(state, &proposals).await?;
    let now = Utc::now();
    let version = input.version.unwrap_or_else(|| {
        let entropy = Uuid::new_v4().simple().to_string();
        format!(
            "{}-v{}-{}",
            ontology_slug(&domain_scope),
            now.format("%Y%m%d%H%M%S"),
            &entropy[..8]
        )
    });
    let release = OntologyRelease {
        id: Uuid::new_v4(),
        version,
        domain_scope,
        source_run_id: Some(run.id),
        parent_release_id: active_release.as_ref().map(|release| release.id),
        rollback_target_release_id: active_release.as_ref().map(|release| release.id),
        status: "candidate".to_string(),
        release_class: ontology_release_class(input.release_class.as_deref())?,
        object_count: proposals
            .iter()
            .filter(|proposal| proposal.proposal_type == "object")
            .count() as i32,
        relation_count: proposals
            .iter()
            .filter(|proposal| proposal.proposal_type == "relation")
            .count() as i32,
        action_count: proposals
            .iter()
            .filter(|proposal| proposal.proposal_type == "action")
            .count() as i32,
        migration_policy: input
            .migration_policy
            .unwrap_or_else(default_ontology_release_migration_policy),
        gate_result: json!({}),
        materialized_object_ids: json!(materialized_object_ids),
        materialized_link_ids: json!(materialized_link_ids),
        evidence_refs: json!(
            proposals
                .iter()
                .map(|proposal| {
                    json!({
                        "proposal_id": proposal.id,
                        "proposal_type": proposal.proposal_type,
                        "review_status": proposal.review_status,
                    })
                })
                .collect::<Vec<_>>()
        ),
        promoted_by: None,
        promoted_at: None,
        rolled_back_by: None,
        rolled_back_at: None,
        archived_at: None,
        created_at: now,
        updated_at: now,
    };
    let release = state.create_ontology_release(release).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "ontology_release.candidate_created",
            "ontology_release",
            Some(release.id),
            json!({
                "subject": actor_subject,
                "release_id": release.id,
                "version": release.version,
                "domain_scope": release.domain_scope,
                "source_run_id": release.source_run_id,
                "release_class": release.release_class,
                "object_count": release.object_count,
                "relation_count": release.relation_count,
                "action_count": release.action_count,
            }),
        ))
        .await?;
    Ok(release)
}

pub(crate) fn default_ontology_release_migration_policy() -> Value {
    json!({
        "compatibility": "backward_compatible",
        "rollback": "previous_active_release",
        "requires_operator_review": true,
    })
}

pub(crate) fn ontology_release_class(value: Option<&str>) -> Result<String, AppError> {
    match value.unwrap_or("repo_controlled") {
        "repo_controlled" | "production_like_pilot" | "customer_grade" => {
            Ok(value.unwrap_or("repo_controlled").to_string())
        }
        other => Err(AppError::bad_request(format!(
            "unsupported ontology release_class: {other}"
        ))),
    }
}

pub(crate) async fn ontology_release_materialized_semantic_object_ids(
    state: &AppState,
    proposals: &[OntologyOnboardingProposalDraft],
) -> Result<Vec<Uuid>, AppError> {
    let proposal_ids = proposals
        .iter()
        .map(|proposal| proposal.id)
        .collect::<BTreeSet<_>>();
    let object_keys = proposals
        .iter()
        .filter_map(ontology_release_materialized_semantic_object_key)
        .collect::<BTreeSet<_>>();
    Ok(state
        .list_semantic_objects()
        .await?
        .into_iter()
        .filter(|object| {
            let proposal_matches = object
                .content
                .get("proposal_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                .or_else(|| {
                    object
                        .provenance
                        .get("proposal_id")
                        .and_then(Value::as_str)
                        .and_then(|value| Uuid::parse_str(value).ok())
                })
                .is_some_and(|proposal_id| proposal_ids.contains(&proposal_id));
            proposal_matches || object_keys.contains(&object.object_key)
        })
        .map(|object| object.id)
        .collect())
}

pub(crate) fn ontology_release_materialized_semantic_object_key(
    proposal: &OntologyOnboardingProposalDraft,
) -> Option<String> {
    let domain_scope = ontology_proposal_domain_scope(proposal);
    match proposal.proposal_type.as_str() {
        "object" => {
            let object_name = proposal
                .content
                .get("object_type")
                .and_then(Value::as_str)
                .unwrap_or(proposal.name.as_str());
            Some(ontology_business_object_key(&domain_scope, object_name))
        }
        "metric" => Some(format!(
            "{}.metric.{}",
            ontology_slug(&domain_scope),
            ontology_slug(&proposal.name)
        )),
        "action" => Some(format!(
            "{}.action.{}",
            ontology_slug(&domain_scope),
            ontology_slug(&proposal.name)
        )),
        "logic" | "logic_rule" => Some(format!(
            "{}.logic.{}",
            ontology_slug(&domain_scope),
            ontology_slug(&proposal.name)
        )),
        _ => None,
    }
}

pub(crate) async fn ontology_release_materialized_semantic_link_ids(
    state: &AppState,
    proposals: &[OntologyOnboardingProposalDraft],
) -> Result<Vec<Uuid>, AppError> {
    let proposal_ids = proposals
        .iter()
        .map(|proposal| proposal.id)
        .collect::<BTreeSet<_>>();
    let semantic_objects = state.list_semantic_objects().await?;
    let object_ids_by_key = semantic_objects
        .into_iter()
        .filter(|object| object.archived_at.is_none())
        .map(|object| (object.object_key, object.id))
        .collect::<BTreeMap<_, _>>();
    let relation_signatures = proposals
        .iter()
        .filter_map(|proposal| {
            ontology_release_relation_signature_for_proposal(proposal, &object_ids_by_key)
        })
        .collect::<BTreeSet<_>>();
    Ok(state
        .list_semantic_links()
        .await?
        .into_iter()
        .filter(|link| {
            let proposal_matches = link
                .metadata
                .get("proposal_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                .or_else(|| {
                    link.provenance
                        .get("proposal_id")
                        .and_then(Value::as_str)
                        .and_then(|value| Uuid::parse_str(value).ok())
                })
                .is_some_and(|proposal_id| proposal_ids.contains(&proposal_id));
            let signature_matches = relation_signatures.contains(&(
                link.from_entity_id.clone(),
                link.relation_type.clone(),
                link.to_entity_id.clone(),
            ));
            proposal_matches || signature_matches
        })
        .map(|link| link.id)
        .collect())
}

pub(crate) fn ontology_release_relation_signature_for_proposal(
    proposal: &OntologyOnboardingProposalDraft,
    object_ids_by_key: &BTreeMap<String, Uuid>,
) -> Option<(String, String, String)> {
    if proposal.proposal_type != "relation" {
        return None;
    }
    let domain_scope = ontology_proposal_domain_scope(proposal);
    let from_object = proposal
        .content
        .get("from_object")
        .and_then(Value::as_str)?;
    let to_object = proposal.content.get("to_object").and_then(Value::as_str)?;
    let relation = proposal.content.get("relation").and_then(Value::as_str)?;
    let from_id = object_ids_by_key
        .get(&ontology_business_object_key(&domain_scope, from_object))?
        .to_string();
    let to_id = object_ids_by_key
        .get(&ontology_business_object_key(&domain_scope, to_object))?
        .to_string();
    Some((from_id, relation.to_string(), to_id))
}

pub(crate) async fn gate_ontology_release_with_actor(
    state: &AppState,
    release_id: Uuid,
    actor_subject: &str,
) -> Result<OntologyRelease, AppError> {
    let mut release = state.get_ontology_release(release_id).await?;
    if !matches!(release.status.as_str(), "candidate" | "failed_gate") {
        return Err(AppError::bad_request(
            "only candidate or failed_gate ontology releases can be gated",
        ));
    }
    let original_status = release.status.clone();
    let active_release = state
        .active_ontology_release_for_domain(&release.domain_scope)
        .await?;
    let mut checks = Vec::new();
    let mut blockers = Vec::new();
    let migration_ok = ontology_release_migration_policy_ready(&release.migration_policy);
    checks.push(json!({
        "id": "migration_policy",
        "status": if migration_ok { "passed" } else { "failed" },
    }));
    if !migration_ok {
        blockers.push("migration policy must declare compatibility and rollback".to_string());
    }
    let materialized_ok = release
        .materialized_object_ids
        .as_array()
        .is_some_and(|ids| !ids.is_empty())
        || release
            .materialized_link_ids
            .as_array()
            .is_some_and(|ids| !ids.is_empty());
    checks.push(json!({
        "id": "materialized_semantics",
        "status": if materialized_ok { "passed" } else { "failed" },
    }));
    if !materialized_ok {
        blockers.push(
            "release candidate must include materialized semantic object or link ids".to_string(),
        );
    }
    let action_profiles_ok = ontology_release_action_profiles_ready(state, &release).await?;
    checks.push(json!({
        "id": "action_transaction_profiles",
        "status": if action_profiles_ok { "passed" } else { "failed" },
    }));
    if !action_profiles_ok {
        blockers.push(
            "write-like ontology actions must carry transaction profile evidence".to_string(),
        );
    }
    if active_release.is_some() && release.rollback_target_release_id.is_none() {
        checks.push(json!({
            "id": "rollback_target",
            "status": "failed",
        }));
        blockers.push(
            "release candidate must declare rollback target when the domain already has an active release"
                .to_string(),
        );
    } else {
        checks.push(json!({
            "id": "rollback_target",
            "status": "passed",
        }));
    }
    let passed = blockers.is_empty();
    release.status = if passed {
        "candidate".to_string()
    } else {
        "failed_gate".to_string()
    };
    release.gate_result = json!({
        "status": if passed { "passed" } else { "failed" },
        "checked_at": Utc::now(),
        "checked_by": actor_subject,
        "checks": checks,
        "blockers": blockers,
    });
    release.updated_at = Utc::now();
    let release = state
        .update_ontology_release(release, Some(original_status.as_str()))
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "ontology_release.gated",
            "ontology_release",
            Some(release.id),
            json!({
                "subject": actor_subject,
                "release_id": release.id,
                "version": release.version,
                "domain_scope": release.domain_scope,
                "gate_status": release.gate_result["status"],
                "blockers": release.gate_result["blockers"],
            }),
        ))
        .await?;
    Ok(release)
}

pub(crate) async fn promote_ontology_release_with_actor(
    state: &AppState,
    release_id: Uuid,
    actor_subject: &str,
) -> Result<OntologyRelease, AppError> {
    let (release, previous_active) = state
        .promote_ontology_release_atomically(release_id, actor_subject)
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "ontology_release.promoted",
            "ontology_release",
            Some(release.id),
            json!({
                "subject": actor_subject,
                "release_id": release.id,
                "version": release.version,
                "domain_scope": release.domain_scope,
                "previous_active_release_id": previous_active.map(|active| active.id),
            }),
        ))
        .await?;
    let release = if let Err(error) =
        trigger_workflow_run_from_ontology_release(state, &release, actor_subject).await
    {
        let error_message = error.message.clone();
        let release = update_ontology_release_workflow_trigger_status(
            state,
            &release,
            ONTOLOGY_RELEASE_STATUS_ACTIVE_TRIGGER_FAILED,
            "failed",
            actor_subject,
            Some(error_message.clone()),
        )
        .await?;
        state
            .append_audit_log(new_audit_log(
                None,
                "system",
                None,
                "ontology_release.workflow_trigger_failed",
                "ontology_release",
                Some(release.id),
                json!({
                    "subject": actor_subject,
                    "release_id": release.id,
                    "version": release.version,
                    "domain_scope": release.domain_scope,
                    "status": release.status,
                    "error": error_message,
                }),
            ))
            .await?;
        release
    } else {
        release
    };
    Ok(release)
}

pub(crate) async fn trigger_workflow_run_from_ontology_release(
    state: &AppState,
    release: &OntologyRelease,
    actor_subject: &str,
) -> Result<Option<WorkflowRun>, AppError> {
    let definitions = ontology_release_workflow_definitions(state, release).await?;
    if definitions.is_empty() {
        state
            .append_audit_log(new_audit_log(
                None,
                "system",
                None,
                "ontology_release.workflow_trigger_skipped",
                "ontology_release",
                Some(release.id),
                json!({
                    "subject": actor_subject,
                    "release_id": release.id,
                    "version": release.version,
                    "domain_scope": release.domain_scope,
                    "reason": "no_matching_workflow_definition",
                }),
            ))
            .await?;
        return Ok(None);
    }

    let mut first_run = None;
    let mut first_error = None;
    for definition in definitions {
        match trigger_workflow_run_from_ontology_release_definition(
            state,
            release,
            actor_subject,
            &definition,
        )
        .await
        {
            Ok(Some(run)) => {
                if first_run.is_none() {
                    first_run = Some(run);
                }
            }
            Ok(None) => {}
            Err(error) => {
                state
                    .append_audit_log(new_audit_log(
                        None,
                        "system",
                        None,
                        "ontology_release.workflow_definition_trigger_failed",
                        "workflow_definition",
                        Some(definition.id),
                        json!({
                            "subject": actor_subject,
                            "release_id": release.id,
                            "ontology_release_id": release.id,
                            "version": release.version,
                            "domain_scope": release.domain_scope,
                            "workflow_definition_id": definition.id,
                            "error": error.message.clone(),
                        }),
                    ))
                    .await?;
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    if let Some(error) = first_error {
        Err(error)
    } else if first_run.is_some() {
        Ok(first_run)
    } else {
        Ok(None)
    }
}

async fn update_ontology_release_workflow_trigger_status(
    state: &AppState,
    release: &OntologyRelease,
    release_status: &str,
    trigger_status: &str,
    actor_subject: &str,
    error_message: Option<String>,
) -> Result<OntologyRelease, AppError> {
    let mut current = state.get_ontology_release(release.id).await?;
    if !ontology_release_current_status(&current.status) {
        return Ok(current);
    }
    let expected_status = current.status.clone();
    current.status = release_status.to_string();
    current.gate_result = ontology_release_gate_result_with_workflow_trigger_status(
        current.gate_result,
        trigger_status,
        actor_subject,
        error_message,
    );
    current.updated_at = Utc::now();
    state
        .update_ontology_release(current, Some(expected_status.as_str()))
        .await
}

fn ontology_release_gate_result_with_workflow_trigger_status(
    gate_result: Value,
    trigger_status: &str,
    actor_subject: &str,
    error_message: Option<String>,
) -> Value {
    let mut gate_result = gate_result.as_object().cloned().unwrap_or_default();
    gate_result.insert(
        "workflow_trigger".to_string(),
        json!({
            "status": trigger_status,
            "checked_at": Utc::now(),
            "checked_by": actor_subject,
            "error": error_message,
        }),
    );
    Value::Object(gate_result)
}

async fn trigger_workflow_run_from_ontology_release_definition(
    state: &AppState,
    release: &OntologyRelease,
    actor_subject: &str,
    definition: &WorkflowDefinition,
) -> Result<Option<WorkflowRun>, AppError> {
    let Some(trigger) = state
        .claim_ontology_release_workflow_trigger(release.id, definition.id)
        .await?
    else {
        return Ok(None);
    };
    if let Some(existing_run) =
        ontology_release_workflow_run_for_definition(state, release.id, definition.id).await?
    {
        state
            .complete_ontology_release_workflow_trigger(
                trigger.id,
                "triggered",
                Some(existing_run.id),
                None,
            )
            .await?;
        return Ok(Some(existing_run));
    }
    let tool_specs = match release.source_run_id {
        Some(run_id) => match ontology_onboarding_tool_specs_for_run(state, run_id).await {
            Ok(tool_specs) => tool_specs,
            Err(error) => {
                state
                    .complete_ontology_release_workflow_trigger(
                        trigger.id,
                        "failed",
                        None,
                        Some(error.message.clone()),
                    )
                    .await?;
                return Err(error);
            }
        },
        None => Vec::new(),
    };
    let input_payload = json!({
        "trigger": "ontology_release.promoted",
        "ontology_release_id": release.id,
        "ontology_version": release.version,
        "domain_scope": release.domain_scope,
        "release_class": release.release_class,
        "status": release.status,
        "ontology_release": {
            "id": release.id,
            "version": release.version,
            "domain_scope": release.domain_scope,
            "release_class": release.release_class,
            "status": release.status,
        },
        "action_catalog": {
            "source": "active_ontology_release",
            "tool_count": tool_specs.len(),
            "tool_specs": tool_specs,
        }
    });
    let run = match create_workflow_run_from_definition(
        state,
        definition,
        format!(
            "Ontology promoted: {} {}",
            release.domain_scope, release.version
        ),
        input_payload,
        json!({
            "trigger": "ontology_release.promoted",
            "ontology_release_id": release.id,
            "ontology_version": release.version,
            "domain_scope": release.domain_scope,
        }),
    )
    .await
    {
        Ok(run) => run,
        Err(error) => {
            state
                .complete_ontology_release_workflow_trigger(
                    trigger.id,
                    "failed",
                    None,
                    Some(error.message.clone()),
                )
                .await?;
            return Err(error);
        }
    };
    state
        .complete_ontology_release_workflow_trigger(trigger.id, "triggered", Some(run.id), None)
        .await?;
    state
        .append_event(
            "system",
            Some(run.id),
            run.primary_session_id,
            "ontology_release.workflow_run_triggered",
            json!({
                "workflow_run_id": run.id,
                "workflow_definition_id": run.workflow_definition_id,
                "ontology_release_id": release.id,
                "ontology_version": release.version,
                "domain_scope": release.domain_scope,
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(run.primary_session_id),
            "system",
            Some(run.id),
            "ontology_release.workflow_run_triggered",
            "ontology_release",
            Some(release.id),
            json!({
                "subject": actor_subject,
                "release_id": release.id,
                "version": release.version,
                "domain_scope": release.domain_scope,
                "workflow_definition_id": run.workflow_definition_id,
                "workflow_run_id": run.id,
                "primary_session_id": run.primary_session_id,
            }),
        ))
        .await?;
    Ok(Some(run))
}

pub(crate) async fn drain_due_ontology_release_workflow_triggers(
    state: &AppState,
    actor_subject: &str,
    limit: usize,
) -> Result<OntologyReleaseWorkflowTriggerDrain, AppError> {
    let checked_at = Utc::now();
    let triggers = state
        .retryable_ontology_release_workflow_triggers(limit)
        .await?;
    let retryable_count = triggers.len();
    let mut triggered_count = 0usize;
    let mut skipped_count = 0usize;
    let mut failed_count = 0usize;
    let mut trigger_ids = Vec::new();
    for trigger in triggers {
        trigger_ids.push(trigger.id);
        let release = match state
            .get_ontology_release(trigger.ontology_release_id)
            .await
        {
            Ok(release) => release,
            Err(error) => {
                failed_count += 1;
                state
                    .complete_ontology_release_workflow_trigger(
                        trigger.id,
                        "failed",
                        None,
                        Some(error.message),
                    )
                    .await?;
                continue;
            }
        };
        if !ontology_release_current_status(&release.status) {
            skipped_count += 1;
            state
                .complete_ontology_release_workflow_trigger(
                    trigger.id,
                    ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_SKIPPED,
                    None,
                    Some(format!(
                        "ontology release status is {}; workflow trigger skipped",
                        release.status
                    )),
                )
                .await?;
            continue;
        }
        match trigger_workflow_run_from_ontology_release(state, &release, actor_subject).await {
            Ok(Some(_)) => {
                triggered_count += 1;
                if release.status == ONTOLOGY_RELEASE_STATUS_ACTIVE_TRIGGER_FAILED {
                    update_ontology_release_workflow_trigger_status(
                        state,
                        &release,
                        ONTOLOGY_RELEASE_STATUS_ACTIVE,
                        "triggered",
                        actor_subject,
                        None,
                    )
                    .await?;
                }
            }
            Ok(None) => {
                skipped_count += 1;
                state
                    .complete_ontology_release_workflow_trigger(
                        trigger.id,
                        ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_SKIPPED,
                        None,
                        Some(
                            "workflow definition no longer matches ontology release trigger"
                                .to_string(),
                        ),
                    )
                    .await?;
            }
            Err(error) => {
                failed_count += 1;
                update_ontology_release_workflow_trigger_status(
                    state,
                    &release,
                    ONTOLOGY_RELEASE_STATUS_ACTIVE_TRIGGER_FAILED,
                    "failed",
                    actor_subject,
                    Some(error.message.clone()),
                )
                .await?;
                state
                    .complete_ontology_release_workflow_trigger(
                        trigger.id,
                        "failed",
                        None,
                        Some(error.message.clone()),
                    )
                    .await?;
                state
                    .append_audit_log(new_audit_log(
                        None,
                        "system",
                        None,
                        "ontology_release.workflow_trigger_retry_failed",
                        "ontology_release_workflow_trigger",
                        Some(trigger.id),
                        json!({
                            "subject": actor_subject,
                            "trigger_id": trigger.id,
                            "release_id": release.id,
                            "version": release.version,
                            "domain_scope": release.domain_scope,
                            "error": error.message,
                        }),
                    ))
                    .await?;
            }
        }
    }
    let status = if failed_count > 0 {
        "failed"
    } else if triggered_count > 0 {
        "triggered"
    } else if skipped_count > 0 {
        "skipped"
    } else {
        "noop"
    }
    .to_string();
    Ok(OntologyReleaseWorkflowTriggerDrain {
        status,
        checked_at,
        retryable_count,
        triggered_count,
        skipped_count,
        failed_count,
        trigger_ids,
    })
}

async fn ontology_release_workflow_run_for_definition(
    state: &AppState,
    release_id: Uuid,
    workflow_definition_id: Uuid,
) -> Result<Option<WorkflowRun>, AppError> {
    state
        .ontology_release_workflow_run_for_trigger(release_id, workflow_definition_id)
        .await
}

pub(crate) async fn ontology_release_workflow_definitions(
    state: &AppState,
    release: &OntologyRelease,
) -> Result<Vec<WorkflowDefinition>, AppError> {
    Ok(state
        .list_workflow_definitions()
        .await?
        .into_iter()
        .filter(|definition| {
            definition.release_state == "released"
                && ontology_release_trigger_matches_definition(definition, release)
        })
        .collect())
}

pub(crate) fn ontology_release_trigger_matches_definition(
    definition: &WorkflowDefinition,
    release: &OntologyRelease,
) -> bool {
    ontology_release_trigger_config(definition)
        .is_some_and(|trigger| ontology_release_trigger_matches_release(trigger, release))
}

pub(crate) fn ontology_release_trigger_config(definition: &WorkflowDefinition) -> Option<&Value> {
    [&definition.handoff_rules, &definition.step_graph]
        .into_iter()
        .find_map(|source| {
            source
                .get("ontology_release_trigger")
                .or_else(|| source.get("ontology_trigger"))
                .filter(|trigger| ontology_release_trigger_enabled(trigger))
        })
}

pub(crate) fn ontology_release_trigger_enabled(trigger: &Value) -> bool {
    match trigger {
        Value::Bool(value) => *value,
        Value::Object(object) => object
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        _ => false,
    }
}

pub(crate) fn ontology_release_trigger_matches_release(
    trigger: &Value,
    release: &OntologyRelease,
) -> bool {
    if trigger == &Value::Bool(true) {
        return true;
    }
    let Some(trigger) = trigger.as_object() else {
        return false;
    };
    let event_matches = trigger
        .get("event")
        .and_then(Value::as_str)
        .map(|event| {
            matches!(
                event.trim(),
                "ontology_release.promoted" | "ontology.promoted" | "promoted"
            )
        })
        .unwrap_or(true);
    event_matches
        && json_domain_scope_matches(trigger.get("domain_scope"), release.domain_scope.as_str())
}

pub(crate) fn json_domain_scope_matches(value: Option<&Value>, domain_scope: &str) -> bool {
    match value {
        None => true,
        Some(Value::String(scope)) => {
            let scope = scope.trim();
            scope.is_empty() || scope == "*" || scope.eq_ignore_ascii_case(domain_scope)
        }
        Some(Value::Array(scopes)) => scopes.iter().filter_map(Value::as_str).any(|scope| {
            let scope = scope.trim();
            scope == "*" || scope.eq_ignore_ascii_case(domain_scope)
        }),
        _ => false,
    }
}

pub(crate) async fn rollback_ontology_release_with_actor(
    state: &AppState,
    release_id: Uuid,
    actor_subject: &str,
) -> Result<OntologyRelease, AppError> {
    let (release, target) = state
        .rollback_ontology_release_atomically(release_id, actor_subject)
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "ontology_release.rolled_back",
            "ontology_release",
            Some(release.id),
            json!({
                "subject": actor_subject,
                "release_id": release.id,
                "version": release.version,
                "domain_scope": release.domain_scope,
                "rollback_target_release_id": target.id,
            }),
        ))
        .await?;
    Ok(target)
}

pub(crate) async fn archive_ontology_release_with_actor(
    state: &AppState,
    release_id: Uuid,
    actor_subject: &str,
) -> Result<OntologyRelease, AppError> {
    let mut release = state.get_ontology_release(release_id).await?;
    if ontology_release_current_status(&release.status) {
        return Err(AppError::bad_request(
            "active ontology releases cannot be archived",
        ));
    }
    let original_status = release.status.clone();
    if state
        .list_ontology_releases()
        .await?
        .into_iter()
        .any(|candidate| {
            ontology_release_current_status(&candidate.status)
                && candidate.rollback_target_release_id == Some(release.id)
                && candidate
                    .domain_scope
                    .eq_ignore_ascii_case(&release.domain_scope)
        })
    {
        return Err(AppError::bad_request(
            "ontology release is rollback target for the active release",
        ));
    }
    release.status = "archived".to_string();
    release.archived_at = Some(Utc::now());
    release.updated_at = Utc::now();
    let release = state
        .update_ontology_release(release, Some(original_status.as_str()))
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "ontology_release.archived",
            "ontology_release",
            Some(release.id),
            json!({
                "subject": actor_subject,
                "release_id": release.id,
                "version": release.version,
                "domain_scope": release.domain_scope,
            }),
        ))
        .await?;
    Ok(release)
}

pub(crate) fn ontology_release_migration_policy_ready(policy: &Value) -> bool {
    policy
        .get("compatibility")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
        && policy
            .get("rollback")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
}

pub(crate) async fn ontology_release_action_profiles_ready(
    state: &AppState,
    release: &OntologyRelease,
) -> Result<bool, AppError> {
    let ids = release
        .materialized_object_ids
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|value| Uuid::parse_str(value).ok())
        .collect::<Vec<_>>();
    for id in ids {
        let object = state.get_semantic_object(id).await?;
        if object.object_type == "ontology_action_type"
            && object
                .content
                .get("transaction_profile")
                .and_then(Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn ontology_onboarding_run_from_objects(
    run_id: Uuid,
    objects: &[SemanticObject],
) -> Result<OntologyOnboardingRun, AppError> {
    let mut proposals = objects
        .iter()
        .map(ontology_onboarding_object_proposal)
        .collect::<Result<Vec<_>, _>>()?;
    let industry = proposals
        .iter()
        .find_map(|proposal| {
            proposal
                .content
                .get("industry")
                .or_else(|| proposal.evidence.get("industry"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "ecommerce".to_string());
    let source_mode = proposals
        .iter()
        .find_map(|proposal| {
            proposal
                .content
                .get("source_mode")
                .or_else(|| proposal.evidence.get("source_mode"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "demo_ecommerce".to_string());
    let (_, source) = ontology_seed_and_source_for_request(&industry, &source_mode)?;
    let datasets = source.datasets;
    let profiles = ontology_profile_demo_datasets(&datasets);
    proposals.sort_by(|left, right| {
        left.proposal_type
            .cmp(&right.proposal_type)
            .then_with(|| left.name.cmp(&right.name))
    });
    let approved_count = proposals
        .iter()
        .filter(|proposal| proposal.review_status == "approved")
        .count();
    let materialized_count = objects
        .iter()
        .filter(|object| ontology_onboarding_object_materialized(object))
        .count();
    let generated_at = objects
        .iter()
        .map(|object| object.created_at)
        .min()
        .unwrap_or_else(Utc::now);
    let status = if materialized_count > 0 {
        "materialized"
    } else if approved_count > 0 {
        "reviewing"
    } else {
        "pending_review"
    };
    Ok(OntologyOnboardingRun {
        id: run_id,
        status: status.to_string(),
        source_mode: source.source_mode,
        dataset_count: datasets.len(),
        profile_count: profiles.len(),
        proposal_count: proposals.len(),
        approved_count,
        materialized_count,
        datasets,
        profiles,
        proposals,
        generated_at,
    })
}

pub(crate) async fn ontology_record_confidence_calibration(
    state: &AppState,
    run_id: Uuid,
    proposal: &OntologyOnboardingProposalDraft,
    reviewer_decision: &str,
    reviewer_status: &str,
    reviewer: &str,
) -> Result<(), AppError> {
    let deterministic_validator_score = ontology_calibration_validator_score(proposal);
    let source_quality_score = ontology_calibration_source_quality_score(proposal);
    let retrieval_similarity_score =
        ontology_calibration_retrieval_similarity_score(state, proposal).await?;
    let record_id = Uuid::new_v4();
    let recorded_at = Utc::now();
    let record = ConfidenceCalibrationRecord {
        id: record_id,
        run_id,
        proposal_id: proposal.id,
        proposal_type: proposal.proposal_type.clone(),
        proposal_name: proposal.name.clone(),
        model_confidence: proposal.confidence,
        deterministic_validator_score,
        retrieval_similarity_score,
        source_quality_score,
        reviewer_decision: reviewer_decision.to_string(),
        reviewer_status: reviewer_status.to_string(),
        runtime_outcome: None,
        evidence: json!({
            "engine": "deterministic_confidence_calibration_v1",
            "reviewer": reviewer,
            "threshold_policy_ref": "advisory_default_v1",
            "thresholds_are_customer_tunable": true,
            "proposal_evidence": proposal.evidence,
        }),
        recorded_at,
    };
    state
        .create_semantic_object(CreateSemanticObject {
            source_id: None,
            object_type: "ontology_confidence_calibration".to_string(),
            object_key: ontology_confidence_calibration_object_key(run_id, proposal.id, record_id),
            title: format!("Confidence calibration: {}", proposal.name),
            summary: format!(
                "{} proposal calibration outcome: {}.",
                proposal.proposal_type, reviewer_status
            ),
            content: serde_json::to_value(&record).map_err(|error| {
                AppError::bad_request(format!("invalid confidence calibration record: {error}"))
            })?,
            semantic_scopes: json!({
                "domain_scope": ontology_proposal_domain_scope(proposal),
                "workflow_scope": "enterprise-ontology-fast-onboarding",
                "memory_scope": "ontology",
                "share_policy": "review_required",
            }),
            source_uri: Some(format!(
                "mandoforge://ontology/intelligence/runs/{run_id}/calibration/{record_id}"
            )),
            provenance: json!({
                "source": "ontology_confidence_calibration.review",
                "run_id": run_id,
                "proposal_id": proposal.id,
                "recorded_at": recorded_at,
            }),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "active".to_string(),
        })
        .await?;
    Ok(())
}

pub(crate) async fn ontology_confidence_calibration_for_run(
    state: &AppState,
    run_id: Uuid,
) -> Result<ConfidenceCalibrationResponse, AppError> {
    let mut records = state
        .list_semantic_objects()
        .await?
        .into_iter()
        .filter(|object| {
            object.object_type == "ontology_confidence_calibration"
                && object.status == "active"
                && object
                    .content
                    .get("run_id")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok())
                    == Some(run_id)
        })
        .map(|object| {
            serde_json::from_value::<ConfidenceCalibrationRecord>(object.content).map_err(|error| {
                AppError::bad_request(format!("invalid confidence calibration record: {error}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    records.sort_by_key(|left| left.recorded_at);
    let buckets = ontology_confidence_calibration_buckets(&records);
    Ok(ConfidenceCalibrationResponse {
        run_id,
        record_count: records.len(),
        records,
        buckets,
        threshold_policy: ontology_confidence_threshold_policy(),
    })
}

pub(crate) fn ontology_confidence_calibration_buckets(
    records: &[ConfidenceCalibrationRecord],
) -> Vec<ConfidenceCalibrationBucket> {
    let mut grouped = BTreeMap::<(String, String), Vec<&ConfidenceCalibrationRecord>>::new();
    for record in records {
        grouped
            .entry((record.proposal_type.clone(), record.reviewer_status.clone()))
            .or_default()
            .push(record);
    }
    grouped
        .into_iter()
        .map(|((proposal_type, reviewer_status), records)| {
            let count = records.len();
            ConfidenceCalibrationBucket {
                proposal_type,
                reviewer_status,
                count,
                average_model_confidence: ontology_average(
                    records.iter().map(|record| record.model_confidence),
                    count,
                ),
                average_validator_score: ontology_average(
                    records
                        .iter()
                        .map(|record| record.deterministic_validator_score),
                    count,
                ),
                average_source_quality_score: ontology_average(
                    records.iter().map(|record| record.source_quality_score),
                    count,
                ),
            }
        })
        .collect()
}

pub(crate) fn ontology_average(values: impl Iterator<Item = f64>, count: usize) -> f64 {
    if count == 0 {
        0.0
    } else {
        values.sum::<f64>() / count as f64
    }
}

pub(crate) fn ontology_confidence_threshold_policy() -> Value {
    json!({
        "policy_id": "advisory_default_v1",
        "customer_tunable": true,
        "not_a_global_production_benchmark": true,
        "configuration_surface": {
            "scope": "customer_or_domain_policy",
            "override_field": "confidence_threshold_policy",
            "stored_as": "review_policy_metadata"
        },
        "bands": [
            {"min": 0.90, "max": 1.00, "action": "draft_ready"},
            {"min": 0.70, "max": 0.89, "action": "quick_review"},
            {"min": 0.50, "max": 0.69, "action": "detailed_review"},
            {"min": 0.00, "max": 0.49, "action": "retry_or_discard"}
        ]
    })
}

pub(crate) fn ontology_calibration_validator_score(
    proposal: &OntologyOnboardingProposalDraft,
) -> f64 {
    match proposal.proposal_type.as_str() {
        "object" => {
            let has_seed = proposal.evidence.get("seed_ontology_match").is_some();
            let has_pk = proposal
                .evidence
                .get("primary_key_candidates")
                .and_then(Value::as_array)
                .map(|values| !values.is_empty())
                .unwrap_or(false);
            let mut score: f64 = 0.50;
            if has_seed {
                score += 0.25;
            }
            if has_pk {
                score += 0.20;
            }
            score.clamp(0.0, 1.0)
        }
        "relation" => proposal
            .evidence
            .get("join_success_rate")
            .and_then(Value::as_f64)
            .unwrap_or(0.50)
            .clamp(0.0, 1.0),
        "metric" => 0.75,
        "logic" | "logic_rule" => {
            if proposal
                .evidence
                .get("primary_key_candidates")
                .and_then(Value::as_array)
                .map(|values| !values.is_empty())
                .unwrap_or(false)
            {
                0.88
            } else {
                0.60
            }
        }
        "action" => {
            let has_policy = proposal.content.get("policy").is_some();
            let has_effects = proposal.content.get("effects").is_some();
            let has_audit = proposal.content.get("audit_event").is_some();
            0.50 + (has_policy as u8 as f64 * 0.15)
                + (has_effects as u8 as f64 * 0.15)
                + (has_audit as u8 as f64 * 0.15)
        }
        _ => 0.50,
    }
    .clamp(0.0, 1.0)
}

pub(crate) fn ontology_calibration_source_quality_score(
    proposal: &OntologyOnboardingProposalDraft,
) -> f64 {
    let row_count = proposal
        .evidence
        .get("row_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let mut score: f64 = if row_count > 0 { 0.55 } else { 0.35 };
    if proposal.evidence.get("source_mode").is_some() {
        score += 0.10;
    }
    if proposal
        .evidence
        .get("pii_candidates")
        .and_then(Value::as_array)
        .map(|values| !values.is_empty())
        .unwrap_or(false)
    {
        score -= 0.05;
    }
    if proposal.evidence.get("join_success_rate").is_some() {
        score += 0.25;
    }
    score.clamp(0.0, 1.0)
}

pub(crate) async fn ontology_calibration_retrieval_similarity_score(
    state: &AppState,
    proposal: &OntologyOnboardingProposalDraft,
) -> Result<Option<f64>, AppError> {
    if proposal.proposal_type != "object" {
        return Ok(None);
    }
    let candidate_name = proposal
        .content
        .get("object_type")
        .and_then(Value::as_str)
        .unwrap_or(&proposal.name);
    let domain_scope = ontology_proposal_domain_scope(proposal);
    let semantic_objects = state.list_semantic_objects().await?;
    let candidate = ontology_entity_resolution_candidate(
        candidate_name,
        candidate_name,
        &domain_scope,
        &semantic_objects,
        0.0,
    );
    Ok(candidate.retrieval_hits.first().map(|hit| hit.score))
}

pub(crate) fn ontology_confidence_calibration_object_key(
    run_id: Uuid,
    proposal_id: Uuid,
    record_id: Uuid,
) -> String {
    format!("ontology:calibration:{run_id}:{proposal_id}:{record_id}")
}

pub(crate) fn normalize_ontology_onboarding_review_decision(
    decision: &str,
) -> Result<(String, String), AppError> {
    let decision = decision.trim().to_ascii_lowercase().replace('-', "_");
    match decision.as_str() {
        "approve" | "approved" => Ok(("approve".to_string(), "approved".to_string())),
        "reject" | "rejected" => Ok(("reject".to_string(), "rejected".to_string())),
        "request_changes" | "changes_requested" => Ok((
            "request_changes".to_string(),
            "changes_requested".to_string(),
        )),
        "merge_into_existing" => Ok((
            "merge_into_existing".to_string(),
            "merge_into_existing".to_string(),
        )),
        "needs_more_evidence" => Ok((
            "needs_more_evidence".to_string(),
            "needs_more_evidence".to_string(),
        )),
        _ => Err(AppError::bad_request(
            "ontology onboarding review decision must be approve, reject, request_changes, merge_into_existing, or needs_more_evidence",
        )),
    }
}

pub(crate) async fn ontology_materialize_business_object(
    state: &AppState,
    proposal: &OntologyOnboardingProposalDraft,
) -> Result<SemanticObject, AppError> {
    let domain_scope = ontology_proposal_domain_scope(proposal);
    let object_name = proposal
        .content
        .get("object_type")
        .and_then(Value::as_str)
        .unwrap_or(proposal.name.as_str());
    ontology_get_or_create_semantic_object(
        state,
        CreateSemanticObject {
            source_id: None,
            object_type: "business_object".to_string(),
            object_key: ontology_business_object_key(&domain_scope, object_name),
            title: object_name.to_string(),
            summary: format!("Approved {domain_scope} ontology business object: {object_name}."),
            content: json!({
                "object_type": object_name,
                "domain_scope": domain_scope,
                "tool_namespace": ontology_proposal_tool_namespace(proposal),
                "proposal_id": proposal.id,
                "source_mapping": proposal.source_mapping,
                "properties": proposal.content.get("properties").cloned().unwrap_or_else(|| json!([])),
            }),
            semantic_scopes: ontology_domain_semantic_scopes(&domain_scope, "published"),
            source_uri: Some(format!(
                "mandoforge://ontology/onboarding/proposals/{}/materialized",
                proposal.id
            )),
            provenance: json!({
                "source": "ontology_onboarding.materialize",
                "proposal_id": proposal.id,
                "proposal_type": proposal.proposal_type,
                "materialized_at": Utc::now(),
            }),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "active".to_string(),
        },
    )
    .await
}

pub(crate) async fn ontology_object_proposal_has_unreviewed_merge_risk(
    state: &AppState,
    proposal: &OntologyOnboardingProposalDraft,
) -> Result<bool, AppError> {
    let candidate_name = proposal
        .content
        .get("object_type")
        .and_then(Value::as_str)
        .unwrap_or(&proposal.name);
    let domain_scope = ontology_proposal_domain_scope(proposal);
    let semantic_objects = state.list_semantic_objects().await?;
    let candidate = ontology_entity_resolution_candidate(
        candidate_name,
        candidate_name,
        &domain_scope,
        &semantic_objects,
        0.80,
    );
    Ok(candidate.decision.is_duplicate && candidate.decision.review_required)
}

pub(crate) async fn ontology_materialize_metric(
    state: &AppState,
    proposal: &OntologyOnboardingProposalDraft,
) -> Result<SemanticObject, AppError> {
    let domain_scope = ontology_proposal_domain_scope(proposal);
    ontology_get_or_create_semantic_object(
        state,
        CreateSemanticObject {
            source_id: None,
            object_type: "business_metric".to_string(),
            object_key: format!(
                "{}.metric.{}",
                ontology_slug(&domain_scope),
                ontology_slug(&proposal.name)
            ),
            title: proposal.name.clone(),
            summary: format!(
                "Approved {domain_scope} semantic metric: {}.",
                proposal.name
            ),
            content: json!({
                "metric_name": proposal.name,
                "domain_scope": domain_scope,
                "tool_namespace": ontology_proposal_tool_namespace(proposal),
                "proposal_id": proposal.id,
                "source_mapping": proposal.source_mapping,
                "definition": proposal.content,
            }),
            semantic_scopes: ontology_domain_semantic_scopes(&domain_scope, "published"),
            source_uri: Some(format!(
                "mandoforge://ontology/onboarding/proposals/{}/metric",
                proposal.id
            )),
            provenance: json!({
                "source": "ontology_onboarding.materialize",
                "proposal_id": proposal.id,
                "proposal_type": proposal.proposal_type,
                "materialized_at": Utc::now(),
            }),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "active".to_string(),
        },
    )
    .await
}

pub(crate) async fn ontology_materialize_action(
    state: &AppState,
    proposal: &OntologyOnboardingProposalDraft,
) -> Result<SemanticObject, AppError> {
    let domain_scope = ontology_proposal_domain_scope(proposal);
    let tool_namespace = ontology_proposal_tool_namespace(proposal);
    let effects = proposal
        .content
        .get("effects")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let read_only = !ontology_action_has_effects(&effects);
    let transaction_profile =
        ontology_action_transaction_profile_for_proposal(proposal, read_only)?;
    let execution_mode = ontology_action_execution_mode(transaction_profile, read_only);
    ontology_get_or_create_semantic_object(
        state,
        CreateSemanticObject {
            source_id: None,
            object_type: "ontology_action_type".to_string(),
            object_key: format!(
                "{}.action.{}",
                ontology_slug(&domain_scope),
                ontology_slug(&proposal.name)
            ),
            title: proposal.name.clone(),
            summary: format!(
                "Approved {domain_scope} ontology action type: {}; policy and audit required.",
                proposal.name
            ),
            content: json!({
                "proposal_id": proposal.id,
                "tool_name": format!("{}.{}", tool_namespace, proposal.name),
                "domain_scope": domain_scope,
                "tool_namespace": tool_namespace,
                "transaction_profile": transaction_profile,
                "execution_mode": execution_mode,
                "action_contract": proposal.content,
            }),
            semantic_scopes: ontology_domain_semantic_scopes(&domain_scope, "published"),
            source_uri: Some(format!(
                "mandoforge://ontology/onboarding/proposals/{}/action",
                proposal.id
            )),
            provenance: json!({
                "source": "ontology_onboarding.materialize",
                "proposal_id": proposal.id,
                "proposal_type": proposal.proposal_type,
                "materialized_at": Utc::now(),
            }),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "active".to_string(),
        },
    )
    .await
}

pub(crate) async fn ontology_materialize_logic_rule(
    state: &AppState,
    proposal: &OntologyOnboardingProposalDraft,
) -> Result<SemanticObject, AppError> {
    let domain_scope = ontology_proposal_domain_scope(proposal);
    ontology_get_or_create_semantic_object(
        state,
        CreateSemanticObject {
            source_id: None,
            object_type: "ontology_logic_rule".to_string(),
            object_key: format!(
                "{}.logic.{}",
                ontology_slug(&domain_scope),
                ontology_slug(&proposal.name)
            ),
            title: proposal.name.clone(),
            summary: format!(
                "Approved {domain_scope} ontology logic rule: {}; disabled until publish policy enables it.",
                proposal.name
            ),
            content: json!({
                "proposal_id": proposal.id,
                "domain_scope": domain_scope,
                "tool_namespace": ontology_proposal_tool_namespace(proposal),
                "enabled": false,
                "logic_rule": proposal.content,
            }),
            semantic_scopes: ontology_domain_semantic_scopes(&domain_scope, "published"),
            source_uri: Some(format!(
                "mandoforge://ontology/onboarding/proposals/{}/logic",
                proposal.id
            )),
            provenance: json!({
                "source": "ontology_onboarding.materialize",
                "proposal_id": proposal.id,
                "proposal_type": proposal.proposal_type,
                "materialized_at": Utc::now(),
                "enabled": false,
            }),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "active".to_string(),
        },
    )
    .await
}

pub(crate) async fn ontology_materialize_relation(
    state: &AppState,
    proposal: &OntologyOnboardingProposalDraft,
) -> Result<SemanticLink, AppError> {
    let domain_scope = ontology_proposal_domain_scope(proposal);
    let from_object = proposal
        .content
        .get("from_object")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::bad_request("relation proposal missing from_object"))?;
    let to_object = proposal
        .content
        .get("to_object")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::bad_request("relation proposal missing to_object"))?;
    let relation = proposal
        .content
        .get("relation")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::bad_request("relation proposal missing relation"))?;
    let from = ontology_ensure_business_object_stub(state, &domain_scope, from_object).await?;
    let to = ontology_ensure_business_object_stub(state, &domain_scope, to_object).await?;
    ontology_create_semantic_link_if_absent(
        state,
        CreateSemanticLink {
            from_entity_type: "semantic_object".to_string(),
            from_entity_id: from.id.to_string(),
            relation_type: relation.to_string(),
            to_entity_type: "semantic_object".to_string(),
            to_entity_id: to.id.to_string(),
            metadata: json!({
                "business_relation": proposal.name,
                "source_mapping": proposal.source_mapping,
                "proposal_id": proposal.id,
                "evidence": proposal.evidence,
            }),
            provenance: json!({
                "source": "ontology_onboarding.materialize",
                "proposal_id": proposal.id,
                "proposal_type": proposal.proposal_type,
                "materialized_at": Utc::now(),
            }),
            confidence: proposal.confidence,
            status: "active".to_string(),
        },
    )
    .await
}

pub(crate) async fn ontology_ensure_business_object_stub(
    state: &AppState,
    domain_scope: &str,
    object_name: &str,
) -> Result<SemanticObject, AppError> {
    ontology_get_or_create_semantic_object(
        state,
        CreateSemanticObject {
            source_id: None,
            object_type: "business_object".to_string(),
            object_key: ontology_business_object_key(domain_scope, object_name),
            title: object_name.to_string(),
            summary: format!("{domain_scope} ontology business object: {object_name}."),
            content: json!({
                "object_type": object_name,
                "domain_scope": domain_scope,
                "stub_created_for_relation": true,
            }),
            semantic_scopes: ontology_domain_semantic_scopes(domain_scope, "published"),
            source_uri: Some(format!(
                "mandoforge://ontology/onboarding/business-objects/{}",
                ontology_slug(object_name)
            )),
            provenance: json!({
                "source": "ontology_onboarding.materialize_relation",
                "materialized_at": Utc::now(),
            }),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "active".to_string(),
        },
    )
    .await
}

pub(crate) async fn ontology_get_or_create_semantic_object(
    state: &AppState,
    input: CreateSemanticObject,
) -> Result<SemanticObject, AppError> {
    if let Some(existing) = state
        .list_semantic_objects()
        .await?
        .into_iter()
        .find(|object| object.archived_at.is_none() && object.object_key == input.object_key)
    {
        return Ok(existing);
    }
    state.create_semantic_object(input).await
}

pub(crate) async fn ontology_create_semantic_link_if_absent(
    state: &AppState,
    input: CreateSemanticLink,
) -> Result<SemanticLink, AppError> {
    if let Some(existing) = state.list_semantic_links().await?.into_iter().find(|link| {
        link.archived_at.is_none()
            && link.from_entity_type == input.from_entity_type
            && link.from_entity_id == input.from_entity_id
            && link.relation_type == input.relation_type
            && link.to_entity_type == input.to_entity_type
            && link.to_entity_id == input.to_entity_id
    }) {
        return Ok(existing);
    }
    state.create_semantic_link(input).await
}

pub(crate) fn ontology_proposal_domain_scope(proposal: &OntologyOnboardingProposalDraft) -> String {
    proposal
        .content
        .get("domain_scope")
        .or_else(|| proposal.evidence.get("domain_scope"))
        .and_then(Value::as_str)
        .unwrap_or("commerce")
        .to_string()
}

pub(crate) fn ontology_proposal_tool_namespace(
    proposal: &OntologyOnboardingProposalDraft,
) -> String {
    proposal
        .content
        .get("tool_namespace")
        .or_else(|| proposal.evidence.get("tool_namespace"))
        .and_then(Value::as_str)
        .unwrap_or("commerce")
        .to_string()
}

pub(crate) fn ontology_run_industry(run: &OntologyOnboardingRun) -> String {
    run.proposals
        .iter()
        .find_map(|proposal| {
            proposal
                .content
                .get("industry")
                .or_else(|| proposal.evidence.get("industry"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| {
            if run.source_mode.contains("insurance") {
                "insurance".to_string()
            } else {
                "ecommerce".to_string()
            }
        })
}

pub(crate) fn ontology_business_object_key(domain_scope: &str, object_name: &str) -> String {
    format!(
        "{}.{}",
        ontology_slug(domain_scope),
        ontology_slug(object_name)
    )
}

pub(crate) fn ontology_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_separator = false;
    for (index, ch) in value.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 && !previous_was_separator {
                slug.push('_');
            }
            slug.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator && !slug.is_empty() {
            slug.push('_');
            previous_was_separator = true;
        }
    }
    slug.trim_matches('_').to_string()
}

pub(crate) fn ontology_domain_semantic_scopes(domain_scope: &str, share_policy: &str) -> Value {
    json!({
        "domain_scope": domain_scope,
        "workflow_scope": "enterprise-ontology-fast-onboarding",
        "memory_scope": "ontology",
        "share_policy": share_policy,
    })
}

pub(crate) fn semantic_object_matches_product_query(
    object: &SemanticObject,
    query: &SemanticProductQuery,
) -> bool {
    text_filter_matches(&query.object_type, &object.object_type)
        && text_filter_matches(&query.status, &object.status)
        && text_filter_matches(&query.trust_level, &object.trust_level)
        && text_filter_matches(&query.freshness, &object.freshness)
        && semantic_scope_filter_matches(
            &query.domain_scope,
            &object.semantic_scopes,
            "domain_scope",
        )
        && semantic_scope_filter_matches(
            &query.workflow_scope,
            &object.semantic_scopes,
            "workflow_scope",
        )
        && semantic_scope_filter_matches(
            &query.memory_scope,
            &object.semantic_scopes,
            "memory_scope",
        )
}

pub(crate) fn text_filter_matches(filter: &Option<String>, actual: &str) -> bool {
    filter
        .as_ref()
        .and_then(|value| normalize_optional_text(value.clone()))
        .map(|expected| expected == actual)
        .unwrap_or(true)
}

pub(crate) fn semantic_scope_filter_matches(
    filter: &Option<String>,
    scopes: &Value,
    key: &str,
) -> bool {
    filter
        .as_ref()
        .and_then(|value| normalize_optional_text(value.clone()))
        .map(|expected| scopes.get(key).and_then(Value::as_str) == Some(expected.as_str()))
        .unwrap_or(true)
}

pub(crate) fn domain_ontology_object_type_suggestions(domain_scope: &str) -> Vec<&'static str> {
    match domain_scope {
        "legal" => vec![
            "contract_clause",
            "legal_position",
            "obligation",
            "risk_finding",
        ],
        "social-media" | "social_media" => {
            vec![
                "content_angle",
                "audience_segment",
                "platform_signal",
                "brand_voice_rule",
            ]
        }
        "ecommerce" | "e-commerce" => vec![
            "campaign_signal",
            "product_offer",
            "pricing_rule",
            "conversion_observation",
        ],
        _ => vec!["policy", "memory", "decision", "evidence"],
    }
}

pub(crate) fn domain_ontology_relation_type_suggestions(domain_scope: &str) -> Vec<&'static str> {
    match domain_scope {
        "legal" => vec!["cites_clause", "constrains", "supersedes", "contradicts"],
        "social-media" | "social_media" => {
            vec![
                "supports_angle",
                "targets_segment",
                "supersedes",
                "contradicts",
            ]
        }
        "ecommerce" | "e-commerce" => {
            vec![
                "drives_metric",
                "constrains_offer",
                "supersedes",
                "contradicts",
            ]
        }
        _ => vec!["supports", "contradicts", "supersedes"],
    }
}

pub(crate) fn ontology_builder_candidate_types(
    domain_scope: &str,
    source_text: Option<&str>,
    agent_draft: Option<&Value>,
    field: &str,
    max_items: usize,
) -> Result<Vec<String>, AppError> {
    let limit = max_items.clamp(1, 50);
    let mut candidates = BTreeSet::<String>::new();
    let domain_defaults = match field {
        "object_types" => domain_ontology_object_type_suggestions(domain_scope),
        "relation_types" => domain_ontology_relation_type_suggestions(domain_scope),
        _ => {
            return Err(AppError::bad_request(
                "ontology builder field must be object_types or relation_types",
            ));
        }
    };
    for candidate in domain_defaults {
        candidates.insert(normalize_ontology_builder_token(field, candidate)?);
    }
    for candidate in ontology_builder_terms_from_source(domain_scope, source_text, field) {
        candidates.insert(normalize_ontology_builder_token(field, &candidate)?);
    }
    for candidate in ontology_builder_agent_draft_terms(agent_draft, field) {
        candidates.insert(normalize_ontology_builder_token(field, &candidate)?);
    }
    Ok(candidates.into_iter().take(limit).collect())
}

pub(crate) fn ontology_builder_agent_draft_terms(
    agent_draft: Option<&Value>,
    field: &str,
) -> Vec<String> {
    let Some(agent_draft) = agent_draft else {
        return Vec::new();
    };
    agent_draft
        .get(field)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|value| normalize_optional_text(value.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn ontology_builder_terms_from_source(
    domain_scope: &str,
    source_text: Option<&str>,
    field: &str,
) -> Vec<String> {
    let Some(source_text) = source_text else {
        return Vec::new();
    };
    let normalized_source = source_text.to_ascii_lowercase().replace(['_', '-'], " ");
    let dictionary = ontology_builder_dictionary(domain_scope, field);
    dictionary
        .into_iter()
        .filter(|(phrase, _)| normalized_source.contains(phrase))
        .map(|(_, token)| token.to_string())
        .collect()
}

pub(crate) fn ontology_builder_dictionary(
    domain_scope: &str,
    field: &str,
) -> Vec<(&'static str, &'static str)> {
    match (domain_scope, field) {
        ("legal", "object_types") => vec![
            ("contract", "contract"),
            ("party", "party"),
            ("clause", "clause"),
            ("obligation", "obligation"),
            ("risk", "risk"),
            ("jurisdiction", "jurisdiction"),
            ("approval requirement", "approval_requirement"),
            ("template", "template"),
            ("negotiation position", "negotiation_position"),
        ],
        ("legal", "relation_types") => vec![
            ("contains", "contains"),
            ("creates obligation", "creates_obligation"),
            ("triggers risk", "triggers_risk"),
            ("requires approval", "requires_approval"),
            ("supersedes", "supersedes"),
        ],
        ("ecommerce" | "e-commerce", "object_types") => vec![
            ("store", "store"),
            ("product", "product"),
            ("sku", "sku"),
            ("inventory", "inventory"),
            ("campaign", "campaign"),
            ("ad set", "ad_set"),
            ("creative", "creative"),
            ("roas", "roas"),
            ("margin", "margin"),
            ("budget rule", "budget_rule"),
            ("customer segment", "customer_segment"),
        ],
        ("ecommerce" | "e-commerce", "relation_types") => vec![
            ("promotes", "promotes"),
            ("has inventory", "has_inventory"),
            ("measured by", "measured_by"),
            ("applies to", "applies_to"),
            ("constrains", "constrains"),
        ],
        ("social-media" | "social_media", "object_types") => vec![
            ("account", "account"),
            ("platform", "platform"),
            ("post", "post"),
            ("topic", "topic"),
            ("content pillar", "content_pillar"),
            ("audience segment", "audience_segment"),
            ("campaign", "campaign"),
            ("engagement metric", "engagement_metric"),
            ("brand risk", "brand_risk"),
            ("publishing approval", "publishing_approval"),
        ],
        ("social-media" | "social_media", "relation_types") => vec![
            ("belongs to", "belongs_to"),
            ("targets", "targets"),
            ("measured by", "measured_by"),
            ("requires approval", "requires_approval"),
            ("supersedes", "supersedes"),
        ],
        (_, "object_types") => vec![
            ("policy", "policy"),
            ("memory", "memory"),
            ("decision", "decision"),
            ("evidence", "evidence"),
        ],
        (_, "relation_types") => vec![
            ("supports", "supports"),
            ("contradicts", "contradicts"),
            ("supersedes", "supersedes"),
        ],
        _ => Vec::new(),
    }
}

pub(crate) fn normalize_ontology_builder_source_refs(
    source_refs: Vec<String>,
) -> Result<Vec<String>, AppError> {
    let mut refs = BTreeSet::<String>::new();
    for source_ref in source_refs {
        let source_ref = require_non_empty(source_ref, "ontology builder source_ref")?;
        if source_ref.len() > 512 {
            return Err(AppError::bad_request(
                "ontology builder source_ref must be 512 characters or shorter",
            ));
        }
        refs.insert(source_ref);
    }
    Ok(refs.into_iter().take(50).collect())
}

pub(crate) async fn ontology_builder_evidence_objects(
    state: &AppState,
    evidence_object_ids: &[Uuid],
) -> Result<Vec<SemanticObject>, AppError> {
    let mut objects = Vec::new();
    for id in evidence_object_ids.iter().take(50) {
        objects.push(state.get_semantic_object(*id).await?);
    }
    Ok(objects)
}

pub(crate) fn semantic_ontology_builder_prompt_packet(
    domain_scope: &str,
    workflow_scope: Option<&str>,
    memory_scope: Option<&str>,
    objective: &str,
    source_text: Option<&str>,
    source_refs: &[String],
) -> Value {
    json!({
        "system": "Draft a domain ontology proposal only. Do not create durable memory, mutate the registry, or broaden sharing scopes. Return object_types, relation_types, rationale, evidence, and review risks.",
        "user": {
            "objective": objective,
            "domain_scope": domain_scope,
            "workflow_scope": workflow_scope,
            "memory_scope": memory_scope,
            "source_text": source_text.unwrap_or(""),
            "source_refs": source_refs,
            "output_schema": {
                "object_types": ["lower_snake_case"],
                "relation_types": ["lower_snake_case"],
                "rationale": "why these concepts are useful",
                "review_risks": ["scope leak, duplicate type, unsupported relation, stale source"]
            }
        },
    })
}

pub(crate) fn normalize_ontology_builder_token(
    field: &str,
    value: &str,
) -> Result<String, AppError> {
    let normalized = ontology_builder_token(value);
    validate_handoff_token(field, &normalized)
}

pub(crate) fn ontology_builder_token(value: &str) -> String {
    let mut token = String::new();
    let mut previous_was_separator = true;
    let mut previous_was_lower_or_digit = false;
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && previous_was_lower_or_digit && !previous_was_separator {
                token.push('_');
            }
            token.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
            previous_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else if !previous_was_separator {
            token.push('_');
            previous_was_separator = true;
            previous_was_lower_or_digit = false;
        }
    }
    token.trim_matches('_').to_string()
}
