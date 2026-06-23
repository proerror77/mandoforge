ALTER TABLE ontology_release_workflow_triggers
    DROP CONSTRAINT IF EXISTS ontology_release_workflow_triggers_status_check;

ALTER TABLE ontology_release_workflow_triggers
    ADD CONSTRAINT ontology_release_workflow_triggers_status_check
    CHECK (status IN ('pending', 'triggered', 'failed', 'skipped'));
