#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "set DATABASE_URL before running seed-platform-events" >&2
  exit 1
fi

COUNT="${COUNT:-48}"

run_psql() {
  if command -v psql >/dev/null 2>&1; then
    psql "$DATABASE_URL" --set=ON_ERROR_STOP=1 --set=count="$COUNT"
    return
  fi
  if command -v docker >/dev/null 2>&1 && docker compose ps postgres >/dev/null 2>&1; then
    docker compose exec -T postgres psql "$DATABASE_URL" --set=ON_ERROR_STOP=1 --set=count="$COUNT"
    return
  fi
  echo "seed-platform-events requires psql or a running docker compose postgres service" >&2
  exit 1
}

run_psql <<'SQL'
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE SCHEMA IF NOT EXISTS generic_demo;

CREATE TABLE IF NOT EXISTS generic_demo.platform_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    session_id UUID,
    event_type TEXT NOT NULL,
    status TEXT,
    latency_ms INT,
    payload JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

WITH generated AS (
    SELECT
        series,
        (ARRAY[
            'session.started',
            'tool.call',
            'policy.requires_approval',
            'approval.approved',
            'artifact.created',
            'session.completed',
            'session.failed'
        ])[1 + (series % 7)] AS event_type
    FROM generate_series(1, :count::int) AS series
)
INSERT INTO generic_demo.platform_events (event_type, status, latency_ms, payload, created_at)
SELECT
    event_type,
    CASE
        WHEN event_type = 'session.failed' THEN 'failed'
        WHEN event_type = 'policy.requires_approval' THEN 'waiting_approval'
        ELSE 'ok'
    END AS status,
    40 + ((series * 37) % 750) AS latency_ms,
    jsonb_build_object(
        'source', 'seed-platform-events',
        'sequence', series,
        'tool', CASE
            WHEN event_type = 'tool.call' THEN 'sql.query'
            WHEN event_type = 'policy.requires_approval' THEN 'shell.exec'
            ELSE NULL
        END
    ) AS payload,
    now() - ((:count::int - series) || ' minutes')::interval AS created_at
FROM generated;
SQL

echo "seeded $COUNT generic_demo.platform_events rows"
