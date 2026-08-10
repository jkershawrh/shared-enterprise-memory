# Projections: Shared Enterprise Memory for AI Agents

**A governed memory architecture that preserves disagreement, tracks decisions with provenance, and serves agents without fabricating consensus.**

---

## Abstract

Enterprise AI agents need organizational context to act — active decisions, current commitments, applicable constraints, unresolved disputes. Today, agents retrieve this context from summaries, embeddings, and retrieval-augmented generation (RAG) pipelines that silently flatten divergent human recollections into a single narrative. The result is institutional hallucination: agents act on manufactured consensus that no human actually agreed to.

Projections is a three-layer memory architecture that separates immutable evidence from derived state. An append-only cryptographic ledger preserves observations, assertions, dissent, decisions, supersession, and outcomes as they occurred. A deterministic projection layer materializes read-optimized views that agents can query without scanning the ledger. A context API returns bounded, authorized, tenant-scoped bundles that expose provenance, trust state, and unresolved disputes alongside effective decisions.

The result: agents receive actionable organizational context where disagreement is visible, supersession is tracked, and no voice is silently erased.

This paper presents the architecture, the memory model, a working MVP with 53 automated tests across 6 behavioral scenarios, and the production path forward.

---

## 1. The Problem: Institutional Hallucination

### 1.1 How Enterprise Memory Fails Today

When an enterprise deploys AI agents that need organizational context, the standard approach is some combination of:

- **Summarization**: An LLM condenses meeting notes, decisions, and discussions into a paragraph. Dissent is lost. Nuance is flattened. The summary becomes the canonical record.

- **Embedding + retrieval**: Documents are chunked, embedded, and stored in a vector database. Semantic similarity determines what an agent sees. Two contradictory reports about the same incident return based on cosine distance, not governance authority.

- **Chat history**: Agents inherit context from prior conversations. What was said most recently, or most often, dominates. A decision that was superseded three months ago may still appear as active if no one explicitly corrected the thread.

None of these approaches distinguish between what someone observed, what someone claimed, what someone disputed, what was officially decided, and what was later changed. They collapse these fundamentally different epistemic categories into a single data type: text that looks authoritative.

### 1.2 The Consequence

When an agent retrieves a summary that says "the team decided to proceed with Option A," it cannot know:

- Whether that decision was contested
- Whether it was later superseded by Option B
- Whether the person who recorded it had the authority to make it
- Whether two other team members remember the meeting differently
- Whether the decision has an expiration date or scope limitation

The agent treats the summary as ground truth. It acts on it. It may recommend actions, allocate resources, or deny requests based on a narrative that no individual human actually endorsed in its totality.

This is institutional hallucination — not a failure of the LLM, but a failure of the memory architecture.

### 1.3 Why RAG Doesn't Solve This

Retrieval-augmented generation improves factual grounding by connecting models to source documents. But RAG does not:

- Distinguish observations from assertions from decisions
- Preserve dissent alongside the decision it contested
- Track supersession (which decision replaced which)
- Enforce tenant isolation or authorization scope
- Expose provenance, effective dates, or claim states
- Prevent the most-frequently-retrieved summary from becoming de facto truth

RAG answers "what documents are most similar to this query." Enterprise memory must answer "what is the current authorized operating position, who disagrees, what preceded it, and what evidence supports it."

---

## 2. Core Thesis: Memory as Claims, Not Facts

### 2.1 The Epistemic Distinction

People do not remember organizational events identically. This is not a bug — it is a fundamental property of how human organizations operate. Two engineers observing the same production incident may genuinely see different root causes. A decision-maker may approve a course of action despite valid dissent. A policy may be superseded by a new policy without the old one being wrong at the time it was made.

Enterprise memory must preserve these distinctions:

| Category | What It Is | Example |
|----------|-----------|---------|
| **Observation** | What a source reports seeing | "Server CPU hit 98% at 09:45" |
| **Assertion** | What someone claims | "Root cause is a memory leak" |
| **Dissent** | A disagreement with a claim | "Disagree — metrics show network saturation" |
| **Decision** | An authorized operating position | "Apply memory limits to auth service" |
| **Supersession** | A decision replacing an earlier one | "Revert to network config fix instead" |
| **Action** | Something requested, authorized, or executed | "Deploy config change to production" |
| **Outcome** | What was subsequently observed | "Latency returned to baseline" |

An approved decision is not objective truth. It is an authorized operating position with scope, evidence, effective dates, dissent, and expected outcomes.

### 2.2 The Design Principle

**Never convert repeated retrieval into truth.** If an agent retrieves the same summary 10,000 times, that summary does not become more true. If a decision was superseded, it must not appear as current regardless of how many embeddings still point to it. If dissent was recorded, it must remain visible even after the organization proceeds with a decision.

The system must make institutional hallucination structurally impossible — not by trusting the model to be careful, but by making the data architecture enforce it.

---

## 3. Architecture: Three-Layer Separation

### 3.1 Overview

```
Agents / Enterprise Systems
    │
    ▼
Context API ──────────────► Projection Store (are_memory)
  (HTTP :8084)                      ▲
  tenant-scoped                     │
  authorized                   Projector
  bounded                     (background task)
                                    ▲
                                    │
                           Immutable Ledger (are_ledger)
                           append-only, hash-chained
```

### 3.2 Layer Responsibilities

**Layer 1: Immutable Ledger (Evidence Spine)**

The ledger is an append-only, hash-chained store of consequential events. Each entry has a cryptographic hash binding it to the previous entry in its chain. No entry can be silently modified or deleted.

The ledger stores observations, assertions, decisions, dissent, actions, and outcomes as opaque content bytes with structured metadata. It does not interpret the content. It preserves it.

The ledger is the evidence plane. It answers: "what was recorded, when, by whom."

**Layer 2: Projections (Memory Serving Plane)**

Projections are derived, read-optimized views materialized from ledger events by a deterministic projector. They answer questions like "what is the current effective decision for subject X" without requiring the caller to scan the full ledger history.

Projections are:
- **Rebuildable** — drop all projection tables, replay from ledger, get identical state
- **Idempotent** — processing the same event twice produces no duplicates
- **Tenant-scoped** — queries are always filtered by tenant
- **Disposable** — the ledger is authoritative; projections are convenience

The projector maintains per-chain checkpoints and processes events in order. It handles supersession (marking old decisions non-current), dissent (flagging contested assertions), and evidence linking (connecting every projected row back to its source ledger entry).

**Layer 3: Context API (Agent Working Memory)**

The Context API returns `ContextBundleV1` — a bounded, authorized response containing:
- Effective decisions (current only, superseded excluded)
- Observations from all actors
- Assertions with dissent flags
- Dissent records with their targets
- Evidence links back to ledger entries
- Projection freshness (staleness, checkpoint info)

The API enforces authorization before any query. A missing or invalid agent identity results in denial. Cross-tenant access is blocked. The agent never touches the ledger.

### 3.3 Why This Separation Matters

| Without separation | With separation |
|---|---|
| Agents scan the full event log | Agents read pre-materialized views |
| Every read hits the ledger | Zero ledger calls on the read path |
| Stale summaries look current | Supersession is tracked explicitly |
| Dissent is invisible | Dissent coexists with decisions |
| No audit trail from agent context to evidence | Every projected row links to its source |
| Rebuilding state requires replaying application logic | Projections rebuild deterministically from events |

---

## 4. Memory Model

### 4.1 MemoryEventV1

Every memory event conforms to a versioned contract (`MemoryEventV1`) containing:

- **event_id** — unique identifier
- **event_type** — one of eight types (see below)
- **tenant_id** — organizational boundary
- **subject_id** — what this event is about
- **actor** — who recorded it (id + type: human / agent / system)
- **recorded_at** — when the system accepted it
- **effective_at** — when it applies to the enterprise (bitemporal)
- **claim_state** — asserted, dissented, decided, superseded
- **classification** — internal, restricted, public
- **supersedes_event_id** — what this event replaces (for dissent or supersession)
- **correlation_id** — cross-system tracing
- **payload** — the substantive content (JSON)

### 4.2 Event Types

| Type | Claim State | Purpose |
|------|------------|---------|
| `observation.recorded` | asserted | A source reports what it saw |
| `assertion.recorded` | asserted | A person or system makes a claim |
| `dissent.recorded` | dissented | Someone disagrees with an assertion or decision |
| `decision.recorded` | decided | An authorized operating position is adopted |
| `decision.superseded` | superseded | A new decision replaces an earlier one |
| `action.authorized` | decided | An action is approved under a decision |
| `action.completed` | asserted | An action finishes |
| `outcome.recorded` | asserted | What happened after |

### 4.3 What Is Not Stored

- Raw chain-of-thought or model internals
- Every OpenShell/OCSF telemetry event (only consequential ones)
- Routine logs, heartbeats, or status polls
- Unfiltered agent conversations
- Sensitive payloads in full (references and digests instead)

---

## 5. Proof: MVP Demonstration

### 5.1 The Scenario

An incident occurs. Two engineers observe it differently. One asserts a root cause. The other dissents. A decision-maker approves a fix despite the dissent. Later, the decision is superseded after further analysis confirms the dissenting view. An agent queries the current state.

### 5.2 What the Agent Receives

```json
{
  "effective_decisions": [{
    "decision_id": "66666666-...",
    "actor": {"actor_id": "carol", "actor_type": "human"},
    "content": {
      "decision": "revert to network config fix after further analysis",
      "rationale": "bob was right - network saturation confirmed by infra team"
    },
    "is_current": true
  }],
  "observations": [
    {"actor": {"actor_id": "alice"}, "content": {"observation": "server was overloaded at 09:45"}},
    {"actor": {"actor_id": "bob"}, "content": {"observation": "network was congested, not the server"}}
  ],
  "assertions": [{
    "actor": {"actor_id": "alice"},
    "content": {"assertion": "root cause is memory leak in auth service"},
    "has_dissent": true
  }],
  "dissent": [{
    "actor": {"actor_id": "bob"},
    "target_event_id": "33333333-...",
    "content": {"reason": "disagree - metrics show network saturation, not memory pressure"}
  }]
}
```

The agent sees:
- **One current decision** — Carol's superseding decision, not the original
- **Two observations** — Alice's and Bob's, both preserved
- **One assertion flagged as contested** — Alice's claim, with `has_dissent: true`
- **The dissent itself** — Bob's reasoning, linked to Alice's assertion
- **Full decision history** available via a separate endpoint, showing both the original and superseding decision

No voice was silenced. No decision was erased. The agent has the current operating position and the full context for why it exists.

### 5.3 Verification

| Test Category | Count | Status |
|--------------|-------|--------|
| Unit tests | 44 | Passing |
| BDD lifecycle scenarios | 6 | Passing |
| Idempotency/replay tests | 3 | Passing |
| **Total** | **53** | **All green** |

Key properties verified:
- Divergent observations from different actors are both preserved
- Dissent is never erased by a subsequent decision
- Supersession updates current state without deleting history
- Replay from empty state produces identical projections
- Duplicate event delivery does not create duplicate rows
- Malformed events are skipped without halting the projector
- Context retrieval makes zero calls to the ledger
- Unauthorized agents are denied access

---

## 6. Governance and Trust

### 6.1 What Immutability Proves (and Doesn't)

A valid hash proves that a recorded event has not been silently rewritten. It does not prove that the event's claim is objectively true. A cryptographic signature can prove source authenticity. It still does not make the signed claim correct.

The system keeps these properties separate:
- Ledger-envelope integrity (hash valid)
- Writer identity (who submitted)
- Writer authority (were they authorized to submit this type)
- Claim state (asserted, dissented, decided, superseded)
- Evidence quality (is the underlying evidence strong)

### 6.2 Tenant Isolation

Every API request requires an agent identity. Authorization is checked before any projection query executes. The `DevAuthProvider` maps tenants to allowed agents. In production, this interface connects to enterprise identity systems (JWT, mTLS, SPIFFE).

Cross-tenant access is structurally prevented: projection queries always include `tenant_id` in the WHERE clause, and the authorization check runs before the query.

### 6.3 Evidence Links

Every projected row links back to its source ledger entry via the `evidence_links` table. An auditor can trace any context bundle element to the immutable record that produced it.

---

## 7. Red Hat Alignment

### 7.1 OpenShift Deployment Target

The service is designed for OpenShift deployment:
- Non-root container execution (Containerfile creates unprivileged user)
- No host namespace or privileged requirements
- PostgreSQL for projection storage (enterprise-grade, OpenShift-native)
- Prometheus-compatible metrics endpoint
- Health and readiness probes
- Structured JSON logging

### 7.2 OpenShell Integration Path

The architecture includes an adapter boundary for OpenShell (NVIDIA sandbox runtime) OCSF events. Consequential events — governed action denials, policy exceptions, security findings — are promoted into immutable evidence. Routine telemetry stays in observability systems.

This boundary is defined but not yet implemented in the MVP. The existing `are-immutable-ledger` repository already has OCSF and OTEL adapters that demonstrate the pattern.

### 7.3 Immutable Ledger Foundation

Projections builds on the `are-immutable-ledger` — an Apache-2.0 Rust-based cryptographic ledger with V2/V3 proof envelopes, per-chain advisory lock serialization, a transactional outbox, and portable proof receipts. PR #8 (V3 hardening) is the baseline.

---

## 8. Why Not Just...

### 8.1 ...a Vector Database?

Vector databases excel at semantic similarity search. They do not distinguish observations from decisions, track supersession, enforce tenant isolation at the data layer, or preserve dissent alongside the decision it contested. Semantic retrieval is a future optional layer over governed projections — not the source of truth.

### 8.2 ...a Knowledge Graph?

Knowledge graphs model relationships between entities. They can represent "Alice asserted X" and "Bob disputed X." But they do not inherently provide immutable provenance, hash-chained evidence, deterministic replay, or the distinction between current and historical state. A knowledge graph could be a projection format; it should not be the evidence spine.

### 8.3 ...RAG with Better Prompting?

Better prompts can instruct models to "consider multiple perspectives" or "note disagreements." But the data layer still delivers whatever is semantically closest. If the summary says "the team decided X," no prompt engineering recovers the dissent that was lost at summarization time. The architecture must preserve disagreement before the model sees it.

### 8.4 ...Event Sourcing?

Event sourcing is the closest analog. Projections is event sourcing applied to enterprise memory with specific additions: claim states (not just events), dissent as a first-class concept, supersession semantics, tenant isolation, authorization-scoped queries, and an immutable cryptographic ledger as the event store.

---

## 9. Production Path

### 9.1 Current Maturity

| Category | Score (0-4) | Status |
|----------|-------------|--------|
| Correctness and deterministic projection | 2 | Demonstrated in fixtures |
| Contracts and compatibility | 2 | Schema validated |
| Security, identity, authorization | 1 | Interface defined, dev impl |
| Tenant isolation and governance | 1 | Application-layer enforcement |
| Resilience, replay, recovery | 2 | Demonstrated in fixtures |
| Performance and scale | 1 | Zero-ledger-call proven, no benchmarks |
| Observability | 1 | Metrics registered, not validated |
| OpenShift / OpenShell integration | 0 | Not yet deployed |
| Supply-chain and evidence integrity | 0 | Not yet addressed |
| Documentation and runbooks | 1 | README and white paper |

**Current score: 30/100. Verdict: NO-GO.** This is expected and honest for an initial vertical slice.

### 9.2 Path to Production

| Phase | What | Score Impact |
|-------|------|-------------|
| **Next** | Real PostgreSQL integration tests, cross-tenant security tests | +15 |
| **Then** | OpenShift manifests, health probes, NetworkPolicy | +10 |
| **Then** | Ledger integration (projector reads from `are-immutable-ledger`) | +10 |
| **Then** | OpenShell OCSF adapter with fixture tests | +5 |
| **Then** | Performance baselines, soak tests | +10 |
| **Then** | Backup/restore, rollback evidence | +10 |
| **Production candidate** | Two consecutive GREEN runs in staging | Gate |

### 9.3 What Production Requires

- Score of 90/100 with no critical RED rows
- No security or tenant-isolation category below GREEN
- Production-like OpenShift deployment evidence
- Deterministic replay evidence (with real PostgreSQL)
- Backup and restore demonstration
- Independent human review and explicit promotion decision

---

## 10. Conclusion

Enterprise memory for AI agents is not a retrieval problem. It is a governance problem.

When agents act on organizational context, they need to know not just what was decided, but who disagreed, what was superseded, and what evidence supports the current position. Systems that flatten this into summaries or embeddings create institutional hallucination — agents acting on consensus that no human endorsed.

Projections solves this by separating immutable evidence from derived state, preserving disagreement as a first-class concept, and serving agents bounded context with full provenance. The MVP demonstrates this with 53 automated tests, 6 behavioral scenarios, and a working service that runs against real PostgreSQL.

The closing slide of this work says it plainly:

> **Enterprise memory that preserves disagreement. Because consensus without dissent is hallucination.**

---

*Projections is an open-source project built on the ARE Immutable Ledger. It is architecturally aligned with Red Hat OpenShift and OpenShell without claiming Red Hat endorsement, certification, or access to proprietary systems.*
