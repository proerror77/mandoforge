WITH ranked_active_leases AS (
    SELECT
        id,
        row_number() OVER (
            PARTITION BY tenant_id, remote_computer_id
            ORDER BY heartbeat_at DESC NULLS LAST, updated_at DESC, created_at DESC, id DESC
        ) AS active_rank
    FROM remote_computer_leases
    WHERE status = 'leased'
),
deduplicated_leases AS (
    UPDATE remote_computer_leases
    SET status = 'failed',
        metadata = metadata || jsonb_build_object(
            'migration_reclaim_reason',
            'duplicate_active_remote_computer_lease_before_unique_index'
        ),
        updated_at = now()
    WHERE id IN (
        SELECT id
        FROM ranked_active_leases
        WHERE active_rank > 1
    )
    RETURNING remote_computer_id
)
UPDATE remote_computers
SET status = 'attention',
    metadata = metadata || jsonb_build_object(
        'migration_reclaim_reason',
        'duplicate_active_remote_computer_lease_before_unique_index'
    ),
    updated_at = now()
WHERE id IN (SELECT remote_computer_id FROM deduplicated_leases);

CREATE UNIQUE INDEX IF NOT EXISTS idx_remote_computer_leases_one_active_per_computer
    ON remote_computer_leases (tenant_id, remote_computer_id)
    WHERE status = 'leased';
