UPDATE sessions
SET status = CASE status
    WHEN 'created' THEN 'idle'
    WHEN 'waiting_approval' THEN 'requires_action'
    WHEN 'completed' THEN 'terminated'
    ELSE status
END
WHERE status IN ('created', 'waiting_approval', 'completed');
