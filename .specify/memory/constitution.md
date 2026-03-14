<!--
  SYNC IMPACT REPORT
  ==================
  Version change: (new) → 1.0.0 (initial ratification)
  Added principles:
    - VII. Observability
  Modified principles:
    - Technical Decision Guidance: removed inline observability bullet (promoted to principle)
    - Quality Gates: added observability review gate
    - Governance: updated principle count reference (six → seven)
  Removed sections: N/A
  Templates reviewed:
    - .specify/templates/plan-template.md  ✅ Constitution Check section present; compatible
    - .specify/templates/spec-template.md  ✅ Functional requirements align with principles
    - .specify/templates/tasks-template.md ✅ Phase structure compatible; no updates required
  Follow-up TODOs: None.
-->

# Tracey Constitution

## Core Principles

### I. Code Quality (NON-NEGOTIABLE)

Every line of code merged into the main branch MUST meet the following baseline:

- Code MUST be reviewed by at least one other contributor before merging; no self-merges on
  shared branches.
- Functions and modules MUST have a single, clearly stated responsibility (Single-Responsibility
  Principle). Functions exceeding 40 logical lines are a signal for decomposition, not a hard limit.
- Cyclomatic complexity MUST NOT exceed 10 per function without explicit justification in code
  comments.
- Dead code, commented-out blocks, and `TODO`/`FIXME` markers MUST NOT be merged; convert open
  todos to tracked issues instead.
- Dependencies MUST be pinned to exact or minimum-compatible versions; transitive dependency
  upgrades require an explicit changelog entry.
- Linting and static analysis MUST pass with zero errors (warnings are permitted but tracked).

**Rationale**: Tracey runs continuously in the background on user systems. Defects, bloat, or
opaque code are disproportionately costly because the system is largely invisible to end-users
and hard to observe without purpose-built tooling.

### II. Testing Standards (NON-NEGOTIABLE)

- Tests MUST be written before or alongside implementation (TDD/BDD); no feature is considered
  complete without passing tests.
- Unit test coverage for business logic MUST be ≥ 80 % (measured by branch coverage, not just
  line coverage).
- Every user story MUST have at least one end-to-end or integration test covering the primary
  acceptance scenario.
- Tests MUST be deterministic and isolated; flaky tests MUST be fixed or quarantined within one
  sprint of detection.
- Test data MUST use synthetic or anonymized datasets — never real user activity recordings.
- Performance regression tests MUST exist for any operation that touches the tracing hot path.

**Rationale**: Because Tracey collects sensitive user activity data, correctness is a safety
issue. Regressions that leak data or corrupt traces are not recoverable "bugs" — they are
privacy incidents.

### III. User Experience Consistency

- All user-facing text (labels, error messages, notifications) MUST follow a single, documented
  tone-of-voice guide stored in `docs/ux/tone.md`.
- Error messages presented to the user MUST state what happened, why it matters to them, and
  what action (if any) they can take — never expose raw stack traces or internal identifiers.
- UI interactions of the same type (e.g., confirmation dialogs, status indicators) MUST behave
  identically across all surfaces of the application.
- Accessibility: all interactive elements MUST meet WCAG 2.1 AA contrast and keyboard-navigation
  requirements.
- Feature changes that alter established user workflows MUST include a migration notice or
  in-product guidance before the old behavior is removed.

**Rationale**: Users grant Tracey deep system access. Trust is built through predictability and
transparency. Inconsistent UX erodes that trust.

### IV. Performance Requirements

- Background tracing MUST consume less than **2 % CPU** averaged over any 10-second window on
  reference hardware (dual-core, 4 GB RAM).
- Memory footprint of the tracing agent MUST remain below **150 MB RSS** under normal operation.
- Any user-initiated operation (e.g., search, export) MUST return a first meaningful result within
  **500 ms** at p95 for a local dataset of up to 1 M events.
- Disk writes from the tracing pipeline MUST be buffered and flushed in batches; individual
  synchronous writes per-event are prohibited on the hot path.
- Performance budgets MUST be enforced via automated benchmark tests in CI; PRs that regress any
  budget by more than 10 % MUST be rejected until justified or the budget is formally revised.

**Rationale**: A tracing agent that noticeably degrades system performance will be disabled by
users, defeating its purpose. Performance is a feature, not an afterthought.

### V. Privacy First (NON-NEGOTIABLE)

- Data minimization: Tracey MUST only collect data that is strictly necessary for the stated
  tracing purpose. Collection of any new data category requires an explicit product decision and
  changelog entry.
- All collected data MUST be stored locally on the user's device by default; no data MUST leave
  the device without explicit, informed, opt-in user consent for each destination.
- User data MUST be deletable on demand — a "delete all my data" operation MUST complete within
  60 seconds and MUST be verifiable (confirmation of deletion provided to user).
- Sensitive fields (e.g., passwords, clipboard contents of sensitive apps) MUST be filtered at
  the collection boundary using a configurable deny-list; filtering rules MUST be user-editable.
- Privacy impact MUST be evaluated for every new feature before implementation begins; this
  evaluation MUST be documented in the feature spec.
- Third-party libraries that perform network calls or telemetry MUST be explicitly audited and
  approved; unapproved libraries that exfiltrate data are a critical defect.

**Rationale**: Tracey observes everything the user does. Misusing or mishandling that data would
be a fundamental betrayal of user trust and may have legal consequences (GDPR, CCPA, etc.).
Privacy is the product's social contract.

### VI. Security

- Threat modeling MUST be performed for every feature that touches data collection, storage, or
  transmission; the threat model MUST be recorded in the feature plan.
- All inter-process communication and local API endpoints MUST enforce authentication; unauthenticated
  local endpoints are prohibited.
- Stored trace data MUST be encrypted at rest using AES-256 or equivalent; the encryption key MUST
  be derived from user credentials or the platform keychain — never hardcoded.
- All inputs from external sources (file paths, user configuration, network responses) MUST be
  validated and sanitized before use; injection vulnerabilities (SQL, command, path traversal) are
  critical defects that block release.
- Dependencies MUST be scanned for known CVEs in CI; builds MUST fail on critical-severity
  vulnerabilities. High-severity vulnerabilities MUST be triaged within 7 days.
- Security incidents and near-misses MUST be documented in a post-mortem within 5 business days.

**Rationale**: Tracey runs with elevated permissions to observe system activity. A compromised
Tracey agent is a keylogger or worse. Security is non-negotiable.

### VII. Observability

- All components MUST emit structured logs (JSON or equivalent) with consistent fields:
  `timestamp`, `level`, `component`, `event`, and `trace_id` where applicable.
- Log levels MUST be used correctly: DEBUG for diagnostic detail, INFO for lifecycle events,
  WARN for recoverable anomalies, ERROR for failures requiring attention.
- Logs MUST NOT contain personal data, raw trace content, or credential material; any field
  that could carry sensitive data MUST be explicitly redacted or omitted at the logging boundary.
- The tracing agent MUST expose a health/status endpoint (local only) that reports: running
  state, last-write timestamp, event throughput (events/sec), and any active error conditions.
- Metrics for key operations (events captured, events dropped, flush latency, memory usage)
  MUST be collected and surfaced through a documented interface (file, local socket, or IPC).
- Alerts or status changes MUST be propagatable to the user interface without polling — push
  notifications or reactive event streams are required for degraded/error states.
- Diagnostic tooling MUST be available in all builds; observability MUST NOT be gated behind
  a debug flag that is stripped in release builds.

**Rationale**: Tracey is a background agent that users cannot directly inspect. Without
  deliberate observability, failures are invisible until they cause data loss or privacy leaks.
  Good observability is what makes the system trustworthy and maintainable over time.


- **Architecture review**: Any architectural decision that trades privacy or security for
  performance MUST be escalated to the full team and documented with explicit trade-off analysis.
- **Dependency selection**: New dependencies MUST be evaluated against all seven principles before
  adoption. Prefer libraries with active maintenance, small attack surface, and no mandatory
  network calls.
- **API design**: Public APIs (local RPC, CLI) MUST version their contracts; breaking changes
  require a major version bump and a migration guide.
- **Defaults**: All default configurations MUST err toward maximum privacy and minimum data
  collection. Opt-out is never acceptable as a default for data sharing.

## Quality Gates

The following gates MUST pass before code is merged or a release is cut:

| Gate | Trigger | Criteria |
|------|---------|----------|
| Lint & static analysis | Every PR | Zero errors |
| Unit tests | Every PR | All pass; ≥ 80 % branch coverage on changed modules |
| Integration tests | Every PR | All pass |
| Performance benchmarks | Every PR to `main` | No budget regression > 10 % |
| Dependency CVE scan | Every PR | No critical CVEs |
| Privacy impact documented | PRs adding a new data field | Spec contains privacy evaluation |
| Security threat model | PRs touching collection/storage/transport | Plan contains threat model |
| Observability review | PRs adding a new component or service | Structured logging, health endpoint, and metrics defined |

## Governance

This constitution supersedes all other team practices and prior agreements. Where a conflict
exists, the constitution takes precedence.

**Amendment procedure**:

1. Any contributor may propose an amendment by opening a pull request that edits this file.
2. Amendments MUST include: motivation, principle changed, migration impact (if any), version bump
   type (MAJOR / MINOR / PATCH) with rationale.
3. Amendments require approval from at least two core contributors before merging.
4. Amendments to Principle V (Privacy First) or Principle VI (Security) additionally require a
   written risk assessment.

**Versioning policy** (semantic, applied to this document):

- MAJOR: Removal or fundamental redefinition of a principle.
- MINOR: New principle added or material expansion of existing guidance.
- PATCH: Wording clarifications, typo fixes, non-semantic refinements.

**Compliance review**: All PRs MUST verify the Constitution Check gate in the associated
`plan.md`. Reviewers are empowered to block any PR that demonstrably violates a principle, even
if all automated gates pass.

**Version**: 1.0.0 | **Ratified**: 2026-03-14 | **Last Amended**: 2026-03-14
