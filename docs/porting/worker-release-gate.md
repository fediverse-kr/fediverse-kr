# PG worker release gate — 2026-09-14

## Decision

**Keep the existing PostgreSQL queue for this release.** No core queue redesign
was found in this bounded gate; do not add Kameo, NATS, a second worker
process, or a generic queue abstraction. `runtime.rs` owns task supervision
only. Lease claim/reclaim, fencing, completion, and directory policy remain
the PostgreSQL repository's concrete responsibility.

## Current evidence

- On the isolated loopback PG17 cluster at `.local/postgres` / `127.0.0.1:16439`
  / `fedkr_test`, the existing exact test
  `backend::db::directory::tests::postgres_queue_leases_fencing_editorial_isolation_and_maintenance`
  passed (one test, 252 filtered). It covers idempotent enqueue, expired lease
  recovery, a new lease token, late/duplicate-result fencing, hidden-site
  collection, closed-site exclusion, operator name/description preservation,
  and closure while a request is in flight.
- The checked implementation records a remote collection failure as an
  observation and completes that job; a database/worker failure returns to the
  supervisor, whose restart delay is 1–60 seconds. This distinction must not
  be collapsed into an HTTP retry loop.
- The runtime has five directory-fetch tasks, a 60-second collection cap, a
  120-second PG lease, bounded 25-second shutdown drain, and lease recovery
  after a hard stop. Profile-refresh and media-cleanup are separate supervised
  loops; their failure returns to the same bounded supervisor policy.
- Earlier records report the full isolated Windows PG suite (253 tests,
  including supervisor/recovery) and a prior Linux/OCI candidate as passing.
  The latest worker monitor is not included in that OCI evidence.
- Final candidate `a092c1cdc640` now passed the existing Linux/OCI runner:
  219 checks in 80.11 seconds, synthetic-only. The raw ELF path verified
  administrator monitor access/redaction, owned-PG interruption, liveness vs
  readiness, recovery, and one newly queued job completed once. It also
  retained expired-lease/profile recovery and bounded SIGTERM checks. The OCI
  path verified its existing baked image/media checks; it did not repeat the
  DB fault injection. See `final-release-candidate.md` for exact hashes.

## Still required before production approval

- Measure one real target-sized collection cycle: throughput, HTTP impact,
  memory, and PostgreSQL connections. The configured limits are not load
  proof.
- Verify real remote servers, deployment TLS/proxy behavior, and live data
  policy separately. Synthetic loopback tests do not prove those conditions.

The local PG17 test cluster was started for this gate and stopped afterward.
