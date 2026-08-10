-- Projections MVP: enterprise memory read-optimized views.
-- All tables live in are_memory schema, logically separate from
-- the immutable are_ledger schema. These tables CAN be dropped
-- and rebuilt from ledger entries at any time.

CREATE SCHEMA IF NOT EXISTS are_memory;

-- Projector checkpoint: tracks last processed position per entry_type chain.
CREATE TABLE are_memory.projection_checkpoints (
    entry_type VARCHAR(50) PRIMARY KEY,
    last_position BIGINT NOT NULL DEFAULT 0,
    last_entry_id UUID NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Current effective decisions by tenant + subject.
CREATE TABLE are_memory.current_decisions (
    decision_id UUID PRIMARY KEY,
    tenant_id VARCHAR(100) NOT NULL,
    subject_id VARCHAR(255) NOT NULL,
    actor_id VARCHAR(255) NOT NULL,
    actor_type VARCHAR(50) NOT NULL,
    decision_content JSONB NOT NULL,
    effective_at TIMESTAMPTZ NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL,
    superseded_by UUID NULL,
    is_current BOOLEAN NOT NULL DEFAULT TRUE,
    ledger_entry_id UUID NOT NULL,
    correlation_id VARCHAR(255) NULL,
    classification VARCHAR(50) NOT NULL DEFAULT 'internal',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_cd_tenant_subject_current
    ON are_memory.current_decisions(tenant_id, subject_id)
    WHERE is_current = TRUE;
CREATE INDEX idx_cd_tenant
    ON are_memory.current_decisions(tenant_id);

-- Full decision history including superseded decisions.
CREATE TABLE are_memory.decision_history (
    history_id UUID PRIMARY KEY,
    decision_id UUID NOT NULL,
    tenant_id VARCHAR(100) NOT NULL,
    subject_id VARCHAR(255) NOT NULL,
    actor_id VARCHAR(255) NOT NULL,
    actor_type VARCHAR(50) NOT NULL,
    decision_content JSONB NOT NULL,
    effective_at TIMESTAMPTZ NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL,
    supersedes_event_id UUID NULL,
    claim_state VARCHAR(50) NOT NULL,
    ledger_entry_id UUID NOT NULL,
    correlation_id VARCHAR(255) NULL,
    classification VARCHAR(50) NOT NULL DEFAULT 'internal',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_dh_tenant_subject
    ON are_memory.decision_history(tenant_id, subject_id);
CREATE INDEX idx_dh_decision
    ON are_memory.decision_history(decision_id);

-- Observations, assertions, actions, outcomes.
CREATE TABLE are_memory.memory_records (
    record_id UUID PRIMARY KEY,
    tenant_id VARCHAR(100) NOT NULL,
    subject_id VARCHAR(255) NOT NULL,
    event_type VARCHAR(50) NOT NULL,
    actor_id VARCHAR(255) NOT NULL,
    actor_type VARCHAR(50) NOT NULL,
    record_content JSONB NOT NULL,
    effective_at TIMESTAMPTZ NULL,
    recorded_at TIMESTAMPTZ NOT NULL,
    claim_state VARCHAR(50) NOT NULL,
    has_dissent BOOLEAN NOT NULL DEFAULT FALSE,
    correlation_id VARCHAR(255) NULL,
    classification VARCHAR(50) NOT NULL DEFAULT 'internal',
    ledger_entry_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_mr_tenant_subject
    ON are_memory.memory_records(tenant_id, subject_id);
CREATE INDEX idx_mr_tenant_event_type
    ON are_memory.memory_records(tenant_id, event_type);

-- Dissent records linked to target events.
CREATE TABLE are_memory.dissent_records (
    dissent_id UUID PRIMARY KEY,
    tenant_id VARCHAR(100) NOT NULL,
    subject_id VARCHAR(255) NOT NULL,
    target_event_id UUID NOT NULL,
    actor_id VARCHAR(255) NOT NULL,
    actor_type VARCHAR(50) NOT NULL,
    dissent_content JSONB NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL,
    ledger_entry_id UUID NOT NULL,
    correlation_id VARCHAR(255) NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_dr_target
    ON are_memory.dissent_records(target_event_id);
CREATE INDEX idx_dr_tenant_subject
    ON are_memory.dissent_records(tenant_id, subject_id);

-- Evidence links: every projected row links back to its source ledger entry.
CREATE TABLE are_memory.evidence_links (
    link_id UUID PRIMARY KEY,
    projected_table VARCHAR(50) NOT NULL,
    projected_id UUID NOT NULL,
    ledger_entry_id UUID NOT NULL,
    entry_type VARCHAR(50) NOT NULL,
    entry_hash VARCHAR(64) NOT NULL,
    chain_position BIGINT NOT NULL,
    linked_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_el_projected_ledger
    ON are_memory.evidence_links(projected_table, projected_id, ledger_entry_id);
CREATE INDEX idx_el_ledger
    ON are_memory.evidence_links(ledger_entry_id);
