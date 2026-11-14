# Shared Enterprise Memory — Evidence Matrix

All rows start RED. Status progresses to YELLOW (fixture-tested) or GREEN (reproducible automated evidence) as testing proves each claim.

## Status Definitions

- **RED**: Missing, failing, untested, non-reproducible, or contradicted by evidence
- **YELLOW**: Partially demonstrated or fixture-tested only; not production-ready
- **GREEN**: Reproducible automated evidence tied to current commit; no contradictory evidence

## Correctness

| ID | Requirement | Falsifiable Claim | Method | Status | Artifact |
|----|------------|-------------------|--------|--------|----------|
| C-01 | Idempotent projection | Replaying the same event twice does not duplicate projected state | TDD | YELLOW | `tests/projection_idempotency.rs::duplicate_delivery_does_not_create_duplicates` |
| C-02 | Deterministic replay | Full replay produces identical projection state | TDD | YELLOW | `tests/projection_idempotency.rs::replay_produces_identical_projection_state` |
| C-03 | Supersession updates current | Decision B superseding A makes B current without deleting A | TDD+BDD | YELLOW | `tests/projection_lifecycle.rs::supersession_preserves_history` |
| C-04 | Dissent preserved | Dissent remains visible after official decision is approved | BDD | YELLOW | `tests/projection_lifecycle.rs::decision_despite_dissent` |
| C-05 | Divergent observations | Two different observations from different actors both preserved | BDD | YELLOW | `tests/projection_lifecycle.rs::divergent_observations_are_preserved` |
| C-06 | Malformed events handled | Projector skips malformed content without stopping | TDD | YELLOW | `tests/projection_idempotency.rs::malformed_content_is_skipped_projector_continues` |
| C-07 | Checkpoint advancement | Projector tracks durable position per chain | TDD | YELLOW | `src/projection/projector.rs::checkpoint_advances_after_batch` |
| C-08 | Decision history complete | History includes both original and superseding decisions | TDD | YELLOW | `src/projection/projector.rs::projects_supersession` |

## Contracts

| ID | Requirement | Falsifiable Claim | Method | Status | Artifact |
|----|------------|-------------------|--------|--------|----------|
| CT-01 | MemoryEventV1 schema valid | JSON round-trip preserves all fields | CDD | YELLOW | `src/contract/memory_event.rs::json_round_trip` |
| CT-02 | ContextBundleV1 serialization | Empty and populated bundles serialize correctly | CDD | YELLOW | `src/contract/context_bundle.rs::*` |
| CT-03 | Contract validation | Invalid events (empty tenant, unknown type) rejected | CDD | YELLOW | `src/contract/memory_event.rs::validates_*` |
| CT-04 | Entry type length | All memory entry types fit within VARCHAR(50) | CDD | YELLOW | `src/contract/memory_event.rs::all_event_types_produce_valid_entry_types` |

## Security & Authorization

| ID | Requirement | Falsifiable Claim | Method | Status | Artifact |
|----|------------|-------------------|--------|--------|----------|
| S-01 | Tenant isolation | Unauthorized agent cannot retrieve tenant context | CBT | YELLOW | `tests/projection_lifecycle.rs::unauthorized_tenant_denied` |
| S-02 | Auth interface | Auth check runs before any projection query | TDD | YELLOW | `src/context/service.rs::forbidden_for_unauthorized_agent` |
| S-03 | Cross-tenant access | Agent for tenant A cannot read tenant B data | CBT | RED | Not yet tested |
| S-04 | Forged identity | Missing or invalid agent header returns 400/403 | CBT | RED | Not yet tested |
| S-05 | Classification enforcement | Restricted records not visible to unclassified agents | CBT | RED | Deferred |

## Performance

| ID | Requirement | Falsifiable Claim | Method | Status | Artifact |
|----|------------|-------------------|--------|--------|----------|
| P-01 | Zero ledger calls on read | Context retrieval makes zero LedgerReader calls | CBT | YELLOW | `tests/projection_lifecycle.rs::zero_ledger_calls_on_read_path` |
| P-02 | Read latency p95 | Context read < 50ms at p95 | EDD | RED | Not measured |
| P-03 | Projection lag | Lag observable and < 5s under normal load | EDD | RED | Not measured |
| P-04 | Storage growth | Projection storage grows linearly with events | EDD | RED | Not measured |

## Resilience

| ID | Requirement | Falsifiable Claim | Method | Status | Artifact |
|----|------------|-------------------|--------|--------|----------|
| R-01 | Projector restart | Projector resumes from checkpoint after restart | TDD | YELLOW | `src/projection/projector.rs::skips_already_processed_entries` |
| R-02 | Poison event isolation | Bad event does not stop processing of good events | TDD | YELLOW | `tests/projection_idempotency.rs::malformed_content_is_skipped_projector_continues` |
| R-03 | Database interruption | Projector handles DB unavailability gracefully | TDD | RED | Not yet tested |
| R-04 | Full rebuild | Rebuild from position 0 produces correct state | TDD | YELLOW | `src/projection/projector.rs::replay_produces_identical_state` |

## Observability

| ID | Requirement | Falsifiable Claim | Method | Status | Artifact |
|----|------------|-------------------|--------|--------|----------|
| O-01 | Metrics emitted | Projection counters and histograms registered | TDD | YELLOW | `src/metrics.rs` |
| O-02 | Projection lag visible | Lag gauge updated per chain | EDD | RED | Not measured |
| O-03 | Error metrics | Projection errors counted by entry_type | TDD | YELLOW | `src/metrics.rs::PROJECTION_ERRORS_TOTAL` |

## OpenShift Integration

| ID | Requirement | Falsifiable Claim | Method | Status | Artifact |
|----|------------|-------------------|--------|--------|----------|
| K-01 | Deployment manifests | Valid OpenShift-compatible manifests exist | EDD | RED | Deferred |
| K-02 | Non-root execution | Container runs as arbitrary UID | EDD | RED | Deferred |
| K-03 | Health probes | Liveness and readiness endpoints respond | EDD | RED | Deferred |
| K-04 | NetworkPolicy | Network boundaries documented and enforced | EDD | RED | Deferred |

## OpenShell Integration

| ID | Requirement | Falsifiable Claim | Method | Status | Artifact |
|----|------------|-------------------|--------|--------|----------|
| OS-01 | OCSF adapter | Consequential events admitted, routine excluded | CBT | YELLOW | tests/ocsf_admission.rs: routine_events_are_dropped, consequential_denials_are_promoted, detection_finding_always_promoted |
| OS-02 | Correlation fields | OpenShell correlation preserved through projection | CDD | YELLOW | tests/ocsf_admission.rs: correlation_fields_preserved, policy_revision_visible_in_projection |
| OS-03 | Fixture vs live labeling | Fixture evidence clearly labeled | EDD | YELLOW | tests/ocsf_admission.rs: fixture_vs_live_labeling |
