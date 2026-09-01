# Ontology SDK MVP — Phase 3 (cumulative Phase 1–3)

Phase 1 publishes a release-bound ontology catalog and an administrative
application manifest. A promoted ontology release carries an immutable
`mandoforge.ontology.release_catalog.v1` snapshot and canonical digest in its
`evidence_refs`. The release gate rejects missing, tampered, conflicting, or
incomplete catalog evidence; legacy active releases without that evidence are
not silently backfilled or exported.

Catalog API names are stable across releases when the semantic identity is
inherited from the parent release. Object names use UpperCamelCase; property,
relation, and action names use lowerCamelCase. Names are ASCII alphanumeric
(maximum 64 bytes) and TypeScript/JavaScript reserved names are rejected.
Dotted runtime names are never exported as API names. Object properties retain
a stable `object stable_key + source_name` identity, source name, API name,
observed/declared type, nullability, and optional primary-key API name; missing
evidence is represented as `unknown`/nullable
rather than fabricated. Relation entries include both object endpoints, and
action entries include their immutable contract digest, runtime name, target
object, and execution mode.

Administrators can create, list, inspect, and fetch the manifest for an
immutable application:

- `POST /api/ontology-sdk/applications`
- `GET /api/ontology-sdk/applications`
- `GET /api/ontology-sdk/applications/{id}`
- `GET /api/ontology-sdk/applications/{id}/manifest`

Application creation requires an active promoted release with valid catalog
evidence. The application records the tenant, authenticated subject, exact
release and catalog digest, and a canonical subset manifest/digest. Subsets may
reference only catalog objects, relations whose endpoints are also selected,
and `proposal_only` actions. The manifest response includes the resolved catalog
definitions for that subset, so a consumer can generate the same client from
the immutable application and release snapshots without querying mutable
proposal state.

Authenticated consumers are bound to the immutable application subject and are
revalidated against the exact active release, catalog digest, and subset on
every call:

- `GET /api/ontology-sdk/applications/{id}/objects/{api_name}`
- `GET /api/ontology-sdk/applications/{id}/objects/{api_name}/{object_id}`
- `GET /api/ontology-sdk/applications/{id}/relations`
- `GET /api/ontology-sdk/applications/{id}/typescript`
- `POST /api/ontology-sdk/applications/{id}/actions/{api_name}`

Consumer object responses include only visible, active business objects in the
application domain and only declared catalog properties. Relation responses
require both visible business-object endpoints and a declared relation. A
requested TaskGrant can further narrow reads through its explicit
`approval_policy.ontology_consumer_scope` allowlists; a missing allowlist is
fail-closed. Action proposals require a session, TaskGrant, and context packet
bound to the same release snapshot and action allowlist, reuse the existing
`ontology.action.execute` policy/approval path, and require both
`ToolsExecute` and `SessionsRun` permission plus resource visibility for every
referenced session, grant, and context packet. Only `approval_required` or
`proposal_created` results are exposed; this phase performs no semantic
writeback or customer-grade Postgres transaction.

TypeScript generation, npm packaging, and consumer data mutation routes remain
split at the boundary: the repository now has a deterministic Rust-generated,
strict TypeScript read/propose-only surface and an isolated Node 18 example
under `examples/mandoforge-osdk-typescript/` (run the local gate with
`scripts/ontology-sdk-consumer-gate.sh`). The generated client embeds the exact
application id and does not accept a caller-supplied id. npm publication,
OpenAPI/ObjectSet, and consumer data mutation remain out of scope.

Migration `0081_ontology_sdk_applications.sql` enables tenant RLS and uses a
restart-safe trigger to reject cross-tenant release bindings. A composite
tenant-plus-release foreign key remains deferred; the trigger is the current
control-plane consistency check, so this is not a customer-grade Postgres
transaction claim.
