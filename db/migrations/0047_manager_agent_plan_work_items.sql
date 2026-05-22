ALTER TABLE manager_agent_plans
    ADD COLUMN IF NOT EXISTS work_item_id UUID REFERENCES work_items(id);

CREATE INDEX IF NOT EXISTS idx_manager_agent_plans_tenant_work_item
    ON manager_agent_plans (tenant_id, work_item_id, created_at ASC)
    WHERE work_item_id IS NOT NULL;
