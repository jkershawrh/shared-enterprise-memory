# Projections

Shared enterprise memory for AI agents. Built on immutable evidence, not fabricated consensus.

## The Problem

Enterprise AI agents need organizational context — active decisions, commitments, constraints, exceptions. Today, agents get that context from summaries, embeddings, or chat history. When two people remember an event differently, the system picks one version or blends them. Dissent disappears. Superseded decisions look current. Nobody can trace why an agent acts on stale or contested information.

## The Insight

Enterprise memory is not truth. It is observations, assertions, dissent, decisions, and outcomes — each with provenance, scope, and accountability. An approved decision is an authorized operating position, not objective fact. The system must preserve disagreement, not flatten it.

## Architecture

```
Agents/Systems ──→ Context API (HTTP :8084) ──→ Projection Store (are_memory)
                                                        ↑
                                                  Projector (background)
                                                        ↑
                                               Immutable Ledger (are_ledger)
```

**Three layers, one principle: agents never scan the ledger.**

| Layer | Responsibility | Mutable? |
|-------|---------------|----------|
| **Immutable Ledger** | Append-only, hash-chained evidence spine | No — never rewritten |
| **Projections** | Read-optimized views derived from ledger events | Yes — rebuildable from scratch |
| **Context API** | Authorized, tenant-scoped context bundles for agents | Computed on demand |

## What It Demonstrates

Six BDD scenarios proven end-to-end:

1. **Divergent observations** — Alice and Bob remember the same incident differently. Both preserved.
2. **Dissent** — Bob disagrees with Alice's assertion. Target marked, dissent visible.
3. **Decision despite dissent** — Carol decides. Dissent is not erased.
4. **Supersession** — Carol changes her mind. New decision is current, old is in history.
5. **Zero ledger calls** — Context retrieval reads only from projections.
6. **Tenant isolation** — Unauthorized agents are denied access.

## Quick Start

```bash
# Prerequisites: Rust, Podman, PostgreSQL (via container)

# Build and test
make check          # fmt + clippy + 53 tests

# Start the database
make up             # Podman: PostgreSQL + migration

# Run the service
make run            # Starts on :18084 (API) and :18085 (metrics)

# Write events and query context
make smoke          # Basic endpoint checks
```

## API

### Write a memory event

```bash
curl -X POST http://localhost:18084/v1/memory/events \
  -H "Content-Type: application/json" \
  -H "X-Agent-ID: alice" \
  -d '{
    "event_id": "11111111-1111-1111-1111-111111111111",
    "event_type": "observation.recorded",
    "schema_version": "1.0.0",
    "tenant_id": "acme",
    "subject_id": "incident-42",
    "actor": {"actor_id": "alice", "actor_type": "human"},
    "recorded_at": "2026-08-10T10:00:00Z",
    "classification": "internal",
    "claim_state": "asserted",
    "payload": {"observation": "server was overloaded at 09:45"}
  }'
```

### Query context (what an agent sees)

```bash
curl -H "X-Agent-ID: ops-agent" \
  http://localhost:18084/v1/memory/context/acme/incident-42
```

Returns a `ContextBundleV1` with:
- `effective_decisions` — current decisions (superseded ones excluded)
- `observations` — all observations from all actors
- `assertions` — claims with `has_dissent` flag
- `dissent` — disagreements targeting specific assertions/decisions
- `evidence` — links back to ledger entries
- `projection_freshness` — staleness and checkpoint info

### Other endpoints

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/v1/memory/context/{tenant}/{subject}` | Context bundle |
| GET | `/v1/memory/decisions/{tenant}/{subject}/history` | Full decision audit trail |
| POST | `/v1/memory/events` | Write a memory event |
| GET | `/v1/memory/status` | Projection checkpoints |
| GET | `/healthz` | Health check |
| GET | `/readyz` | Readiness check |
| GET | `/metrics` (port 8085) | Prometheus metrics |

## Memory Model

Eight event types, stored as `MemoryEventV1` JSON:

| Event Type | Purpose |
|-----------|---------|
| `observation.recorded` | What someone saw |
| `assertion.recorded` | What someone claims |
| `dissent.recorded` | Disagreement with an assertion or decision |
| `decision.recorded` | An authorized operating position |
| `decision.superseded` | A decision replacing a previous one |
| `action.authorized` | An action approved under a decision |
| `action.completed` | An action that finished |
| `outcome.recorded` | What happened after |

Claims have states (`asserted`, `dissented`, `decided`, `superseded`) — never `true` or `false`.

## Production Readiness

**Current status: NO-GO (30/100)** — expected for an initial OSS vertical slice.

The evidence matrix tracks 35 claims across correctness, contracts, security, performance, resilience, and observability. 18 are YELLOW (fixture-tested), 17 are RED (deferred or not yet measured). See `tests/shared_memory/EVIDENCE_MATRIX.md`.

Production requires real PostgreSQL integration tests under load, OpenShift deployment evidence, security hardening, and operational validation.

## Project Structure

```
src/
  contract/          MemoryEventV1, ContextBundleV1
  ledger/            LedgerReader trait (read-only access to ledger entries)
  projection/        ProjectionRepository, Projector engine
  context/           ContextService, HTTP API
  auth/              AuthorizationProvider trait, DevAuthProvider
  metrics.rs         Prometheus counters and histograms
  config.rs          Environment-based configuration
migrations/          PostgreSQL schema (are_memory)
tests/               53 tests: unit, BDD lifecycle, idempotency
```

## What's Next

1. **OpenShell OCSF adapter** — consequential-event admission from sandbox runtime
2. **OpenShift manifests** — deployment, SCC, NetworkPolicy, probes
3. **Ledger integration** — projector reads from real `are-immutable-ledger` database
4. **Security hardening** — cross-tenant tests, classification enforcement
5. **Performance baselines** — latency budgets, projection lag measurement
6. **White paper** — formal treatment of the divergent-memory thesis

## License

Apache-2.0
