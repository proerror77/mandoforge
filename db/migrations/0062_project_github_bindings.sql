-- Migration: project_github_bindings
-- Maps a GitHub repo to a WorkflowPack installation, enabling webhook-triggered SWE loops.

CREATE TABLE IF NOT EXISTS project_github_bindings (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID NOT NULL,
    repo_full_name      TEXT NOT NULL,           -- e.g. "org/repo"
    pack_installation_id UUID NOT NULL,           -- FK to workflow_pack_installations.id
    webhook_secret_ref  TEXT NOT NULL DEFAULT '', -- key name in secrets store
    active              BOOLEAN NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT uq_tenant_repo UNIQUE (tenant_id, repo_full_name),
    CONSTRAINT fk_tenant FOREIGN KEY (tenant_id)
        REFERENCES tenants(id) ON DELETE CASCADE,
    CONSTRAINT fk_pack_installation FOREIGN KEY (pack_installation_id)
        REFERENCES workflow_pack_installations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS ix_pgb_tenant ON project_github_bindings (tenant_id);
CREATE INDEX IF NOT EXISTS ix_pgb_repo ON project_github_bindings (lower(repo_full_name));
