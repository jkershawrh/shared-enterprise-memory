#!/usr/bin/env python3
"""Generate Projections deck using the Red Hat template layouts only.

Creates fresh slides from the template's slide layouts — no Cascade
Compression content carried over. Only the visual branding.
"""

from pptx import Presentation
from pptx.enum.text import PP_ALIGN
import os

TEMPLATE = os.path.expanduser(
    "~/Library/CloudStorage/GoogleDrive-jkershaw@redhat.com/"
    "My Drive/Cascade-Compression-Whitepaper-Deck.pptx"
)
OUTPUT = os.path.join(os.path.dirname(os.path.dirname(__file__)),
                      "Projections-Enterprise-Memory-Deck.pptx")


def find_layout(prs, name):
    for layout in prs.slide_layouts:
        if layout.name == name:
            return layout
    raise ValueError(f"Layout '{name}' not found")


def set_placeholder(slide, idx, text):
    """Set placeholder text by index, preserving formatting."""
    for ph in slide.placeholders:
        if ph.placeholder_format.idx == idx:
            ph.text = text
            return ph
    return None


def set_placeholder_lines(slide, idx, lines):
    """Set placeholder with multiple paragraphs."""
    for ph in slide.placeholders:
        if ph.placeholder_format.idx == idx:
            tf = ph.text_frame
            tf.paragraphs[0].text = lines[0]
            for line in lines[1:]:
                p = tf.add_paragraph()
                p.text = line
            return ph
    return None


def add_notes(slide, text):
    ns = slide.notes_slide
    ns.notes_text_frame.text = text


def main():
    prs = Presentation(TEMPLATE)

    # Delete all existing slides
    while len(prs.slides) > 0:
        rId = prs.slides._sldIdLst[0].get('{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id')
        if rId is None:
            rId_attr = list(prs.slides._sldIdLst[0].attrib.keys())
            for attr in rId_attr:
                if 'id' in attr.lower() and 'r:' in attr.lower() or 'relationships' in attr.lower():
                    rId = prs.slides._sldIdLst[0].get(attr)
                    break
        prs.part.drop_rel(rId)
        prs.slides._sldIdLst.remove(prs.slides._sldIdLst[0])

    # ============================================================
    # SLIDE 1: TITLE  (layout: TITLE_2_1)
    # ph[0]=TITLE, ph[1]=SUBTITLE, ph[2]=SUBTITLE, ph[3]=SUBTITLE
    # ============================================================
    slide = prs.slides.add_slide(find_layout(prs, "TITLE_2_1"))
    set_placeholder(slide, 0, "Projections")
    set_placeholder(slide, 1, "Shared Enterprise Memory for AI Agents")
    set_placeholder(slide, 2, "Technical Foundation & MVP")
    add_notes(slide, "Title slide. Projections: shared enterprise memory that preserves disagreement, tracks decisions with provenance, and serves AI agents without fabricating consensus.")

    # ============================================================
    # SLIDE 2: THE PROBLEM  (layout: CUSTOM_4_1_1)
    # ph[0]=TITLE, ph[1]=SUBTITLE, ph[2]=SUBTITLE, ph[3]=BODY, ph[4]=BODY
    # ============================================================
    slide = prs.slides.add_slide(find_layout(prs, "CUSTOM_4_1_1"))
    set_placeholder(slide, 0, "The Problem")
    set_placeholder_lines(slide, 3, [
        "Agents fabricate organizational memory.",
        "",
        "Enterprise AI agents retrieve context from summaries, embeddings, "
        "and chat history. When two people remember an event differently, "
        "the system picks one version or blends them.",
        "",
        "Dissent disappears. Superseded decisions look current. "
        "Nobody can trace why an agent acts on stale or contested information.",
        "",
        "This is institutional hallucination — not a failure of the model, "
        "but a failure of the memory architecture.",
    ])
    set_placeholder_lines(slide, 4, [
        "The Core Insight",
        "",
        "Enterprise memory is not truth.",
        "",
        "It is observations, assertions, dissent, decisions, "
        "and outcomes — each with provenance, scope, and accountability.",
        "",
        "An approved decision is an authorized operating position, "
        "not objective fact.",
    ])
    add_notes(slide, "The problem: agents fabricate organizational memory. Current systems — RAG, embeddings, summaries — flatten divergent human recollections into a single narrative. The result is institutional hallucination: agents acting on manufactured consensus that no human actually agreed to.")

    # ============================================================
    # SLIDE 3: THREE-LAYER ARCHITECTURE  (layout: CUSTOM_4_17_1)
    # ph[0]=TITLE, ph[1]=SUBTITLE, ph[2]=BODY, ph[3]=BODY, ph[4]=SUBTITLE, ph[5]=SUBTITLE
    # ============================================================
    slide = prs.slides.add_slide(find_layout(prs, "CUSTOM_4_17_1"))
    set_placeholder(slide, 0, "Three-Layer Architecture")
    set_placeholder(slide, 4, "Ledger records. Projections interpret. Agents query.")
    set_placeholder_lines(slide, 2, [
        "EVIDENCE LAYER",
        "Append-only, hash-chained cryptographic ledger",
        "Preserves all consequential events immutably",
        "V2/V3 proof envelopes, transactional outbox",
        "Cannot be silently rewritten",
        "",
        "PROJECTION LAYER",
        "Idempotent projector materializes read-optimized views",
        "Tracks decisions, dissent, supersession, observations",
        "Deterministic: rebuildable from scratch",
        "Zero ledger calls on the agent read path",
        "",
        "CONTEXT API",
        "Authorized, bounded context bundles for agents",
        "Tenant-scoped with provenance and dissent",
        "Exposes what's current, who disagreed, and why",
    ])
    set_placeholder_lines(slide, 5, [
        "6 BDD scenarios proven end-to-end",
        "53 tests passing (fmt + clippy + test)",
    ])
    add_notes(slide, "Three layers: immutable ledger (evidence spine), projections (derived read-optimized views), context API (authorized bundles for agents). The key constraint: agents never scan the ledger. Normal reads make zero ledger calls.")

    # ============================================================
    # SLIDE 4: ARCHITECTURE DETAIL  (layout: CUSTOM_4_1_1)
    # ============================================================
    slide = prs.slides.add_slide(find_layout(prs, "CUSTOM_4_1_1"))
    set_placeholder(slide, 0, "Architecture")
    set_placeholder_lines(slide, 3, [
        "Three layers. Agents never scan the ledger.",
        "",
        "Events are written to the immutable ledger.",
        "The projector consumes committed entries and materializes "
        "tenant-scoped views into the are_memory schema.",
        "The Context API serves authorized bundles from projections "
        "only — zero ledger calls on the read path.",
        "",
        "POST /v1/memory/events     Write memory events",
        "GET  /v1/memory/context     Read context bundles",
        "GET  /v1/memory/status      Projection health",
        "GET  /healthz               Liveness probe",
        "GET  /readyz                Readiness probe",
        "GET  /metrics               Prometheus metrics",
    ])
    set_placeholder_lines(slide, 4, [
        "Projector Engine",
        "",
        "1. Read committed ledger entries by chain",
        "2. Deserialize MemoryEventV1 from content",
        "3. Dispatch by event type",
        "4. Upsert into projection tables (idempotent)",
        "5. Advance per-chain checkpoint",
        "6. Link every row back to its ledger evidence",
        "",
        "PostgreSQL: are_memory schema",
        "6 tables, fully rebuildable",
    ])
    add_notes(slide, "Architecture detail. Events flow in via POST, projector materializes views, agents query projections. Every projected row links back to its source ledger entry via evidence_links. The projector handles malformed events, duplicate delivery, and checkpoint recovery.")

    # ============================================================
    # SLIDE 5: MEMORY MODEL  (layout: CUSTOM_4_1_1)
    # ============================================================
    slide = prs.slides.add_slide(find_layout(prs, "CUSTOM_4_1_1"))
    set_placeholder(slide, 0, "Memory Model")
    set_placeholder_lines(slide, 3, [
        "Not facts. Claims with provenance and accountability.",
        "",
        "8 Event Types:",
        "  observation.recorded    What someone saw",
        "  assertion.recorded      What someone claims",
        "  dissent.recorded        Disagreement with a claim",
        "  decision.recorded       Authorized operating position",
        "  decision.superseded     New decision replaces old",
        "  action.authorized       Action approved under decision",
        "  action.completed        Action finished",
        "  outcome.recorded        What happened after",
        "",
        "Claim states: asserted | dissented | decided | superseded",
        "Never: true | false | verified | fact",
    ])
    set_placeholder_lines(slide, 4, [
        "MemoryEventV1 Contract",
        "",
        "event_id, event_type, schema_version",
        "tenant_id, subject_id",
        "actor (id + type: human/agent/system)",
        "recorded_at, effective_at (bitemporal)",
        "correlation_id, supersedes_event_id",
        "classification, claim_state",
        "payload (JSON)",
        "",
        "Tenant Isolation",
        "Auth checked before every query",
        "Cross-tenant access denied",
    ])
    add_notes(slide, "Memory model. Eight event types with four claim states. Claims are never labeled true or false — they are asserted, dissented, decided, or superseded. Tenant isolation is enforced at the application layer: authorization runs before any projection query.")

    # ============================================================
    # SLIDE 6: WHAT THE MVP PROVES  (layout: CUSTOM_4_17_1)
    # ============================================================
    slide = prs.slides.add_slide(find_layout(prs, "CUSTOM_4_17_1"))
    set_placeholder(slide, 0, "What the MVP Proves")
    set_placeholder(slide, 4, "Six scenarios. Real PostgreSQL. Zero fabricated consensus.")
    set_placeholder_lines(slide, 2, [
        "Scenario 1: Divergent Observations",
        "Alice and Bob observe the same incident differently. Both preserved.",
        "",
        "Scenario 2: Dissent",
        "Bob disagrees with Alice's root cause assertion. Target flagged.",
        "",
        "Scenario 3: Decision Despite Dissent",
        "Carol decides. Dissent is NOT erased.",
        "",
        "Scenario 4: Supersession",
        "Carol changes her mind. New decision current, old in history.",
        "",
        "Scenario 5: Zero Ledger Calls",
        "Context retrieval makes zero calls to the evidence spine.",
        "",
        "Scenario 6: Tenant Isolation",
        "Unauthorized agent denied. Authorized agent gets full context.",
    ])
    set_placeholder_lines(slide, 5, [
        "53 tests | 6 BDD scenarios | fmt + clippy clean",
        "Dissent preserved: 100%",
    ])
    add_notes(slide, "Six BDD scenarios proven end-to-end. The critical proof: dissent survives decisions, supersession never erases history, context retrieval makes zero ledger calls, and unauthorized agents are denied access. All 53 tests pass.")

    # ============================================================
    # SLIDE 7: WHY NOT RAG  (layout: CUSTOM_4_1_1)
    # ============================================================
    slide = prs.slides.add_slide(find_layout(prs, "CUSTOM_4_1_1"))
    set_placeholder(slide, 0, "Why Not Just RAG?")
    set_placeholder_lines(slide, 3, [
        "RAG answers: what documents are similar to this query?",
        "",
        "Enterprise memory must answer:",
        "  What is the current authorized operating position?",
        "  Who disagreed, and why?",
        "  What preceded this decision?",
        "  What evidence supports it?",
        "  Is this decision still effective?",
        "",
        "RAG does not:",
        "  Distinguish observations from decisions",
        "  Preserve dissent alongside decisions",
        "  Track supersession",
        "  Enforce tenant isolation",
        "  Expose provenance or claim states",
    ])
    set_placeholder_lines(slide, 4, [
        "Projections vs Alternatives",
        "",
        "Vector DB",
        "Semantic similarity, not governance. No dissent,",
        "no supersession, no authorization scope.",
        "",
        "Knowledge Graph",
        "Models relationships, but no immutable provenance,",
        "no deterministic replay, no current vs historical.",
        "",
        "Event Sourcing",
        "Closest analog. Projections adds: claim states,",
        "dissent as first-class, tenant isolation,",
        "cryptographic evidence spine.",
    ])
    add_notes(slide, "Why not RAG, vector DBs, or knowledge graphs? RAG answers what's semantically similar. Enterprise memory must answer what's current, who disagreed, and what evidence supports it. Projections is event sourcing applied to enterprise memory with dissent, supersession, and tenant isolation as first-class concepts.")

    # ============================================================
    # SLIDE 8: LIVE DEMO  (layout: CUSTOM_4_17_1)
    # ============================================================
    slide = prs.slides.add_slide(find_layout(prs, "CUSTOM_4_17_1"))
    set_placeholder(slide, 0, "Live Demo")
    set_placeholder(slide, 4, "Write events. Query context. See disagreement preserved.")
    set_placeholder_lines(slide, 2, [
        "Step 1: Alice observes \"server was overloaded\"",
        "  POST /v1/memory/events  (observation.recorded)",
        "",
        "Step 2: Bob observes \"network was congested\"",
        "  POST /v1/memory/events  (observation.recorded)",
        "",
        "Step 3: Alice asserts \"root cause is memory leak\"",
        "  POST /v1/memory/events  (assertion.recorded)",
        "",
        "Step 4: Bob dissents — \"metrics show network saturation\"",
        "  POST /v1/memory/events  (dissent.recorded)",
        "",
        "Step 5: Carol decides — \"apply memory limit fix\"",
        "  POST /v1/memory/events  (decision.recorded)",
        "",
        "Step 6: Carol supersedes — \"revert to network config fix\"",
        "  POST /v1/memory/events  (decision.superseded)",
        "",
        "Query: GET /v1/memory/context/acme/incident-42",
        "Result: 1 current decision, 2 observations, 1 contested assertion, 1 dissent",
    ])
    add_notes(slide, "Live demo walkthrough. Six events posted via curl, then the context bundle shows: one current decision (the superseding one), both observations preserved, Alice's assertion flagged as contested, Bob's dissent visible. Full decision history available via separate endpoint showing both decisions.")

    # ============================================================
    # SLIDE 9: GOVERNANCE  (layout: CUSTOM_4_1_1)
    # ============================================================
    slide = prs.slides.add_slide(find_layout(prs, "CUSTOM_4_1_1"))
    set_placeholder(slide, 0, "Trust & Governance")
    set_placeholder_lines(slide, 3, [
        "Three boundaries. Evidence is immutable. Views are derived.",
        "",
        "1. Ledger Preserves",
        "Append-only, hash-chained. No UPDATE, no DELETE.",
        "V2/V3 proof envelopes. Tamper-evident.",
        "",
        "2. Projections Derive",
        "Rebuildable views materialized from ledger events.",
        "Idempotent. Deterministic replay = identical state.",
        "",
        "3. Context API Enforces",
        "Tenant isolation. Authorization before query.",
        "Cross-tenant access denied. Classification-aware.",
    ])
    set_placeholder_lines(slide, 4, [
        "Evidence Matrix: 35 Claims Tracked",
        "",
        "RED: Missing, failing, or untested",
        "YELLOW: Demonstrated in development/fixtures",
        "GREEN: Reproducible automated evidence",
        "",
        "Current: 18 YELLOW, 17 RED",
        "Score: 30/100 — NO-GO",
        "(Expected for initial vertical slice)",
        "",
        "Production requires:",
        "  90/100, no critical RED rows",
        "  OpenShift deployment evidence",
        "  Load, soak, backup/restore evidence",
        "  Independent human review",
    ])
    add_notes(slide, "Trust and governance. Three boundaries: ledger preserves (immutable), projections derive (rebuildable), context API enforces (authorization). Evidence matrix tracks 35 falsifiable claims. Current score 30/100, NO-GO — honest and expected for a first vertical slice. Production readiness is earned, not declared.")

    # ============================================================
    # SLIDE 10: GET STARTED  (layout: CUSTOM_4_1_1)
    # ============================================================
    slide = prs.slides.add_slide(find_layout(prs, "CUSTOM_4_1_1"))
    set_placeholder(slide, 0, "Get Started")
    set_placeholder_lines(slide, 3, [
        "From zero to working enterprise memory.",
        "",
        "Prerequisites:",
        "  Podman, Rust toolchain (1.87+)",
        "",
        "Quick Start:",
        "  git clone <repo> && cd projections",
        "  make check    # fmt + clippy + 53 tests",
        "  make up       # Podman: PostgreSQL + migration",
        "  make run      # Start on :18084 (API) :18085 (metrics)",
        "  make smoke    # Endpoint checks",
        "  make build    # Container image (110 MB)",
    ])
    set_placeholder_lines(slide, 4, [
        "What's Next",
        "",
        "OpenShell OCSF adapter",
        "  Consequential-event admission from sandbox runtime",
        "",
        "OpenShift deployment manifests",
        "  SCC, NetworkPolicy, probes, non-root",
        "",
        "Ledger integration",
        "  Projector reads from real are-immutable-ledger",
        "",
        "Security hardening",
        "  Cross-tenant tests, classification enforcement",
        "",
        "White paper",
        "  Formal treatment of the divergent-memory thesis",
    ])
    add_notes(slide, "Get started: three commands. What's next: OpenShell adapter, OpenShift manifests, ledger integration, security hardening, and the formal white paper. No GPU needed, no vector DB, no knowledge graph. Podman + PostgreSQL + Rust.")

    # ============================================================
    # SLIDE 11: CLOSING  (layout: TITLE_2_1)
    # ============================================================
    slide = prs.slides.add_slide(find_layout(prs, "TITLE_2_1"))
    set_placeholder(slide, 0, "Projections")
    set_placeholder(slide, 1, "Enterprise memory that preserves disagreement.")
    set_placeholder(slide, 2, "Because consensus without dissent is hallucination.")
    add_notes(slide, "Closing. Enterprise memory that preserves disagreement. Because consensus without dissent is hallucination.")

    prs.save(OUTPUT)
    print(f"Deck saved to: {OUTPUT}")
    print(f"Slides: {len(prs.slides)}")


if __name__ == "__main__":
    main()
