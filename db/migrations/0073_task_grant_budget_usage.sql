ALTER TABLE task_grants
    ADD COLUMN IF NOT EXISTS turns_used INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS tool_calls_used INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS cost_usd_micros_used BIGINT NOT NULL DEFAULT 0;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'task_grants_usage_nonnegative_check'
    ) THEN
        ALTER TABLE task_grants
            ADD CONSTRAINT task_grants_usage_nonnegative_check
            CHECK (
                turns_used >= 0
                AND tool_calls_used >= 0
                AND cost_usd_micros_used >= 0
            );
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'task_grants_budget_positive_check'
    ) THEN
        ALTER TABLE task_grants
            ADD CONSTRAINT task_grants_budget_positive_check
            CHECK (
                (max_turns IS NULL OR max_turns > 0)
                AND (max_tool_calls IS NULL OR max_tool_calls > 0)
                AND (max_runtime_seconds IS NULL OR max_runtime_seconds > 0)
                AND (max_cost_usd_micros IS NULL OR max_cost_usd_micros > 0)
            );
    END IF;
END $$;
