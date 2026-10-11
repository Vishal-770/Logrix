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

- **Never Code on or Push to `main`**: Never write code directly on `main` or push directly to `main`. Always create a descriptive branch (e.g. `fix/...` or `feat/...`) before making changes.
- **Pull Requests & CI/CD Gate**: Raise a pull request for the branch, verify all automated CI/CD checks pass, and wait for confirmation before merging.
- **Never Commit or Push Without Explicit Request**: Never run `git commit` or `git push` autonomously. Only commit or push if the user explicitly instructs you to do so.
- **Ask After Coding**: After completing any code changes or running verification tests, stop and ask the user what they want to do next. Do not take next steps on your own.
- **Follow User Intent**: Always strictly adhere to the user's instructions.
- **Continuous Rule Self-Persistence**: Whenever the user specifies or refines any new rule, constraint, or engineering invariant in conversation, immediately update this `GEMINI.md` file so the rule is permanently codified and strictly enforced across all subsequent prompts.
