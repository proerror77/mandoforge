ALTER TABLE organizations ADD COLUMN IF NOT EXISTS owner_subject TEXT;

CREATE INDEX IF NOT EXISTS idx_organizations_owner_subject ON organizations(tenant_id, owner_subject)
WHERE owner_subject IS NOT NULL;
