# Production Readiness Rubric — Projections

## Scoring: 0-4 per category

- **0**: Absent
- **1**: Designed but not demonstrated
- **2**: Demonstrated in development or fixtures
- **3**: Demonstrated in a production-like environment
- **4**: Reproducible, controlled, observed, documented, and operationally owned

## Categories and Weights

| Category | Weight | Current Score | Weighted |
|----------|--------|--------------|----------|
| Correctness and deterministic projection | 15 | 2 | 30 |
| Contracts and compatibility | 10 | 2 | 20 |
| Security, identity, and authorization | 15 | 1 | 15 |
| Tenant isolation, privacy, and data governance | 10 | 1 | 10 |
| Resilience, replay, backup, and recovery | 10 | 2 | 20 |
| Performance, scale, and backpressure | 10 | 1 | 10 |
| Observability and operations | 10 | 1 | 10 |
| OpenShift and OpenShell integration | 10 | 0 | 0 |
| Supply-chain and evidence integrity | 5 | 0 | 0 |
| Documentation, usability, and runbooks | 5 | 1 | 5 |

**Current total: 120/400 = 30/100**

## Production Requirements (not yet met)

- [ ] At least 90/100
- [ ] No critical RED rows
- [ ] Every P0 and P1 requirement GREEN
- [ ] No security or tenant-isolation category below GREEN
- [ ] No expired waiver
- [ ] Two consecutive successful production-candidate executions
- [ ] Production-like OpenShift deployment
- [ ] Contract-compatibility evidence
- [ ] Representative load evidence
- [ ] Soak evidence
- [ ] Backup and restore evidence
- [ ] Deterministic replay evidence (with real PostgreSQL)
- [ ] Upgrade and rollback evidence
- [ ] Security and abuse-test evidence
- [ ] Observability and alert validation
- [ ] Independent human review
- [ ] Explicit human promotion approval

## Current Verdict: NO-GO

This is expected for an initial OSS vertical slice. The architecture is sound, the core behaviors are demonstrated with in-memory fixtures, but production readiness requires real PostgreSQL integration tests, OpenShift deployment evidence, security hardening, and operational validation.

## Next Steps to Improve Score

1. Run integration tests against real PostgreSQL (+1 on correctness, resilience)
2. Add cross-tenant security tests (+1 on security)
3. Add OpenShift deployment manifests (+1 on OpenShift)
4. Add performance benchmarks (+1 on performance)
5. Add health/readiness probes (+1 on observability)
