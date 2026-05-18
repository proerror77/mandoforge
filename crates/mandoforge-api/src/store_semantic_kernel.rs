use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::{
    semantic_link_from_row, semantic_object_from_row, semantic_source_from_row,
};
use crate::{
    AppError, AppState, CreateSemanticLink, CreateSemanticObject, CreateSemanticSource,
    SemanticLink, SemanticObject, SemanticSource, UpdateSemanticLink, UpdateSemanticObject,
    UpdateSemanticSource,
};

impl AppState {
    pub(crate) async fn list_semantic_sources(&self) -> Result<Vec<SemanticSource>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut sources: Vec<_> = inner
                    .read()
                    .await
                    .semantic_sources
                    .values()
                    .filter(|source| source.archived_at.is_none())
                    .cloned()
                    .collect();
                sources.sort_by_key(|source| source.created_at);
                Ok(sources)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, source_type, source_uri, display_name, owner_type, owner_id, metadata, provenance, freshness, status, last_ingested_at, created_at, updated_at, archived_at
                     FROM semantic_sources
                     WHERE tenant_id = $1 AND archived_at IS NULL
                     ORDER BY created_at ASC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(semantic_source_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_semantic_source(&self, id: Uuid) -> Result<SemanticSource, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .semantic_sources
                .get(&id)
                .filter(|source| source.archived_at.is_none())
                .cloned()
                .ok_or_else(|| AppError::not_found("semantic source not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, source_type, source_uri, display_name, owner_type, owner_id, metadata, provenance, freshness, status, last_ingested_at, created_at, updated_at, archived_at
                     FROM semantic_sources
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("semantic source not found"))?;
                semantic_source_from_row(row)
            }
        }
    }

    pub(crate) async fn create_semantic_source(
        &self,
        input: CreateSemanticSource,
    ) -> Result<SemanticSource, AppError> {
        let now = Utc::now();
        let source = SemanticSource {
            id: Uuid::new_v4(),
            source_type: normalize_semantic_source_type(&input.source_type)?,
            source_uri: normalize_required_text(&input.source_uri, "semantic source_uri")?,
            display_name: normalize_required_text(&input.display_name, "semantic display_name")?,
            owner_type: input.owner_type.and_then(normalize_optional_text),
            owner_id: input.owner_id,
            metadata: validate_json_object(input.metadata, "semantic source metadata")?,
            provenance: validate_json_object(input.provenance, "semantic source provenance")?,
            freshness: validate_json_object(input.freshness, "semantic source freshness")?,
            status: normalize_semantic_source_status(&input.status)?,
            last_ingested_at: input.last_ingested_at,
            created_at: now,
            updated_at: now,
            archived_at: None,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store.semantic_sources.values().any(|existing| {
                    existing.archived_at.is_none()
                        && existing.source_uri.eq_ignore_ascii_case(&source.source_uri)
                }) {
                    return Err(AppError::bad_request(format!(
                        "semantic source already exists: {}",
                        source.source_uri
                    )));
                }
                store.semantic_sources.insert(source.id, source.clone());
                Ok(source)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO semantic_sources
                        (id, tenant_id, source_type, source_uri, display_name, owner_type, owner_id, metadata, provenance, freshness, status, last_ingested_at, created_at, updated_at, archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NULL)
                     RETURNING id, source_type, source_uri, display_name, owner_type, owner_id, metadata, provenance, freshness, status, last_ingested_at, created_at, updated_at, archived_at",
                )
                .bind(source.id)
                .bind(self.current_tenant_id())
                .bind(&source.source_type)
                .bind(&source.source_uri)
                .bind(&source.display_name)
                .bind(&source.owner_type)
                .bind(source.owner_id)
                .bind(&source.metadata)
                .bind(&source.provenance)
                .bind(&source.freshness)
                .bind(&source.status)
                .bind(source.last_ingested_at)
                .bind(source.created_at)
                .bind(source.updated_at)
                .fetch_one(pool)
                .await?;
                semantic_source_from_row(row)
            }
        }
    }

    pub(crate) async fn update_semantic_source(
        &self,
        id: Uuid,
        input: UpdateSemanticSource,
    ) -> Result<SemanticSource, AppError> {
        let mut source = self.get_semantic_source(id).await?;
        if let Some(display_name) = input.display_name {
            source.display_name = normalize_required_text(&display_name, "semantic display_name")?;
        }
        if let Some(owner_type) = input.owner_type {
            source.owner_type = owner_type.and_then(normalize_optional_text);
        }
        if let Some(owner_id) = input.owner_id {
            source.owner_id = owner_id;
        }
        if let Some(metadata) = input.metadata {
            source.metadata = validate_json_object(metadata, "semantic source metadata")?;
        }
        if let Some(provenance) = input.provenance {
            source.provenance = validate_json_object(provenance, "semantic source provenance")?;
        }
        if let Some(freshness) = input.freshness {
            source.freshness = validate_json_object(freshness, "semantic source freshness")?;
        }
        if let Some(status) = input.status {
            source.status = normalize_semantic_source_status(&status)?;
        }
        if let Some(last_ingested_at) = input.last_ingested_at {
            source.last_ingested_at = last_ingested_at;
        }
        source.updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .semantic_sources
                    .insert(source.id, source.clone());
                Ok(source)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE semantic_sources
                     SET display_name = $3,
                         owner_type = $4,
                         owner_id = $5,
                         metadata = $6,
                         provenance = $7,
                         freshness = $8,
                         status = $9,
                         last_ingested_at = $10,
                         updated_at = $11
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
                     RETURNING id, source_type, source_uri, display_name, owner_type, owner_id, metadata, provenance, freshness, status, last_ingested_at, created_at, updated_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(source.id)
                .bind(&source.display_name)
                .bind(&source.owner_type)
                .bind(source.owner_id)
                .bind(&source.metadata)
                .bind(&source.provenance)
                .bind(&source.freshness)
                .bind(&source.status)
                .bind(source.last_ingested_at)
                .bind(source.updated_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("semantic source not found"))?;
                semantic_source_from_row(row)
            }
        }
    }

    pub(crate) async fn archive_semantic_source(
        &self,
        id: Uuid,
    ) -> Result<SemanticSource, AppError> {
        let mut source = self.get_semantic_source(id).await?;
        let archived_at = Utc::now();
        source.status = "archived".to_string();
        source.updated_at = archived_at;
        source.archived_at = Some(archived_at);
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .semantic_sources
                    .insert(source.id, source.clone());
                Ok(source)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE semantic_sources
                     SET status = 'archived', updated_at = $3, archived_at = $3
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
                     RETURNING id, source_type, source_uri, display_name, owner_type, owner_id, metadata, provenance, freshness, status, last_ingested_at, created_at, updated_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(source.id)
                .bind(archived_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("semantic source not found"))?;
                semantic_source_from_row(row)
            }
        }
    }

    pub(crate) async fn list_semantic_objects(&self) -> Result<Vec<SemanticObject>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut objects: Vec<_> = inner
                    .read()
                    .await
                    .semantic_objects
                    .values()
                    .filter(|object| object.archived_at.is_none())
                    .cloned()
                    .collect();
                objects.sort_by_key(|object| object.created_at);
                Ok(objects)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, source_id, object_type, object_key, title, summary, content, semantic_scopes, source_uri, provenance, trust_level, freshness, status, created_at, updated_at, archived_at
                     FROM semantic_objects
                     WHERE tenant_id = $1 AND archived_at IS NULL
                     ORDER BY created_at ASC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(semantic_object_from_row).collect()
            }
        }
    }

    pub(crate) async fn list_semantic_objects_for_source(
        &self,
        source_id: Uuid,
    ) -> Result<Vec<SemanticObject>, AppError> {
        self.get_semantic_source(source_id).await?;
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut objects: Vec<_> = inner
                    .read()
                    .await
                    .semantic_objects
                    .values()
                    .filter(|object| {
                        object.source_id == Some(source_id) && object.archived_at.is_none()
                    })
                    .cloned()
                    .collect();
                objects.sort_by_key(|object| object.created_at);
                Ok(objects)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, source_id, object_type, object_key, title, summary, content, semantic_scopes, source_uri, provenance, trust_level, freshness, status, created_at, updated_at, archived_at
                     FROM semantic_objects
                     WHERE tenant_id = $1 AND source_id = $2 AND archived_at IS NULL
                     ORDER BY created_at ASC",
                )
                .bind(self.current_tenant_id())
                .bind(source_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(semantic_object_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_semantic_object(&self, id: Uuid) -> Result<SemanticObject, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .semantic_objects
                .get(&id)
                .filter(|object| object.archived_at.is_none())
                .cloned()
                .ok_or_else(|| AppError::not_found("semantic object not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, source_id, object_type, object_key, title, summary, content, semantic_scopes, source_uri, provenance, trust_level, freshness, status, created_at, updated_at, archived_at
                     FROM semantic_objects
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("semantic object not found"))?;
                semantic_object_from_row(row)
            }
        }
    }

    pub(crate) async fn create_semantic_object(
        &self,
        input: CreateSemanticObject,
    ) -> Result<SemanticObject, AppError> {
        let source_uri = match (input.source_id, input.source_uri) {
            (Some(source_id), maybe_uri) => {
                let source = self.get_semantic_source(source_id).await?;
                Some(match maybe_uri.and_then(normalize_optional_text) {
                    Some(uri) => uri,
                    None => source.source_uri,
                })
            }
            (None, Some(uri)) => Some(normalize_required_text(&uri, "semantic object source_uri")?),
            (None, None) => {
                return Err(AppError::bad_request(
                    "semantic object requires source_id or source_uri",
                ));
            }
        };
        let now = Utc::now();
        let object = SemanticObject {
            id: Uuid::new_v4(),
            source_id: input.source_id,
            object_type: normalize_semantic_object_type(&input.object_type)?,
            object_key: normalize_required_text(&input.object_key, "semantic object_key")?,
            title: normalize_required_text(&input.title, "semantic title")?,
            summary: normalize_required_text(&input.summary, "semantic summary")?,
            content: validate_json_object(input.content, "semantic object content")?,
            semantic_scopes: validate_json_object(
                input.semantic_scopes,
                "semantic object semantic_scopes",
            )?,
            source_uri,
            provenance: validate_json_object(input.provenance, "semantic object provenance")?,
            trust_level: normalize_semantic_trust_level(&input.trust_level)?,
            freshness: normalize_semantic_freshness(&input.freshness)?,
            status: normalize_semantic_record_status(&input.status)?,
            created_at: now,
            updated_at: now,
            archived_at: None,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store.semantic_objects.values().any(|existing| {
                    existing.archived_at.is_none()
                        && existing.object_key.eq_ignore_ascii_case(&object.object_key)
                }) {
                    return Err(AppError::bad_request(format!(
                        "semantic object already exists: {}",
                        object.object_key
                    )));
                }
                store.semantic_objects.insert(object.id, object.clone());
                Ok(object)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO semantic_objects
                        (id, tenant_id, source_id, object_type, object_key, title, summary, content, semantic_scopes, source_uri, provenance, trust_level, freshness, status, created_at, updated_at, archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, NULL)
                     RETURNING id, source_id, object_type, object_key, title, summary, content, semantic_scopes, source_uri, provenance, trust_level, freshness, status, created_at, updated_at, archived_at",
                )
                .bind(object.id)
                .bind(self.current_tenant_id())
                .bind(object.source_id)
                .bind(&object.object_type)
                .bind(&object.object_key)
                .bind(&object.title)
                .bind(&object.summary)
                .bind(&object.content)
                .bind(&object.semantic_scopes)
                .bind(&object.source_uri)
                .bind(&object.provenance)
                .bind(&object.trust_level)
                .bind(&object.freshness)
                .bind(&object.status)
                .bind(object.created_at)
                .bind(object.updated_at)
                .fetch_one(pool)
                .await?;
                semantic_object_from_row(row)
            }
        }
    }

    pub(crate) async fn update_semantic_object(
        &self,
        id: Uuid,
        input: UpdateSemanticObject,
    ) -> Result<SemanticObject, AppError> {
        let mut object = self.get_semantic_object(id).await?;
        if let Some(title) = input.title {
            object.title = normalize_required_text(&title, "semantic title")?;
        }
        if let Some(summary) = input.summary {
            object.summary = normalize_required_text(&summary, "semantic summary")?;
        }
        if let Some(content) = input.content {
            object.content = validate_json_object(content, "semantic object content")?;
        }
        if let Some(semantic_scopes) = input.semantic_scopes {
            object.semantic_scopes =
                validate_json_object(semantic_scopes, "semantic object semantic_scopes")?;
        }
        if let Some(source_uri) = input.source_uri {
            object.source_uri = source_uri
                .map(|value| normalize_required_text(&value, "semantic object source_uri"))
                .transpose()?;
        }
        if let Some(provenance) = input.provenance {
            object.provenance = validate_json_object(provenance, "semantic object provenance")?;
        }
        if let Some(trust_level) = input.trust_level {
            object.trust_level = normalize_semantic_trust_level(&trust_level)?;
        }
        if let Some(freshness) = input.freshness {
            object.freshness = normalize_semantic_freshness(&freshness)?;
        }
        if let Some(status) = input.status {
            object.status = normalize_semantic_record_status(&status)?;
        }
        object.updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .semantic_objects
                    .insert(object.id, object.clone());
                Ok(object)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE semantic_objects
                     SET title = $3,
                         summary = $4,
                         content = $5,
                         semantic_scopes = $6,
                         source_uri = $7,
                         provenance = $8,
                         trust_level = $9,
                         freshness = $10,
                         status = $11,
                         updated_at = $12
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
                     RETURNING id, source_id, object_type, object_key, title, summary, content, semantic_scopes, source_uri, provenance, trust_level, freshness, status, created_at, updated_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(object.id)
                .bind(&object.title)
                .bind(&object.summary)
                .bind(&object.content)
                .bind(&object.semantic_scopes)
                .bind(&object.source_uri)
                .bind(&object.provenance)
                .bind(&object.trust_level)
                .bind(&object.freshness)
                .bind(&object.status)
                .bind(object.updated_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("semantic object not found"))?;
                semantic_object_from_row(row)
            }
        }
    }

    pub(crate) async fn archive_semantic_object(
        &self,
        id: Uuid,
    ) -> Result<SemanticObject, AppError> {
        let mut object = self.get_semantic_object(id).await?;
        let archived_at = Utc::now();
        object.status = "archived".to_string();
        object.updated_at = archived_at;
        object.archived_at = Some(archived_at);
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .semantic_objects
                    .insert(object.id, object.clone());
                Ok(object)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE semantic_objects
                     SET status = 'archived', updated_at = $3, archived_at = $3
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
                     RETURNING id, source_id, object_type, object_key, title, summary, content, semantic_scopes, source_uri, provenance, trust_level, freshness, status, created_at, updated_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(object.id)
                .bind(archived_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("semantic object not found"))?;
                semantic_object_from_row(row)
            }
        }
    }

    pub(crate) async fn list_semantic_links(&self) -> Result<Vec<SemanticLink>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut links: Vec<_> = inner
                    .read()
                    .await
                    .semantic_links
                    .values()
                    .filter(|link| link.archived_at.is_none())
                    .cloned()
                    .collect();
                links.sort_by_key(|link| link.created_at);
                Ok(links)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, from_entity_type, from_entity_id, relation_type, to_entity_type, to_entity_id, metadata, provenance, confidence, status, created_at, updated_at, archived_at
                     FROM semantic_links
                     WHERE tenant_id = $1 AND archived_at IS NULL
                     ORDER BY created_at ASC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(semantic_link_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_semantic_link(&self, id: Uuid) -> Result<SemanticLink, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .semantic_links
                .get(&id)
                .filter(|link| link.archived_at.is_none())
                .cloned()
                .ok_or_else(|| AppError::not_found("semantic link not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, from_entity_type, from_entity_id, relation_type, to_entity_type, to_entity_id, metadata, provenance, confidence, status, created_at, updated_at, archived_at
                     FROM semantic_links
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("semantic link not found"))?;
                semantic_link_from_row(row)
            }
        }
    }

    pub(crate) async fn create_semantic_link(
        &self,
        input: CreateSemanticLink,
    ) -> Result<SemanticLink, AppError> {
        let now = Utc::now();
        let link = SemanticLink {
            id: Uuid::new_v4(),
            from_entity_type: normalize_semantic_entity_type(&input.from_entity_type)?,
            from_entity_id: normalize_required_text(&input.from_entity_id, "from_entity_id")?,
            relation_type: normalize_semantic_relation_type(&input.relation_type)?,
            to_entity_type: normalize_semantic_entity_type(&input.to_entity_type)?,
            to_entity_id: normalize_required_text(&input.to_entity_id, "to_entity_id")?,
            metadata: validate_json_object(input.metadata, "semantic link metadata")?,
            provenance: validate_json_object(input.provenance, "semantic link provenance")?,
            confidence: validate_semantic_confidence(input.confidence)?,
            status: normalize_semantic_record_status(&input.status)?,
            created_at: now,
            updated_at: now,
            archived_at: None,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store.semantic_links.values().any(|existing| {
                    existing.archived_at.is_none()
                        && existing
                            .from_entity_type
                            .eq_ignore_ascii_case(&link.from_entity_type)
                        && existing.from_entity_id == link.from_entity_id
                        && existing
                            .relation_type
                            .eq_ignore_ascii_case(&link.relation_type)
                        && existing
                            .to_entity_type
                            .eq_ignore_ascii_case(&link.to_entity_type)
                        && existing.to_entity_id == link.to_entity_id
                }) {
                    return Err(AppError::bad_request("semantic link already exists"));
                }
                store.semantic_links.insert(link.id, link.clone());
                Ok(link)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO semantic_links
                        (id, tenant_id, from_entity_type, from_entity_id, relation_type, to_entity_type, to_entity_id, metadata, provenance, confidence, status, created_at, updated_at, archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NULL)
                     RETURNING id, from_entity_type, from_entity_id, relation_type, to_entity_type, to_entity_id, metadata, provenance, confidence, status, created_at, updated_at, archived_at",
                )
                .bind(link.id)
                .bind(self.current_tenant_id())
                .bind(&link.from_entity_type)
                .bind(&link.from_entity_id)
                .bind(&link.relation_type)
                .bind(&link.to_entity_type)
                .bind(&link.to_entity_id)
                .bind(&link.metadata)
                .bind(&link.provenance)
                .bind(link.confidence)
                .bind(&link.status)
                .bind(link.created_at)
                .bind(link.updated_at)
                .fetch_one(pool)
                .await?;
                semantic_link_from_row(row)
            }
        }
    }

    pub(crate) async fn update_semantic_link(
        &self,
        id: Uuid,
        input: UpdateSemanticLink,
    ) -> Result<SemanticLink, AppError> {
        let mut link = self.get_semantic_link(id).await?;
        if let Some(metadata) = input.metadata {
            link.metadata = validate_json_object(metadata, "semantic link metadata")?;
        }
        if let Some(provenance) = input.provenance {
            link.provenance = validate_json_object(provenance, "semantic link provenance")?;
        }
        if let Some(confidence) = input.confidence {
            link.confidence = validate_semantic_confidence(confidence)?;
        }
        if let Some(status) = input.status {
            link.status = normalize_semantic_record_status(&status)?;
        }
        link.updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .semantic_links
                    .insert(link.id, link.clone());
                Ok(link)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE semantic_links
                     SET metadata = $3,
                         provenance = $4,
                         confidence = $5,
                         status = $6,
                         updated_at = $7
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
                     RETURNING id, from_entity_type, from_entity_id, relation_type, to_entity_type, to_entity_id, metadata, provenance, confidence, status, created_at, updated_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(link.id)
                .bind(&link.metadata)
                .bind(&link.provenance)
                .bind(link.confidence)
                .bind(&link.status)
                .bind(link.updated_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("semantic link not found"))?;
                semantic_link_from_row(row)
            }
        }
    }

    pub(crate) async fn archive_semantic_link(&self, id: Uuid) -> Result<SemanticLink, AppError> {
        let mut link = self.get_semantic_link(id).await?;
        let archived_at = Utc::now();
        link.status = "archived".to_string();
        link.updated_at = archived_at;
        link.archived_at = Some(archived_at);
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .semantic_links
                    .insert(link.id, link.clone());
                Ok(link)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE semantic_links
                     SET status = 'archived', updated_at = $3, archived_at = $3
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
                     RETURNING id, from_entity_type, from_entity_id, relation_type, to_entity_type, to_entity_id, metadata, provenance, confidence, status, created_at, updated_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(link.id)
                .bind(archived_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("semantic link not found"))?;
                semantic_link_from_row(row)
            }
        }
    }
}

fn normalize_required_text(value: &str, label: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppError::bad_request(format!("{label} cannot be empty")))
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn validate_json_object(value: Value, label: &str) -> Result<Value, AppError> {
    if value.is_object() {
        Ok(value)
    } else {
        Err(AppError::bad_request(format!(
            "{label} must be a JSON object"
        )))
    }
}

fn normalize_semantic_source_type(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "repo_doc" | "session_history" | "artifact" | "workflow_pack" | "mcp_source" | "feishu"
        | "lark" | "github" | "upload" | "external" | "memory" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "semantic source_type must be repo_doc, session_history, artifact, workflow_pack, mcp_source, feishu, lark, github, upload, external, or memory",
        )),
    }
}

fn normalize_semantic_source_status(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "active" | "discovered" | "ingested" | "stale" | "archived" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "semantic source status must be active, discovered, ingested, stale, or archived",
        )),
    }
}

fn normalize_semantic_object_type(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "decision" | "runbook" | "code_module" | "workflow" | "policy" | "memory" | "artifact"
        | "project" | "repo" | "service" | "pack" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "semantic object_type must be decision, runbook, code_module, workflow, policy, memory, artifact, project, repo, service, or pack",
        )),
    }
}

fn normalize_semantic_entity_type(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "agent" | "project" | "repo" | "service" | "workflow" | "policy" | "pack" | "memory"
        | "artifact" | "session" | "manager_plan" | "runtime_profile" | "semantic_source"
        | "semantic_object" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "semantic entity type is not supported by the minimal semantic kernel",
        )),
    }
}

fn normalize_semantic_relation_type(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    if normalized.is_empty() {
        Err(AppError::bad_request(
            "semantic relation_type cannot be empty",
        ))
    } else {
        Ok(normalized)
    }
}

fn normalize_semantic_trust_level(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "unverified" | "source_attested" | "human_verified" | "system_verified" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "semantic trust_level must be unverified, source_attested, human_verified, or system_verified",
        )),
    }
}

fn normalize_semantic_freshness(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "unknown" | "current" | "stale" | "expired" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "semantic freshness must be unknown, current, stale, or expired",
        )),
    }
}

fn normalize_semantic_record_status(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "active" | "archived" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "semantic record status must be active or archived",
        )),
    }
}

fn validate_semantic_confidence(value: f64) -> Result<f64, AppError> {
    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(AppError::bad_request(
            "semantic link confidence must be between 0.0 and 1.0",
        ))
    }
}
