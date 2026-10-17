# Contributing to Logrix

Keep changes focused, preserve unrelated work, and describe the user-visible
behavior and failure boundaries. Be respectful and welcoming in all project
communication.

## Reproducible development setup

Use the workspace root for Rust commands and `packages/logrix-sdk` for SDK npm
commands. Runtime deployment uses containers; development/build preparation
happens on a workstation or CI machine, not production nodes.

- Rust 1.96.0 with Cargo, rustfmt and Clippy (`rust-toolchain.toml`).
- Node.js 24.10.0 (`.nvmrc`), npm, and the SDK's `package-lock.json`.
- Linux build essentials (C/C++ compiler, make), CMake and `pkg-config` for native
  dependencies; the Rust Debian Docker builder also supplies build tools.
- Docker/Compose for explicit service-backed checks; Helm, kubectl/Kustomize,
  and Terraform for deployment validation as described in [deploy](deploy/README.md).

These pinned development versions are not a statement of minimum supported
Rust/Node versions. Install them using your existing toolchain manager before
offline checks. For rustup, initial setup is:

```bash
rustup toolchain install 1.96.0 --profile minimal --component rustfmt --component clippy
```

With the tools available, prepare dependencies once (network may be required):

```bash
# Repository root
cargo fetch --locked
npm --prefix packages/logrix-sdk ci
```

No global npm dependencies are needed. Do not regenerate lockfiles or upgrade
dependencies as a side effect of documentation changes. An offline test cannot
bootstrap a missing runtime, Cargo cache, npm installation, or native compiler.

## Single offline test command

```bash
make test
```

The root Makefile runs:

```bash
cargo test --workspace --offline --locked
npm --prefix packages/logrix-sdk --offline test
```

Cargo tests include local mocked HTTP endpoints and in-memory/lazy-pool tests;
they do not require live RPC/AWS services, but are not wholly socket-free. SDK
tests compile the CLI and run converter/secret-template tests. The test command
does not provision services or certify real broker/database behavior. SDK
builds regenerate CLI output, including tracked `dist` files; inspect the diff
afterward and keep generated documentation templates synchronized with source.

If a moving `stable` alias already reports exactly Rust 1.96.0 while the named
toolchain is absent, `RUSTUP_TOOLCHAIN=stable make test` uses that installed alias.
Verify `RUSTUP_TOOLCHAIN=stable rustc --version` first; this is a local setup
workaround, not a replacement for the repository pin.

## Quality gates

From the repository root:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo test --workspace --offline --locked
```

CI also uses `cargo test --workspace --all-targets`, builds/packs the SDK, renders
Helm/Kustomize and validates Terraform. See [CI source](.github/workflows/ci.yml)
for its exact tool versions and network-enabled setup. Workflow setup currently
requests Node 20 and moving Rust stable; Cargo still honors `rust-toolchain.toml`
unless explicitly overridden, while setup-node's explicit version does not use
`.nvmrc`. README examples and external
service semantics are not automatically tested by Markdown rendering or `cargo test`.

For source-only deployment checks, use
[the deployment commands](deploy/README.md#offline-source-validation). Docker
builds and first-time dependency installs need network/cache preparation and
are separate from offline tests. Never run cloud apply/destroy as a test step.

## Documentation and code expectations

- Create a descriptive feature/fix branch for implementation changes.
- Keep Rust files under 200 lines; use ASCII text in new code/docs/log messages.
- Use exact integer types or decimal strings for EVM amounts. Do not convert
  balances or signed contract values through floating point.
- Return typed errors; do not replace failed external operations with empty
  results or advertise error classification as an automatic recovery policy.
- Add meaningful boundary/regression tests for behavior changes. Explicitly
  distinguish mock tests from real database/queue/infrastructure integration.
- Update root/crate READMEs and [configuration](docs/configuration.md) whenever
  defaults, APIs, effective settings or limitations change. Trace each setting
  to both parser and use site, not only to a struct field or comment.
- Keep generated-project documentation in
  `packages/logrix-sdk/src/cli/templates.ts` aligned with emitted files/scripts.
- Keep performance claims tied to reproducible evidence. Mark design proposals
  and reports as historical rather than operational instructions.
- Never commit real credentials, state files, populated Secret examples, local
  environment, logs, or generated `graphify-out/` artifacts.

Before requesting review, inspect the full diff and list the exact checks run,
their results and any skipped/blocked checks. Commit or push only when requested.
