ALTER TABLE approval_notification_channel_policies
    ADD COLUMN IF NOT EXISTS max_attempts INT NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS backoff_seconds INT NOT NULL DEFAULT 0;
