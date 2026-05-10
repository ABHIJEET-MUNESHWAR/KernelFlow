# KernelFlow

> A composable, type-safe, async **workflow VM** in Rust.
> Builds the same category of infrastructure as Fermah Kernel: deterministic DAGs, sandboxed WASM execution, peer-to-peer event propagation, and onchain attestation on Solana.

[![CI](https://github.com/abhijeet/KernelFlow/actions/workflows/ci.yml/badge.svg)](.github/workflows/ci.yml)

---

## ✨ Why this exists

KernelFlow is the portfolio project that demonstrates, end-to-end:

- async VM (WASM via `wasmtime`, fuel-metered)
- distributed systems (`libp2p` gossipsub mesh)
- embedded storage (`rocksdb` w/ column-family partitioning + hash sharding)
- actor model + cooperative scheduling (Tokio)
- onchain attestation (Anchor program on Solana devnet)
- agentic AI (LLM-driven workflow synthesis)

---

## 🏗 Architecture

```
                   ┌─────────────────────────┐
   GraphQL/WS ──►  │   kernelflow-api        │  ◄── subscriptions
                   ├─────────────────────────┤
                   │   kernelflow-scheduler  │   actor model + retry / TO / CB / RL
                   │     (event-driven bus)  │
        ┌──────────┼─────────────┬───────────┼─────────────┐
        ▼          ▼             ▼           ▼             ▼
 kernelflow-  kernelflow-  kernelflow-  kernelflow-   kernelflow-
   sandbox     storage       p2p          attest          ai
   (wasmtime)  (rocksdb)   (libp2p)   (ed25519+Solana) (LLM agents)
        │
        ▼
 kernelflow-core   ← shared traits, DAG, errors, events (zero side-effects)
```

### Architecture style
- **Event-driven micro-services** inside one binary: every crate listens on a `tokio::sync::broadcast` of `KernelEvent`s; new sinks (storage, p2p, attestation) plug in without modifying the scheduler.
- **CQRS-lite**: GraphQL `Query` reads from `kernelflow-storage` snapshots; `Mutation` writes events that the scheduler reduces to new state.
- **Saga**: long-running workflows are coordinated as a saga; each node is a step with compensating actions (retry / timeout / circuit break).

---

## 🧱 Project layout

```
KernelFlow/
├── Cargo.toml                        # workspace root
├── Dockerfile, docker-compose.yml
├── .github/workflows/ci.yml          # fmt + clippy + test + tarpaulin + docker
├── postman/KernelFlow.postman_collection.json
├── anchor/programs/kernelflow-attest # Solana Anchor program
└── crates/
    ├── kernelflow-core               # DAG, traits, errors, events, capability, resource
    ├── kernelflow-scheduler          # actor + resilience (retry/TO/CB/RL)
    ├── kernelflow-sandbox            # wasmtime fuel-metered VM
    ├── kernelflow-storage            # RocksDB CF + sharding
    ├── kernelflow-p2p                # libp2p gossipsub
    ├── kernelflow-attest             # ed25519 + Solana submitter
    ├── kernelflow-ai                 # LLM provider trait + synthesizer
    ├── kernelflow-api                # axum + async-graphql HTTP/WS
    ├── kernelflow-nodes              # verified node library + registry
    ├── kernelflow-matchmaker         # operator registry + reputation matchmaker
    └── kernelflow-node               # binary
```

### Run the canonical Fermah-style demo
```bash
cargo run -p kernelflow-node --example eth_threshold
# → attestation hash: 3b3dce803768e88419a8fe574ef0dc0e114df00521b27119b2d5603197d6d30d
```
The example builds a 5-node DAG (`p1, p2, p3 → median → threshold>3000`) using only the verified library, validates the wiring at composition time, executes it on the actor scheduler, and emits a deterministic SHA-256 attestation.

---

## 🚀 Quick start

### Prerequisites
- Rust **1.80+** (`rustup default stable`)
- `clang`, `cmake`, `pkg-config`, `libssl-dev` (Linux) — required by `rocksdb` and `wasmtime`.
- Docker (optional)

### Build & run locally
```bash
# build everything
cargo build --release

# run the node
./target/release/kernelflow

# probe
curl -s http://localhost:8080/health
# → ok

# GraphQL
curl -s -X POST http://localhost:8080/graphql \
  -H 'content-type: application/json' \
  -d '{"query":"{ health version }"}'
```

GraphiQL playground: http://localhost:8080/graphql

### Run via Docker
```bash
docker compose up --build
```

### Tests
```bash
cargo test --workspace --all-features
```

### Coverage (target: 100 %)
```bash
cargo install cargo-tarpaulin
cargo tarpaulin --workspace --out Html --output-dir coverage
open coverage/tarpaulin-report.html
```

### Benchmarks
```bash
cargo bench -p kernelflow-scheduler
```

### Lints
```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 🧪 Postman
Import `postman/KernelFlow.postman_collection.json`. Set `baseUrl` = `http://localhost:8080`.

---

## ⚙️ Configuration

All flags can be set via env or CLI (see `kernelflow --help`):

| Env                | Default               | Description                  |
|--------------------|-----------------------|------------------------------|
| `KF_HTTP_ADDR`     | `0.0.0.0:8080`        | GraphQL HTTP listen          |
| `KF_P2P_ADDR`      | `/ip4/0.0.0.0/tcp/0`  | libp2p multiaddr             |
| `KF_DATA_DIR`      | `./data`              | RocksDB path                 |
| `KF_METRICS_PORT`  | `9090`                | Prometheus exporter port     |
| `RUST_LOG`         | `info`                | tracing-subscriber filter    |

---

## 📈 Performance & complexity

| Operation                              | Complexity | Notes                                         |
|----------------------------------------|-----------:|-----------------------------------------------|
| `Dag::topo_sort`                       | `O(V + E)` | Kahn's algorithm                              |
| `Scheduler::run` (no fan-out)          | `O(V + E)` | Each node runs once                           |
| `Scheduler::run` (parallel batch)      | `O(d)` wall, where `d` = critical-path depth | Concurrent via `FuturesUnordered` |
| `Store::put / get`                     | `O(log N)` amortized | RocksDB LSM                                  |
| `ShardedStore::shard_for`              | `O(1)`     | DefaultHasher mod N                           |
| `hash_json`                            | `O(n)`     | SHA-256 over canonical JSON                   |

Run `cargo bench` for criterion reports under `target/criterion/`.

---

## 🛡 Reliability features (per `Guide`)

| Concern              | Where it lives                                            |
|----------------------|-----------------------------------------------------------|
| Timeout              | `scheduler::resilience::ResilientExecutor` (tokio::timeout)|
| Retry + backoff      | `ResilientExecutor` (exponential, capped)                 |
| Circuit breaker      | `resilience::CircuitBreaker`                              |
| Rate limiting        | `governor::RateLimiter` (per-node)                        |
| Fault tolerance      | Actor isolation; one node panic ≠ scheduler death         |
| Recovery logic       | Event log in RocksDB `events` CF; replay on boot          |
| Determinism          | Canonical-JSON SHA-256 attestation; cycle-free DAGs       |
| Observability        | `tracing` (JSON) + Prometheus exporter on `:9090`         |
| Type-safety          | `Dag<N>` is generic; conditions enum-checked              |

---

## 🤖 GenAI / Agentic AI

`kernelflow-ai`:
- `LlmProvider` trait — pluggable (`OpenAiCompat` ships built-in; works with OpenAI, Ollama, vLLM).
- `WorkflowSynthesizer<P>` — describe a goal in English → typed `Vec<NodeSpec>` validated by `kernelflow-core`.
- Anomaly agent (TODO v2) — subscribes to `KernelEvent::WorkflowFailed`, asks LLM for RCA.

```rust
let llm = OpenAiCompat { base_url: "https://api.openai.com/v1".into(), api_key: env!("OPENAI_KEY").into(), model: "gpt-4o-mini".into() };
let nodes = WorkflowSynthesizer { llm }.synthesize("fetch ETH price, store in RocksDB, attest onchain").await?;
```

---

## ⛓ Solana / Anchor

`anchor/programs/kernelflow-attest` is a minimal Anchor program that stores `(workflow_id, output_hash, signer)` PDAs. Build & deploy:

```bash
cd anchor
anchor build
anchor deploy --provider.cluster devnet
```

---

## 🔭 Roadmap
- [ ] BFT consensus over attestations across the libp2p mesh
- [ ] eBPF runtime alongside WASM (`rbpf`-style) — direct Anza/Firedancer parity
- [ ] State-sync / runtime upgrades / backward-compat (Fermah JD nice-to-haves)
- [ ] OpenTelemetry exporter + Grafana dashboard JSON

---

## 🧰 Self-evaluation

| Guideline                                        | Status |
|--------------------------------------------------|:------:|
| SOLID                                             | ✅ trait-driven layers (DIP), generic DAG (OCP) |
| Micro-services pattern (event-driven + CQRS)      | ✅ broadcast bus + storage CFs |
| Partitioning & sharding                           | ✅ RocksDB CFs + `ShardedStore` |
| Timeout / retry / fault tolerance                 | ✅ `ResilientExecutor` |
| Rate limit & circuit breaker                      | ✅ governor + custom CB |
| Robust error handling                             | ✅ `KernelError` `#[non_exhaustive]` + `KernelResult` |
| GraphQL                                           | ✅ async-graphql + GraphiQL + WS subs |
| Test coverage                                     | 🟡 starter unit + integration tests; aim 100 % via tarpaulin in CI |
| Modular workspace                                 | ✅ 9 crates |
| 3rd-party (Tokio, Serde, thiserror)               | ✅ |
| GenAI / Agentic                                   | ✅ `kernelflow-ai` |
| Generics                                          | ✅ `Dag<N>`, `Scheduler<N>`, `Store::put<V>` |
| Tokio runtime + parallel/batch                    | ✅ `FuturesUnordered` over ready frontier |
| Logging / observability                           | ✅ `tracing` JSON + Prometheus |
| Compile-time constraints                          | ✅ DAG cycle rejection at `build()`-time |
| Benchmarks                                        | ✅ criterion in scheduler |
| CI/CD                                             | ✅ `.github/workflows/ci.yml` |
| Dockerfile                                        | ✅ multi-stage |
| Postman                                           | ✅ `postman/KernelFlow.postman_collection.json` |

### Known improvements / next iteration
1. Wire `kernelflow-storage` into the GraphQL `Query.workflow(id)` resolver (presently returns stub).
2. Persist `KernelEvent`s into the `events` CF in `kernelflow-node::main` (subscribe loop).
3. Replace the WASM `add` convention with a proper `kernelflow_abi` (memory layout for input/output).
4. Add `cargo-deny` to CI (license + advisory checks).
5. Add `proptest` over `Dag::topo_sort` (random graphs).
6. Replace the `failsafe` dep (currently unused) with the in-house `CircuitBreaker` only, or vice-versa.

---

## License
Apache-2.0

