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

CREATE TABLE IF NOT EXISTS generic_demo.sample_documents (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS generic_demo.sample_metrics (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    metric_name TEXT NOT NULL,
    metric_value NUMERIC NOT NULL,
    dimensions JSONB NOT NULL DEFAULT '{}',
    observed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_platform_events_created_at ON generic_demo.platform_events(created_at);
CREATE INDEX IF NOT EXISTS idx_platform_events_status ON generic_demo.platform_events(status);
CREATE INDEX IF NOT EXISTS idx_sample_metrics_name_observed ON generic_demo.sample_metrics(metric_name, observed_at);

