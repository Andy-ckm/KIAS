# GxP Compliance Matrix — KIAS

> **Date**: 2026-05-18  
> **Status**: Gap analysis (Step 3 of design methodology)  
> **Sources**: FDA 21 CFR Part 11 (full text, April 2024), EU Annex 11 (2011), GAMP 5 (2nd ed.), ICH Q10, ALCOA+  
> **Reference Projects**: paper_trail (Ruby), DriftDB (Rust), tradememory-protocol (Python)

---

## Legend

| Symbol | Meaning |
|--------|---------|
| ✅ Met | Requirement fully addressed by existing KIAS module |
| ⚠️ Partial | Partial coverage; enhancement needed |
| ❌ Not Met | No coverage; new module/feature required |

---

## 1. FDA 21 CFR Part 11 — Electronic Records; Electronic Signatures

### Subpart A: General Provisions

| Req ID | Section | Description | KIAS Module | Status | Priority |
|--------|---------|-------------|-------------|--------|----------|
| CFR-11.01 | §11.1 | Scope — electronic records created/modified/maintained/transmitted | System-wide | ✅ Met | — |
| CFR-11.02 | §11.2 | Implementation — electronic records in lieu of paper | System-wide | ✅ Met | — |
| CFR-11.03 | §11.3 | Definitions (closed/open system, electronic record, electronic signature) | System-wide | ✅ Met | — |

### Subpart B: Electronic Records — §11.10 Controls for Closed Systems

| Req ID | Section | Description | KIAS Module | Status | Priority |
|--------|---------|-------------|-------------|--------|----------|
| CFR-11.10a | §11.10(a) | **System validation** — accuracy, reliability, consistent performance, ability to discern invalid/altered records | `gxp_audit.rs` (hash chain verification) | ⚠️ Partial | P0 |
| CFR-11.10b | §11.10(b) | **Record copying** — accurate, complete copies in human-readable and electronic form for FDA inspection | `GxpAuditLog::export_json()` | ⚠️ Partial | P1 |
| CFR-11.10c | §11.10(c) | **Record protection** — accurate and ready retrieval throughout retention period | `gxp_audit.rs` (append-only) | ⚠️ Partial | P0 |
| CFR-11.10d | §11.10(d) | **Access control** — limiting system access to authorized individuals | `auth` (RBAC) | ⚠️ Partial | P0 |
| CFR-11.10e | §11.10(e) | **Audit trails** — secure, computer-generated, time-stamped, independent recording of operator entries/actions that create/modify/delete records. Changes shall not obscure previously recorded information. Retained for records retention period. Available for agency review. | `gxp_audit.rs` (SHA-256 hash chain, append-only, timestamped) | ✅ Met | — |
| CFR-11.10f | §11.10(f) | **Operational system checks** — enforce permitted sequencing of steps/events | `approval.rs` (6-state machine) | ⚠️ Partial | P1 |
| CFR-11.10g | §11.10(g) | **Authority checks** — only authorized individuals can use system, sign records, access devices, alter records, perform operations | `auth` (RBAC) | ⚠️ Partial | P0 |
| CFR-11.10h | §11.10(h) | **Device checks** — determine validity of data input source | None | ❌ Not Met | P2 |
| CFR-11.10i | §11.10(i) | **Personnel qualification** — education, training, experience to perform assigned tasks | None (organizational policy) | ❌ Not Met | P3 |
| CFR-11.10j | §11.10(j) | **Accountability policies** — written policies holding individuals accountable for actions under electronic signatures | None (organizational policy) | ❌ Not Met | P3 |
| CFR-11.10k1 | §11.10(k)(1) | **Documentation controls** — distribution, access, use of system documentation | None | ❌ Not Met | P2 |
| CFR-11.10k2 | §11.10(k)(2) | **Change control** — revision/change control with time-sequenced audit trail for documentation | None | ❌ Not Met | P1 |

### Subpart B: Electronic Records — §11.30 Controls for Open Systems

| Req ID | Section | Description | KIAS Module | Status | Priority |
|--------|---------|-------------|-------------|--------|----------|
| CFR-11.30 | §11.30 | **Open system controls** — document encryption, digital signature standards for record authenticity/integrity/confidentiality from creation to receipt | None (KIAS operates as closed system) | ❌ Not Met | P2 |

### Subpart B: Electronic Records — §11.50 Signature Manifestations

| Req ID | Section | Description | KIAS Module | Status | Priority |
|--------|---------|-------------|-------------|--------|----------|
| CFR-11.50a1 | §11.50(a)(1) | **Signer name** — printed name of signer in signed electronic record | `ElectronicSignature.signer_id` | ⚠️ Partial | P0 |
| CFR-11.50a2 | §11.50(a)(2) | **Signature date/time** — date and time when signature was executed | `ElectronicSignature.signed_at` | ✅ Met | — |
| CFR-11.50a3 | §11.50(a)(3) | **Signature meaning** — meaning associated with signature (review, approval, responsibility, authorship) | `ElectronicSignature.meaning` | ✅ Met | — |
| CFR-11.50b | §11.50(b) | Signature info subject to same controls as electronic records; included in human-readable forms | `GxpAuditEntry` (signature is part of immutable entry) | ✅ Met | — |

### Subpart B: Electronic Records — §11.70 Signature/Record Linking

| Req ID | Section | Description | KIAS Module | Status | Priority |
|--------|---------|-------------|-------------|--------|----------|
| CFR-11.70 | §11.70 | **Signature/record linking** — signatures linked to records to prevent excision, copying, or transfer to falsify a record | `gxp_audit.rs` (signature embedded in hashed audit entry; hash chain prevents excision) | ✅ Met | — |

### Subpart C: Electronic Signatures — §11.100 General Requirements

| Req ID | Section | Description | KIAS Module | Status | Priority |
|--------|---------|-------------|-------------|--------|----------|
| CFR-11.100a | §11.100(a) | **Unique signatures** — each electronic signature unique to one individual, not reused/reassigned | `auth` (user IDs) + `ElectronicSignature.signer_id` | ⚠️ Partial | P0 |
| CFR-11.100b | §11.100(b) | **Identity verification** — verify identity before establishing/sanctioning electronic signature | None (organizational process) | ❌ Not Met | P1 |
| CFR-11.100c | §11.100(c) | **FDA certification** — certify to agency that electronic signatures are legally binding equivalent of handwritten signatures | None (organizational submission) | ❌ Not Met | P3 |

### Subpart C: Electronic Signatures — §11.200 Electronic Signature Components and Controls

| Req ID | Section | Description | KIAS Module | Status | Priority |
|--------|---------|-------------|-------------|--------|----------|
| CFR-11.200a1i | §11.200(a)(1)(i) | **Two-factor signing** — first signing uses all components; subsequent during continuous session use at least one component only executable by individual | None | ❌ Not Met | P0 |
| CFR-11.200a1ii | §11.200(a)(1)(ii) | **Non-continuous signing** — each signing outside continuous session uses all components | None | ❌ Not Met | P0 |
| CFR-11.200a2 | §11.200(a)(2) | **Genuine owner use** — signatures used only by genuine owners | `auth` (session management) | ⚠️ Partial | P0 |
| CFR-11.200a3 | §11.200(a)(3) | **Collaboration requirement** — attempted use by non-owner requires collaboration of two or more individuals | None | ❌ Not Met | P1 |
| CFR-11.200b | §11.200(b) | **Biometric signatures** — if biometric-based, ensure only genuine owner can use | N/A | N/A | — |

### Subpart C: Electronic Signatures — §11.300 Controls for Identification Codes/Passwords

| Req ID | Section | Description | KIAS Module | Status | Priority |
|--------|---------|-------------|-------------|--------|----------|
| CFR-11.300a | §11.300(a) | **Unique combinations** — maintain uniqueness of combined identification code and password | `auth` (user management) | ⚠️ Partial | P0 |
| CFR-11.300b | §11.300(b) | **Password aging** — periodically check, recall, revise identification codes/passwords | None | ❌ Not Met | P1 |
| CFR-11.300c | §11.300(c) | **Loss management** — deauthorize lost/stolen/compromised tokens/cards/devices, issue replacements | None | ❌ Not Met | P2 |
| CFR-11.300d | §11.300(d) | **Transaction safeguards** — prevent unauthorized use, detect and report attempts immediately | `gxp_audit.rs` (audit logging) + `auth` | ⚠️ Partial | P1 |
| CFR-11.300e | §11.300(e) | **Device testing** — initial and periodic testing of tokens/cards for proper function and unauthorized alteration | N/A | N/A | — |

---

## 2. EU Annex 11 — Computerised Systems

| Req ID | Clause | Description | KIAS Module | Status | Priority |
|--------|--------|-------------|-------------|--------|----------|
| EU-01 | 1 | **Risk Management** — risk management applied throughout lifecycle of computerised system | None | ❌ Not Met | P1 |
| EU-02 | 2 | **Personnel** — appropriate management, business, and technical expertise | None (org policy) | ❌ Not Met | P3 |
| EU-03 | 3 | **Suppliers** — suppliers/vendors evaluated; written quality agreements | None (procurement) | ❌ Not Met | P3 |
| EU-04 | 4 | **Validation** — validated computerised system; validation documentation and reports; evidence that controls work in practice | `gxp_audit.rs` (hash chain verify) | ⚠️ Partial | P0 |
| EU-05 | 5 | **Data** — data stored in manner readable and accessible; data transfer checked for accuracy; data verified for alteration; backup/restore tested | `GxpAuditLog` (append-only, export) | ⚠️ Partial | P0 |
| EU-06 | 6 | **Accuracy Checks** — data verified during input and processing | None | ❌ Not Met | P1 |
| EU-07 | 7 | **Data Storage** — data protected by regular backups; tested regularly; stored at secure alternate site | None | ❌ Not Met | P1 |
| EU-08 | 8 | **Physical/Logical Access** — physical and/or logical controls for access; documented policy for granting/withdrawing access | `auth` (RBAC) | ⚠️ Partial | P0 |
| EU-09 | 9 | **Audit Trail** — creation, modification, and deletion of records recorded with timestamp, operator ID, old/new values; available for agency review | `gxp_audit.rs` | ✅ Met | — |
| EU-10 | 10 | **Change Management** — changes managed per GMP requirements; proposed changes evaluated for impact; changes authorized before implementation; documented | `approval.rs` | ⚠️ Partial | P0 |
| EU-11 | 11 | **Periodic Evaluation** — computerised systems periodically evaluated for continued compliance | None | ❌ Not Met | P2 |
| EU-12 | 12 | **Security** — physical and logical security measures; documented policy | `auth` + `gxp_audit.rs` | ⚠️ Partial | P0 |
| EU-13 | 13 | **Incident Management** — incidents documented and reported; corrective actions tracked | None | ❌ Not Met | P1 |
| EU-14 | 14 | **Electronic Signature** — equivalent to handwritten; linked to respective record; date/time; meaning; signer ID | `ElectronicSignature` | ⚠️ Partial | P0 |
| EU-15 | 15 | **Batch Release** — electronic batch release permitted if requirements met | None (app-level) | ❌ Not Met | P2 |
| EU-16 | 16 | **Business Continuity** — availability measures; contingency plans; tested periodically | None | ❌ Not Met | P2 |
| EU-17 | 17 | **Archiving** — archived data accessible, readable; media tested periodically; retention periods | `gxp_audit.rs` (append-only) | ⚠️ Partial | P1 |

---

## 3. GAMP 5 — Categorization & Validation

| Req ID | Category | Description | KIAS Classification | Validation Required |
|--------|----------|-------------|---------------------|-------------------|
| GAMP-01 | Cat 1 | **Infrastructure** — operating systems, databases, network components | KIAS depends on: Linux, PostgreSQL (future), network | Validation by vendor qualification |
| GAMP-02 | Cat 3 | **Standard Software** — off-the-shelf software with no configuration | KIAS itself is NOT Cat 3 | N/A |
| GAMP-03 | Cat 4 | **Configured Products** — standard software with configuration | KIAS with configuration = Cat 4 aspects | Config-based testing |
| GAMP-04 | Cat 5 | **Custom Software** — custom-developed application | **KIAS = Category 5** (custom Rust application with AI agents) | Full V-model validation required |
| GAMP-05 | — | **Risk-based approach** — validation scope proportional to risk | High risk: audit trail, e-signatures, change control | Risk assessment needed |
| GAMP-06 | — | **V-Model** — user requirements → functional specs → design → build → testing | KIAS needs URS, FRS, DS, IQ, OQ, PQ | All phases required |
| GAMP-07 | — | **Traceability** — bidirectional traceability from requirements to tests | None | ❌ Not Met | P0 |

### KIAS GAMP 5 Classification: Category 5 (Custom)

**Rationale**: KIAS is a custom-developed enterprise AI agent system with bespoke audit, approval, and signature modules. It is not off-the-shelf or configured from a standard product.

**Validation Requirements (Category 5)**:
1. User Requirements Specification (URS)
2. Functional Specification (FS)
3. Design Specification (DS)
4. Installation Qualification (IQ)
5. Operational Qualification (OQ)
6. Performance Qualification (PQ)
7. Traceability Matrix (this document serves as input)

---

## 4. ICH Q10 — Pharmaceutical Quality System

| Req ID | Section | Description | KIAS Module | Status | Priority |
|--------|---------|-------------|-------------|--------|----------|
| ICH-01 | 1.4 | **Management Responsibility** — management commitment, resource allocation, organizational alignment | None (org policy) | ❌ Not Met | P3 |
| ICH-02 | 2.2 | **Knowledge Management** — systematic approach to acquire, analyze, store, and disseminate knowledge | `graph.rs` (knowledge graph) | ⚠️ Partial | P1 |
| ICH-03 | 2.3 | **Quality Risk Management** — systematic process for assessment, communication, review of risks | None | ❌ Not Met | P1 |
| ICH-04 | 3.1 | **Process Performance & Product Quality Monitoring** — ongoing monitoring of manufacturing performance | None (app-level) | ❌ Not Met | P2 |
| ICH-05 | 3.2.1 | **Change Control System** — systematic approach to proposing, evaluating, approving, implementing changes | `approval.rs` (6-state machine) | ⚠️ Partial | P0 |
| ICH-06 | 3.2.2 | **Change Management** — evaluate impact on product quality, validated state, regulatory filing | None | ❌ Not Met | P1 |
| ICH-07 | 3.2.3 | **Change Implementation** — implement approved changes; evaluate effectiveness | None | ❌ Not Met | P1 |
| ICH-08 | 4 | **Continual Improvement** — identify and implement improvements to processes and products | None | ❌ Not Met | P2 |

---

## 5. ALCOA+ Principles

| Req ID | Principle | Description | KIAS Module | Status | Priority |
|--------|-----------|-------------|-------------|--------|----------|
| ALCOA-01 | **A**ttributable | Who performed the action and when | `GxpAuditEntry.actor_id`, `actor_type`, `timestamp` | ✅ Met | — |
| ALCOA-02 | **L**egible | Data recorded in permanent, readable format | `GxpAuditEntry` (structured JSON via `export_json()`) | ✅ Met | — |
| ALCOA-03 | **C**ontemporaneous | Data recorded at time of activity | `GxpAuditEntry.timestamp` (set at `Utc::now()` on creation) | ✅ Met | — |
| ALCOA-04 | **O**riginal | First recording of data (or certified copy) | `gxp_audit.rs` (append-only, no modification possible) | ✅ Met | — |
| ALCOA-05 | **A**ccurate | Data correct, truthful, free from errors | `before_state` / `after_state` diffs; hash chain integrity | ✅ Met | — |
| ALCOA-06 | **C**omplete | All data recorded; no deletions | `gxp_audit.rs` (append-only; no delete operations) | ✅ Met | — |
| ALCOA-07 | **C**onsistent | Data elements consistent with each other and time sequence | `sequence` number + `prev_hash` chain + `entry_hash` | ✅ Met | — |
| ALCOA-08 | **E**nduring | Recorded on durable medium; retained for required period | `gxp_audit.rs` (persisted; but needs durable storage backend) | ⚠️ Partial | P1 |
| ALCOA-09 | **A**vailable | Available for review throughout retention period | `query_by_*()`, `as_of()`, `export_json()` | ✅ Met | — |

---

## Summary Dashboard

### Coverage by Regulatory Source

| Source | ✅ Met | ⚠️ Partial | ❌ Not Met | Total | Coverage |
|--------|--------|-----------|----------|-------|----------|
| **21 CFR Part 11** | 6 | 12 | 10 | 28 | 21% met, 43% partial |
| **EU Annex 11** | 1 | 8 | 8 | 17 | 6% met, 47% partial |
| **GAMP 5** | 0 | 0 | 1 | 1 | 0% met (traceability) |
| **ICH Q10** | 0 | 2 | 6 | 8 | 0% met, 25% partial |
| **ALCOA+** | 7 | 1 | 0 | 8 | **88% met** |
| **TOTAL** | **14** | **23** | **25** | **62** | **23% met, 37% partial** |

### Priority Distribution

| Priority | Count | Description |
|----------|-------|-------------|
| P0 | 15 | Critical — must implement before GxP deployment |
| P1 | 16 | High — required for full compliance |
| P2 | 11 | Medium — operational completeness |
| P3 | 5 | Low — organizational/process policy |

### KIAS Module Gap Map

| Module | What It Covers | What's Missing |
|--------|---------------|----------------|
| `gxp_audit.rs` | ALCOA+, audit trails, hash chain, append-only, e-signature structure, time-travel query | Durable storage backend, retention enforcement, encryption-at-rest |
| `approval.rs` | Change control state machine | Impact assessment, verified stage, multi-level approval chain, effectiveness monitoring |
| `auth` | RBAC, user management | SoD (separation of duties), password aging, 2FA for signing, session management, device checks |
| `graph.rs` | Knowledge management | Regulatory knowledge domain model |
| *None* | — | Risk management, validation framework, incident management, business continuity, archival policies, periodic evaluation |

---

## Reference Project Analysis

### 1. PaperTrail (Ruby) — 7k+ stars

**Core Pattern**: Model-level change tracking via ActiveRecord callbacks.

**Key Data Structures**:
- `Version` model: `item_type`, `item_id`, `event` (create/update/destroy), `object` (before state), `object_changes` (diff), `whodunnit` (actor), `created_at`
- Polymorphic `belongs_to :item` — any model can be versioned
- Metadata injection via `controller_info` (request context) and `model.meta` (declarative)

**Relevance to KIAS**:
- ✅ `object`/`object_changes` pattern directly maps to `before_state`/`after_state` in `GxpAuditEntry`
- ✅ `whodunnit` = `actor_id`; `event` = `GxpAuditAction`
- ⚠️ No hash chain — PaperTrail trusts database integrity (not GxP-grade tamper detection)
- ⚠️ No electronic signature support
- **Takeaway**: Use PaperTrail's metadata pattern as design reference; upgrade with KIAS's SHA-256 hash chain

### 2. DriftDB (Rust) — 135 stars

**Core Pattern**: Append-only database with CRC-verified segments, WAL, RBAC, security audit.

**Key Data Structures**:
- **WAL Entry**: `sequence`, `transaction_id`, `operation` (enum), `timestamp`, `checksum` (CRC32)
- **Frame**: `[length][crc32][seq][timestamp][event_type][msgpack_payload]`
- **Security Audit**: `event_id`, `timestamp`, `event_type` (enum: Login/AccessDenied/UserCreated/...), `username`, `severity`, `outcome`, `checksum` (SHA-256 per entry)
- **RBAC**: `Permission` enum (35 variants), `Role` with `HashSet<Permission>`, `RbacManager` with grant/revoke/check

**Relevance to KIAS**:
- ✅ CRC32 per frame + SHA-256 per audit entry — integrity at two levels
- ✅ RBAC with 35 fine-grained permissions — reference for KIAS `auth` module expansion
- ✅ Security audit with brute-force detection, file logging, severity levels
- ✅ Append-only semantics (segment-based, soft deletes preserve data)
- ✅ Time-travel via `FOR SYSTEM_TIME AS OF` SQL syntax
- ⚠️ No hash chain linking entries (CRC32 per entry, not chained)
- **Takeaway**: Adopt DriftDB's RBAC permission model and security audit severity/outcome patterns for KIAS

### 3. TradeMemory Protocol (Python) — 927 stars

**Core Pattern**: SHA-256 audit chain with daily Merkle roots and RFC 3161 TSA attestation.

**Key Data Structures**:
- **Audit Chain Entry**: `record_id`, `sequence_num`, `content_hash`, `prev_hash`, `data_hash` (= `SHA256(prev_hash || content_hash)`), `chained_at`
- **Daily Root**: `period_start`, `period_end`, `root_hash` (Merkle root), `prev_root_hash`, `record_count`, `first_sequence`, `last_sequence`, `tsa_token` (RFC 3161)
- **Chain Builder**: `append()` (idempotent, tamper-refuse), `verify_chain()` (walk chain, check links), `build_daily_root()` (Merkle tree), `verify_daily_root()`

**Tamper Semantics**:
- Modifying any historical TDR content breaks `content_hash` AND every subsequent `data_hash`
- Modifying `data_hash` directly without re-linking forwards = detected by `verify_chain`
- Modifying a daily root requires forging SHA-256 collisions (Merkle property)

**Relevance to KIAS**:
- ✅ **Direct architectural ancestor** — KIAS `gxp_audit.rs` already uses the same `SHA256(prev_hash || content)` chain pattern
- ✅ Merkle root aggregation — **KIAS gap**: no daily Merkle roots yet
- ✅ RFC 3161 TSA timestamping — **KIAS gap**: no external timestamp authority integration
- ✅ `verify_chain()` with `first_break_at` — KIAS has basic `verify_chain()` but no break location reporting
- ✅ Idempotent append with tamper-refuse — KIAS needs this
- **Takeaway**: Add Merkle root aggregation and TSA timestamping to KIAS Phase 2+

---

## Recommended Implementation Order

### Phase 1 (P0 — 15 items): Core Compliance Foundation
1. Extend `gxp_audit.rs`: durable storage backend (PostgreSQL), retention enforcement
2. Extend `auth`: password aging, unique credential enforcement, session management
3. Implement 2FA for electronic signing (`§11.200`)
4. Add `signer_name` field to `ElectronicSignature` (not just `signer_id`)
5. System validation framework (IQ/OQ/PQ scripts)
6. Operational system checks: enforce permitted action sequencing
7. Bidirectional traceability matrix (GAMP-07)

### Phase 2 (P1 — 16 items): Full Compliance
1. Risk management module (EU-01, ICH-03)
2. Change control enhancement: impact assessment, effectiveness monitoring
3. Document change control (§11.10(k)(2))
4. Password aging and rotation (§11.300(b))
5. Incident management (EU-13)
6. Data storage: backup, restore, alternate site (EU-07)
7. Merkle root aggregation + TSA timestamping (tradememory pattern)
8. Accurate/complete record export for FDA inspection (§11.10(b))

### Phase 3 (P2 — 11 items): Operational Completeness
1. Device checks (§11.10(h))
2. Open system controls / encryption (§11.30) — if remote access needed
3. Periodic evaluation (EU-11)
4. Business continuity (EU-16)
5. Batch release integration (EU-15)

### Phase 4 (P3 — 5 items): Organizational Policy
1. Personnel qualification tracking (§11.10(i))
2. Accountability policies (§11.10(j))
3. FDA e-signature certification (§11.100(c))
4. Management responsibility framework (ICH-01)
5. Supplier/vendor qualification (EU-03)

---

## Appendix: Source Files Analyzed

### KIAS
- `/workspace/kias/crates/common/src/gxp_audit.rs` — GxpAuditEntry, ElectronicSignature, GxpAuditLog, hash chain
- `/workspace/kias/docs/design-docs/gxp-compliance-architecture.md` — existing design doc

### Reference Projects
- `/mnt/reference-projects/paper_trail/lib/paper_trail/events/base.rb` — change detection, metadata, serialization
- `/mnt/reference-projects/paper_trail/lib/paper_trail/version_concern.rb` — Version model, reification, changeset queries
- `/mnt/reference-projects/DriftDB/crates/driftdb-core/src/wal.rs` — WalEntry, CRC32 checksums, sequence, replay
- `/mnt/reference-projects/DriftDB/crates/driftdb-server/src/security/rbac.rs` — Permission enum, Role, RbacManager
- `/mnt/reference-projects/DriftDB/crates/driftdb-server/src/security_audit.rs` — AuditEntry, SHA-256 checksum, severity/outcome
- `/mnt/reference-projects/tradememory-protocol/src/tradememory/audit/chain.py` — AuditChainEntry, chained_hash, verify_chain, DailyRoot, Merkle
