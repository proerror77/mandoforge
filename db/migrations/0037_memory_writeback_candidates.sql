CREATE TABLE IF NOT EXISTS memory_writeback_candidates (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    candidate_type TEXT NOT NULL,
    source_event_id UUID,
    source_artifact_id UUID REFERENCES artifacts(id),
    source_approval_id UUID REFERENCES approvals(id),
    source_handoff_id UUID REFERENCES agent_handoff_events(id),
    proposed_object_type TEXT NOT NULL DEFAULT 'memory',
    proposed_object_key TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    content JSONB NOT NULL DEFAULT '{}',
    semantic_scopes JSONB NOT NULL DEFAULT '{}',
    source_refs JSONB NOT NULL DEFAULT '[]',
    provenance JSONB NOT NULL DEFAULT '{}',
    trust_level TEXT NOT NULL DEFAULT 'source_attested',
    freshness TEXT NOT NULL DEFAULT 'current',
    status TEXT NOT NULL DEFAULT 'pending',
    reviewer_subject TEXT,
    review_reason TEXT,
    semantic_object_id UUID REFERENCES semantic_objects(id),
    audit_trace_id UUID REFERENCES audit_logs(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_at TIMESTAMPTZ
);

-- tracked tenant table: 'memory_writeback_candidates'

CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_writeback_candidates_unique_source_pending
    ON memory_writeback_candidates (
        tenant_id,
        candidate_type,
        COALESCE(source_event_id::text, ''),
        COALESCE(source_artifact_id::text, ''),
        COALESCE(source_approval_id::text, ''),
        COALESCE(source_handoff_id::text, '')
    )
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS idx_memory_writeback_candidates_session_status
    ON memory_writeback_candidates (tenant_id, session_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_memory_writeback_candidates_semantic_object
    ON memory_writeback_candidates (tenant_id, semantic_object_id)
    WHERE semantic_object_id IS NOT NULL;

ALTER TABLE memory_writeback_candidates ENABLE ROW LEVEL SECURITY;
ALTER TABLE memory_writeback_candidates FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_memory_writeback_candidates ON memory_writeback_candidates;
CREATE POLICY tenant_isolation_memory_writeback_candidates ON memory_writeback_candidates
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
