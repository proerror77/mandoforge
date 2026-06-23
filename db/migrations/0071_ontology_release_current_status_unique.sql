DROP INDEX IF EXISTS idx_ontology_releases_one_active_per_domain;

WITH ranked_current AS (
    SELECT id,
           row_number() OVER (
               PARTITION BY tenant_id, lower(domain_scope)
               ORDER BY promoted_at DESC NULLS LAST, updated_at DESC, created_at DESC
           ) AS current_rank
    FROM ontology_releases
    WHERE status IN ('active', 'active_trigger_failed')
)
UPDATE ontology_releases
SET status = 'superseded',
    updated_at = now()
FROM ranked_current
WHERE ontology_releases.id = ranked_current.id
  AND ranked_current.current_rank > 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_ontology_releases_one_current_per_domain
    ON ontology_releases (tenant_id, lower(domain_scope))
    WHERE status IN ('active', 'active_trigger_failed');
