-- Complete AgentVersion as an immutable runtime behavior snapshot.
ALTER TABLE agent_versions
    ADD COLUMN IF NOT EXISTS provider TEXT,
    ADD COLUMN IF NOT EXISTS runtime_profile_id UUID REFERENCES agent_runtime_profiles(id),
    ADD COLUMN IF NOT EXISTS runtime_profile_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS remote_computer_profile JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE agent_versions av
SET provider = a.provider,
    runtime_profile_id = a.runtime_profile_id,
    remote_computer_profile = a.remote_computer_profile,
    runtime_profile_snapshot = COALESCE(
        (
            SELECT jsonb_build_object(
                'id', arp.id,
                'name', arp.name,
                'runtime_type', arp.runtime_type,
                'command', arp.command,
                'default_args', arp.default_args,
                'env', arp.env,
                'timeout_seconds', arp.timeout_seconds,
                'remote_computer_required', arp.remote_computer_required,
                'status', arp.status,
                'created_at', arp.created_at,
                'updated_at', arp.updated_at,
                'archived_at', arp.archived_at
            )
            FROM agent_runtime_profiles arp
            WHERE arp.id = a.runtime_profile_id
        ),
        '{}'::jsonb
    )
FROM agents a
WHERE a.id = av.agent_id
  AND av.provider IS NULL;

ALTER TABLE agent_versions
    ALTER COLUMN provider SET NOT NULL;
