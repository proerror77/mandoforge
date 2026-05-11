INSERT INTO generic_demo.platform_events (event_type, status, latency_ms, payload, created_at) VALUES
('session.started', 'ok', 84, '{"source":"runtime-worker"}', now() - interval '23 hours'),
('tool.call', 'ok', 210, '{"tool":"file.read"}', now() - interval '20 hours'),
('tool.call', 'ok', 418, '{"tool":"sql.query"}', now() - interval '18 hours'),
('policy.requires_approval', 'waiting_approval', 35, '{"tool":"shell.exec","risk":"high"}', now() - interval '13 hours'),
('approval.approved', 'ok', 42, '{"tool":"shell.exec"}', now() - interval '12 hours'),
('artifact.created', 'ok', 67, '{"name":"diagnostics.md"}', now() - interval '9 hours'),
('session.completed', 'ok', 93, '{"result":"diagnostics complete"}', now() - interval '6 hours'),
('session.failed', 'failed', 3200, '{"error":"tool timeout"}', now() - interval '2 hours');

INSERT INTO generic_demo.sample_documents (title, body, metadata) VALUES
('Runtime Kernel Notes', 'Session events are append-only and replayable. Tool calls must pass through policy before execution.', '{"kind":"architecture"}'),
('Sandbox Policy', 'Shell execution and file writes require approval in Stage 1. Network is disabled by default.', '{"kind":"policy"}'),
('Provider Router Notes', 'OpenAI-compatible providers are the first provider target. Tool-call parsing is the next harness slice.', '{"kind":"provider"}');

INSERT INTO generic_demo.sample_metrics (metric_name, metric_value, dimensions, observed_at) VALUES
('sessions_started', 12, '{"window":"24h"}', now() - interval '1 hour'),
('sessions_completed', 9, '{"window":"24h"}', now() - interval '1 hour'),
('approvals_requested', 3, '{"window":"24h"}', now() - interval '1 hour'),
('tool_success_rate', 0.91, '{"window":"24h"}', now() - interval '1 hour');

