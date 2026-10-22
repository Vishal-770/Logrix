# Logrix Project Rules & Invariants

These rules apply strictly across this repository on every prompt and action.

---

## 1. Hard Constraints (Zero Tolerance)

- **File Size Limit**: Every single Rust source file (`*.rs`) must remain strictly under 200 lines (`<= 199` lines). Split any file approaching 190 lines into modular sub-files within that crate.
- **Zero Emojis**: Absolutely no unicode emojis anywhere in code, comments, log strings, docs, or commit messages. Only standard ASCII and punctuation.
- **Strict Quality Gates**:
  - `cargo test --workspace` must pass with 100% success rate.
  - `cargo clippy --workspace --all-targets -- -D warnings` must produce zero warnings.
  - `cargo fmt --check` must be clean.
- **Meaningful, High-Coverage Testing**: For every bug fix, feature, or update, write meaningful, comprehensive unit and integration tests. Test real edge cases, boundary conditions, state transitions, and error paths. Avoid duplicate, trivial, or cosmetic tests. Genuine verification depth and high test coverage are mandatory.
- **No Error Swallowing**: Never return empty collections (`Vec::new()`) or `Ok(None)` to mask query, network, or database failures. Always return explicit, typed errors (`LogrixResult`, `async_graphql::Error`, etc.).

---

## 2. Production System Design & Engineering Invariants

- **EVM Numeric Precision (Zero Floating-Point Truncation)**: Never parse or store EVM balances, transaction values, block numbers, or wei amounts as floating-point numbers (`f64`/`f32`). Always use `U256`, `alloy_primitives::U256`, or numeric decimal strings. Floating-point conversions cause silent precision loss on large numbers.
- **Graceful Shutdown & Buffer Flush Safety**: All worker loops (RabbitMQ, SQS, Ingester, Reconciler) must handle graceful termination signals (Tokio `select!`, cancellation tokens, `ctrl_c`), finish their in-flight block or message, commit checkpoints and offsets, and flush staging buffers before exiting. Dropping tasks abruptly must never leave half-committed transactions or orphan locks.
- **Secret Hygiene in Logs & Tracing**: Never log raw RPC URLs containing embedded provider API keys, webhook signing secrets, private keys, or database credentials. Always format secrets with masked redaction (`secret: "***"`).
- **Strict Hexagonal Boundary Hygiene**: `logrix-core` must remain strictly independent of concrete database, queue, or RPC drivers (no `sqlx`, `lapin`, or cloud SDKs in `core`). `logrix-core` defines traits and ports; concrete adapter crates implement them. No circular dependencies between workspace crates.
- **Idempotency Across Ingestion & Processing**: State mutations, entity inserts, and checkpoint updates must be idempotent (`ON CONFLICT DO UPDATE` or deterministic state transitions) so replaying a block or receiving duplicate queue deliveries (at-least-once delivery) never corrupts state or creates duplicate rows.

---

## 3. Engineering Rigor, Safety & Optimization (No "Name Sake" Work)

- **Production-Grade Implementation**: Never write superficial, stubbed, placeholder, or "name sake" code. Implement complete, robust, end-to-end solutions that thoroughly handle edge cases.
- **Memory Safety & Leaks**: Ensure strict memory safety, proper RAII lifecycle management, bounded collections, and zero memory leaks.
- **Network & Cancellation Safety**: Ensure asynchronous operations are cancellation-safe (e.g. Tokio `select!`, socket drops, and graceful task shutdowns must not corrupt state or leave half-committed transactions).
- **Sensible Rust Optimizations**: Proactively implement idiomatic Rust performance optimizations (zero-copy slicing, `Bytes`, minimizing unnecessary `.clone()` calls, efficient indexing) whenever beneficial and safe without breaking existing behavior. Avoid premature over-engineering where not justified.
- **Extensibility & Forward Compatibility**: Design code, interfaces, and domain types with an open-minded, forward-compatible architecture. Anticipate future integrations (new chains, custom plugins, alternative storage engines, future protocol upgrades) and leave clean extension room without requiring breaking architectural rewrites.

---

## 4. Ripple Effects, Cross-Crate Transparency & Documentation

- **Identify Downstream Impacts**: When a requested change impacts, conflicts with, or requires corresponding updates in other modules or crates, always clearly inform the user.
- **Synchronized Updates**: Update all affected sites end-to-end so no subsystem, configuration, or test suite is left in a broken or inconsistent state.
- **Always Keep READMEs Updated**: Whenever features, APIs, configs, or crate behaviors are updated or fixed, properly update the crate's `README.md`, the root project `README.md`, and any related documentation. Keep code examples, architecture overviews, and instructions completely accurate and synchronized. Never leave documentation stale or missing changes.

---

## 5. Always Use & Update Graphify

- **Always Update Graphify After File Changes**: Whenever files, crates, modules, or entities are modified, added, or refactored, always update graphify (`graphify --update` or `/graphify`) so `graphify-out/` (graph.json, graph.html, GRAPH_REPORT.md) stays completely synchronized with the codebase.
- **Query First**: For any codebase architecture questions, cross-crate interactions, or bug tracing, use `graphify query` against `graphify-out/graph.json` to inspect dependencies and relationships before making changes.
- **Architecture Respect**: Keep `graphify-out/` in `.gitignore` so local graph artifacts are never pushed to GitHub. Respect the hexagonal boundaries identified by graphify (ports in `logrix-core`, adapters in specific crates, no circular dependencies).

---

## 6. User Direction, Branching & Git Autonomy Restrictions

- **Never Code on or Push to `main` for Logic/Feature Code**: Never write application code, crate implementations, or architecture changes directly on `main` or push them directly to `main`. Always create a descriptive branch (e.g. `fix/...` or `feat/...`) before making code changes.
- **Pull Requests & CI/CD Gate for Code**: For features, bug fixes, refactors, and core code changes, raise a pull request for the branch, verify all automated CI/CD checks pass, and wait for confirmation before merging.
- **Direct Commits Allowed for Minor Docs & Assets**: For minor documentation edits, README styling, typography, banner/asset tweaks, and `GEMINI.md` rule updates, full PR overhead is not required—direct commits and pushes to `main` (or quick fast-forward merges) can be made directly when requested by the user.
- **Never Commit or Push Without Explicit Request**: Never run `git commit` or `git push` autonomously. Only commit or push if the user explicitly instructs you to do so.
- **Ask After Coding**: After completing any code changes or running verification tests, stop and ask the user what they want to do next. Do not take next steps on your own.
- **Follow User Intent**: Always strictly adhere to the user's instructions.
- **Continuous Rule Self-Persistence**: Whenever the user specifies or refines any new rule, constraint, or engineering invariant in conversation, immediately update this `GEMINI.md` file so the rule is permanently codified and strictly enforced across all subsequent prompts.

---

## 7. SDK Scope, Deployment Architecture & User-Centric Design

- **Pure Containerized Deployment Only (Never Pull Code on Servers)**:
  - Deployments must strictly run pre-built, versioned Docker container images (e.g. from container registries like GHCR or ECR).
  - Never pull source code from GitHub, run `git clone`, or build binaries directly on remote servers or AWS nodes.
- **Remote Published OCI Helm Charts Only (Never Deploy From Local Chart Paths)**:
  - Deployments must strictly install from remote, published, versioned OCI Helm charts (e.g. `oci://ghcr.io/vishal-770/charts/logrix:<version>`).
  - Never run `helm install` or `helm upgrade` targeting local filesystem chart paths (`./deploy/helm/...`) for live deployments. All chart fixes and updates must be packaged, published to the remote OCI registry, and consumed remotely.
  - Never deploy local backing containers (Postgres, RabbitMQ, MinIO) in production environments; all cloud deployments strictly use managed cloud infrastructure (AWS Aurora Serverless v2, SQS, S3).
- **TypeScript SDK Scope (Templates & Config Only, Never Deployment)**:
  - `@logrix/sdk` (and any TypeScript tooling) is strictly reserved for developer templating, schema definition, type generation, and client-side indexer configuration.
  - The SDK must never handle or trigger deployments. Production deployment is strictly infrastructure-as-code: pure Kubernetes (`deploy/k8s`, Helm) or AWS Terraform (`deploy/terraform`). Keep this separation crystal clear in code, CLI commands, and documentation.
- **User-Centric & Highly Customizable Ergonomics**:
  - Always design interfaces, APIs, and tools from the end-user's perspective: intuitive, straightforward, and zero-friction.
  - Provide extensive, granular customization: make it effortless for users to plug in custom logic, toggle optional components on or off, and tune behaviors to their specific requirements.
  - Core/essential capabilities (e.g. data consistency, reorg safety, idempotency, typed errors) must remain solidly enabled by default, while peripheral or specialized features (custom plugins, extra sinks, verbose tracing) can be enabled or disabled cleanly without breaking core functionality.


