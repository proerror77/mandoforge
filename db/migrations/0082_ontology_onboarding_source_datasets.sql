ALTER TABLE ontology_onboarding_runs
    ADD COLUMN IF NOT EXISTS source_dataset_manifest JSONB,
    ADD COLUMN IF NOT EXISTS source_profiles JSONB;

ALTER TABLE ontology_onboarding_runs
    ADD CONSTRAINT ontology_onboarding_source_snapshot_shape
    CHECK (
        (source_dataset_manifest IS NULL AND source_profiles IS NULL)
        OR (
            source_dataset_manifest IS NOT NULL
            AND source_profiles IS NOT NULL
            AND jsonb_typeof(source_dataset_manifest) = 'array'
            AND jsonb_array_length(source_dataset_manifest) = dataset_count
            AND NOT jsonb_path_exists(source_dataset_manifest, '$[*].rows[*]')
            AND NOT jsonb_path_exists(
                source_dataset_manifest,
                '$[*].fields[*].sample_values[*]'
            )
            AND jsonb_typeof(source_profiles) = 'array'
            AND jsonb_array_length(source_profiles) = profile_count
        )
    );

-- Legacy proposal-only runs can still be referenced by ontology releases.
-- Backfill those run records before adding a source_run_id foreign key.
