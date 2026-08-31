use super::*;

fn proposal(proposal_type: &str, name: &str, content: Value) -> OntologyOnboardingProposalDraft {
    OntologyOnboardingProposalDraft {
        id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        proposal_type: proposal_type.to_string(),
        name: name.to_string(),
        source_mapping: "test".to_string(),
        confidence: 1.0,
        evidence: json!({"source": "test"}),
        recommendation: "approve".to_string(),
        review_status: "approved".to_string(),
        content,
    }
}

#[test]
fn ontology_sdk_catalog_derives_stable_ascii_names_and_digest() {
    let object = proposal(
        "object",
        "Customer",
        json!({
            "object_type": "Customer",
            "primary_key": "customer_id",
            "properties": [
                {"name": "customer_id", "type": "integer", "nullable": false},
                {"name": "display_name"}
            ]
        }),
    );
    let (catalog, digest) =
        build_ontology_release_catalog("commerce", &[object], None).expect("catalog");
    assert_eq!(catalog.objects[0].api_name, "Customer");
    assert_eq!(
        catalog.objects[0].primary_key_api_name.as_deref(),
        Some("customerId")
    );
    assert_eq!(catalog.objects[0].properties[0].api_name, "customerId");
    assert_eq!(catalog.objects[0].properties[0].value_type, "integer");
    assert!(!catalog.objects[0].properties[0].nullable);
    assert_eq!(catalog.objects[0].properties[1].value_type, "unknown");
    assert!(catalog.objects[0].properties[1].nullable);
    assert!(digest.starts_with("sha256:"));
    assert_eq!(
        digest,
        canonical_json_sha256(&serde_json::to_value(&catalog).expect("catalog value"))
    );
}

#[test]
fn ontology_sdk_catalog_converts_runtime_names_but_rejects_invalid_explicit_api_names() {
    let relation = proposal(
        "relation",
        "Customer places Order",
        json!({
            "from_object": "Customer",
            "relation": "places",
            "to_object": "Order",
            "api_name": "commerce.customer_places_order"
        }),
    );
    let customer = proposal("object", "Customer", json!({"object_type": "Customer"}));
    let order = proposal("object", "Order", json!({"object_type": "Order"}));
    let (catalog, _) =
        build_ontology_release_catalog("commerce", &[customer, order, relation], None)
            .expect("runtime dotted names are not API names");
    assert_eq!(catalog.relations[0].api_name, "customerPlacesOrder");

    let invalid = proposal(
        "object",
        "Customer",
        json!({"object_type": "Customer", "api_name": "customer_name"}),
    );
    let error = build_ontology_release_catalog("commerce", &[invalid], None)
        .expect_err("object API names must be UpperCamelCase");
    assert!(error.message.contains("strict ASCII v1 casing"));
}

#[test]
fn ontology_sdk_catalog_rejects_duplicate_object_proposals() {
    let first = proposal("object", "Order", json!({"object_type": "Order"}));
    let second = proposal(
        "object",
        "Order replacement",
        json!({"object_type": "Order", "properties": [{"name": "status"}]}),
    );

    let error = build_ontology_release_catalog("commerce", &[first, second], None)
        .expect_err("duplicate object identities must fail");

    assert!(error.message.contains("duplicate object identities"));
}

#[test]
fn ontology_sdk_catalog_rejects_unresolved_primary_keys_and_unsupported_property_types() {
    let unresolved_primary_key = proposal(
        "object",
        "Order",
        json!({
            "object_type": "Order",
            "primary_key": "missing_id",
            "properties": [{"name": "id", "type": "uuid"}]
        }),
    );
    let error = build_ontology_release_catalog("commerce", &[unresolved_primary_key], None)
        .expect_err("explicit primary key must resolve to a property");
    assert!(error.message.contains("primary key missing_id"));

    let unsupported_property = proposal(
        "object",
        "Order",
        json!({
            "object_type": "Order",
            "properties": [{"name": "total", "type": "money"}]
        }),
    );
    let error = build_ontology_release_catalog("commerce", &[unsupported_property], None)
        .expect_err("unsupported property types must fail the catalog gate");
    assert!(error.message.contains("property type money is unsupported"));
}

#[test]
fn ontology_action_parameters_accept_object_schema_without_explicit_type() {
    let schema = json!({
        "properties": {
            "order_id": {"type": "string"},
            "amount": {"type": "number"},
            "attempt": {"type": "number", "enum": [1]},
            "integral_attempt": {"type": "integer", "enum": [1.0]},
            "status": {"type": "string", "enum": ["open", "closed"]},
            "request_id": {"type": "uuid"},
            "requested_on": {"type": "date"},
            "created_at": {"type": "timestamp"},
            "reviewed_at": {"type": "datetime"},
            "metadata": {"type": "json"},
            "lines": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "sku": {"type": "text"},
                        "quantity": {"type": "int64"},
                        "active": {"type": "bool"}
                    },
                    "required": ["sku", "quantity"]
                }
            }
        },
        "required": ["order_id"]
    });
    validate_ontology_action_parameters(&schema, &json!({"order_id": "order-1"}))
        .expect("type-omitted object schema should match generated SDK semantics");
    validate_ontology_action_parameters(&schema, &json!({"order_id": "order-1", "status": "open"}))
        .expect("published enum value");
    validate_ontology_action_parameters(&schema, &json!({"order_id": "order-1", "attempt": 1.0}))
        .expect("integer and floating JSON representations of the same enum number");
    validate_ontology_action_parameters(
        &schema,
        &json!({"order_id": "order-1", "integral_attempt": 1}),
    )
    .expect("integer schemas must accept equivalent integral floating enum members");
    validate_ontology_action_parameters(
        &schema,
        &json!({
            "order_id": "order-1",
            "request_id": "8b783dbc-d482-46df-8fc6-1812fe94b8a7",
            "requested_on": "2026-08-12",
            "created_at": "2026-08-12T00:00:00Z",
            "reviewed_at": "2026-08-12T08:00:00+08:00",
            "metadata": ["arbitrary", 1],
            "lines": [{"sku": "sku-1", "quantity": 2.0, "active": true}]
        }),
    )
    .expect("nested schemas and published aliases must share generated SDK semantics");
    let error = validate_ontology_action_parameters(
        &schema,
        &json!({"order_id": "order-1", "status": "pending"}),
    )
    .expect_err("undeclared enum values must fail closed");
    assert!(error.message.contains("not an allowed enum value"));
    for (field, value, expected_type) in [
        ("request_id", "not-a-uuid", "uuid"),
        ("requested_on", "yesterday", "date"),
        ("created_at", "noon", "timestamp"),
        ("reviewed_at", "tomorrow", "datetime"),
    ] {
        let mut parameters = json!({"order_id": "order-1"});
        parameters[field] = json!(value);
        let error = validate_ontology_action_parameters(&schema, &parameters)
            .expect_err("formatted scalar aliases must fail closed");
        assert!(error.message.contains(&format!("must be {expected_type}")));
    }
    let error = validate_ontology_action_parameters(&schema, &json!({"amount": 12.5}))
        .expect_err("required properties must still be enforced");
    assert!(error.message.contains("missing required field order_id"));
    let error = validate_ontology_action_parameters(
        &schema,
        &json!({"order_id": "order-1", "lines": [{"sku": "sku-1", "quantity": "two"}]}),
    )
    .expect_err("nested item types must be enforced");
    assert!(
        error
            .message
            .contains("parameters.lines[0].quantity must be int64")
    );
    let error = validate_ontology_action_parameters(
        &schema,
        &json!({"order_id": "order-1", "lines": [{"sku": "sku-1", "quantity": 2, "extra": true}]}),
    )
    .expect_err("nested undeclared fields must fail closed");
    assert!(
        error
            .message
            .contains("parameters.lines[0].extra is not declared")
    );
}

#[test]
fn ontology_sdk_catalog_requires_action_target_in_object_catalog() {
    let action = proposal(
        "action",
        "refund_order",
        json!({
            "action": "refund_order",
            "target_object": "Order",
            "inputs": [],
            "reads": [],
            "effects": [],
            "policy": {"approval_required": true, "transaction_profile": "proposal_only"},
            "transaction_profile": "proposal_only",
            "executor": "local",
            "audit_event": "commerce.refund_order"
        }),
    );
    let error = build_ontology_release_catalog("commerce", &[action], None)
        .expect_err("action target must be a catalog object");
    assert!(
        error
            .message
            .contains("target object is not in the release catalog")
    );
}

#[test]
fn ontology_sdk_catalog_rejects_non_array_action_enums() {
    let order = proposal("object", "Order", json!({"object_type": "Order"}));
    let mut action = proposal(
        "action",
        "refund_order",
        json!({
            "action": "refund_order",
            "target_object": "Order",
            "inputs": {"status": {"type": "string", "enum": "open"}},
            "reads": [],
            "effects": [],
            "policy": {"approval_required": true, "transaction_profile": "proposal_only"},
            "transaction_profile": "proposal_only",
            "executor": "local",
            "audit_event": "commerce.refund_order"
        }),
    );

    let error = build_ontology_release_catalog("commerce", &[order.clone(), action.clone()], None)
        .expect_err("non-array action enums must fail before publication");
    assert!(error.message.contains("enum must be an array"));

    action.content["inputs"] = json!({
        "enum": {"type": "string", "enum": ["open"]}
    });
    build_ontology_release_catalog("commerce", &[order.clone(), action.clone()], None)
        .expect("a flat action parameter may be named enum");

    action.content["inputs"] = json!({
        "type": "object",
        "properties": {
            "enum": {"type": "string"},
            "selector": {"type": "object", "enum": [{"enum": "literal"}]}
        }
    });
    build_ontology_release_catalog("commerce", &[order.clone(), action.clone()], None)
        .expect("schema property names and enum member objects are data, not schema keywords");

    action.content["inputs"] = json!({
        "status": {"type": "string", "enum": []}
    });
    let error = build_ontology_release_catalog("commerce", &[order.clone(), action.clone()], None)
        .expect_err("empty action enums must fail before publication");
    assert!(error.message.contains("enum must not be empty"));

    action.content["inputs"] = json!({
        "status": {"type": "string", "enum": ["open", 1]}
    });
    let error = build_ontology_release_catalog("commerce", &[order.clone(), action.clone()], None)
        .expect_err("type-mismatched action enum members must fail before publication");
    assert!(error.message.contains("enum member must be string"));

    action.content["inputs"] = json!({
        "request_id": {"type": "uuid", "enum": ["not-a-uuid"]}
    });
    let error = build_ontology_release_catalog("commerce", &[order.clone(), action.clone()], None)
        .expect_err("malformed formatted enum values must fail before publication");
    assert!(error.message.contains("enum member must be uuid"));

    action.content["inputs"] = json!({
        "type": "array",
        "items": {"type": "string"}
    });
    let error = build_ontology_release_catalog("commerce", &[order.clone(), action.clone()], None)
        .expect_err("top-level action schemas must describe an object");
    assert!(error.message.contains("top-level type must be object"));

    action.content["inputs"] = json!({"type": "object"});
    let error = build_ontology_release_catalog("commerce", &[order.clone(), action.clone()], None)
        .expect_err("top-level object schemas must declare properties");
    assert!(error.message.contains("must declare properties"));

    action.content["inputs"] = json!({
        "selector": {
            "type": "object",
            "properties": {"x": {"type": "string"}},
            "required": ["x"],
            "enum": [{"x": 1}]
        }
    });
    let error = build_ontology_release_catalog("commerce", &[order.clone(), action.clone()], None)
        .expect_err("object enum members must satisfy nested declarations");
    assert!(error.message.contains("does not satisfy its declaration"));

    action.content["inputs"] = json!({
        "items": {
            "type": "array",
            "items": {"type": "integer"},
            "enum": [[1, "two"]]
        }
    });
    let error = build_ontology_release_catalog("commerce", &[order, action], None)
        .expect_err("array enum members must satisfy item declarations");
    assert!(error.message.contains("does not satisfy its declaration"));
}

#[test]
fn ontology_sdk_release_catalog_rejects_action_runtime_and_target_rebinding() {
    let order = proposal("object", "Order", json!({"object_type": "Order"}));
    let customer = proposal("object", "Customer", json!({"object_type": "Customer"}));
    let action = proposal(
        "action",
        "refund_order",
        json!({
            "action": "refund_order",
            "target_object": "Order",
            "inputs": {},
            "reads": [],
            "effects": [],
            "policy": {"approval_required": true, "transaction_profile": "proposal_only"},
            "transaction_profile": "proposal_only",
            "executor": "local",
            "audit_event": "commerce.refund_order"
        }),
    );
    let tool_spec =
        ontology_tool_spec_from_action_proposal(action.run_id, &action).expect("action tool spec");
    let tool_spec_value = serde_json::to_value(&tool_spec).expect("tool spec value");
    let contract_digest = normalized_json_sha256(&tool_spec_value);
    let (catalog, catalog_digest) =
        build_ontology_release_catalog("commerce", &[order, customer, action.clone()], None)
            .expect("catalog");
    let release = OntologyRelease {
        id: Uuid::new_v4(),
        version: "commerce-action-binding-v1".to_string(),
        domain_scope: "commerce".to_string(),
        source_run_id: Some(action.run_id),
        parent_release_id: None,
        rollback_target_release_id: None,
        status: "active".to_string(),
        release_class: "repo_controlled".to_string(),
        object_count: 2,
        relation_count: 0,
        action_count: 1,
        migration_policy: json!({}),
        gate_result: json!({"status": "passed"}),
        materialized_object_ids: json!([]),
        materialized_link_ids: json!([]),
        evidence_refs: json!([
            {
                "proposal_id": action.id,
                "proposal_type": "action",
                "review_status": "approved",
                "tool_spec": tool_spec_value,
                "contract_digest": contract_digest,
            },
            catalog_evidence(&catalog, &catalog_digest),
        ]),
        promoted_by: Some("test".to_string()),
        promoted_at: Some(Utc::now()),
        rolled_back_by: None,
        rolled_back_at: None,
        archived_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    release_catalog_from_evidence(&release).expect("untampered catalog");

    let mut runtime_catalog = catalog.clone();
    runtime_catalog.actions[0].runtime_name = "commerce.other_action".to_string();
    let runtime_digest = canonical_json_sha256(
        &serde_json::to_value(&runtime_catalog).expect("runtime catalog value"),
    );
    let mut runtime_release = release.clone();
    runtime_release.evidence_refs[1] = catalog_evidence(&runtime_catalog, &runtime_digest);
    let runtime_error = release_catalog_from_evidence(&runtime_release)
        .expect_err("runtime action rebinding must fail");
    assert!(runtime_error.message.contains("action identity"));

    let mut identity_catalog = catalog.clone();
    identity_catalog
        .objects
        .iter_mut()
        .find(|object| object.api_name == "Customer")
        .expect("customer")
        .stable_key = "object:Other".to_string();
    let identity_digest = canonical_json_sha256(
        &serde_json::to_value(&identity_catalog).expect("identity catalog value"),
    );
    let mut identity_release = release.clone();
    identity_release.evidence_refs[1] = catalog_evidence(&identity_catalog, &identity_digest);
    let identity_error = release_catalog_from_evidence(&identity_release)
        .expect_err("object identity rebinding must fail");
    assert!(identity_error.message.contains("object identity"));

    let mut duplicate_release = release.clone();
    let duplicate_evidence = duplicate_release.evidence_refs[0].clone();
    duplicate_release
        .evidence_refs
        .as_array_mut()
        .expect("release evidence")
        .push(duplicate_evidence);
    let duplicate_error = release_catalog_from_evidence(&duplicate_release)
        .expect_err("duplicate action evidence must fail");
    assert!(duplicate_error.message.contains("action set"));

    let mut target_catalog = catalog;
    target_catalog.actions[0].target_object_api_name = "Customer".to_string();
    let target_digest = canonical_json_sha256(
        &serde_json::to_value(&target_catalog).expect("target catalog value"),
    );
    let mut target_release = release;
    target_release.evidence_refs[1] = catalog_evidence(&target_catalog, &target_digest);
    let target_error = release_catalog_from_evidence(&target_release)
        .expect_err("target object rebinding must fail");
    assert!(target_error.message.contains("target"));
}

#[test]
fn ontology_sdk_catalog_requires_explicit_name_for_non_ascii_labels() {
    let object = proposal("object", "客户", json!({"object_type": "客户"}));
    let error = build_ontology_release_catalog("commerce", &[object], None)
        .expect_err("non-ASCII implicit names must fail");
    assert!(error.message.contains("explicit ASCII api_name"));
}

#[test]
fn ontology_sdk_catalog_inherits_parent_api_names() {
    let parent_object = proposal(
        "object",
        "Customer",
        json!({
            "object_type": "Customer",
            "api_name": "Customer",
            "primary_key": "display_name",
            "properties": [{"name": "display_name", "api_name": "displayName", "type": "string"}]
        }),
    );
    let parent_order = proposal("object", "Order", json!({"object_type": "Order"}));
    let parent_relation = proposal(
        "relation",
        "Customer places Order",
        json!({
            "from_object": "Customer",
            "relation": "places",
            "to_object": "Order"
        }),
    );
    let parent_action = proposal(
        "action",
        "refund_order",
        json!({
            "action": "refund_order",
            "target_object": "Order",
            "inputs": {"reason": {"type": "text"}},
            "reads": [],
            "effects": [],
            "policy": {"approval_required": true, "transaction_profile": "proposal_only"},
            "transaction_profile": "proposal_only",
            "executor": "local",
            "audit_event": "commerce.refund_order"
        }),
    );
    let parent_action_id = parent_action.id;
    let parent_action_run_id = parent_action.run_id;
    let parent_action_spec =
        ontology_tool_spec_from_action_proposal(parent_action.run_id, &parent_action)
            .expect("parent action tool spec");
    let parent_action_spec = serde_json::to_value(parent_action_spec).expect("parent action JSON");
    let parent_action_digest = normalized_json_sha256(&parent_action_spec);
    let (parent_catalog, parent_digest) = build_ontology_release_catalog(
        "commerce",
        &[parent_object, parent_order, parent_relation, parent_action],
        None,
    )
    .expect("parent");
    let parent = OntologyRelease {
        id: Uuid::new_v4(),
        version: "v1".to_string(),
        domain_scope: "commerce".to_string(),
        source_run_id: Some(parent_action_run_id),
        parent_release_id: None,
        rollback_target_release_id: None,
        status: "active".to_string(),
        release_class: "repo_controlled".to_string(),
        object_count: 2,
        relation_count: 1,
        action_count: 1,
        migration_policy: json!({}),
        gate_result: json!({"status": "passed"}),
        materialized_object_ids: json!([]),
        materialized_link_ids: json!([]),
        evidence_refs: json!([
            {
                "proposal_id": parent_action_id,
                "proposal_type": "action",
                "review_status": "approved",
                "tool_spec": parent_action_spec.clone(),
                "contract_digest": parent_action_digest.clone(),
            },
            catalog_evidence(&parent_catalog, &parent_digest)
        ]),
        promoted_by: Some("test".to_string()),
        promoted_at: Some(Utc::now()),
        rolled_back_by: None,
        rolled_back_at: None,
        archived_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let child_object = proposal("object", "Customer", json!({"object_type": "Customer"}));
    let child_run_id = child_object.run_id;
    let (child_catalog, child_digest) =
        build_ontology_release_catalog("commerce", &[child_object], Some(&parent)).expect("child");
    let child_customer = child_catalog
        .objects
        .iter()
        .find(|object| object.object_type == "Customer")
        .expect("inherited customer");
    assert_eq!(child_customer.api_name, "Customer");
    assert_eq!(child_customer.properties[0].api_name, "displayName");
    assert_eq!(
        child_customer.properties[0].stable_key,
        "object:Customer:property:display_name"
    );
    assert_eq!(
        child_customer.primary_key_api_name.as_deref(),
        Some("displayName")
    );
    assert_eq!(child_catalog.relations.len(), 1);
    assert_eq!(
        child_catalog.relations[0].stable_key,
        parent_catalog.relations[0].stable_key
    );
    assert_eq!(child_catalog.actions.len(), 1);
    assert_eq!(
        child_catalog.actions[0].stable_key,
        parent_catalog.actions[0].stable_key
    );
    let mut child_evidence = Vec::new();
    inherit_parent_action_contract_evidence(Some(&parent), &child_catalog, &mut child_evidence)
        .expect("inherit parent action evidence");
    assert_eq!(
        child_evidence[0]["inherited_from_release_id"],
        json!(parent.id)
    );
    child_evidence.push(catalog_evidence(&child_catalog, &child_digest));
    let child_release = OntologyRelease {
        id: Uuid::new_v4(),
        version: "v2".to_string(),
        domain_scope: "commerce".to_string(),
        source_run_id: Some(child_run_id),
        parent_release_id: Some(parent.id),
        rollback_target_release_id: Some(parent.id),
        status: "candidate".to_string(),
        release_class: "repo_controlled".to_string(),
        object_count: child_catalog.objects.len() as i32,
        relation_count: child_catalog.relations.len() as i32,
        action_count: child_catalog.actions.len() as i32,
        migration_policy: json!({}),
        gate_result: json!({}),
        materialized_object_ids: json!([]),
        materialized_link_ids: json!([]),
        evidence_refs: json!(child_evidence),
        promoted_by: None,
        promoted_at: None,
        rolled_back_by: None,
        rolled_back_at: None,
        archived_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    validate_release_catalog(&child_release).expect("child release catalog");
    ontology_action_tool_spec_for_release(&child_release, &child_catalog.actions[0].runtime_name)
        .expect("inherited action contract belongs to the child release lineage");

    let invalid_primary_key = proposal(
        "object",
        "Customer",
        json!({"object_type": "Customer", "primary_key": "missing_id"}),
    );
    let error = build_ontology_release_catalog("commerce", &[invalid_primary_key], Some(&parent))
        .expect_err("an explicit child primary key must not silently retain the parent key");
    assert!(error.message.contains("primary key missing_id"));

    let renamed_child = proposal(
        "object",
        "Customer",
        json!({
            "object_type": "Customer",
            "properties": [{"name": "display_name", "api_name": "customerName"}]
        }),
    );
    let error = build_ontology_release_catalog("commerce", &[renamed_child], Some(&parent))
        .expect_err("parent property API names are immutable");
    assert!(error.message.contains("property api_name cannot change"));
}

#[test]
fn ontology_sdk_catalog_rejects_case_conflicts_and_reserved_names() {
    let first = proposal(
        "object",
        "Customer",
        json!({"object_type": "Customer", "api_name": "Customer"}),
    );
    let second = proposal(
        "object",
        "Order",
        json!({"object_type": "Order", "api_name": "CUSTOMER"}),
    );
    let error = build_ontology_release_catalog("commerce", &[first, second], None)
        .expect_err("case-insensitive duplicate names must fail");
    assert!(error.message.contains("case-insensitive"));

    let reserved = proposal(
        "object",
        "Class",
        json!({"object_type": "Class", "api_name": "Class"}),
    );
    let error = build_ontology_release_catalog("commerce", &[reserved], None)
        .expect_err("reserved names must fail");
    assert!(error.message.contains("reserved"));
}

#[test]
fn ontology_sdk_subset_rejects_non_proposal_only_actions() {
    let catalog = OntologyReleaseCatalogV1 {
        schema: ONTOLOGY_RELEASE_CATALOG_SCHEMA.to_string(),
        domain_scope: "commerce".to_string(),
        objects: vec![OntologySdkCatalogObject {
            stable_key: "object:Order".to_string(),
            api_name: "Order".to_string(),
            object_type: "Order".to_string(),
            properties: Vec::new(),
            primary_key_api_name: None,
        }],
        relations: Vec::new(),
        actions: vec![OntologySdkCatalogAction {
            stable_key: "action:commerce.update_order".to_string(),
            api_name: "updateOrder".to_string(),
            runtime_name: "commerce.update_order".to_string(),
            contract_digest: "sha256:test".to_string(),
            execution_mode: "local_serializable".to_string(),
            target_object_api_name: "Order".to_string(),
            input_schema: json!({}),
            approval_required: true,
        }],
    };
    let error = normalize_and_validate_subset(
        &catalog,
        &OntologySdkSubsetManifest {
            objects: vec!["Order".to_string()],
            relations: Vec::new(),
            actions: vec!["updateOrder".to_string()],
        },
    )
    .expect_err("non-proposal-only actions must fail");
    assert!(error.message.contains("proposal_only"));
}

#[tokio::test]
async fn ontology_sdk_application_api_persists_release_bound_manifest() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let catalog = OntologyReleaseCatalogV1 {
        schema: ONTOLOGY_RELEASE_CATALOG_SCHEMA.to_string(),
        domain_scope: "commerce".to_string(),
        objects: vec![OntologySdkCatalogObject {
            stable_key: "object:Order".to_string(),
            api_name: "Order".to_string(),
            object_type: "Order".to_string(),
            properties: Vec::new(),
            primary_key_api_name: None,
        }],
        relations: Vec::new(),
        actions: Vec::new(),
    };
    let catalog_digest =
        canonical_json_sha256(&serde_json::to_value(&catalog).expect("catalog value"));
    let release = OntologyRelease {
        id: Uuid::new_v4(),
        version: "commerce-sdk-v1".to_string(),
        domain_scope: "commerce".to_string(),
        source_run_id: None,
        parent_release_id: None,
        rollback_target_release_id: None,
        status: "active".to_string(),
        release_class: "repo_controlled".to_string(),
        object_count: 1,
        relation_count: 0,
        action_count: 0,
        migration_policy: json!({}),
        gate_result: json!({"status": "passed"}),
        materialized_object_ids: json!([]),
        materialized_link_ids: json!([]),
        evidence_refs: json!([catalog_evidence(&catalog, &catalog_digest)]),
        promoted_by: Some("test".to_string()),
        promoted_at: Some(Utc::now()),
        rolled_back_by: None,
        rolled_back_at: None,
        archived_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    state
        .create_ontology_release(release.clone())
        .await
        .expect("release");
    let app = build_router(state.clone());
    let application: OntologySdkApplication = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/ontology-sdk/applications",
            json!({
                "ontology_release_id": release.id,
                "manifest": {"objects": ["Order"]}
            }),
            &[("x-mandoforge-roles", "admin")],
        ),
    )
    .await;
    assert_eq!(application.ontology_release_id, release.id);
    assert_eq!(application.catalog_digest, catalog_digest);
    let manifest: OntologySdkApplicationManifest = request_json(
        app,
        json_request_with_headers(
            "GET",
            &format!("/api/ontology-sdk/applications/{}/manifest", application.id),
            Value::Null,
            &[("x-mandoforge-roles", "admin")],
        ),
    )
    .await;
    assert_eq!(manifest.subset_manifest.objects, vec!["Order".to_string()]);
    assert_eq!(manifest.subset_digest, application.subset_digest);
    assert_eq!(manifest.resolved_catalog.objects.len(), 1);
    assert_eq!(manifest.resolved_catalog.objects[0].api_name, "Order");
}

#[tokio::test]
async fn memory_ontology_sdk_applications_are_tenant_isolated() {
    let state_a = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let mut state_b = state_a.clone();
    state_b.tenant_id = Uuid::new_v4();
    let application = OntologySdkApplication {
        id: Uuid::new_v4(),
        tenant_id: state_a.current_tenant_id(),
        subject: "tenant-a-consumer".to_string(),
        ontology_release_id: Uuid::new_v4(),
        release_version: "tenant-a-v1".to_string(),
        domain_scope: "commerce".to_string(),
        catalog_digest: "sha256:catalog".to_string(),
        subset_manifest: OntologySdkSubsetManifest {
            objects: Vec::new(),
            relations: Vec::new(),
            actions: Vec::new(),
        },
        subset_digest: "sha256:subset".to_string(),
        status: ONTOLOGY_SDK_APPLICATION_STATUS_ACTIVE.to_string(),
        created_at: Utc::now(),
    };
    let audit_log = new_audit_log(
        None,
        "user",
        None,
        "ontology_sdk.application_created",
        "ontology_sdk_application",
        Some(application.id),
        json!({"application_id": application.id}),
    );
    state_a
        .create_ontology_sdk_application(application.clone(), audit_log)
        .await
        .expect("create tenant A application");

    assert!(
        state_b
            .get_ontology_sdk_application(application.id)
            .await
            .is_err()
    );
    assert!(
        state_b
            .list_ontology_sdk_applications(None)
            .await
            .expect("list tenant B applications")
            .is_empty()
    );
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_ontology_sdk_application_round_trips_without_cross_tenant_read() {
    let database_url = std::env::var("MANDOFORGE_TEST_POSTGRES_URL")
        .expect("MANDOFORGE_TEST_POSTGRES_URL is required");
    let bootstrap_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("connect bootstrap postgres");
    run_migrations(&bootstrap_pool)
        .await
        .expect("run migrations");
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    for (tenant_id, label) in [(tenant_a, "a"), (tenant_b, "b")] {
        sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)")
            .bind(tenant_id)
            .bind(format!("OSDK Postgres tenant {label}"))
            .bind(format!("osdk-postgres-{label}-{}", tenant_id.simple()))
            .execute(&bootstrap_pool)
            .await
            .expect("insert tenant");
    }

    let tenant_pool = |tenant_id| {
        let database_url = database_url.clone();
        async move {
            let tenant_setting = format!("SET mandoforge.tenant_id = '{tenant_id}'");
            PgPoolOptions::new()
                .max_connections(2)
                .after_connect(move |connection, _| {
                    let tenant_setting = tenant_setting.clone();
                    Box::pin(async move {
                        connection.execute(tenant_setting.as_str()).await?;
                        Ok(())
                    })
                })
                .connect(&database_url)
                .await
                .expect("connect tenant postgres")
        }
    };
    let pool_a = tenant_pool(tenant_a).await;
    let mut state_a = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state_a.store = StoreBackend::Postgres(pool_a.clone());
    state_a.execution_queue = ExecutionQueue::postgres(pool_a, tenant_a);
    state_a.tenant_id = tenant_a;

    let catalog = OntologyReleaseCatalogV1 {
        schema: ONTOLOGY_RELEASE_CATALOG_SCHEMA.to_string(),
        domain_scope: "commerce".to_string(),
        objects: vec![OntologySdkCatalogObject {
            stable_key: "object:Order".to_string(),
            api_name: "Order".to_string(),
            object_type: "Order".to_string(),
            properties: Vec::new(),
            primary_key_api_name: None,
        }],
        relations: Vec::new(),
        actions: Vec::new(),
    };
    let catalog_digest =
        canonical_json_sha256(&serde_json::to_value(&catalog).expect("catalog value"));
    let release = OntologyRelease {
        id: Uuid::new_v4(),
        version: format!("commerce-postgres-{}", tenant_a.simple()),
        domain_scope: "commerce".to_string(),
        source_run_id: None,
        parent_release_id: None,
        rollback_target_release_id: None,
        status: "active".to_string(),
        release_class: "repo_controlled".to_string(),
        object_count: 1,
        relation_count: 0,
        action_count: 0,
        migration_policy: json!({}),
        gate_result: json!({"status": "passed"}),
        materialized_object_ids: json!([]),
        materialized_link_ids: json!([]),
        evidence_refs: json!([catalog_evidence(&catalog, &catalog_digest)]),
        promoted_by: Some("postgres-test".to_string()),
        promoted_at: Some(Utc::now()),
        rolled_back_by: None,
        rolled_back_at: None,
        archived_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    state_a
        .create_ontology_release(release.clone())
        .await
        .expect("create release");
    let (subset_manifest, subset_digest) = normalize_and_validate_subset(
        &catalog,
        &OntologySdkSubsetManifest {
            objects: vec!["Order".to_string()],
            relations: Vec::new(),
            actions: Vec::new(),
        },
    )
    .expect("valid subset");
    let application = OntologySdkApplication {
        id: Uuid::new_v4(),
        tenant_id: tenant_a,
        subject: "postgres-osdk-subject".to_string(),
        ontology_release_id: release.id,
        release_version: release.version,
        domain_scope: release.domain_scope,
        catalog_digest,
        subset_manifest,
        subset_digest,
        status: ONTOLOGY_SDK_APPLICATION_STATUS_ACTIVE.to_string(),
        created_at: Utc::now(),
    };
    let audit_log = new_audit_log(
        None,
        "user",
        None,
        "ontology_sdk.application_created",
        "ontology_sdk_application",
        Some(application.id),
        json!({"application_id": application.id}),
    );
    let application = state_a
        .create_ontology_sdk_application(application, audit_log)
        .await
        .expect("create application");
    assert_eq!(
        state_a
            .get_ontology_sdk_application(application.id)
            .await
            .expect("read application")
            .subject,
        "postgres-osdk-subject"
    );
    assert!(
        state_a
            .list_audit_logs(None)
            .await
            .expect("audits")
            .iter()
            .any(|audit| audit.action == "ontology_sdk.application_created"
                && audit.resource_id == Some(application.id))
    );

    let rollback_application = OntologySdkApplication {
        id: Uuid::new_v4(),
        subject: "postgres-osdk-rollback-subject".to_string(),
        created_at: Utc::now(),
        ..application.clone()
    };
    let rollback_id = rollback_application.id;
    let invalid_audit = new_audit_log(
        Some(Uuid::new_v4()),
        "user",
        None,
        "ontology_sdk.application_created",
        "ontology_sdk_application",
        Some(rollback_id),
        json!({"application_id": rollback_id}),
    );
    assert!(
        state_a
            .create_ontology_sdk_application(rollback_application, invalid_audit)
            .await
            .is_err()
    );
    assert!(
        state_a
            .get_ontology_sdk_application(rollback_id)
            .await
            .is_err(),
        "application insert must roll back when its audit insert fails"
    );

    let pool_b = tenant_pool(tenant_b).await;
    let mut state_b = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state_b.store = StoreBackend::Postgres(pool_b.clone());
    state_b.execution_queue = ExecutionQueue::postgres(pool_b, tenant_b);
    state_b.tenant_id = tenant_b;
    assert!(
        state_b
            .get_ontology_sdk_application(application.id)
            .await
            .is_err()
    );
}

#[test]
fn ontology_sdk_subset_requires_relation_endpoints_and_proposal_only_actions() {
    let customer = proposal("object", "Customer", json!({"object_type": "Customer"}));
    let order = proposal("object", "Order", json!({"object_type": "Order"}));
    let relation = proposal(
        "relation",
        "Customer places Order",
        json!({
            "from_object": "Customer",
            "relation": "places",
            "to_object": "Order",
            "link_type": "customer_places_order"
        }),
    );
    let (catalog, _) =
        build_ontology_release_catalog("commerce", &[customer, order, relation], None)
            .expect("catalog");
    let error = normalize_and_validate_subset(
        &catalog,
        &OntologySdkSubsetManifest {
            objects: vec!["Customer".to_string()],
            relations: vec!["customerPlacesOrder".to_string()],
            actions: Vec::new(),
        },
    )
    .expect_err("relation endpoint omission must fail");
    assert!(error.message.contains("both endpoint objects"));
}

fn value_for_sdk_property_type(value_type: &str) -> Value {
    match value_type.trim().to_ascii_lowercase().as_str() {
        "integer" | "int" | "int32" | "int64" => json!(1),
        "number" | "decimal" | "float" | "double" => json!(1.5),
        "boolean" | "bool" => json!(true),
        "timestamp" | "datetime" => json!("2026-08-12T00:00:00Z"),
        "date" => json!("2026-08-12"),
        "object" | "json" => json!({"fixture": true}),
        "array" => json!(["fixture"]),
        _ => json!("fixture-value"),
    }
}

fn sdk_fixture_properties(object: &OntologySdkCatalogObject) -> Value {
    let mut properties = serde_json::Map::new();
    for property in &object.properties {
        properties.insert(
            property.source_name.clone(),
            value_for_sdk_property_type(&property.value_type),
        );
    }
    properties.insert("secret_internal".to_string(), json!("must-not-export"));
    Value::Object(properties)
}

#[tokio::test]
async fn ontology_sdk_consumer_http_enforces_subject_subset_visibility_and_proposal_boundary() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let run = create_demo_ontology_onboarding_run_for_test(&state)
        .await
        .expect("demo run");
    for proposal in &run.proposals {
        if matches!(
            proposal.proposal_type.as_str(),
            "object" | "relation" | "action"
        ) {
            review_ontology_onboarding_proposal_for_test(
                &state,
                proposal.id,
                "approve",
                Some("SDK consumer fixture"),
            )
            .await
            .expect("approve fixture proposal");
        }
    }
    materialize_ontology_onboarding_run_for_test(&state, run.id)
        .await
        .expect("materialize fixture");
    let mut release = create_ontology_release_candidate_with_actor(
        &state,
        run.id,
        CreateOntologyReleaseCandidateRequest {
            version: Some("commerce-sdk-consumer-http".to_string()),
            migration_policy: Some(default_ontology_release_migration_policy()),
            release_class: None,
        },
        "test",
    )
    .await
    .expect("candidate fixture");
    gate_ontology_release_with_actor(&state, release.id, "test")
        .await
        .expect("gate fixture");
    release = promote_ontology_release_with_actor(&state, release.id, "test")
        .await
        .expect("promote fixture");
    let (catalog, catalog_digest) = release_catalog_from_evidence(&release).expect("catalog");
    let relation = catalog.relations.first().expect("relation catalog entry");
    let from_object = catalog
        .objects
        .iter()
        .find(|object| object.api_name == relation.from_object_api_name)
        .expect("from object");
    let to_object = catalog
        .objects
        .iter()
        .find(|object| object.api_name == relation.to_object_api_name)
        .expect("to object");
    let action = catalog
        .actions
        .iter()
        .find(|action| action.execution_mode == "proposal_only")
        .expect("proposal-only action");
    let subset = OntologySdkSubsetManifest {
        objects: vec![from_object.api_name.clone(), to_object.api_name.clone()],
        relations: vec![relation.api_name.clone()],
        actions: vec![action.api_name.clone()],
    };
    let app = build_router(state.clone());
    let application: OntologySdkApplication = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/ontology-sdk/applications",
            json!({
                "ontology_release_id": release.id,
                "manifest": subset,
            }),
            &[
                ("x-mandoforge-subject", "consumer-a"),
                ("x-mandoforge-roles", "admin"),
            ],
        ),
    )
    .await;
    assert_eq!(application.catalog_digest, catalog_digest);

    let typescript_response = app
        .clone()
        .oneshot(json_request_with_headers(
            "GET",
            &format!(
                "/api/ontology-sdk/applications/{}/typescript",
                application.id
            ),
            Value::Null,
            &[
                ("x-mandoforge-subject", "consumer-a"),
                ("x-mandoforge-roles", "operator"),
            ],
        ))
        .await
        .expect("typescript request");
    let typescript_status = typescript_response.status();
    let typescript_body = to_bytes(typescript_response.into_body(), usize::MAX)
        .await
        .expect("typescript body");
    let typescript = String::from_utf8(typescript_body.to_vec()).expect("typescript utf8");
    assert_eq!(
        typescript_status,
        StatusCode::OK,
        "typescript: {typescript}"
    );
    let resolved_catalog = resolved_catalog_for_subset(&catalog, &application.subset_manifest)
        .expect("resolved catalog");
    let expected_typescript =
        generate_typescript_sdk(application.id, &resolved_catalog).expect("generated typescript");
    assert_eq!(typescript, expected_typescript);
    let application_literal = format!(
        "export const MANDOFORGE_APPLICATION_ID: string = \"{}\";",
        application.id
    );
    let nil_application_literal = "export const MANDOFORGE_APPLICATION_ID: string = \"00000000-0000-0000-0000-000000000000\";";
    // The bound application id is the only expected difference from the nil-id
    // fixture; do not normalize any other part of the HTTP-generated source.
    assert_eq!(
        typescript.matches(&application_literal).count(),
        1,
        "HTTP-generated source must contain exactly one bound application id literal"
    );
    let fixture_source = typescript.replacen(&application_literal, nil_application_literal, 1);
    assert_ne!(fixture_source, typescript);
    assert_eq!(
        fixture_source,
        include_str!("../../../../examples/mandoforge-osdk-typescript/src/generated/fixture.ts")
    );

    let from = state
        .create_semantic_object(CreateSemanticObject {
            source_id: None,
            object_type: "business_object".to_string(),
            object_key: "sdk-fixture-from".to_string(),
            title: "SDK fixture from".to_string(),
            summary: "visible SDK fixture".to_string(),
            content: json!({
                "object_type": from_object.object_type,
                "domain_scope": "commerce",
                "properties": sdk_fixture_properties(from_object),
            }),
            semantic_scopes: json!({"domain_scope": "commerce"}),
            source_uri: Some("mandoforge://sdk-test/from".to_string()),
            provenance: json!({"source": "sdk-test"}),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "active".to_string(),
        })
        .await
        .expect("from object");
    let to = state
        .create_semantic_object(CreateSemanticObject {
            source_id: None,
            object_type: "business_object".to_string(),
            object_key: "sdk-fixture-to".to_string(),
            title: "SDK fixture hidden endpoint".to_string(),
            summary: "different-domain endpoint".to_string(),
            content: json!({
                "object_type": to_object.object_type,
                "domain_scope": "other-domain",
                "properties": sdk_fixture_properties(to_object),
            }),
            semantic_scopes: json!({"domain_scope": "other-domain"}),
            source_uri: Some("mandoforge://sdk-test/to".to_string()),
            provenance: json!({"source": "sdk-test"}),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "active".to_string(),
        })
        .await
        .expect("to object");
    state
        .create_semantic_link(CreateSemanticLink {
            from_entity_type: "semantic_object".to_string(),
            from_entity_id: from.id.to_string(),
            relation_type: relation.relation_type.clone(),
            to_entity_type: "semantic_object".to_string(),
            to_entity_id: to.id.to_string(),
            metadata: json!({"source": "sdk-test"}),
            provenance: json!({"source": "sdk-test"}),
            confidence: 1.0,
            status: "active".to_string(),
        })
        .await
        .expect("fixture relation");

    let (status, object_body) = request_value(
        app.clone(),
        json_request_with_headers(
            "GET",
            &format!(
                "/api/ontology-sdk/applications/{}/objects/{}",
                application.id, from_object.api_name
            ),
            Value::Null,
            &[
                ("x-mandoforge-subject", "consumer-a"),
                ("x-mandoforge-roles", "operator"),
            ],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let object_values = object_body.as_array().expect("object array");
    assert!(
        object_values
            .iter()
            .any(|object| object["id"] == json!(from.id))
    );
    for object in object_values.iter() {
        let projected_properties = object["properties"]
            .as_object()
            .expect("projected properties");
        assert!(!projected_properties.contains_key("secretInternal"));
    }
    assert!(object_values.iter().any(|object| {
        object["id"] == json!(from.id)
            && object["properties"].as_object().is_some_and(|properties| {
                properties.contains_key(&from_object.properties[0].api_name)
            })
    }));
    assert!(!object_values.iter().any(|object| {
        release
            .materialized_object_ids
            .as_array()
            .is_some_and(|ids| ids.contains(&object["id"]))
    }));

    state
        .create_semantic_object(CreateSemanticObject {
            source_id: None,
            object_type: "business_object".to_string(),
            object_key: "sdk-fixture-malformed".to_string(),
            title: "SDK malformed sibling".to_string(),
            summary: "must not break a valid object lookup".to_string(),
            content: json!({
                "object_type": from_object.object_type,
                "domain_scope": "commerce",
                "properties": {}
            }),
            semantic_scopes: json!({"domain_scope": "commerce"}),
            source_uri: Some("mandoforge://sdk-test/malformed".to_string()),
            provenance: json!({"source": "sdk-test"}),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "active".to_string(),
        })
        .await
        .expect("malformed sibling");
    let (status, object_body) = request_value(
        app.clone(),
        json_request_with_headers(
            "GET",
            &format!(
                "/api/ontology-sdk/applications/{}/objects/{}/{}",
                application.id, from_object.api_name, from.id
            ),
            Value::Null,
            &[
                ("x-mandoforge-subject", "consumer-a"),
                ("x-mandoforge-roles", "operator"),
            ],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(object_body["id"], json!(from.id));

    let (status, _) = request_value(
        app.clone(),
        json_request_with_headers(
            "GET",
            &format!(
                "/api/ontology-sdk/applications/{}/objects/{}",
                application.id, from_object.api_name
            ),
            Value::Null,
            &[
                ("x-mandoforge-subject", "different-subject"),
                ("x-mandoforge-roles", "operator"),
            ],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, relation_body) = request_value(
        app.clone(),
        json_request_with_headers(
            "GET",
            &format!(
                "/api/ontology-sdk/applications/{}/relations?object_id={}",
                application.id, from.id
            ),
            Value::Null,
            &[
                ("x-mandoforge-subject", "consumer-a"),
                ("x-mandoforge-roles", "operator"),
            ],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(relation_body.as_array().is_some_and(Vec::is_empty));

    let action_organization = state
        .create_organization(
            CreateOrganization {
                name: "SDK consumer action organization".to_string(),
                slug: format!("sdk-consumer-action-{}", Uuid::new_v4().simple()),
            },
            Some("admin-1".to_string()),
        )
        .await
        .expect("action organization");
    let action_team = state
        .create_team(
            action_organization.id,
            CreateTeam {
                name: "SDK consumer action team".to_string(),
                slug: format!("sdk-consumer-action-{}", Uuid::new_v4().simple()),
            },
        )
        .await
        .expect("action team");
    state
        .create_provider_access(
            action_team.id,
            CreateProviderAccess {
                provider_name: "openai-compatible".to_string(),
                model_allowlist: vec!["gpt-5.5-mini".to_string()],
            },
        )
        .await
        .expect("action provider access");
    state
        .create_membership(
            action_organization.id,
            CreateMembership {
                user_id: "consumer-a".to_string(),
                team_id: Some(action_team.id),
                project_id: None,
                role: "operator".to_string(),
            },
        )
        .await
        .expect("consumer action membership");

    let agent = state
        .create_agent(CreateAgent {
            name: "SDK consumer action agent".to_string(),
            kind: "specialist".to_string(),
            provider: "openai-compatible".to_string(),
            model: "gpt-5.5-mini".to_string(),
            team_id: Some(action_team.id),
            project_id: None,
            runtime_profile_id: None,
            agent_role: "specialist".to_string(),
            system_prompt: "Use only governed ontology action proposals.".to_string(),
            runtime_config: empty_json_object(),
            tools: vec!["ontology.action.execute".to_string()],
            tool_policy: empty_json_object(),
            mcp_server_ids: Vec::new(),
            skill_ids: Vec::new(),
            workflow_pack_ids: Vec::new(),
            remote_computer_profile: empty_json_object(),
            semantic_scopes: json!({
                "domain_scope": "commerce",
                "workflow_scope": "ontology-sdk-consumer",
                "share_policy": "tenant_only"
            }),
            release_state: "active".to_string(),
        })
        .await
        .expect("action agent");
    let now = Utc::now();
    let definition = state
        .create_workflow_definition(WorkflowDefinition {
            id: Uuid::new_v4(),
            pack_installation_id: None,
            pack_id: None,
            pack_version: None,
            name: "SDK consumer action workflow".to_string(),
            entrypoint: "ontology-sdk-consumer".to_string(),
            trigger_type: "manual".to_string(),
            default_agent_id: agent.id,
            default_environment_id: None,
            input_schema_ref: None,
            output_schema_ref: None,
            step_graph: empty_json_object(),
            handoff_rules: json!({
                "root_task_grant": {
                    "semantic_scopes": {
                        "domain_scope": "commerce",
                        "workflow_scope": "ontology-sdk-consumer",
                        "share_policy": "tenant_only"
                    },
                    "tool_scope": {
                        "read": [],
                        "write": ["ontology.action.execute"],
                        "external_write": []
                    },
                    "approval_policy": {
                        "ontology_consumer_scope": {
                            "actions": [action.api_name]
                        }
                    }
                }
            }),
            execution_strategy: default_workflow_execution_strategy(),
            runtime_adapter: None,
            runtime_mode: None,
            runtime_capability_contract: empty_json_object(),
            event_ingestion_policy: default_event_ingestion_policy(),
            approval_policy_ref: None,
            eval_gate_refs: Vec::new(),
            release_state: "released".to_string(),
            created_at: now,
            updated_at: now,
            archived_at: None,
        })
        .await
        .expect("action definition");
    let workflow_run = create_workflow_run_from_definition(
        &state,
        &definition,
        "SDK consumer action proposal".to_string(),
        empty_json_object(),
        empty_json_object(),
    )
    .await
    .expect("action workflow run");
    let packet = generate_and_persist_context_packet(&state, workflow_run.primary_session_id)
        .await
        .expect("action context packet");
    let grant_id = workflow_run.root_task_grant_id.expect("action grant");
    state
        .update_task_grant_context_packet(grant_id, packet.id)
        .await
        .expect("bind action context packet");
    let before_objects = state.list_semantic_objects().await.expect("objects").len();
    let before_links = state.list_semantic_links().await.expect("links").len();
    let before_tool_calls = state
        .list_tool_calls(Some(workflow_run.primary_session_id))
        .await
        .expect("tool calls")
        .len();
    let before_audits = state.list_audit_logs(None).await.expect("audits").len();
    let action_spec = ontology_action_tool_spec_for_release(&release, &action.runtime_name)
        .expect("action contract")
        .0;
    let parameters = action_spec
        .input_schema
        .get("properties")
        .and_then(Value::as_object)
        .or_else(|| action_spec.input_schema.as_object())
        .map(|properties| {
            properties
                .iter()
                .filter(|(name, _)| *name != "type" && *name != "required")
                .map(|(name, declaration)| {
                    (
                        name.clone(),
                        value_for_sdk_property_type(
                            declaration
                                .get("type")
                                .and_then(Value::as_str)
                                .or_else(|| declaration.as_str())
                                .unwrap_or("string"),
                        ),
                    )
                })
                .collect::<serde_json::Map<_, _>>()
        })
        .map(Value::Object)
        .unwrap_or_else(|| json!({}));

    for role in ["viewer", "approver"] {
        let (status, _) = request_value(
            app.clone(),
            json_request_with_headers(
                "POST",
                &format!(
                    "/api/ontology-sdk/applications/{}/actions/{}",
                    application.id, action.api_name
                ),
                json!({
                    "session_id": workflow_run.primary_session_id,
                    "task_grant_id": grant_id,
                    "context_packet_id": packet.id,
                    "parameters": parameters.clone(),
                }),
                &[
                    ("x-mandoforge-subject", "consumer-a"),
                    ("x-mandoforge-roles", role),
                ],
            ),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "role {role} must not propose"
        );
    }

    let application_b: OntologySdkApplication = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/ontology-sdk/applications",
            json!({
                "ontology_release_id": release.id,
                "manifest": application.subset_manifest.clone(),
            }),
            &[
                ("x-mandoforge-subject", "consumer-b"),
                ("x-mandoforge-roles", "admin"),
            ],
        ),
    )
    .await;
    let (status, _) = request_value(
        app.clone(),
        json_request_with_headers(
            "POST",
            &format!(
                "/api/ontology-sdk/applications/{}/actions/{}",
                application_b.id, action.api_name
            ),
            json!({
                "session_id": workflow_run.primary_session_id,
                "task_grant_id": grant_id,
                "context_packet_id": packet.id,
                "parameters": parameters.clone(),
            }),
            &[
                ("x-mandoforge-subject", "consumer-b"),
                ("x-mandoforge-roles", "operator"),
            ],
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an app-bound subject without session/team visibility must not borrow the grant"
    );

    let (status, action_body) = request_value(
        app.clone(),
        json_request_with_headers(
            "POST",
            &format!(
                "/api/ontology-sdk/applications/{}/actions/{}",
                application.id, action.api_name
            ),
            json!({
                "session_id": workflow_run.primary_session_id,
                "task_grant_id": grant_id,
                "context_packet_id": packet.id,
                "parameters": parameters,
            }),
            &[
                ("x-mandoforge-subject", "consumer-a"),
                ("x-mandoforge-roles", "operator"),
            ],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "action body: {action_body}");
    assert!(matches!(
        action_body.get("status").and_then(Value::as_str),
        Some("approval_required") | Some("proposal_created")
    ));
    assert_eq!(
        state.list_semantic_objects().await.expect("objects").len(),
        before_objects
    );
    assert_eq!(
        state.list_semantic_links().await.expect("links").len(),
        before_links
    );
    assert!(
        state
            .list_tool_calls(Some(workflow_run.primary_session_id))
            .await
            .expect("tool calls")
            .len()
            > before_tool_calls
    );
    assert!(state.list_audit_logs(None).await.expect("audits").len() > before_audits);

    let (status, _) = request_value(
        build_router(state),
        json_request_with_headers(
            "GET",
            &format!(
                "/api/ontology-sdk/applications/{}/objects/{}?task_grant_id={}",
                application.id, from_object.api_name, grant_id
            ),
            Value::Null,
            &[
                ("x-mandoforge-subject", "consumer-a"),
                ("x-mandoforge-roles", "operator"),
            ],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
