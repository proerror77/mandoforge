WITH ranked_promotions AS (
    SELECT
        id,
        row_number() OVER (
            PARTITION BY tenant_id, agent_id, agent_version_id, lower(environment)
            ORDER BY promoted_at DESC NULLS LAST, created_at DESC, id DESC
        ) AS promotion_rank
    FROM agent_releases
    WHERE status = 'promoted'
)
UPDATE agent_releases AS releases
SET status = 'superseded'
FROM ranked_promotions
WHERE releases.id = ranked_promotions.id
  AND ranked_promotions.promotion_rank > 1;

CREATE UNIQUE INDEX IF NOT EXISTS uq_agent_releases_promoted_target
ON agent_releases (tenant_id, agent_id, agent_version_id, lower(environment))
WHERE status = 'promoted';
