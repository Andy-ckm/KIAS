# Comprehensive Commercial Software Quality Standards Report

*Generated: 2026-05-20*

---

## 1. ISO/IEC 25010 — Systems and Software Quality Requirements and Evaluation (SQuaRE)

### Overview
ISO 25010 defines a quality model for computer software. It is part of the SQuaRE series (ISO 25000) and provides a framework for evaluating software product quality.

### Key Categories / Requirements

**Product Quality Model (8 characteristics):**

| # | Characteristic | Description |
|---|---------------|-------------|
| 1 | **Functional Suitability** | Degree to which product provides functions that meet stated needs |
| 2 | **Performance Efficiency** | Performance relative to resources used (time, resources) |
| 3 | **Compatibility** | Can exchange information / co-exist with other products |
| 4 | **Usability** | Effort needed to use the product; individual assessment of use |
| 5 | **Reliability** | Maturity, availability, fault tolerance, recoverability |
| 6 | **Security** | Confidentiality, integrity, non-repudiation, accountability, authenticity |
| 7 | **Maintainability** | Modularity, reusability, analysability, modifiability, testability |
| 8 | **Portability** | Adaptability, installability, replaceability |

**Quality in Use Model (5 characteristics):**
- **Effectiveness** — accuracy and completeness of goals achieved
- **Efficiency** — resources expended vs. effectiveness
- **Satisfaction** — comfort, trust, pleasure
- **Freedom from Risk** — economic, health/safety, environmental risk mitigation
- **Context Coverage** — completeness across contexts of use

### Specific Metrics / Thresholds

| Characteristic | Sub-Characteristic | Metric Example | Threshold |
|---|---|---|---|
| Functional Suitability | Functional Correctness | % of functions producing correct output | ≥ 99% |
| Performance Efficiency | Time Behaviour | Response time for 95th percentile | ≤ 2s (web), ≤ 200ms (API) |
| Performance Efficiency | Resource Utilization | CPU/memory consumption under load | ≤ 80% sustained |
| Reliability | Maturity | Mean Time Between Failures (MTBF) | ≥ 720 hours |
| Reliability | Availability | System uptime percentage | ≥ 99.9% |
| Reliability | Recoverability | Recovery Time Objective (RTO) | ≤ 4 hours |
| Security | Confidentiality | Unauthorized access attempts blocked | 100% |
| Usability | Operability | Task completion rate for new users | ≥ 90% |
| Maintainability | Testability | Code coverage of test suites | ≥ 80% |
| Maintainability | Modularity | Coupling metrics (efferent/efferent coupling) | Low coupling |
| Portability | Adaptability | Time to deploy to new platform | ≤ 1 sprint |

### Verification / Compliance

- **Method:** Independent evaluation against ISO 25010 quality model
- **Process:**
  1. Define quality requirements mapped to ISO 25010 characteristics
  2. Design quality measures per ISO 25020 (measurement reference model)
  3. Execute evaluation per ISO 25040 (evaluation process)
  4. Produce quality evaluation report with measured values vs. targets
- **Certification:** Not directly certifiable; organizations align internal QA processes
- **Tools:** Static analysis, performance testing, penetration testing, usability testing, code review

---

## 2. SOC 2 (System and Organization Controls 2)

### Overview
SOC 2 is an auditing framework developed by the American Institute of CPAs (AICPA). It defines criteria for managing customer data based on five Trust Services Criteria (TSC).

### Key Categories / Requirements

**Five Trust Services Criteria:**

| # | Category | Description | Mandatory? |
|---|----------|-------------|------------|
| 1 | **Security** | Protection against unauthorized access | ✅ Yes (always) |
| 2 | **Availability** | System operational and usable | Optional |
| 3 | **Processing Integrity** | System processing is complete, valid, accurate, timely | Optional |
| 4 | **Confidentiality** | Confidential information is restricted | Optional |
| 5 | **Privacy** | Personal information is collected, used, retained per commitments | Optional |

**Common Criteria (CC) Series — mapped to Security:**
- CC1: Control Environment
- CC2: Communication and Information
- CC3: Risk Assessment
- CC4: Monitoring Activities
- CC5: Control Activities
- CC6: Logical and Physical Access Controls
- CC7: System Operations
- CC8: Change Management
- CC9: Risk Mitigation

### Specific Metrics / Thresholds

| Control Area | Requirement | Metric / Threshold |
|---|---|---|
| Access Management | Multi-factor authentication | MFA required for all production access |
| Access Reviews | Periodic access reviews | Quarterly minimum |
| Encryption (at rest) | Data encryption | AES-256 |
| Encryption (in transit) | TLS | TLS 1.2+ |
| Incident Response | Incident detection to response | ≤ 1 hour detection; ≤ 4 hour response |
| Change Management | Code review requirements | 100% peer review before deploy |
| Vulnerability Management | Patch critical vulnerabilities | ≤ 30 days for critical, ≤ 90 days for high |
| Logging | Audit log retention | ≥ 12 months (≥ 90 days immediately searchable) |
| Backup | Backup frequency | Daily minimum |
| DR Testing | Disaster recovery drills | Annually minimum |
| Employee Training | Security awareness | Annually, ≥ 95% completion |
| Background Checks | New hires | Within 30 days of hire |
| Uptime | System availability target | ≥ 99.9% (typical SaaS) |

### Verification / Compliance

- **Type I:** Auditor evaluates design of controls at a point in time
- **Type II:** Auditor evaluates design AND operating effectiveness over 6–12 month period
- **Auditor:** Must be an independent CPA firm
- **Frequency:** Annual audit; SOC 2 Type II reports cover a review period
- **Process:**
  1. Gap assessment against TSC
  2. Implement controls and policies
  3. Evidence collection (screenshots, configs, logs)
  4. Auditor examination
  5. Report issuance with opinion
- **Evidence:** Automated via tools like Vanta, Drata, Secureframe, or manual collection

---

## 3. GxP (Good Practice Regulations)

### Overview
GxP is an umbrella term for Good Practice quality guidelines and regulations across industries regulated by the FDA and other authorities. The "x" stands for the specific domain.

### Key Categories / Requirements

| Abbreviation | Full Name | Domain |
|---|---|---|
| GMP | Good Manufacturing Practice | Manufacturing |
| GLP | Good Laboratory Practice | Laboratory |
| GCP | Good Clinical Practice | Clinical trials |
| GDP | Good Distribution Practice | Distribution |
| GVP | Good Pharmacovigilance Practice | Post-market safety |
| GDocP | Good Documentation Practice | Documentation |

**Core GxP Principles:**
1. **Data Integrity — ALCOA+**
   - **A**ttributable — Who performed the action?
   - **L**egible — Can it be read and understood?
   - **C**ontemporaneous — Recorded at time of activity
   - **O**riginal — First recording or certified true copy
   - **A**ccurate — No errors or editing without audit trail
   - **+** Complete, Consistent, Enduring, Available (ALCOA+)

2. **Quality Management System (QMS)**
3. **Validation / Qualification** (IQ, OQ, PQ)
4. **Change Control**
5. **Training and Competency**
6. **Audit Trails**
7. **CAPA (Corrective and Preventive Actions)**

### Specific Metrics / Thresholds

| Requirement | Metric | Threshold |
|---|---|---|
| Audit Trail | 100% of data modifications captured | No gaps allowed |
| System Validation | IQ/OQ/PQ documented | Required before go-live |
| Data Backup | Frequency | Real-time or daily minimum |
| Backup Restoration Testing | Frequency | Quarterly minimum |
| Change Control | Approval before implementation | 100% |
| Periodic Review | System revalidation | Annually or upon significant change |
| Training | GxP training completion | 100% before system access |
| CAPA Closure | Time to close corrective action | ≤ 30 days (typical), ≤ 90 days (complex) |
| Electronic Records | System access controls | Role-based, individual accounts |
| Retention | Record retention period | Varies: 2–15+ years depending on type |

### Verification / Compliance

- **Internal Audits:** Scheduled and ad-hoc GxP audits
- **Regulatory Inspections:** FDA, EMA, WHO inspections
- **Validation Documentation:**
  - Validation Master Plan (VMP)
  - User Requirements Specification (URS)
  - Functional/Design Specifications
  - IQ/OQ/PQ protocols and reports
  - Traceability Matrix
- **Computer System Validation (CSV):** Per GAMP 5 risk-based approach
- **Tools:** MasterControl, Veeva Vault, ComplianceWire

---

## 4. FDA 21 CFR Part 11 — Electronic Records; Electronic Signatures

### Overview
21 CFR Part 11 is a US FDA regulation establishing criteria under which electronic records and electronic signatures are considered trustworthy, reliable, and equivalent to paper records and handwritten signatures.

### Key Categories / Requirements

**Three Major Sections:**

#### A. General Provisions (§11.1 – §11.3)
- Scope: applies to records in electronic form maintained per FDA predicate rules
- Implementation: risk-based approach to determine what controls apply

#### B. Electronic Records (§11.10)
| Requirement | Description |
|---|---|
| **§11.10(a)** | System validation — ensure accuracy, reliability, consistent performance |
| **§11.10(b)** | Ability to generate accurate and complete copies of records |
| **§11.10(c)** | Protection of records to enable accurate and ready retrieval |
| **§11.10(d)** | Limiting system access to authorized individuals |
| **§11.10(e)** | Secure, computer-generated, time-stamped audit trails |
| **§11.10(f)** | Operational system checks (enforcing permitted sequencing) |
| **§11.10(g)** | Authority checks (only authorized individuals can sign) |
| **§11.10(h)** | Device checks (input/output validity) |
| **§11.10(i)** | Education, training, and experience of personnel |
| **§11.10(j)** | Written policies for accountability and shared responsibility |
| **§11.10(k)** | Controls over system documentation |
| **§11.10(l)** | Use of physical and logical controls (e.g., encryption) |

#### C. Electronic Signatures (§11.50 – §11.100)
- **§11.50:** Signed records must include: printed name, date/time, meaning of signature
- **§11.70:** Signature/record linking (signatures must be linked to respective records)
- **§11.100:** Each signature must be unique to one individual
- **§11.200:** Non-biometric signatures must have two distinct identification components (user ID + password or similar)

### Specific Metrics / Thresholds

| Control | Metric / Threshold |
|---|---|
| Audit Trail | 100% of create, modify, delete operations logged |
| Audit Trail Content | Must include: who, what, when, why (before/after values) |
| System Validation | Documented IQ/OQ/PQ; revalidation on significant changes |
| Access Controls | Individual user accounts; no shared accounts |
| Password Policy | ≥ 8 characters, complexity requirements, periodic change |
| Session Timeout | ≤ 30 minutes of inactivity |
| Signature Linking | Electronic signature permanently linked to signed record |
| Record Retention | Per predicate rule requirements (often 2–10+ years) |
| Record Copies | Must be producible in human-readable format |
| Backup/Recovery | Tested regularly; documented procedures |
| Training | Documented before system access; periodic refresher |

### Verification / Compliance

- **FDA Inspection:** During pre-approval or routine GMP inspections
- **Gap Analysis:** Against 21 CFR Part 11 checklist
- **Validation Documentation:**
  - Validation plan/protocol
  - Requirements specification
  - Risk assessment
  - IQ/OQ/PQ execution records
  - Traceability matrix
  - Validation summary report
- **Periodic Review:** At least annually or upon changes
- **Tools:** Veeva Vault QMS, MasterControl, Sparta Systems TrackWise

---

## 5. EU AI Act (Regulation (EU) 2024/1689)

### Overview
The EU AI Act is the world's first comprehensive AI regulation. Entered into force August 2024, with phased implementation through 2027. It classifies AI systems by risk level and imposes obligations accordingly.

### Key Categories / Requirements

**Risk-Based Classification:**

| Risk Level | Description | Examples |
|---|---|---|
| **Unacceptable** | Banned outright | Social scoring, real-time biometric identification (exceptions), manipulation of vulnerabilities |
| **High-Risk** | Strict requirements | Biometric ID, critical infrastructure, education, employment, law enforcement, migration, justice |
| **Limited Risk** | Transparency obligations | Chatbots, deepfakes, emotion recognition |
| **Minimal Risk** | No specific obligations | Spam filters, AI-enabled video games |

**Requirements for High-Risk AI Systems (Title III, Art. 8–15):**

| # | Requirement | Description |
|---|---|---|
| 1 | **Risk Management System** | Continuous lifecycle risk management (Art. 9) |
| 2 | **Data Governance** | Training, validation, testing datasets must be relevant, representative, free of errors (Art. 10) |
| 3 | **Technical Documentation** | Detailed documentation enabling assessment of compliance (Art. 11) |
| 4 | **Record-Keeping / Logging** | Automatic logging of events for traceability (Art. 12) |
| 5 | **Transparency & Information** | Users must be informed they are interacting with AI (Art. 13) |
| 6 | **Human Oversight** | Effective human oversight mechanisms (Art. 14) |
| 7 | **Accuracy, Robustness, Cybersecurity** | Appropriate levels throughout lifecycle (Art. 15) |
| 8 | **Conformity Assessment** | Before placing on market (Art. 43) |
| 9 | **EU Database Registration** | Registration in EU database (Art. 49) |
| 10 | **Post-Market Monitoring** | Ongoing monitoring after deployment (Art. 72) |

### Specific Metrics / Thresholds

| Requirement | Metric / Threshold |
|---|---|
| Risk Assessment | Documented before deployment; updated at least annually |
| Dataset Quality | Bias testing across protected characteristics; documented data lineage |
| Technical Documentation | Must cover: system description, design specs, development process, validation results |
| Logging | All predictions/decisions with confidence scores logged |
| Log Retention | ≥ 6 months (or longer per applicable law) |
| Transparency | Users informed of AI interaction at first contact |
| Human Oversight | Ability to intervene/override at any time |
| Accuracy | Documented accuracy metrics; comparable to or better than human performance where applicable |
| Robustness | Tested against adversarial attacks; graceful degradation |
| Conformity Assessment | Prior to market placement; CE marking |
| Incident Reporting | Serious incidents reported to authorities within 15 days (10 days for serious harm) |
| Penalties | Up to €35M or 7% of global turnover (unacceptable risk violations) |
| Penalties (other) | Up to €15M or 3% for other violations; €7.5M or 1.5% for incorrect info |

### Verification / Compliance

- **Conformity Assessment:** Self-assessment or third-party (depending on risk category)
- **Notified Bodies:** For high-risk systems requiring third-party assessment
- **CE Marking:** Required for high-risk AI systems
- **Documentation:** Technical file maintained and available to authorities
- **Post-Market Monitoring:** Continuous system for collecting performance data
- **Timeline:**
  - February 2025: Prohibited AI practices apply
  - August 2025: General-purpose AI model obligations
  - August 2026: Full application (high-risk systems)
- **Standards (harmonized):** EN standards being developed by CEN/CENELEC

---

## 6. Google SRE (Site Reliability Engineering)

### Overview
Google SRE is a set of principles and practices for IT operations, coined by Ben Treynor Sloss. It applies software engineering to infrastructure and operations problems.

### Key Categories / Requirements

**Core Principles:**

| # | Principle | Description |
|---|---|---|
| 1 | **Embracing Risk** | 100% reliability is wrong target; error budgets define acceptable risk |
| 2 | **Service Level Objectives (SLOs)** | Define what "reliable enough" means for each service |
| 3 | **Eliminating Toil** | Automate manual, repetitive, automatable work |
| 4 | **Monitoring** | Four golden signals: latency, traffic, errors, saturation |
| 5 | **Release Engineering** | CI/CD, progressive rollouts, canary deployments |
| 6 | **Simplicity** | Simplicity as a prerequisite for reliability |
| 7 | **Postmortems / Blameless Culture** | Learn from failures without blame |
| 8 | **Capacity Planning** | Plan for demand and ensure efficient resource use |

**SLI/SLO/SLA Framework:**

| Concept | Definition |
|---|---|
| **SLI (Service Level Indicator)** | Quantitative measure of a service level (e.g., request latency, error rate) |
| **SLO (Service Level Objective)** | Target value for an SLI (e.g., 99.9% of requests < 200ms) |
| **SLA (Service Level Agreement)** | Contractual agreement with consequences for missing SLOs |
| **Error Budget** | 1 − SLO = acceptable failure rate (e.g., 99.9% SLO = 0.1% error budget) |

### Specific Metrics / Thresholds

**Four Golden Signals:**

| Signal | Definition | Typical SLO |
|---|---|---|
| **Latency** | Time to serve a request | p50 ≤ 100ms, p95 ≤ 300ms, p99 ≤ 1s |
| **Traffic** | Demand on the system | Requests per second (capacity planning) |
| **Errors** | Rate of failed requests | ≤ 0.1% (99.9% success rate) |
| **Saturation** | How "full" the system is | CPU ≤ 80%, Memory ≤ 85%, Disk ≤ 75% |

**Common SLO Targets:**

| Tier | Availability | Downtime/Year | Use Case |
|---|---|---|---|
| Tier 1 | 99.99% | 52.6 minutes | Critical user-facing |
| Tier 2 | 99.9% | 8.76 hours | Standard services |
| Tier 3 | 99% | 3.65 days | Internal tools |

**Toil Budget:** ≤ 50% of SRE time on toil (ideal < 30%)

| Practice | Metric | Target |
|---|---|---|
| Toil Reduction | % SRE time on toil | ≤ 50% (stretch: ≤ 30%) |
| Change Failure Rate | % of deploys causing incident | ≤ 5% |
| MTTR | Mean Time to Recovery | ≤ 30 minutes |
| MTBF | Mean Time Between Failures | ≥ 720 hours |
| On-Call | Alerts per shift | ≤ 2 (actionable) |
| Postmortem | Completed within | ≤ 72 hours of incident |
| Postmortem Action Items | Closure rate | ≥ 90% within 90 days |
| Deployment Frequency | Deploys per day | Multiple (continuous delivery) |

### Verification / Compliance

- **SLO Reviews:** Regular (bi-weekly/monthly) SLO performance reviews
- **Error Budget Reviews:** When budget is exhausted, freeze deployments
- **Blameless Postmortems:** After every significant incident
- **Toil Tracking:** Regular measurement and reporting of toil percentage
- **Monitoring & Alerting:** Automated via Prometheus, Grafana, Stackdriver
- **Chaos Engineering:** Regular failure injection (see Netflix Chaos Monkey model)
- **Production Readiness Reviews (PRR):** Before launching new services
- **Key Books:**
  - *Site Reliability Engineering* (Beyer et al., O'Reilly, 2016)
  - *The Site Reliability Workbook* (Beyer et al., O'Reilly, 2018)
  - *Building Secure and Reliable Systems* (O'Reilly, 2020)

---

## 7. AWS Well-Architected Framework

### Overview
The AWS Well-Architected Framework provides a consistent approach for customers and partners to evaluate architectures and implement scalable designs on AWS. Originally 5 pillars, expanded to 6.

### Key Categories / Requirements

**Six Pillars:**

| # | Pillar | Description |
|---|---|---|
| 1 | **Operational Excellence** | Run and monitor systems; continuously improve |
| 2 | **Security** | Protect data, systems, and assets |
| 3 | **Reliability** | Ensure workloads perform correctly and consistently |
| 4 | **Performance Efficiency** | Use computing resources efficiently |
| 5 | **Cost Optimization** | Avoid unnecessary costs |
| 6 | **Sustainability** | Minimize environmental impacts |

### Pillar Details

#### Pillar 1: Operational Excellence
| Practice | Requirement |
|---|---|
| Organization | Define priorities, operating model, organizational culture |
| Prepare | Design telemetry, manage workload, mitigate deployment risks |
| Operate | Define KPIs, respond to events, learn from failures |
| Evolve | Continuous improvement via feedback loops |

#### Pillar 2: Security
| Practice | Requirement |
|---|---|
| Security Foundations | Separate accounts, security governance |
| Identity & Access Management | Least privilege, IAM roles, MFA, temporary credentials |
| Detection | Logging, monitoring, alerting (CloudTrail, GuardDuty, Security Hub) |
| Infrastructure Protection | Network segmentation, WAF, Security Groups, NACLs |
| Data Protection | Encryption at rest (KMS), encryption in transit (TLS 1.2+), classification |
| Incident Response | Runbooks, automation, forensics readiness |

#### Pillar 3: Reliability
| Practice | Requirement |
|---|---|
| Foundations | Service quotas, network topology, multi-AZ/multi-Region |
| Workload Architecture | Microservices, loose coupling, distributed systems patterns |
| Change Management | Auto-scaling, monitoring, load testing |
| Failure Management | Backup/restore, fault isolation, automatic failover |

#### Pillar 4: Performance Efficiency
| Practice | Requirement |
|---|---|
| Compute Selection | Right-size instances, serverless where appropriate |
| Storage | Select right storage type (S3, EBS, EFS, etc.) |
| Database | Select appropriate database (RDS, DynamoDB, Aurora, etc.) |
| Network | Edge locations, CDN, latency optimization |
| Review | Regular architecture reviews, new technology adoption |

#### Pillar 5: Cost Optimization
| Practice | Requirement |
|---|---|
| Cloud Financial Management | Cost awareness culture, FinOps practices |
| Expenditure Awareness | Cost allocation tags, budgets, alerts |
| Cost-Effective Resources | Right-sizing, Reserved Instances, Savings Plans, Spot |
| Manage Demand & Supply | Auto-scaling, demand shaping |
| Optimize Over Time | Review and adopt new services, decommission waste |

#### Pillar 6: Sustainability
| Practice | Requirement |
|---|---|
| Region Selection | Choose Region with lowest carbon intensity |
| User Behavior Patterns | Reduce resources during low-demand periods |
| Software & Architecture | Efficient code, serverless, event-driven |
| Data Management | Lifecycle policies, compression, deduplication |
| Hardware & Services | Graviton ARM instances, managed services |

### Specific Metrics / Thresholds

| Pillar | Metric | Target |
|---|---|---|
| **Reliability** | Availability | ≥ 99.99% for critical workloads |
| **Reliability** | RTO | ≤ 1 hour for Tier 1; ≤ 24 hours for Tier 3 |
| **Reliability** | RPO | ≤ 1 hour for Tier 1; ≤ 24 hours for Tier 3 |
| **Security** | MFA Coverage | 100% of human users |
| **Security** | Encryption | 100% at rest and in transit |
| **Security** | Critical Vulnerabilities Patched | ≤ 30 days |
| **Security** | Access Reviews | Quarterly minimum |
| **Performance** | CPU Utilization | 40–70% average (right-sized) |
| **Performance** | p99 Latency | ≤ 1s for user-facing (customize per workload) |
| **Performance** | Auto-scaling Response | ≤ 5 minutes |
| **Cost** | Resource Utilization | ≥ 60% average |
| **Cost** | RI/SP Coverage | ≥ 70% of steady-state compute |
| **Cost** | Zombie Resources | 0% (unused resources identified monthly) |
| **Operational** | Deployment Frequency | Daily or more for mature teams |
| **Operational** | MTTR | ≤ 30 minutes for automated recovery |
| **Operational** | Change Failure Rate | ≤ 5% |
| **Sustainability** | vCPU utilization | ≥ 60% (avoid waste) |

### Verification / Compliance

- **AWS Well-Architected Tool:** Free tool in AWS Console for self-assessment
  - Review against each pillar
  - Identify high-risk issues (HRIs) and medium-risk issues (MRIs)
  - Generate improvement plan
- **AWS Well-Architected Lenses:** Domain-specific guidance (SaaS, IoT, ML, Financial Services, etc.)
- **AWS Well-Architected Reviews:**
  - Self-service via the tool
  - Conducted by AWS Solutions Architects
  - Conducted by AWS Partners (Well-Architected Partner Program)
- **Frequency:** At least annually; before major launches; after significant changes
- **Remediation:** Tracked via improvement plan with milestones
- **Reference:** https://docs.aws.amazon.com/wellarchitected/

---

## Cross-Standard Comparison Matrix

| Dimension | ISO 25010 | SOC 2 | GxP | FDA Part 11 | EU AI Act | Google SRE | AWS WA |
|---|---|---|---|---|---|---|---|
| **Focus** | Software quality | Trust/security controls | Regulated industries | Electronic records | AI systems | Reliability/ops | Cloud architecture |
| **Mandatory?** | Voluntary | Contractual/market-driven | Regulatory | Regulatory (FDA) | Regulatory (EU) | Best practice | Best practice |
| **Audit Type** | Self/third-party | CPA audit | Regulatory inspection | FDA inspection | Conformity assessment | Internal review | Self/partner review |
| **Scope** | Software product | Service organization | GxP systems | Electronic records | AI systems | Production services | AWS workloads |
| **Industry** | All | SaaS/cloud | Pharma/biotech/food | Pharma/biotech | All (EU market) | All (tech) | AWS customers |
| **Key Metric** | Quality characteristics | Trust criteria compliance | ALCOA+ data integrity | Audit trail completeness | Risk classification | SLOs/error budgets | Pillar best practices |
| **Update Cycle** | Periodic re-evaluation | Annual | As needed + periodic | Continuous | Phased through 2027 | Continuous | Continuous |

---

## Appendix: Quick Reference Card

### Minimum Requirements Summary

| Standard | Must-Have for Compliance |
|---|---|
| **ISO 25010** | Document quality requirements per 8 characteristics; measure and evaluate |
| **SOC 2** | Security criteria (always) + chosen optional criteria; annual Type II audit |
| **GxP** | ALCOA+ compliant data; validated system (CSV/GAMP 5); audit trails |
| **FDA 21 CFR 11** | Validated system; unique e-signatures; complete audit trails; access controls |
| **EU AI Act** | Risk classification; technical documentation; conformity assessment (high-risk) |
| **Google SRE** | Defined SLOs; error budget policy; monitoring golden signals; blameless postmortems |
| **AWS WA** | Six-pillar review; remediation plan; ongoing architecture reviews |

---

*This report provides a comprehensive overview. Each standard should be consulted in its authoritative source form for implementation. Official documents: ISO 25010:2011, AICPA TSC 2017, FDA 21 CFR Part 11, EU Regulation 2024/1689, Google SRE books, AWS Well-Architected documentation.*
