# Contributing to Logrix

Thank you for your interest in contributing to Logrix! Logrix is a high-performance, Kubernetes-native blockchain indexer written in Rust with TypeScript tooling.

## Code of Conduct

All contributors are expected to uphold a respectful, welcoming, and harassment-free environment for everyone.

## Development Setup

### Prerequisites

- **Rust**: Stable toolchain (edition 2021) with `cargo`, `rustfmt`, and `clippy`.
- **Node.js**: Node 18+ and `npm` (for `@logrix/sdk` and TypeScript starter).
- **Docker**: For local testing with PostgreSQL and RabbitMQ.

### Build and Test

```bash
# Clone the repository
git clone https://github.com/Vishal-770/Logrix.git
cd Logrix

# Run automated tests
cargo test --workspace

# Run linter
cargo clippy --workspace --all-targets -- -D warnings

# Check code formatting
cargo fmt --check
```

## Pull Request Guidelines

1. **Branching**: Always branch off `main` with a descriptive name (e.g. `feat/new-sink` or `fix/rpc-backoff`).
2. **Quality Gates**:
   - `cargo test --workspace` must pass with 100% success.
   - `cargo clippy --workspace --all-targets -- -D warnings` must produce zero warnings.
   - `cargo fmt --check` must be clean.
3. **Engineering Invariants**:
   - **Precision**: Never use floating-point numbers (`f32`/`f64`) for EVM balances, transaction values, or wei amounts. Always use `U256`.
   - **File Limits**: Rust source files (`*.rs`) must remain strictly under 200 lines (`<= 199` lines).
   - **Zero Emojis**: Keep code, comments, log messages, commit messages, and documentation free of unicode emojis.
   - **Error Handling**: Do not swallow errors or mask failures with empty vectors.
4. **Testing**: Write comprehensive unit and integration tests covering edge cases and error paths.
