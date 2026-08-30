UPDATE workflow_definitions
SET execution_strategy = 'native_steps'
WHERE execution_strategy IN ('native_dynamic', 'dynamic_workflow');

UPDATE workflow_definitions
SET runtime_mode = 'normal'
WHERE runtime_mode = 'dynamic_workflow';

UPDATE workflow_runs
SET execution_strategy = 'native_steps'
WHERE execution_strategy IN ('native_dynamic', 'dynamic_workflow');

UPDATE workflow_runs
SET runtime_mode = 'normal'
WHERE runtime_mode = 'dynamic_workflow';

DROP TABLE IF EXISTS dynamic_workflow_plans;
