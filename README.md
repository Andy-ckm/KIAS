<p align="center">
  <img src="docs/logo/kias-logo.svg" alt="KIAS logo" width="460">
</p>

<h1 align="center">KIAS</h1>
<p align="center"><strong>Control, evidence, and recovery for AI agents that execute tools.</strong></p>
<p align="center">A self-hosted Agent Operations Control Plane built in Rust.</p>

<p align="center">
  <a href="https://github.com/Andy-ckm/KIAS/actions/workflows/ci.yml"><img src="https://github.com/Andy-ckm/KIAS/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/Andy-ckm/KIAS/actions/workflows/runtime-smoke.yml"><img src="https://github.com/Andy-ckm/KIAS/actions/workflows/runtime-smoke.yml/badge.svg" alt="Runtime smoke"></a>
  <a href="https://github.com/Andy-ckm/KIAS/actions/workflows/codeql.yml"><img src="https://github.com/Andy-ckm/KIAS/actions/workflows/codeql.yml/badge.svg" alt="CodeQL"></a>
  <a href="https://scorecard.dev/viewer/?uri=github.com/Andy-ckm/KIAS"><img src="https://api.scorecard.dev/projects/github.com/Andy-ckm/KIAS/badge" alt="OpenSSF Scorecard"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License"></a>
</p>

> [!IMPORTANT]
> KIAS is pre-1.0. The Core path is designed for evaluation and controlled pilots. It has not completed an independent security audit. Read the [known limitations](#known-limitations) before deployment.

## Why KIAS exists

Agent frameworks help developers create agents. KIAS focuses on the operational problem that begins after an agent can call tools:

- Who is allowed to run it?
- Which image and command were admitted?
- What resource and network boundaries were enforced?
- What actually happened during execution?
- Can an operator cancel, retry, or recover the work?
- Is there durable evidence after the process restarts?

KIAS treats an Agent and each Agent Run as managed resources.

```text
AgentSpec
   │
   ▼
Policy admission
   │ allow / deny + reasons
   ▼
Isolated Docker Run
   │ network=none · read-only rootfs · no host mounts
   ▼
Status + logs + resource observations
   │
   ▼
Evidence digest + replay checkpoint
   │
   ├── Cancel
   ├── Retry
   └── Recover after control-plane restart
```

The product contract is:

- **Control** — authenticated APIs, role boundaries, explicit execution opt-in, image allowlists, timeouts and resource limits.
- **Evidence** — durable Run state, bounded logs, policy decisions, sandbox facts, resource observations, lineage and SHA-256 evidence digests.
- **Recovery** — cancellation, bounded retries, interrupted-run detection and replay-based recovery.

## Who it is for

KIAS is aimed at:

- AI platform engineers operating tool-using agents;
- security and governance engineers defining execution boundaries;
- SRE and operations teams responsible for failure and recovery;
- architects evaluating transparent, self-hosted agent infrastructure.

KIAS is **not** a hosted model API, model-training platform, no-code chatbot builder, or a replacement for an Agent SDK. Agents may be implemented in any language as long as they can run as a bounded container command.

## What is verified today

The `Runtime smoke` workflow proves the following path on a clean runner:

1. build the KIAS binary;
2. start the authenticated control plane;
3. register an AgentSpec;
4. admit a pinned image through policy;
5. execute it in a Docker sandbox;
6. capture stdout, stderr and resource observations;
7. verify the evidence digest and enforced sandbox settings;
8. exercise a failed Run with bounded automatic retries;
9. create a lineage-linked manual retry;
10. cancel a running container;
11. stop the control plane during a Run;
12. restart, mark the Run interrupted, and create a recovery Run;
13. verify raw Run input is absent from SQLite;
14. persist Agent and Run metadata in SQLite;
15. shut down cleanly.

A green workflow proves the configured checks for that revision and environment. It is not a production certification.

## Five-minute control-plane start

This starts the API and Dashboard. The standard Compose stack intentionally does **not** mount the host container socket, so `sandboxed-runs` is reported as unavailable in that topology.

### Prerequisites

- Docker Engine or Docker Desktop;
- Docker Compose v2;
- `curl`;
- OpenSSL or Python 3.

```bash
git clone https://github.com/Andy-ckm/KIAS.git
cd KIAS
bash scripts/dev-up.sh
```

Open:

- Dashboard: `http://127.0.0.1:3000`
- API health: `http://127.0.0.1:8080/health`
- Operator token: `.kias-dev/operator-token`

Paste the token into the Dashboard connection screen. The browser keeps it only in the current tab through `sessionStorage`.

Stop without deleting the data volume:

```bash
bash scripts/dev-down.sh
```

## Run a real bounded Agent

For the current pre-1.0 implementation, run the KIAS process on a trusted development host that has access to a dedicated Docker daemon. KIAS never pulls an image during a Run; the image must already exist and must be explicitly allowed.

### 1. Prepare a pinned fixture image

```bash
docker pull busybox:1.36
```

### 2. Configure KIAS

```bash
export KIAS_API_SERVER__JWT_SECRET="$(openssl rand -hex 32)"
export KIAS_API_SERVER__JWT_ISSUER="kias-local"
export KIAS_DB_PATH="$PWD/kias-local.db"
export KIAS_RUN_ALLOWED_IMAGES="busybox:1.36"
```

### 3. Start the control plane

```bash
cargo run -p kias-main --bin kias --locked -- server
```

In a second terminal, export the same configuration and issue an Operator token:

```bash
export KIAS_OPERATOR_TOKEN="$(
  cargo run -q -p kias-main --bin kias --locked -- \
    token --role operator --subject local-operator
)"
```

### 4. Register an execution-enabled AgentSpec

```bash
AGENT_RESPONSE="$(
  curl --fail --silent \
    -X POST \
    -H "Authorization: Bearer ${KIAS_OPERATOR_TOKEN}" \
    -H "Content-Type: application/json" \
    -H "X-Idempotency-Key: quickstart-agent-v1" \
    --data '{
      "name": "stdin-worker",
      "image": "busybox:1.36",
      "command": ["sh", "-c", "cat"],
      "resource_request": {
        "cpu": "500m",
        "memory": "64Mi",
        "gpu": "0"
      },
      "labels": {
        "kias.io/execution": "enabled"
      }
    }' \
    http://127.0.0.1:8080/api/v1/agents
)"

export KIAS_AGENT_ID="$(
  python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["id"])' \
    <<<"${AGENT_RESPONSE}"
)"
```

### 5. Start the Run

```bash
RUN_RESPONSE="$(
  curl --fail --silent \
    -X POST \
    -H "Authorization: Bearer ${KIAS_OPERATOR_TOKEN}" \
    -H "Content-Type: application/json" \
    -H "X-Idempotency-Key: quickstart-run-v1" \
    --data '{
      "input": "hello from KIAS",
      "timeout_seconds": 20,
      "max_retries": 0
    }' \
    "http://127.0.0.1:8080/api/v1/agents/${KIAS_AGENT_ID}/runs"
)"

export KIAS_RUN_ID="$(
  python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])' \
    <<<"${RUN_RESPONSE}"
)"
```

### 6. Observe and prove it

```bash
curl --fail --silent \
  -H "Authorization: Bearer ${KIAS_OPERATOR_TOKEN}" \
  "http://127.0.0.1:8080/api/v1/runs/${KIAS_RUN_ID}"

curl --fail --silent \
  -H "Authorization: Bearer ${KIAS_OPERATOR_TOKEN}" \
  "http://127.0.0.1:8080/api/v1/runs/${KIAS_RUN_ID}/logs"

curl --fail --silent \
  -H "Authorization: Bearer ${KIAS_OPERATOR_TOKEN}" \
  "http://127.0.0.1:8080/api/v1/runs/${KIAS_RUN_ID}/evidence"

curl --fail --silent \
  -H "Authorization: Bearer ${KIAS_OPERATOR_TOKEN}" \
  "http://127.0.0.1:8080/api/v1/runs/${KIAS_RUN_ID}/checkpoint"
```

The evidence response includes:

- the policy version and admission decision;
- image, timeout, CPU, memory and PID constraints;
- retry and recovery lineage;
- exit status and bounded stdout/stderr;
- observed peak memory and CPU when available;
- sandbox facts such as `network=none`, read-only root filesystem, dropped capabilities, non-root user and no host mounts;
- a SHA-256 digest over the evidence envelope.

## Intervene: cancel, retry and recover

```bash
# Cancel a queued or running Run
curl --fail --silent \
  -X POST \
  -H "Authorization: Bearer ${KIAS_OPERATOR_TOKEN}" \
  -H "X-Idempotency-Key: cancel-${KIAS_RUN_ID}" \
  "http://127.0.0.1:8080/api/v1/runs/${KIAS_RUN_ID}/cancel"

# Create a new Run linked to a failed or cancelled Run.
# Resupply the identical original input; KIAS stores only its digest.
curl --fail --silent \
  -X POST \
  -H "Authorization: Bearer ${KIAS_OPERATOR_TOKEN}" \
  -H "Content-Type: application/json" \
  -H "X-Idempotency-Key: retry-${KIAS_RUN_ID}" \
  --data '{"input":"hello from KIAS"}' \
  "http://127.0.0.1:8080/api/v1/runs/${KIAS_RUN_ID}/retry"

# Replay a Run that was marked interrupted after control-plane restart
curl --fail --silent \
  -X POST \
  -H "Authorization: Bearer ${KIAS_OPERATOR_TOKEN}" \
  -H "Content-Type: application/json" \
  -H "X-Idempotency-Key: recover-${KIAS_RUN_ID}" \
  --data '{"input":"hello from KIAS"}' \
  "http://127.0.0.1:8080/api/v1/runs/${KIAS_RUN_ID}/recover"
```

Recovery is deliberately explicit. KIAS does not claim to snapshot arbitrary process memory. It persists the admitted AgentSpec, input digest, policy decision and lineage. The caller must resupply the identical input, which is verified against the stored SHA-256 digest before a replay Run starts.

## Core Run API

| Method | Path | Minimum role | Purpose |
|---|---|---:|---|
| `POST` | `/api/v1/agents/:id/runs` | Operator | Admit and start a Run |
| `GET` | `/api/v1/runs` | Viewer | List durable Runs |
| `GET` | `/api/v1/runs/:id` | Viewer | Read lifecycle state |
| `GET` | `/api/v1/runs/:id/logs` | Viewer | Read bounded stdout/stderr |
| `GET` | `/api/v1/runs/:id/evidence` | Viewer | Read policy, sandbox, resource and lineage evidence |
| `GET` | `/api/v1/runs/:id/checkpoint` | Viewer | Read replay checkpoint metadata |
| `POST` | `/api/v1/runs/:id/cancel` | Operator | Stop the named sandbox container |
| `POST` | `/api/v1/runs/:id/retry` | Operator | Create a lineage-linked retry |
| `POST` | `/api/v1/runs/:id/recover` | Operator | Replay an interrupted Run |

Instance capabilities are discoverable through authenticated:

```text
GET /api/v1/system/capabilities
```

Clients must check `sandboxed-runs.enabled`; repository code existing on disk does not mean a particular deployment has a runner.

## Default sandbox policy

A Core Agent Run is admitted only when:

- the Agent has `kias.io/execution=enabled`;
- its image exactly matches `KIAS_RUN_ALLOWED_IMAGES`;
- the image is already present on the runner;
- the image does not use the mutable `:latest` tag;
- the AgentSpec does not contain environment values;
- CPU is at most 1 core;
- memory is at most 512 MiB;
- GPU is absent or `0`;
- timeout is between 1 and 300 seconds;
- automatic retries are at most 3;
- input is at most 64 KiB.

The Docker executor applies:

```text
network:             none
root filesystem:    read-only
host mounts:         none
Linux capabilities: dropped
new privileges:     disabled
container user:     65534:65534
PID limit:          64
writable storage:   bounded /tmp tmpfs
image pull:         never
```

Do not pass secrets through AgentSpec environment values. The Core path accepts bounded stdin input for the current execution but persists only its SHA-256 digest and byte count. Automatic retries retain input only in the active process; manual retry and recovery require the caller to resupply the identical value. Logs and tool output may still contain sensitive data, so configure retention and access controls accordingly.

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│ API / Dashboard                                              │
│ JWT authentication · Viewer/Operator/Admin authorization     │
├──────────────────────────────────────────────────────────────┤
│ Agent and Run control plane                                  │
│ AgentSpec · policy admission · lifecycle · lineage           │
├───────────────────────────────┬──────────────────────────────┤
│ Dedicated Docker runner       │ Evidence and recovery        │
│ bounded container execution   │ logs · resources · digest    │
│ cancel by Run identity        │ retry · replay checkpoint    │
├───────────────────────────────┴──────────────────────────────┤
│ SQLite persistence · audit · idempotency digest · DLQ         │
└──────────────────────────────────────────────────────────────┘
```

The repository has three tiers:

- **Core** — default, supported control-plane contracts;
- **Extensions** — optional integrations;
- **Labs** — disabled-by-default research without compatibility guarantees.

See [architecture](docs/architecture.md), [capability maturity](docs/capability-maturity.md) and [product strategy](docs/product-strategy.md).

## Security and privacy properties

- management APIs are authenticated by default;
- Viewer is read-only, Operator mutates control-plane resources, Admin reads configuration surfaces;
- request logs exclude query strings and bodies;
- idempotency stores retain operation digests, not request bodies;
- credentials and secrets use redacted diagnostics;
- incomplete webhook verification fails closed;
- raw external payload retention is disabled by default;
- Agent Run environment values are denied;
- images use an explicit allowlist and are never pulled at execution time;
- Runner containers have no network, no host mounts and no Linux capabilities;
- public repository scanning blocks likely secrets, PII and private organization aliases.

Read [SECURITY.md](SECURITY.md), [PRIVACY.md](PRIVACY.md) and the [threat model](docs/threat-model.md).

## Known limitations

KIAS is not yet a general production multi-tenant runtime.

- The current runner uses a local Docker CLI. Production deployment should move execution behind an independently authenticated runner service; do not mount a host container socket into the API container.
- The standard Compose stack starts the control plane and Dashboard but intentionally has no execution privilege.
- SQLite is the current single-node authority. High availability and distributed transaction semantics are not implemented.
- Object-level and tenant-level authorization are pre-1.0 blockers.
- Replay recovery restarts the admitted command; it is not a memory or filesystem snapshot.
- Raw Run input is not persisted. Manual retry and recovery require the caller to resupply the identical input; KIAS verifies it against the durable digest.
- Resource observations are best-effort samples; configured limits and final exit state are authoritative.
- Image signature verification and software-bill-of-material admission are not yet implemented.
- Network policy is currently `none`; selectively controlled egress is not implemented.
- The Dashboard does not yet expose the complete Run evidence and intervention workflow.

These limits are intentional and visible. KIAS fails closed where a claimed security control is unavailable.

## Verify locally

```bash
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo check --workspace --all-features --locked
cargo test --workspace --all-features --locked
cargo fmt --all -- --check

cd dashboard
npm ci
npm run lint
npm run build
```

For the complete execution lifecycle, prepare `busybox:1.36`, export the environment from the real-run quickstart, build `kias`, then run:

```bash
bash scripts/runtime-smoke-agent-run.sh
```

The script proves execution, evidence, cancellation, retry, restart recovery and the absence of raw Run input from SQLite.

## Project status and roadmap

The current priority order is:

1. keep the complete Run lifecycle green in runtime smoke;
2. separate the execution runner from the API process;
3. add image digest/signature and provenance admission;
4. add external secret references and configurable evidence-log retention;
5. deliver object/tenant authorization;
6. expose Runs, evidence and intervention in the Dashboard;
7. add a high-availability persistence option.

Detailed readiness evidence is maintained in [docs/project-status.md](docs/project-status.md). Planned work is tracked in [ROADMAP.md](ROADMAP.md).

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

Contributions that improve correctness, security, privacy, interoperability, documentation and operator experience are welcome.

Read [CONTRIBUTING.md](CONTRIBUTING.md), [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), [GOVERNANCE.md](GOVERNANCE.md) and [MAINTAINERS.md](MAINTAINERS.md).

For vulnerabilities or privacy incidents, use the private process in [SECURITY.md](SECURITY.md), not a public issue.

## Independence and data policy

KIAS is a general-purpose community project. The public repository must not contain employer, customer, partner, tenant, internal-system or personal identifiers. Examples use synthetic data and reserved domains. The project does not claim endorsement by, or affiliation with, any employer or customer.

## License

KIAS is released under the [MIT License](LICENSE).
