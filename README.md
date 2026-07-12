<p align="center">
  <img src="docs/logo/kias-logo.svg" alt="KIAS logo" width="480">
</p>

<h1 align="center">KIAS</h1>
<p align="center"><strong>Control, evidence, and recovery for tool-using AI agents.</strong></p>
<p align="center">A self-hosted Agent Operations Control Plane built in Rust.</p>

<p align="center">
  <a href="https://github.com/Andy-ckm/KIAS/actions/workflows/ci.yml"><img src="https://github.com/Andy-ckm/KIAS/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/Andy-ckm/KIAS/actions/workflows/runtime-smoke.yml"><img src="https://github.com/Andy-ckm/KIAS/actions/workflows/runtime-smoke.yml/badge.svg" alt="Runtime smoke"></a>
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

The core operating loop is:

```text
Register → Constrain → Run → Observe → Intervene → Prove
```

Read the complete boundary in [`PRODUCT.md`](PRODUCT.md) and the adoption strategy in [`docs/product-strategy.md`](docs/product-strategy.md).

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

- **Core** — the supported default control-plane boundary;
- **Extensions** — optional integrations and higher-level capabilities;
- **Labs** — disabled-by-default research with no compatibility promise.

The running instance reports its effective profile and surfaces through authenticated `GET /api/v1/system/capabilities`. Optional routes are absent until explicitly enabled; the Dashboard does not infer capability from repository contents.

See [`docs/architecture.md`](docs/architecture.md) and [`docs/capability-maturity.md`](docs/capability-maturity.md).

## Current capabilities

| Outcome | Capability examples |
|---|---|
| Control | authenticated APIs, RBAC, tool policy, autonomy levels, budgets, rate limits |
| Lifecycle | desired/observed state, health, bounded retries, reconciliation, graceful shutdown |
| Scheduling | load-aware, resource-aware and optional cache-affinity placement |
| Workflows | DAG execution, conditional routing, fan-out, cancellation and checkpoints |
| Evidence | metrics, traces, state transitions and pseudonymous audit records |
| Recovery | durable Agent state, dead-letter handling and checkpoint-based continuation |
| Security | redacted credential types, runtime secret references, TLS deployment checks and privacy gates |
| Interoperability | model, tool and agent protocol interfaces behind explicit adapters |

A capability appearing in the workspace does not imply that its route is enabled or its adapter is production-ready. Unsupported or incomplete security integrations must fail closed.

## Fastest verified startup

### Prerequisites

- Docker Engine or Docker Desktop;
- Docker Compose v2 (`docker compose`);
- `curl`;
- OpenSSL or Python 3 for local secret generation.

### Start the API and Dashboard

```bash
git clone https://github.com/Andy-ckm/KIAS.git
cd KIAS
bash scripts/dev-up.sh
```

The startup helper generates an ignored local JWT signing secret, builds both images, waits for the API and Dashboard, issues an Operator token, and verifies the authenticated capability endpoint.

Open:

- Dashboard: `http://127.0.0.1:3000`
- API health: `http://127.0.0.1:8080/health`
- Operator token: `.kias-dev/operator-token`

Paste the token into the Dashboard connection screen. The browser stores it only in the current tab through `sessionStorage`.

Stop the local stack without deleting the data volume:

```bash
bash scripts/dev-down.sh
```

To delete local persisted data as well:

```bash
export KIAS_JWT_SECRET="$(cat .kias-dev/jwt-secret)"
docker compose down --volumes
```

Both published ports bind to host loopback. The API container listens on `0.0.0.0` only inside the Docker network and refuses that mode unless authentication and the explicit local-container acknowledgement are active.

## Native authenticated quickstart

### Prerequisites

- a current stable Rust toolchain;
- Git;
- Node.js and npm only when running the Dashboard outside Docker;
- OpenSSL or another secure random-value generator for local credentials.

### 1. Build the default Core surface

```bash
cargo build --locked
```

### 2. Generate a runtime signing secret and Operator token

Do not place the secret or token in a tracked file.

```bash
export KIAS_API_SERVER__JWT_SECRET="$(openssl rand -hex 32)"
export KIAS_OPERATOR_TOKEN="$(cargo run -q -p kias-main --bin kias --locked -- token --role operator)"
```

### 3. Start the loopback-only control plane

```bash
cargo run -p kias-main --bin kias --locked -- server
```

In another terminal, export the same secret and token, then inspect the effective product contract:

```bash
curl -s \
  -H "Authorization: Bearer ${KIAS_OPERATOR_TOKEN}" \
  http://127.0.0.1:8080/api/v1/system/capabilities
```

### 4. Run the Dashboard outside Docker

```bash
cd dashboard
npm ci
npm run dev
```

Open the local Vite URL and paste the Operator token.

## Runtime evidence

The `Runtime smoke` workflow does more than compile. On every relevant pull request it:

1. builds the runnable binary;
2. starts the authenticated Core control plane;
3. issues an Operator JWT;
4. creates an Agent;
5. stops and restarts the process;
6. verifies that the Agent is restored from SQLite and that secret environment values were not persisted;
7. repeats the restart test through Docker Compose;
8. shuts the stack down cleanly.

This is evidence for the tested revision and environment, not a production certification.

## Verify the complete workspace

```bash
cargo check --workspace --all-features --locked
cargo test --workspace --all-features --locked
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --all -- --check
```

Configuration starts from [`config/default.toml`](config/default.toml). Nested environment overrides use `__`, such as `KIAS_API_SERVER__PORT`. Supply secrets through environment injection or an external secret provider; never commit a populated `.env` file.

Non-loopback listeners are refused unless authentication is active and `KIAS_TRUSTED_TLS_PROXY=true` explicitly acknowledges a trusted TLS-terminating proxy. Native TLS is not yet wired into the `kias` binary; the process fails rather than pretending that `tls=true` is effective.

## Runtime product profiles

The default profile is `core`. Optional surfaces require explicit opt-in:

```bash
# Extensions
export KIAS_SURFACES__KNOWLEDGE=true
export KIAS_SURFACES__CONTEXT=true
export KIAS_SURFACES__A2A=true
export KIAS_SURFACES__TIER_ROUTING=true
export KIAS_SURFACES__REALTIME=true

# Labs
export KIAS_SURFACES__NL_COMMANDS=true
export KIAS_SURFACES__IM=true
export KIAS_SURFACES__VISUALIZATION=true
```

Synthetic nodes are available only for demonstrations and handler fixtures:

```bash
export KIAS_DEV_FIXTURES=true
```

Never use fixture state as evidence of real runtime discovery or health.

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
- dependency, static-analysis, secret, privacy, provenance, and runtime smoke checks run in CI.

Read [`SECURITY.md`](SECURITY.md), [`PRIVACY.md`](PRIVACY.md), and [`docs/threat-model.md`](docs/threat-model.md) before deployment.

## Project status

The project uses pre-1.0 semantic versioning. Interfaces may change while security boundaries and deployment contracts are stabilized.

The repository configures the following quality gates:

- runtime startup, authentication, restart persistence, and graceful-shutdown smoke tests;
- Core tests and Clippy with warnings denied;
- complete-workspace build and tests;
- Rust formatting;
- machine-enforced Core, Extensions, and Labs dependency boundaries;
- Dashboard dependency, lint, production-build, and container checks;
- CodeQL static analysis;
- OpenSSF Scorecard analysis;
- dependency audit and update automation;
- masked secret, PII, and private-organization scanning.

A badge or green run proves only that its configured checks passed for a revision. It is not a security certification.

Known limitations and readiness evidence are maintained in [`docs/project-status.md`](docs/project-status.md). Planned work is tracked in [`ROADMAP.md`](ROADMAP.md).

## Documentation

- [Product definition](PRODUCT.md)
- [Product strategy](docs/product-strategy.md)
- [Architecture](docs/architecture.md)
- [Capability maturity](docs/capability-maturity.md)
- [Project status and evidence](docs/project-status.md)
- [Threat model](docs/threat-model.md)
- [Security policy](SECURITY.md)
- [Privacy policy](PRIVACY.md)
- [Support policy](SUPPORT.md)
- [Release process](RELEASING.md)

## Contributing

Contributions that improve correctness, security, privacy, interoperability, documentation, and operator experience are welcome.

Please read [`CONTRIBUTING.md`](CONTRIBUTING.md), [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md), [`GOVERNANCE.md`](GOVERNANCE.md), and [`MAINTAINERS.md`](MAINTAINERS.md).

For vulnerabilities or privacy incidents, use the private process in [`SECURITY.md`](SECURITY.md), not a public issue.

## Independence and data policy

KIAS is a general-purpose community project. The public repository must not contain employer, customer, partner, tenant, internal-system, or personal identifiers. Examples use synthetic data and reserved domains. The project does not claim endorsement by, or affiliation with, any employer or customer.

## License

KIAS is released under the [MIT License](LICENSE).
