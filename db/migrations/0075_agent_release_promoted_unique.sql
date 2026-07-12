UPDATE agent_releases
SET automation_policy = jsonb_set(
    automation_policy,
    '{workflow_pack_installation_ids}',
    jsonb_build_array(automation_policy ->> 'workflow_pack_installation_id'),
    true
)
WHERE automation_policy ->> 'source' = 'workflow_pack_release'
  AND automation_policy ->> 'workflow_pack_installation_id' IS NOT NULL
  AND NOT (automation_policy ? 'workflow_pack_installation_ids');

WITH merged_pack_references AS (
    SELECT
        tenant_id,
        agent_id,
        agent_version_id,
        lower(environment) AS normalized_environment,
        jsonb_agg(DISTINCT reference.value ORDER BY reference.value) AS installation_ids
    FROM agent_releases
    CROSS JOIN LATERAL jsonb_array_elements_text(
        agent_releases.automation_policy -> 'workflow_pack_installation_ids'
    ) AS reference(value)
    WHERE status = 'promoted'
      AND automation_policy ->> 'source' = 'workflow_pack_release'
    GROUP BY tenant_id, agent_id, agent_version_id, lower(environment)
)
UPDATE agent_releases AS releases
SET automation_policy = jsonb_set(
    releases.automation_policy,
    '{workflow_pack_installation_ids}',
    merged_pack_references.installation_ids,
    true
)
FROM merged_pack_references
WHERE releases.tenant_id = merged_pack_references.tenant_id
  AND releases.agent_id = merged_pack_references.agent_id
  AND releases.agent_version_id = merged_pack_references.agent_version_id
  AND lower(releases.environment) = merged_pack_references.normalized_environment
  AND releases.status = 'promoted'
  AND releases.automation_policy ->> 'source' = 'workflow_pack_release';

WITH ranked_promotions AS (
    SELECT
        id,
        row_number() OVER (
            PARTITION BY tenant_id, agent_id, agent_version_id, lower(environment)
            ORDER BY
                CASE
                    WHEN automation_policy ->> 'source' = 'workflow_pack_release' THEN 1
                    ELSE 0
                END ASC,
                promoted_at DESC NULLS LAST,
                created_at DESC,
                id DESC
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
