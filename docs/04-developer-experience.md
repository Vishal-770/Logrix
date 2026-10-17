# Developer workflow

## Two tools named `logrix`

| Tool | How to identify it | Commands |
| --- | --- | --- |
| Rust runtime | `./target/debug/logrix --help` after a Cargo build | `migrate`, `status`, `ingester`, `processor`, `backfill`, `api`, `webhook-dispatcher`, `all-in-one` |
| SDK project CLI | Local npm `logrix` binary from `@logrix/sdk` | `init`, `add`, `codegen`, `validate`, `status`, `build`, `export-values`, `test` |

The runtime does not provide `init` or `dev`; the SDK does not run or deploy the
runtime. The repository does not supply a Homebrew tap, `get.logrix.dev` shell
installer, embedded SQLite dev mode, auto-created kind cluster or configuration
hot reload. Use the actual [runtime quickstart](../README.md#local-runtime-quickstart)
and [SDK reference](../packages/logrix-sdk/README.md).

## Run the native ERC-20 path first

From the repository root, build the Rust CLI, start Compose PostgreSQL/RabbitMQ,
then run `all-in-one`. This path needs neither WASM nor custom schema. Compose
only starts dependencies. Without a schema the API exposes core `health`,
`checkpoint(chainId:)`, and `transfers(chainId:, filter:)` fields on port 4000.
GraphiQL is at `/`; queries use `POST /graphql`.

For split roles, run migrations first and launch each role with consistent
chain/RPC/database/queue settings. Root flags precede the command; `api --port`
and `all-in-one --port` are command-local flags. Set credentials using your
process launcher; neither CLI automatically loads `.env`.

## Custom declarative handlers

Native manifests define `contracts[].declarative` rules over raw log fields.
Use a topic0 hash rather than assuming arbitrary ABI names/signatures are
resolved. The [handlers README](../crates/logrix-handlers/README.md#native-runtime-manifest)
shows the supported shape and expression syntax.

An API entity schema is separate from a handler manifest. Supply `SCHEMA_PATH`
explicitly. A missing schema path silently falls back to the core API. Entity
schemas describe JSON-backed entity fields, not automatically generated SQL
tables/indexes or complete relationship resolvers.

## AssemblyScript projects

The SDK generates `logrix.yaml`, `schema.graphql`, ABI JSON, `src/mapping.ts`,
package scripts and an AWS/Secret example. Codegen adds `src/generated/events.ts`
and `src/generated/schema.ts`; compilation produces `build/mapping.wasm`.

Handlers are AssemblyScript, not asynchronous Node.js functions. The host calls
`handle_event(ptr, len)` with raw log JSON; guest code parses it, routes by address
and topic0, and emits entities. It does not receive an `EventHandlerContext`
from an `@logrix/indexer` package or arbitrary database/network APIs.

Read the SDK's [quickstart and workaround](../packages/logrix-sdk/README.md#quick-start)
before compiling: current package-root imports expose incompatible testing syntax,
and the build command can choose its existing WASM output as input on repeat
builds. Generated projects also do not include an `npm test` script. These
limitations are documented rather than disguised as a working one-command flow.

Use deterministic entity IDs, verify unsigned/signed numeric decoding, and test
duplicate delivery and replay. A successful compile does not establish correct
Uniswap arithmetic, durable general handler state, or atomic aggregate updates.
See the [example limitations](../arbitrum-uniswap-indexer/README.md).

## Export, then deploy separately

`export-values` requires existing `logrix.yaml`, `schema.graphql`, and the fixed
`build/mapping.wasm`. It writes Helm values and an embedded runtime manifest;
it does not build, validate or deploy automatically. Review symbolic secret
references, generated image versions, and external-service settings.

Operational SDK tuning can reach the runtime through export -> Helm -> env.
Direct Rust manifest loading consumes a smaller subset. See
[configuration](configuration.md) and the [Helm guide](../deploy/helm/logrix/README.md).
Production nodes run reviewed containers, not checked-out source builds.

## Verify the right behavior

- `make test` runs offline Rust workspace and SDK converter tests after dependency
  setup. [Contribution instructions](../CONTRIBUTING.md) list lint/format gates.
- SDK `validate` checks selected file/schema shapes; it does not compile or run
  the guest. SDK `test` performs limited local checks and optional Node tests,
  not a WASM host simulation; `--watch` currently has no effect.
- Render Helm/Kustomize and validate Terraform separately. These checks do not
  prove a successful cloud deployment, registry access or runtime migrations.
- Real database/queue/RPC/guest workflows need explicit integration checks.
  Mock unit tests do not certify rollback, shutdown, isolation or throughput.
