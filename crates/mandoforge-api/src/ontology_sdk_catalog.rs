use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    AppError, ONTOLOGY_RELEASE_CATALOG_SCHEMA, OntologyOnboardingProposalDraft,
    OntologyOnboardingToolSpec, OntologyRelease, OntologyReleaseCatalogV1,
    OntologySdkCatalogAction, OntologySdkCatalogObject, OntologySdkCatalogProperty,
    OntologySdkCatalogRelation, OntologySdkSubsetManifest, normalized_json_sha256,
    ontology_tool_spec_from_action_proposal,
};

#[derive(Debug, Clone)]
struct ParentNames {
    objects: BTreeMap<String, String>,
    relations: BTreeMap<String, String>,
    actions: BTreeMap<String, String>,
}

pub(crate) fn build_ontology_release_catalog(
    domain_scope: &str,
    proposals: &[OntologyOnboardingProposalDraft],
    parent_release: Option<&OntologyRelease>,
) -> Result<(OntologyReleaseCatalogV1, String), AppError> {
    let parent_catalog = match parent_release {
        Some(release)
            if release.evidence_refs.as_array().is_some_and(|entries| {
                entries.iter().any(|entry| {
                    entry.get("schema").and_then(Value::as_str)
                        == Some(ONTOLOGY_RELEASE_CATALOG_SCHEMA)
                })
            }) =>
        {
            Some(release_catalog_from_evidence(release)?.0)
        }
        _ => None,
    };
    let parent_names = parent_catalog
        .as_ref()
        .map(parent_catalog_names)
        .unwrap_or_else(|| ParentNames {
            objects: BTreeMap::new(),
            relations: BTreeMap::new(),
            actions: BTreeMap::new(),
        });

    let mut object_identity_to_api = BTreeMap::new();
    let mut objects = parent_catalog
        .as_ref()
        .map(|catalog| {
            catalog
                .objects
                .iter()
                .cloned()
                .map(|object| (object.stable_key.clone(), object))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    for object in objects.values() {
        object_identity_to_api.insert(object.object_type.clone(), object.api_name.clone());
    }
    let mut proposal_object_identities = BTreeSet::new();
    for proposal in proposals
        .iter()
        .filter(|proposal| proposal.proposal_type == "object")
    {
        let object_type = proposal
            .content
            .get("object_type")
            .and_then(Value::as_str)
            .unwrap_or(&proposal.name)
            .trim()
            .to_string();
        let stable_key = format!("object:{object_type}");
        if !proposal_object_identities.insert(stable_key.clone()) {
            return Err(AppError::bad_request(
                "ontology release proposals contain duplicate object identities",
            ));
        }
        let api_name = choose_api_name(
            proposal,
            explicit_api_name(proposal),
            parent_names.objects.get(&stable_key),
            &object_type,
            ApiNameKind::Object,
        )?;
        let parent_object = parent_catalog.as_ref().and_then(|catalog| {
            catalog
                .objects
                .iter()
                .find(|object| object.stable_key == stable_key)
        });
        let (properties, primary_key_api_name) =
            catalog_object_properties(proposal, &stable_key, parent_object)?;
        object_identity_to_api.insert(object_type.clone(), api_name.clone());
        objects.insert(
            stable_key.clone(),
            OntologySdkCatalogObject {
                stable_key,
                api_name,
                object_type,
                properties,
                primary_key_api_name,
            },
        );
    }

    let mut relations = parent_catalog
        .as_ref()
        .map(|catalog| {
            catalog
                .relations
                .iter()
                .cloned()
                .map(|relation| (relation.stable_key.clone(), relation))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let mut proposal_relation_identities = BTreeSet::new();
    for proposal in proposals
        .iter()
        .filter(|proposal| proposal.proposal_type == "relation")
    {
        let from_object = required_content_string(proposal, "from_object", "relation")?;
        let to_object = required_content_string(proposal, "to_object", "relation")?;
        let relation_type = required_content_string(proposal, "relation", "relation")?;
        let from_object_api_name = object_identity_to_api
            .get(&from_object)
            .cloned()
            .ok_or_else(|| {
                AppError::bad_request(format!(
                    "ontology relation endpoint object is not in the release catalog: {from_object}"
                ))
            })?;
        let to_object_api_name = object_identity_to_api.get(&to_object).cloned().ok_or_else(
            || {
                AppError::bad_request(format!(
                    "ontology relation endpoint object is not in the release catalog: {to_object}"
                ))
            },
        )?;
        let stable_key = format!("relation:{from_object}:{relation_type}:{to_object}");
        if !proposal_relation_identities.insert(stable_key.clone()) {
            return Err(AppError::bad_request(
                "ontology release proposals contain duplicate relation identities",
            ));
        }
        let fallback = proposal
            .content
            .get("link_type")
            .and_then(Value::as_str)
            .unwrap_or(&proposal.name);
        let api_name = choose_api_name(
            proposal,
            explicit_api_name(proposal),
            parent_names.relations.get(&stable_key),
            fallback,
            ApiNameKind::Relation,
        )?;
        relations.insert(
            stable_key.clone(),
            OntologySdkCatalogRelation {
                stable_key,
                api_name,
                from_object_api_name,
                relation_type,
                to_object_api_name,
            },
        );
    }

    let mut actions = parent_catalog
        .as_ref()
        .map(|catalog| {
            catalog
                .actions
                .iter()
                .cloned()
                .map(|action| (action.stable_key.clone(), action))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let mut proposal_action_identities = BTreeSet::new();
    for proposal in proposals
        .iter()
        .filter(|proposal| proposal.proposal_type == "action")
    {
        let tool_spec = ontology_tool_spec_from_action_proposal(proposal.run_id, proposal)?;
        let tool_spec_value = serde_json::to_value(&tool_spec).map_err(|error| {
            AppError::bad_request(format!(
                "failed to serialize ontology action contract: {error}"
            ))
        })?;
        let contract_digest = normalized_json_sha256(&tool_spec_value);
        let stable_key = format!("action:{}", tool_spec.name);
        if !proposal_action_identities.insert(stable_key.clone()) {
            return Err(AppError::bad_request(
                "ontology release proposals contain duplicate action identities",
            ));
        }
        let api_name = choose_api_name(
            proposal,
            explicit_api_name(proposal),
            parent_names.actions.get(&stable_key),
            &proposal.name,
            ApiNameKind::Action,
        )?;
        let target_object_api_name = object_identity_to_api
            .get(&tool_spec.target_object)
            .cloned()
            .ok_or_else(|| {
                AppError::bad_request(format!(
                    "ontology action target object is not in the release catalog: {}",
                    tool_spec.target_object
                ))
            })?;
        actions.insert(
            stable_key.clone(),
            OntologySdkCatalogAction {
                stable_key,
                api_name,
                runtime_name: tool_spec.name,
                contract_digest,
                execution_mode: tool_spec.execution_mode,
                target_object_api_name,
                input_schema: tool_spec.input_schema,
                approval_required: tool_spec.approval_required,
            },
        );
    }

    let mut objects = objects.into_values().collect::<Vec<_>>();
    let mut relations = relations.into_values().collect::<Vec<_>>();
    let mut actions = actions.into_values().collect::<Vec<_>>();
    objects.sort_by(|left, right| left.api_name.cmp(&right.api_name));
    relations.sort_by(|left, right| left.api_name.cmp(&right.api_name));
    actions.sort_by(|left, right| left.api_name.cmp(&right.api_name));
    let catalog = OntologyReleaseCatalogV1 {
        schema: ONTOLOGY_RELEASE_CATALOG_SCHEMA.to_string(),
        domain_scope: domain_scope.to_string(),
        objects,
        relations,
        actions,
    };
    validate_catalog(&catalog)?;
    let digest = canonical_json_sha256(&serde_json::to_value(&catalog).map_err(|error| {
        AppError::bad_request(format!(
            "failed to serialize ontology release catalog: {error}"
        ))
    })?);
    Ok((catalog, digest))
}

pub(crate) fn catalog_evidence(catalog: &OntologyReleaseCatalogV1, digest: &str) -> Value {
    json!({
        "schema": ONTOLOGY_RELEASE_CATALOG_SCHEMA,
        "snapshot": catalog,
        "digest": digest,
    })
}

pub(crate) fn inherit_parent_action_contract_evidence(
    parent_release: Option<&OntologyRelease>,
    catalog: &OntologyReleaseCatalogV1,
    evidence_refs: &mut Vec<Value>,
) -> Result<(), AppError> {
    let Some(parent_release) = parent_release else {
        return Ok(());
    };
    let parent_evidence = parent_release.evidence_refs.as_array().ok_or_else(|| {
        AppError::forbidden("parent ontology release action contract evidence is missing")
    })?;
    for action in &catalog.actions {
        if evidence_refs.iter().any(|evidence| {
            evidence["tool_spec"]["name"].as_str() == Some(action.runtime_name.as_str())
        }) {
            continue;
        }
        let mut inherited = parent_evidence
            .iter()
            .find(|evidence| {
                evidence["tool_spec"]["name"].as_str() == Some(action.runtime_name.as_str())
            })
            .cloned()
            .ok_or_else(|| {
                AppError::forbidden("parent ontology release action contract is missing")
            })?;
        inherited["inherited_from_release_id"] = json!(parent_release.id);
        evidence_refs.push(inherited);
    }
    Ok(())
}

pub(crate) fn release_catalog_from_evidence(
    release: &OntologyRelease,
) -> Result<(OntologyReleaseCatalogV1, String), AppError> {
    let evidence_refs = release
        .evidence_refs
        .as_array()
        .ok_or_else(|| AppError::forbidden("ontology release catalog evidence is missing"))?;
    let entries = evidence_refs
        .iter()
        .filter(|entry| {
            entry.get("schema").and_then(Value::as_str) == Some(ONTOLOGY_RELEASE_CATALOG_SCHEMA)
        })
        .collect::<Vec<_>>();
    if entries.len() != 1 {
        return Err(AppError::forbidden(
            "ontology release catalog snapshot is missing or ambiguous",
        ));
    }
    let entry = entries[0];
    let catalog = serde_json::from_value::<OntologyReleaseCatalogV1>(
        entry
            .get("snapshot")
            .cloned()
            .ok_or_else(|| AppError::forbidden("ontology release catalog snapshot is missing"))?,
    )
    .map_err(|error| {
        AppError::forbidden(format!("ontology release catalog is invalid: {error}"))
    })?;
    if catalog.domain_scope != release.domain_scope {
        return Err(AppError::forbidden(
            "ontology release catalog domain does not match the release",
        ));
    }
    if catalog.objects.len() != release.object_count.max(0) as usize
        || catalog.relations.len() != release.relation_count.max(0) as usize
        || catalog.actions.len() != release.action_count.max(0) as usize
    {
        return Err(AppError::forbidden(
            "ontology release catalog counts do not match the release",
        ));
    }
    let digest = entry
        .get("digest")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::forbidden("ontology release catalog digest is missing"))?
        .to_string();
    let calculated = canonical_json_sha256(&serde_json::to_value(&catalog).map_err(|error| {
        AppError::forbidden(format!(
            "ontology release catalog cannot be serialized: {error}"
        ))
    })?);
    if digest != calculated {
        return Err(AppError::forbidden(
            "ontology release catalog digest does not match its snapshot",
        ));
    }
    validate_catalog(&catalog)?;
    validate_action_contract_digests(release, &catalog)?;
    Ok((catalog, digest))
}

pub(crate) fn validate_release_catalog(release: &OntologyRelease) -> Result<(), AppError> {
    release_catalog_from_evidence(release).map(|_| ())
}

pub(crate) fn normalize_and_validate_subset(
    catalog: &OntologyReleaseCatalogV1,
    subset: &OntologySdkSubsetManifest,
) -> Result<(OntologySdkSubsetManifest, String), AppError> {
    let mut normalized = subset.clone();
    normalize_names(&mut normalized.objects, "object")?;
    normalize_names(&mut normalized.relations, "relation")?;
    normalize_names(&mut normalized.actions, "action")?;
    let object_names = catalog
        .objects
        .iter()
        .map(|entry| entry.api_name.as_str())
        .collect::<BTreeSet<_>>();
    let relation_map = catalog
        .relations
        .iter()
        .map(|entry| (entry.api_name.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let action_map = catalog
        .actions
        .iter()
        .map(|entry| (entry.api_name.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    for name in &normalized.objects {
        if !object_names.contains(name.as_str()) {
            return Err(AppError::bad_request(format!(
                "ontology SDK subset references an unknown object: {name}"
            )));
        }
    }
    for name in &normalized.relations {
        let relation = relation_map.get(name.as_str()).ok_or_else(|| {
            AppError::bad_request(format!(
                "ontology SDK subset references an unknown relation: {name}"
            ))
        })?;
        if !normalized
            .objects
            .iter()
            .any(|object| object == &relation.from_object_api_name)
            || !normalized
                .objects
                .iter()
                .any(|object| object == &relation.to_object_api_name)
        {
            return Err(AppError::bad_request(format!(
                "ontology SDK subset relation {name} requires both endpoint objects"
            )));
        }
    }
    for name in &normalized.actions {
        let action = action_map.get(name.as_str()).ok_or_else(|| {
            AppError::bad_request(format!(
                "ontology SDK subset references an unknown action: {name}"
            ))
        })?;
        if action.execution_mode != "proposal_only" {
            return Err(AppError::forbidden(format!(
                "ontology SDK subset action {name} is not proposal_only"
            )));
        }
    }
    let value = serde_json::to_value(&normalized).map_err(|error| {
        AppError::bad_request(format!("failed to serialize ontology SDK subset: {error}"))
    })?;
    Ok((normalized, canonical_json_sha256(&value)))
}

pub(crate) fn resolved_catalog_for_subset(
    catalog: &OntologyReleaseCatalogV1,
    subset: &OntologySdkSubsetManifest,
) -> Result<OntologyReleaseCatalogV1, AppError> {
    let (subset, _) = normalize_and_validate_subset(catalog, subset)?;
    let objects = subset
        .objects
        .iter()
        .filter_map(|name| catalog.objects.iter().find(|entry| entry.api_name == *name))
        .cloned()
        .collect::<Vec<_>>();
    let relations = subset
        .relations
        .iter()
        .filter_map(|name| {
            catalog
                .relations
                .iter()
                .find(|entry| entry.api_name == *name)
        })
        .cloned()
        .collect::<Vec<_>>();
    let actions = subset
        .actions
        .iter()
        .filter_map(|name| catalog.actions.iter().find(|entry| entry.api_name == *name))
        .cloned()
        .collect::<Vec<_>>();
    Ok(OntologyReleaseCatalogV1 {
        schema: catalog.schema.clone(),
        domain_scope: catalog.domain_scope.clone(),
        objects,
        relations,
        actions,
    })
}

fn parent_catalog_names(catalog: &OntologyReleaseCatalogV1) -> ParentNames {
    ParentNames {
        objects: catalog
            .objects
            .iter()
            .map(|entry| (entry.stable_key.clone(), entry.api_name.clone()))
            .collect(),
        relations: catalog
            .relations
            .iter()
            .map(|entry| (entry.stable_key.clone(), entry.api_name.clone()))
            .collect(),
        actions: catalog
            .actions
            .iter()
            .map(|entry| (entry.stable_key.clone(), entry.api_name.clone()))
            .collect(),
    }
}

#[derive(Debug, Clone, Copy)]
enum ApiNameKind {
    Object,
    Property,
    Relation,
    Action,
}

fn choose_api_name(
    proposal: &OntologyOnboardingProposalDraft,
    explicit: Option<&str>,
    inherited: Option<&String>,
    fallback: &str,
    kind: ApiNameKind,
) -> Result<String, AppError> {
    if let Some(inherited) = inherited {
        let inherited = validate_api_name(inherited.clone(), kind)?;
        if let Some(explicit) = explicit
            .map(str::trim)
            .filter(|value| !value.is_empty() && !value.contains('.'))
            && explicit != inherited
        {
            return Err(AppError::bad_request(
                "ontology catalog api_name cannot change an inherited parent release name",
            ));
        }
        return Ok(inherited);
    }
    if let Some(explicit) = explicit
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.contains('.'))
    {
        return validate_api_name(explicit.to_string(), kind);
    }
    if !fallback.is_ascii() {
        return Err(AppError::bad_request(format!(
            "ontology {} requires an explicit ASCII api_name when its label is non-ASCII",
            proposal.proposal_type
        )));
    }
    validate_api_name(camel_api_name(fallback, kind)?, kind)
}

fn explicit_api_name(proposal: &OntologyOnboardingProposalDraft) -> Option<&str> {
    proposal
        .content
        .get("api_name")
        .or_else(|| proposal.content.get("apiName"))
        .and_then(Value::as_str)
        .or_else(|| proposal.evidence.get("api_name").and_then(Value::as_str))
        .or_else(|| proposal.evidence.get("apiName").and_then(Value::as_str))
}

fn required_content_string(
    proposal: &OntologyOnboardingProposalDraft,
    key: &str,
    kind: &str,
) -> Result<String, AppError> {
    proposal
        .content
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| AppError::bad_request(format!("{kind} proposal missing {key}")))
}

fn camel_api_name(value: &str, kind: ApiNameKind) -> Result<String, AppError> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if character.is_ascii_uppercase()
                && !token.is_empty()
                && token
                    .chars()
                    .last()
                    .is_some_and(|last| last.is_ascii_lowercase())
            {
                tokens.push(std::mem::take(&mut token));
            }
            token.push(character.to_ascii_lowercase());
        } else if !token.is_empty() {
            tokens.push(std::mem::take(&mut token));
        }
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    if tokens.is_empty() {
        return Err(AppError::bad_request(
            "ontology catalog api_name cannot be derived from an empty label",
        ));
    }
    let mut result = String::new();
    for (index, token) in tokens.iter().enumerate() {
        if index == 0 && matches!(kind, ApiNameKind::Object) {
            let mut chars = token.chars();
            if let Some(first) = chars.next() {
                result.push(first.to_ascii_uppercase());
                result.extend(chars);
            }
        } else if index == 0 {
            result.push_str(token);
        } else {
            let mut chars = token.chars();
            if let Some(first) = chars.next() {
                result.push(first.to_ascii_uppercase());
                result.extend(chars);
            }
        }
    }
    Ok(result)
}

fn validate_api_name(name: String, kind: ApiNameKind) -> Result<String, AppError> {
    if name.is_empty()
        || !name.is_ascii()
        || !name.as_bytes()[0].is_ascii_alphabetic()
        || name.len() > 64
        || name
            .bytes()
            .any(|character| !character.is_ascii_alphanumeric())
        || (matches!(kind, ApiNameKind::Object) && !name.as_bytes()[0].is_ascii_uppercase())
        || (matches!(
            kind,
            ApiNameKind::Property | ApiNameKind::Relation | ApiNameKind::Action
        ) && !name.as_bytes()[0].is_ascii_lowercase())
    {
        return Err(AppError::bad_request(
            "ontology catalog api_name must use the strict ASCII v1 casing and character rules",
        ));
    }
    const RESERVED: &[&str] = &[
        "as",
        "await",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "function",
        "if",
        "implements",
        "import",
        "in",
        "instanceof",
        "interface",
        "let",
        "new",
        "null",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "static",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "typeof",
        "undefined",
        "var",
        "void",
        "while",
        "with",
        "yield",
        "any",
        "boolean",
        "never",
        "number",
        "object",
        "readonly",
        "string",
        "symbol",
        "unknown",
        "keyof",
        "declare",
        "namespace",
        "module",
        "type",
        "satisfies",
    ];
    if RESERVED.contains(&name.to_ascii_lowercase().as_str()) {
        return Err(AppError::bad_request(format!(
            "ontology catalog api_name is reserved: {name}"
        )));
    }
    Ok(name)
}

fn catalog_object_properties(
    proposal: &OntologyOnboardingProposalDraft,
    object_stable_key: &str,
    parent_object: Option<&OntologySdkCatalogObject>,
) -> Result<(Vec<OntologySdkCatalogProperty>, Option<String>), AppError> {
    let primary_key_source = match proposal
        .content
        .get("primary_key_api_name")
        .or_else(|| proposal.content.get("primary_key"))
    {
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.trim()),
        Some(_) => {
            return Err(AppError::bad_request(
                "ontology object primary key must be a non-empty string",
            ));
        }
        None => None,
    };
    let property_source = proposal
        .content
        .get("properties")
        .or_else(|| proposal.content.get("fields"))
        .or_else(|| {
            proposal
                .content
                .get("schema")
                .and_then(|schema| schema.get("properties").or_else(|| schema.get("fields")))
        });
    let mut entries = Vec::<(Option<String>, &Value)>::new();
    let mut properties = Vec::new();
    match property_source {
        Some(Value::Array(values)) => {
            entries.extend(values.iter().map(|value| (None, value)));
        }
        Some(Value::Object(values)) => {
            entries.extend(
                values
                    .iter()
                    .map(|(name, value)| (Some(name.clone()), value)),
            );
        }
        Some(_) => {
            return Err(AppError::bad_request(
                "ontology object properties/fields must be an array or object",
            ));
        }
        None if parent_object.is_some() => {
            properties = parent_object
                .expect("guarded parent object")
                .properties
                .clone();
        }
        None => {}
    }
    for (map_name, value) in entries {
        let source_name = value
            .get("source_name")
            .or_else(|| value.get("name"))
            .or_else(|| value.get("field_name"))
            .and_then(Value::as_str)
            .or(map_name.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                AppError::bad_request(
                    "ontology object property is missing a source_name/name; catalog cannot fabricate it",
                )
            })?
            .to_string();
        let explicit_api = value
            .get("api_name")
            .or_else(|| value.get("apiName"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty() && !value.contains('.'));
        let stable_key = format!("{object_stable_key}:property:{source_name}");
        let inherited = parent_object.and_then(|parent| {
            parent.properties.iter().find(|property| {
                property.source_name == source_name || property.stable_key == stable_key
            })
        });
        let api_name = if let Some(parent_property) = inherited {
            let inherited_api =
                validate_api_name(parent_property.api_name.clone(), ApiNameKind::Property)?;
            if explicit_api.is_some_and(|explicit| explicit != inherited_api) {
                return Err(AppError::bad_request(
                    "ontology object property api_name cannot change an inherited parent release name",
                ));
            }
            inherited_api
        } else if let Some(explicit_api) = explicit_api {
            validate_api_name(explicit_api.to_string(), ApiNameKind::Property)?
        } else {
            if !source_name.is_ascii() {
                return Err(AppError::bad_request(format!(
                    "ontology object property {source_name} requires an explicit ASCII api_name"
                )));
            }
            validate_api_name(
                camel_api_name(&source_name, ApiNameKind::Property)?,
                ApiNameKind::Property,
            )?
        };
        let value_type = value
            .get("value_type")
            .or_else(|| value.get("type"))
            .or_else(|| value.get("field_type"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("unknown")
            .to_string();
        let nullable = value
            .get("nullable")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        properties.push(OntologySdkCatalogProperty {
            stable_key,
            source_name,
            api_name,
            value_type,
            nullable,
        });
    }
    properties.sort_by(|left, right| left.api_name.cmp(&right.api_name));
    for pair in properties.windows(2) {
        if pair[0].api_name.eq_ignore_ascii_case(&pair[1].api_name) {
            return Err(AppError::bad_request(
                "ontology object properties have a case-insensitive api_name conflict",
            ));
        }
    }
    let primary_key_api_name = match primary_key_source {
        Some(primary_key) => Some(
            properties
                .iter()
                .find(|property| {
                    property.source_name == primary_key || property.api_name == primary_key
                })
                .ok_or_else(|| {
                    AppError::bad_request(format!(
                        "ontology object primary key {primary_key} is not a declared property"
                    ))
                })?
                .api_name
                .clone(),
        ),
        None => parent_object.and_then(|parent| parent.primary_key_api_name.clone()),
    };
    Ok((properties, primary_key_api_name))
}

pub(crate) fn ontology_catalog_property_type(value_type: &str) -> Option<&'static str> {
    match value_type.trim().to_ascii_lowercase().as_str() {
        "unknown" | "" => Some("unknown"),
        "string" | "text" => Some("string"),
        "uuid" => Some("uuid"),
        "integer" | "int" | "int32" | "int64" => Some("integer"),
        "number" | "decimal" | "float" | "double" => Some("number"),
        "boolean" | "bool" => Some("boolean"),
        "object" => Some("object"),
        "json" => Some("json"),
        "array" => Some("array"),
        "date" => Some("date"),
        "timestamp" | "datetime" => Some("timestamp"),
        _ => None,
    }
}

pub(crate) fn ontology_action_schema_type_supported(value_type: &str) -> bool {
    matches!(
        value_type.trim().to_ascii_lowercase().as_str(),
        "string"
            | "text"
            | "uuid"
            | "date"
            | "timestamp"
            | "datetime"
            | "integer"
            | "int"
            | "int32"
            | "int64"
            | "number"
            | "decimal"
            | "float"
            | "double"
            | "boolean"
            | "bool"
            | "object"
            | "json"
            | "array"
            | "null"
    )
}

pub(crate) fn ontology_action_schema_value_matches_type(value: &Value, expected: &str) -> bool {
    match expected.trim().to_ascii_lowercase().as_str() {
        "string" | "text" => value.is_string(),
        "uuid" => value
            .as_str()
            .is_some_and(|value| uuid::Uuid::parse_str(value).is_ok()),
        "date" => value
            .as_str()
            .is_some_and(|value| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()),
        "timestamp" | "datetime" => value
            .as_str()
            .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_ok()),
        "integer" | "int" | "int32" | "int64" => ontology_action_schema_integer(value),
        "number" | "decimal" | "float" | "double" => value.is_number(),
        "boolean" | "bool" => value.is_boolean(),
        "object" => value.is_object(),
        "json" => true,
        "array" => value.is_array(),
        "null" => value.is_null(),
        _ => false,
    }
}

fn ontology_action_schema_integer(value: &Value) -> bool {
    if value.as_i64().is_some() || value.as_u64().is_some() {
        return true;
    }
    value.as_f64().is_some_and(|number| {
        number.is_finite()
            && number.fract() == 0.0
            && (((-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&number)
                && (number as i64) as f64 == number)
                || ((0.0..18_446_744_073_709_551_616.0).contains(&number)
                    && (number as u64) as f64 == number))
    })
}

fn validate_action_schema(schema: &Value) -> Result<(), AppError> {
    let Some(object) = schema.as_object() else {
        return Ok(());
    };
    let is_object_schema = match object.get("type").and_then(Value::as_str) {
        Some(value_type) if value_type.eq_ignore_ascii_case("object") => true,
        Some(_) => {
            return Err(AppError::forbidden(
                "ontology release catalog action input schema top-level type must be object",
            ));
        }
        None => object.get("properties").is_some(),
    };
    if is_object_schema {
        if object.get("properties").is_none() {
            return Err(AppError::forbidden(
                "ontology release catalog action top-level object input schema must declare properties",
            ));
        }
        validate_action_schema_node(schema, false)?;
    } else {
        for declaration in object.values() {
            validate_action_schema_node(declaration, true)?;
        }
    }
    Ok(())
}

fn validate_action_schema_node(
    schema: &Value,
    shorthand_declaration: bool,
) -> Result<(), AppError> {
    if let Some(value_type) = schema.as_str() {
        return if ontology_action_schema_type_supported(value_type) {
            Ok(())
        } else {
            Err(AppError::forbidden(format!(
                "ontology release catalog action input schema type {value_type} is unsupported"
            )))
        };
    }
    let Some(object) = schema.as_object() else {
        return Err(AppError::forbidden(
            "ontology release catalog action input schema declaration must be an object or type name",
        ));
    };
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if object.contains_key(keyword) {
            return Err(AppError::forbidden(format!(
                "ontology release catalog action input schema keyword {keyword} is unsupported"
            )));
        }
    }
    let value_type = match object.get("type") {
        Some(Value::String(value_type)) if ontology_action_schema_type_supported(value_type) => {
            Some(value_type.as_str())
        }
        Some(Value::String(value_type)) => {
            return Err(AppError::forbidden(format!(
                "ontology release catalog action input schema type {value_type} is unsupported"
            )));
        }
        Some(_) => {
            return Err(AppError::forbidden(
                "ontology release catalog action input schema type must be a string",
            ));
        }
        None if object.get("properties").is_some() => Some("object"),
        None => None,
    };
    if let Some(values) = object.get("enum") {
        let values = values.as_array().ok_or_else(|| {
            AppError::forbidden(
                "ontology release catalog action input schema enum must be an array",
            )
        })?;
        if values.is_empty() {
            return Err(AppError::forbidden(
                "ontology release catalog action input schema enum must not be empty",
            ));
        }
        if let Some(value_type) = value_type
            && values
                .iter()
                .any(|value| !ontology_action_schema_value_matches_type(value, value_type))
        {
            return Err(AppError::forbidden(format!(
                "ontology release catalog action input schema enum member must be {value_type}"
            )));
        }
        if value_type.is_some_and(|value_type| {
            matches!(
                value_type.trim().to_ascii_lowercase().as_str(),
                "object" | "array"
            )
        }) && values
            .iter()
            .any(|value| !ontology_action_schema_value_matches_declaration(value, object))
        {
            return Err(AppError::forbidden(
                "ontology release catalog action input schema enum member does not satisfy its declaration",
            ));
        }
    }
    if let Some(properties_value) = object.get("properties") {
        let properties = properties_value.as_object().ok_or_else(|| {
            AppError::forbidden(
                "ontology release catalog action input schema properties must be an object",
            )
        })?;
        for property in properties.values() {
            validate_action_schema_node(property, false)?;
        }
        if let Some(required) = object.get("required")
            && !(shorthand_declaration && required.is_boolean())
        {
            let required = required.as_array().ok_or_else(|| {
                AppError::forbidden(
                    "ontology release catalog action input schema required must be an array",
                )
            })?;
            for name in required {
                let name = name.as_str().ok_or_else(|| {
                    AppError::forbidden(
                        "ontology release catalog action input schema required names must be strings",
                    )
                })?;
                if !properties.contains_key(name) {
                    return Err(AppError::forbidden(format!(
                        "ontology release catalog action input schema required field {name} is undeclared"
                    )));
                }
            }
        }
    }
    if let Some(items) = object.get("items") {
        validate_action_schema_node(items, false)?;
    }
    if value_type.is_none() && object.get("enum").is_none() {
        return Err(AppError::forbidden(
            "ontology release catalog action input schema declaration has no supported type",
        ));
    }
    Ok(())
}

fn ontology_action_schema_value_matches_node(value: &Value, schema: &Value) -> bool {
    match schema {
        Value::String(value_type) => ontology_action_schema_value_matches_type(value, value_type),
        Value::Object(declaration) => {
            ontology_action_schema_value_matches_declaration(value, declaration)
        }
        _ => false,
    }
}

fn ontology_action_schema_value_matches_declaration(
    value: &Value,
    declaration: &serde_json::Map<String, Value>,
) -> bool {
    let value_type = declaration
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| declaration.get("properties").map(|_| "object"));
    if value_type
        .is_some_and(|value_type| !ontology_action_schema_value_matches_type(value, value_type))
    {
        return false;
    }
    if declaration
        .get("enum")
        .and_then(Value::as_array)
        .is_some_and(|values| !crate::json_schema_enum_contains(values, value))
    {
        return false;
    }
    match value_type.map(|value_type| value_type.trim().to_ascii_lowercase()) {
        Some(value_type) if value_type == "object" => {
            let Some(value) = value.as_object() else {
                return false;
            };
            let Some(properties) = declaration.get("properties").and_then(Value::as_object) else {
                return true;
            };
            if value.keys().any(|name| !properties.contains_key(name)) {
                return false;
            }
            if declaration
                .get("required")
                .and_then(Value::as_array)
                .is_some_and(|required| {
                    required
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|name| !value.contains_key(name))
                })
            {
                return false;
            }
            value.iter().all(|(name, value)| {
                properties
                    .get(name)
                    .is_some_and(|schema| ontology_action_schema_value_matches_node(value, schema))
            })
        }
        Some(value_type) if value_type == "array" => declaration.get("items").is_none_or(|items| {
            value.as_array().is_some_and(|values| {
                values
                    .iter()
                    .all(|value| ontology_action_schema_value_matches_node(value, items))
            })
        }),
        _ => true,
    }
}

fn validate_catalog(catalog: &OntologyReleaseCatalogV1) -> Result<(), AppError> {
    if catalog.schema != ONTOLOGY_RELEASE_CATALOG_SCHEMA {
        return Err(AppError::forbidden(
            "ontology release catalog schema is unsupported",
        ));
    }
    let mut api_names = BTreeSet::new();
    let mut stable_keys = BTreeSet::new();
    for (stable_key, api_name) in catalog
        .objects
        .iter()
        .map(|entry| (&entry.stable_key, &entry.api_name))
        .chain(
            catalog
                .relations
                .iter()
                .map(|entry| (&entry.stable_key, &entry.api_name)),
        )
        .chain(
            catalog
                .actions
                .iter()
                .map(|entry| (&entry.stable_key, &entry.api_name)),
        )
    {
        let kind = if catalog
            .objects
            .iter()
            .any(|entry| entry.api_name == *api_name)
        {
            ApiNameKind::Object
        } else if catalog
            .relations
            .iter()
            .any(|entry| entry.api_name == *api_name)
        {
            ApiNameKind::Relation
        } else {
            ApiNameKind::Action
        };
        validate_api_name(api_name.clone(), kind)?;
        if !api_names.insert(api_name.to_ascii_lowercase()) {
            return Err(AppError::forbidden(
                "ontology release catalog has a case-insensitive api_name conflict",
            ));
        }
        if !stable_keys.insert(stable_key.clone()) {
            return Err(AppError::forbidden(
                "ontology release catalog has duplicate semantic identities",
            ));
        }
    }
    let object_names = catalog
        .objects
        .iter()
        .map(|entry| entry.api_name.as_str())
        .collect::<BTreeSet<_>>();
    for relation in &catalog.relations {
        validate_api_name(relation.api_name.clone(), ApiNameKind::Relation)?;
        let from_object = catalog
            .objects
            .iter()
            .find(|object| object.api_name == relation.from_object_api_name)
            .ok_or_else(|| {
                AppError::forbidden("ontology release catalog relation endpoint is not an object")
            })?;
        let to_object = catalog
            .objects
            .iter()
            .find(|object| object.api_name == relation.to_object_api_name)
            .ok_or_else(|| {
                AppError::forbidden("ontology release catalog relation endpoint is not an object")
            })?;
        if relation.relation_type.trim().is_empty()
            || relation.stable_key
                != format!(
                    "relation:{}:{}:{}",
                    from_object.object_type, relation.relation_type, to_object.object_type
                )
        {
            return Err(AppError::forbidden(
                "ontology release catalog relation identity is invalid",
            ));
        }
    }
    for action in &catalog.actions {
        validate_api_name(action.api_name.clone(), ApiNameKind::Action)?;
        if action.contract_digest.is_empty() || action.execution_mode.is_empty() {
            return Err(AppError::forbidden(
                "ontology release catalog action contract metadata is incomplete",
            ));
        }
        if action.runtime_name.is_empty()
            || action.stable_key != format!("action:{}", action.runtime_name)
        {
            return Err(AppError::forbidden(
                "ontology release catalog action identity is invalid",
            ));
        }
        if !object_names.contains(action.target_object_api_name.as_str()) {
            return Err(AppError::forbidden(
                "ontology release catalog action target is not an object",
            ));
        }
        if !action.input_schema.is_object() {
            return Err(AppError::forbidden(
                "ontology release catalog action input schema must be a JSON object",
            ));
        }
        validate_action_schema(&action.input_schema)?;
    }
    for object in &catalog.objects {
        validate_api_name(object.api_name.clone(), ApiNameKind::Object)?;
        if object.object_type.trim().is_empty()
            || object.stable_key != format!("object:{}", object.object_type)
        {
            return Err(AppError::forbidden(
                "ontology release catalog object identity is invalid",
            ));
        }
        let mut property_names = BTreeSet::new();
        let mut property_stable_keys = BTreeSet::new();
        for property in &object.properties {
            validate_api_name(property.api_name.clone(), ApiNameKind::Property)?;
            let expected_stable_key =
                format!("{}:property:{}", object.stable_key, property.source_name);
            if property.source_name.trim().is_empty()
                || property.stable_key != expected_stable_key
                || !property_stable_keys.insert(property.stable_key.clone())
                || !property_names.insert(property.api_name.to_ascii_lowercase())
            {
                return Err(AppError::forbidden(
                    "ontology release catalog object property identity is invalid",
                ));
            }
            if property.value_type.trim().is_empty() {
                return Err(AppError::forbidden(
                    "ontology release catalog object property type is missing",
                ));
            }
            if ontology_catalog_property_type(&property.value_type).is_none() {
                return Err(AppError::forbidden(format!(
                    "ontology release catalog object property type {} is unsupported",
                    property.value_type
                )));
            }
        }
        if let Some(primary_key) = &object.primary_key_api_name {
            validate_api_name(primary_key.clone(), ApiNameKind::Property)?;
            if !property_names.contains(&primary_key.to_ascii_lowercase()) {
                return Err(AppError::forbidden(
                    "ontology release catalog primary key is not a declared property",
                ));
            }
        }
    }
    Ok(())
}

fn validate_action_contract_digests(
    release: &OntologyRelease,
    catalog: &OntologyReleaseCatalogV1,
) -> Result<(), AppError> {
    let evidence_refs = release.evidence_refs.as_array().ok_or_else(|| {
        AppError::forbidden("ontology release action contract evidence is missing")
    })?;
    let evidence_action_names = evidence_refs
        .iter()
        .filter_map(|entry| entry["tool_spec"]["name"].as_str())
        .collect::<Vec<_>>();
    let unique_evidence_action_names = evidence_action_names
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if evidence_action_names.len() != catalog.actions.len()
        || unique_evidence_action_names.len() != catalog.actions.len()
    {
        return Err(AppError::forbidden(
            "ontology release catalog action set does not match its contract snapshots",
        ));
    }
    for action in &catalog.actions {
        let tool_name = action
            .stable_key
            .strip_prefix("action:")
            .ok_or_else(|| AppError::forbidden("ontology release action identity is invalid"))?;
        let evidence = evidence_refs
            .iter()
            .find(|entry| entry["tool_spec"]["name"].as_str() == Some(tool_name))
            .ok_or_else(|| AppError::forbidden("ontology release action contract is missing"))?;
        let tool_spec_value = evidence
            .get("tool_spec")
            .cloned()
            .ok_or_else(|| AppError::forbidden("ontology release action contract is missing"))?;
        let evidence_digest = evidence
            .get("contract_digest")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AppError::forbidden("ontology release action contract digest is missing")
            })?;
        if normalized_json_sha256(&tool_spec_value) != evidence_digest
            || action.contract_digest != evidence_digest
        {
            return Err(AppError::forbidden(
                "ontology release action contract digest does not match the catalog",
            ));
        }
        let tool_spec = serde_json::from_value::<OntologyOnboardingToolSpec>(tool_spec_value)
            .map_err(|error| {
                AppError::forbidden(format!("ontology action contract is invalid: {error}"))
            })?;
        let target_stable_key = format!("object:{}", tool_spec.target_object);
        let target_object = catalog
            .objects
            .iter()
            .find(|object| object.stable_key == target_stable_key)
            .ok_or_else(|| {
                AppError::forbidden(
                    "ontology release action target object is missing from the catalog",
                )
            })?;
        if action.target_object_api_name != target_object.api_name {
            return Err(AppError::forbidden(
                "ontology release action target does not match the catalog",
            ));
        }
        if tool_spec.execution_mode != action.execution_mode {
            return Err(AppError::forbidden(
                "ontology release action execution mode does not match the catalog",
            ));
        }
        if tool_spec.input_schema != action.input_schema
            || tool_spec.approval_required != action.approval_required
        {
            return Err(AppError::forbidden(
                "ontology release action input or approval metadata does not match the catalog",
            ));
        }
    }
    if catalog.actions.iter().any(|action| {
        action
            .stable_key
            .strip_prefix("action:")
            .is_none_or(|tool_name| !unique_evidence_action_names.contains(tool_name))
    }) {
        return Err(AppError::forbidden(
            "ontology release catalog action set does not match its contract snapshots",
        ));
    }
    Ok(())
}

fn normalize_names(names: &mut Vec<String>, kind: &str) -> Result<(), AppError> {
    for name in names.iter_mut() {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(AppError::bad_request(format!(
                "ontology SDK subset contains an empty {kind} name"
            )));
        }
        *name = trimmed.to_string();
    }
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    if unique.len() != names.len() {
        return Err(AppError::bad_request(format!(
            "ontology SDK subset contains duplicate {kind} names"
        )));
    }
    *names = unique;
    Ok(())
}

pub(crate) fn canonical_json_sha256(value: &Value) -> String {
    let mut canonical = String::new();
    write_canonical_json(value, &mut canonical);
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(canonical.as_bytes()))
    )
}

fn write_canonical_json(value: &Value, output: &mut String) {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::String(value) => {
            output.push_str(&serde_json::to_string(value).unwrap_or_else(|_| String::from("\"\"")));
        }
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_canonical_json(value, output);
            }
            output.push(']');
        }
        Value::Object(values) => {
            let sorted = values.iter().collect::<BTreeMap<_, _>>();
            output.push('{');
            for (index, (key, value)) in sorted.into_iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output
                    .push_str(&serde_json::to_string(key).unwrap_or_else(|_| String::from("\"\"")));
                output.push(':');
                write_canonical_json(value, output);
            }
            output.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::canonical_json_sha256;
    use serde_json::json;

    #[test]
    fn catalog_digest_is_independent_of_object_key_order() {
        assert_eq!(
            canonical_json_sha256(&json!({"b": 2, "a": 1})),
            canonical_json_sha256(&json!({"a": 1, "b": 2}))
        );
    }
}
