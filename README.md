<p align="center">
  <img src="docs/logo/kias-logo.svg" alt="KIAS logo" width="480">
</p>

<h1 align="center">KIAS</h1>
<p align="center"><strong>Control, evidence, and recovery for tool-using AI agents.</strong></p>
<p align="center">A self-hosted, policy-driven agent control plane built in Rust.</p>

<p align="center">
  <a href="https://github.com/Andy-ckm/KIAS/actions/workflows/ci.yml"><img src="https://github.com/Andy-ckm/KIAS/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/Andy-ckm/KIAS/actions/workflows/codeql.yml"><img src="https://github.com/Andy-ckm/KIAS/actions/workflows/codeql.yml/badge.svg" alt="CodeQL"></a>
  <a href="https://scorecard.dev/viewer/?uri=github.com/Andy-ckm/KIAS"><img src="https://api.scorecard.dev/projects/github.com/Andy-ckm/KIAS/badge" alt="OpenSSF Scorecard"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License"></a>
</p>

> [!IMPORTANT]
> KIAS is a pre-1.0 project under active development. It is suitable for research, evaluation, and controlled pilots. It has not completed an independent security audit, and production deployment requires threat modeling, hardened configuration, external secret management, tenant-isolation validation, and operational review.

## What KIAS is

Building an agent demo is easy; operating agents predictably is not. Real systems need lifecycle control, policy enforcement, bounded tools, scheduling, recovery, auditability, and privacy-aware operations.

KIAS treats an agent as a managed resource rather than an unbounded script. Its product contract is organized around three outcomes:

- **Control** — identity, tools, autonomy, budgets, rate limits, and resources require explicit decisions.
- **Evidence** — important behavior produces privacy-aware state, metrics, traces, and pseudonymous audit records.
- **Recovery** — work has bounded retries, cancellation, checkpoints, reconciliation, and graceful shutdown.

Read the complete product boundary in [`PRODUCT.md`](PRODUCT.md).

## Who it is for

KIAS is designed primarily for:

- AI platform engineers operating multiple agents;
- security and governance engineers defining control boundaries;
- SRE and operations teams responsible for health, failure, and recovery;
- architects evaluating a transparent self-hosted agent control plane.

KIAS is not a hosted model service, model-training platform, no-code chatbot builder, generic Linux automation suite, or repository for organization-specific workflows and compliance mappings.

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                     API and control plane                    │
│         authentication · authorization · desired state      │
├───────────────────┬───────────────────┬──────────────────────┤
│ Scheduler         │ Lifecycle control │ Workflow execution   │
│ placement policy  │ reconcile/recover │ checkpoint / retry   │
├───────────────────┼───────────────────┼──────────────────────┤
│ Tool boundary     │ Policy engine     │ Observability / audit│
│ timeout/isolation │ role/cost/autonomy│ metrics/evidence     │
├───────────────────┴───────────────────┴──────────────────────┤
│ Persistence · normalized integrations · shared contracts     │
└──────────────────────────────────────────────────────────────┘
```

The repository is divided into three product tiers:

- **Core** — the supported control-plane boundary;
- **Extensions** — optional integrations and higher-level capabilities;
- **Labs** — disabled-by-default research with no compatibility promise.

See [`docs/architecture.md`](docs/architecture.md) and [`docs/capability-maturity.md`](docs/capability-maturity.md).

## Current capabilities

| Outcome | Capability examples |
|---|---|
| Control | authenticated APIs, RBAC, tool policy, autonomy levels, budgets, rate limits |
| Lifecycle | desired/observed state, health, bounded retries, reconciliation, graceful shutdown |
| Scheduling | load-aware, resource-aware and optional cache-affinity placement |
| Workflows | DAG execution, conditional routing, fan-out, cancellation and checkpoints |
| Evidence | metrics, traces, state transitions and pseudonymous audit records |
| Recovery | durable state primitives, dead-letter handling and checkpoint-based continuation |
| Security | redacted credential types, runtime secret references, TLS options and privacy gates |
| Interoperability | model, tool and agent protocol interfaces behind explicit adapters |

A capability appearing in the workspace does not imply every adapter or backend is production-ready. Unsupported or incomplete security integrations must fail closed.

## Quickstart

### Prerequisites

- a current stable Rust toolchain;
- Git;
- OpenSSL or another secure random-value generator for local credentials;
- optional external services only for the integration being tested.

### Build the default Core surface

```bash
git clone https://github.com/Andy-ckm/KIAS.git
cd KIAS

cargo build --locked
cargo test --locked
```

### Verify the complete workspace

```bash
cargo check --workspace --all-features --locked
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo fmt --all -- --check
```

### Start a local authenticated control plane

The shipped configuration binds only to loopback and enables authentication. Generate a temporary local JWT secret at runtime rather than placing one in a file:

```bash
export KIAS_API_SERVER__JWT_SECRET="$(openssl rand -hex 32)"
cargo run -p kias-main --bin kias --locked -- server
```

Inspect command-line options:

```bash
cargo run -p kias-main --bin kias --locked -- --help
```

Configuration starts from [`config/default.toml`](config/default.toml). Nested environment overrides use `__`, such as `KIAS_API_SERVER__PORT`. Supply secrets through environment injection or an external secret provider; never commit a populated `.env` file.

Non-loopback listeners are refused unless authentication is active and `KIAS_TRUSTED_TLS_PROXY=true` explicitly acknowledges a trusted TLS-terminating proxy. Native TLS is not yet wired into the `kias` binary; the process fails rather than pretending that `tls=true` is effective.

## Security model

KIAS assumes that prompts, model output, uploaded files, webhook bodies, identity claims, and tool parameters may be hostile or sensitive.

Default project expectations are:

- authentication is enabled before exposing management APIs;
- credentials never use plaintext diagnostics or serialization;
- request logs exclude query strings and message bodies;
- webhook adapters verify origin and replay windows or fail closed;
- raw external payload retention is disabled by default;
- audit subjects are pseudonymous where direct identity is unnecessary;
- tool execution is restricted by explicit policy and isolation;
- dependency, static-analysis, secret, privacy, and provenance checks run in CI.

Read [`SECURITY.md`](SECURITY.md), [`PRIVACY.md`](PRIVACY.md), and [`docs/threat-model.md`](docs/threat-model.md) before deployment.

## Project status

The project uses pre-1.0 semantic versioning. Interfaces may change while security boundaries and deployment contracts are stabilized.

The repository configures the following quality gates:

- workspace build, tests, formatting, and Clippy with warnings denied;
- machine-enforced Core, Extensions, and Labs dependency boundaries;
- dashboard dependency, lint, and production-build checks;
- CodeQL static analysis;
- OpenSSF Scorecard analysis;
- dependency audit and update automation;
- masked secret, PII, and private-organization scanning.

A badge or green run proves only that its configured checks passed for a revision. It is not a security certification.

Known limitations and readiness evidence are maintained in [`docs/project-status.md`](docs/project-status.md). Planned work is tracked in [`ROADMAP.md`](ROADMAP.md).

## Documentation

- [Product definition](PRODUCT.md)
- [Architecture](docs/architecture.md)
- [Capability maturity](docs/capability-maturity.md)
- [Project status and evidence](docs/project-status.md)
- [Development guide](docs/development.md)
- [API documentation](docs/api.md)
- [Threat model](docs/threat-model.md)
- [Security policy](SECURITY.md)
- [Privacy policy](PRIVACY.md)
- [Support policy](SUPPORT.md)
- [Release process](RELEASING.md)

## Contributing

Contributions that improve correctness, security, privacy, interoperability, documentation, and operator experience are welcome.

Please read:

- [`CONTRIBUTING.md`](CONTRIBUTING.md)
- [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
- [`GOVERNANCE.md`](GOVERNANCE.md)
- [`MAINTAINERS.md`](MAINTAINERS.md)

For vulnerabilities or privacy incidents, use the private process in [`SECURITY.md`](SECURITY.md), not a public issue.

## Independence and data policy

KIAS is a general-purpose community project. The public repository must not contain employer, customer, partner, tenant, internal-system, or personal identifiers. Examples use synthetic data and reserved domains. The project does not claim endorsement by, or affiliation with, any employer or customer.

## License

KIAS is released under the [MIT License](LICENSE).