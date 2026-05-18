CREATE TABLE IF NOT EXISTS semantic_sources (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    source_type TEXT NOT NULL,
    source_uri TEXT NOT NULL,
    display_name TEXT NOT NULL,
    owner_type TEXT,
    owner_id UUID,
    metadata JSONB NOT NULL DEFAULT '{}',
    provenance JSONB NOT NULL DEFAULT '{}',
    freshness JSONB NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'active',
    last_ingested_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

-- tracked tenant table: 'semantic_sources'

CREATE UNIQUE INDEX IF NOT EXISTS idx_semantic_sources_tenant_uri_active
    ON semantic_sources (tenant_id, lower(source_uri))
    WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_semantic_sources_tenant_type_status
    ON semantic_sources (tenant_id, source_type, status, created_at DESC);

ALTER TABLE semantic_sources ENABLE ROW LEVEL SECURITY;
ALTER TABLE semantic_sources FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_semantic_sources ON semantic_sources;
CREATE POLICY tenant_isolation_semantic_sources ON semantic_sources
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());

CREATE TABLE IF NOT EXISTS semantic_objects (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    source_id UUID REFERENCES semantic_sources(id),
    object_type TEXT NOT NULL,
    object_key TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    content JSONB NOT NULL DEFAULT '{}',
    semantic_scopes JSONB NOT NULL DEFAULT '{}',
    source_uri TEXT,
    provenance JSONB NOT NULL DEFAULT '{}',
    trust_level TEXT NOT NULL DEFAULT 'unverified',
    freshness TEXT NOT NULL DEFAULT 'unknown',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

-- tracked tenant table: 'semantic_objects'

CREATE UNIQUE INDEX IF NOT EXISTS idx_semantic_objects_tenant_key_active
    ON semantic_objects (tenant_id, lower(object_key))
    WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_semantic_objects_tenant_type_status
    ON semantic_objects (tenant_id, object_type, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_semantic_objects_tenant_source
    ON semantic_objects (tenant_id, source_id, created_at DESC);

ALTER TABLE semantic_objects ENABLE ROW LEVEL SECURITY;
ALTER TABLE semantic_objects FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_semantic_objects ON semantic_objects;
CREATE POLICY tenant_isolation_semantic_objects ON semantic_objects
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());

CREATE TABLE IF NOT EXISTS semantic_links (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    from_entity_type TEXT NOT NULL,
    from_entity_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    to_entity_type TEXT NOT NULL,
    to_entity_id TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    provenance JSONB NOT NULL DEFAULT '{}',
    confidence DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

-- tracked tenant table: 'semantic_links'

CREATE UNIQUE INDEX IF NOT EXISTS idx_semantic_links_tenant_relation_active
    ON semantic_links (
        tenant_id,
        lower(from_entity_type),
        from_entity_id,
        lower(relation_type),
        lower(to_entity_type),
        to_entity_id
    )
    WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_semantic_links_tenant_from
    ON semantic_links (tenant_id, from_entity_type, from_entity_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_semantic_links_tenant_to
    ON semantic_links (tenant_id, to_entity_type, to_entity_id, created_at DESC);

ALTER TABLE semantic_links ENABLE ROW LEVEL SECURITY;
ALTER TABLE semantic_links FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_semantic_links ON semantic_links;
CREATE POLICY tenant_isolation_semantic_links ON semantic_links
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
