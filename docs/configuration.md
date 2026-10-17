# Configuration reference

This guide describes the current checkout, not the broader design plans.
Configuration has several entry points with different consumers and defaults.
The [Rust CLI reference](../crates/logrix-cli/README.md#complete-root-optionenvironment-reference)
lists every runtime flag, environment binding, type/unit, and CLI default.
Each [crate guide](README.md#crate-guides) documents its public Rust configuration
APIs and fixed behavior. The [Helm reference](../deploy/helm/logrix/README.md)
documents chart values, deployment resources, and settings with no template consumer.

## Choose the correct configuration surface

| Surface | Consumer and purpose | What it does not do |
| --- | --- | --- |
| Rust `logrix [OPTIONS] <COMMAND>` | Clap options wire the selected runtime role | Does not scaffold, codegen, compile handlers, or deploy |
| Runtime environment | CLI bindings and a few direct library/AWS settings | No automatic `.env` loading; arbitrary names have no effect |
| Native handler manifest | `Manifest::from_file`, selected ingester settings, handler engine, dedicated webhook seeding | Not a complete RPC/database/queue/API configuration file |
| SDK `logrix.yaml` loaded directly by Rust | Rust SDK-manifest adapter reads a limited subset | SDK network URL/start-block fields do not configure the process |
| SDK `logrix.yaml` through `export-values` | TypeScript converter produces Helm configuration and a native manifest | Does not deploy or prove that the selected runtime implements a feature |
| Helm values | Templates render environment, mounts, Services, Deployments, and scalers | Values with no template consumer do not become runtime settings |
| Public Rust structs/builders | Embedding application configures a library directly | CLI defaults do not automatically apply |
| Terraform variables | Infrastructure dimensions, identifiers, and reference AWS resources | Do not set application environment, run migrations, or install operators |

The two CLIs are different executables with the same command name. Use
`./target/debug/logrix` for the built Rust runtime and the SDK's local npm binary
for project commands. Check each command's `--help` in its own context.

## Precedence and lifecycle

There is no universal `flags > env > manifest > defaults` hierarchy.

1. **Clap:** explicit root flag > its environment binding > declared CLI default.
   Root flags precede the subcommand; `--port` belongs after `api`/`all-in-one`.
2. **Ingester tuning:** a loaded manifest's `chunkSize`, `maxQueueDepth`, and
   `chainHeadPollMs` override the resolved CLI values, even explicit flags.
3. **WASM load limits:** per-contract values > manifest `indexing` WASM values >
   engine global values (set by CLI) > instance environment/defaults.
4. **Initial block:** persisted checkpoint + 1 > CLI `START_BLOCK` > current
   head minus 50, saturating at zero. Changing `START_BLOCK` does not reset a
   checkpoint. Use an intentional backfill for additional history.
5. **Helm:** later `-f` files override earlier files, then explicit Helm overrides.
   Many template expressions use `default`, which substitutes for numeric zero,
   false, and empty values. See the chart guide before trying zero-valued tuning.
6. **Pod environment:** explicit `env` > Secret imported second by `envFrom` >
   ConfigMap imported first. A duplicate Secret key can hide an exported value.
7. **Library builders:** explicit setters usually replace constructor defaults;
   injected clients/pools have their own configuration. Consult the crate API.

CLI configuration, manifests, schemas, and loaded WASM are read at startup or
handler load. There is no file watcher/hot reload for these settings. Restart
the affected process after changes and prevent ingester overlap. The webhook
dispatcher reads registered database endpoints while processing jobs, so REST
registration changes do not require recompiling handlers.

Sources: [Clap definitions](../crates/logrix-cli/src/args.rs),
[extended arguments](../crates/logrix-cli/src/args_tuning.rs),
[ingester overrides](../crates/logrix-cli/src/ingester_types.rs),
[engine limits](../crates/logrix-handlers/src/engine.rs), and
[chart environment](../deploy/helm/logrix/templates/configmap.yaml).

## Runtime configuration by role

All role options are parsed by the same CLI; parsing does not imply a role uses
the option. The table summarizes the actual consumers.

| Command | Relevant runtime configuration |
| --- | --- |
| `migrate` | Database URL/pool, fixed `default` pipeline; no queue or RPC initialization |
| `status [--watch]` | Database URL; its connection path does not apply CLI pool tuning; status queries are not scoped by selected chain |
| `ingester` | Chain/RPC, queue, database checkpoint/pool, initial block, chunk/poll/backpressure, manifest ingestion tuning, optional elector |
| `processor` | Chain/RPC, queue, database/pool, target/manifest contracts, schema warmup, WASM, reconciler/ring, optional embedded webhook dispatcher, live S3 archival |
| `api [--port]` | Database/pool, entity schema, GraphQL settings, dynamic default chain; no handler-manifest loading |
| `backfill --from-block N --to-block M` | Queue, chain ID, inclusive range, chunk size; `--dry-run` still connects to queue before estimating |
| `webhook-dispatcher` / `webhook` | Queue, optional PostgreSQL endpoint store, manifest subscription seeding, webhook HTTP/worker settings |
| `all-in-one [--port]` | Migrations and embedded ingester/processor/API, with local leadership regardless of election flag |

For split-role deployments, migrate before starting the application. Each
constructed pool has its own connection cap; `DATABASE_MAX_CONNECTIONS=10` is
not a total process/pod/cluster limit. Status and dedicated webhook paths have
different pool wiring. Reconciler instances and handler state are per processor,
not coordinated across replicas.

## Effective defaults and library differences

These selected defaults are easy to confuse. Use the complete
[CLI tables](../crates/logrix-cli/README.md) for the rest.

| Control | Runtime CLI default | Direct library/default boundary |
| --- | --- | --- |
| Chain and RPC | `421614`, `https://sepolia-rollup.arbitrum.io/rpc` | Chain/client identifiers are constructor arguments; remote chain IDs are not automatically verified |
| RPC timeout | 10 seconds | `EvmClientConfig`: 15 seconds |
| Provider breaker | 5 failures, 2 successes, 30 seconds recovery | `GatewayConfig`: 3 failures, 2 successes, 10 seconds |
| Head cache | No CLI control | `GatewayConfig.block_cache_ttl`: 500 ms |
| Ingestion chunk cap | 10 blocks, minimum 1 | EVM client initial/min/max recommendation: 500/5/2000; not the job-size cap |
| Head polling / queue depth | 1500 ms / 1000 jobs | Ingestion throttle, not a hard queue/memory bound |
| Reconciler interval / lag | 30 seconds / 50 blocks | Interval required by constructor; library lag default 10 blocks |
| Reconciler chunk | Unset | One complete lagging interval, not inherited from `CHUNK_SIZE` |
| Ring buffer | 128 references | Constructor-supplied capacity; default ancestor walk `max(2 * capacity, 128)` |
| WASM fuel / memory | 1,000,000 / 128 MiB | Direct instance fallback: 25,000,000 / 64 MiB; memory values are checks, not enforced growth ceilings |
| API port | 4000 | Library requires a socket address; no port default |
| Dynamic API chain | CLI injects 421614 | `ApiConfig.chain_id=None`; dynamic queries then use chain 1; core queries require explicit chain |
| API depth/complexity/page/timeout | 7 / 200 / 100 default, 1000 max / 5 seconds | Depth/complexity/pagination controls apply to dynamic schema; HTTP timeout also applies to core queries |
| Webhook timeout / retry setting | 5 seconds / 1 total attempt for fallback URL | Database endpoint count means additional retries; direct endpoint default 0, SDK exporter default 3 |
| RabbitMQ prefetch / setup timeout | 100 / 10 seconds | QoS plus pull-based `basic_get`; timeout is connection/topology setup, not every operation |
| SQS poll / receive batch | 2 seconds / 1 message | Receive count clamps to 1..10; wait is not locally range-validated |
| Database pool | Max 10 / min 2 | `PostgresConfig` reads these two env names; URL and pipeline ID are explicit arguments |

Numeric types do not imply semantic validation. For example, zero reconciler
interval panics in Tokio, an unknown queue-driver string selects RabbitMQ, and
AWS constraints can reject values passed unchanged by the CLI. The per-crate
guides document validation, clamping, and error classes.

## Manifest keys: direct loading versus export

Native manifests use `schema_version`, `chain_id`, and `contracts`; SDK project
manifests use `specVersion`, `network`, and `dataSources`. See the complete
[native/SDK examples](../crates/logrix-handlers/README.md) and
[SDK conversion reference](../packages/logrix-sdk/README.md#configuration-sdk-to-helm-versus-direct-runtime).
Unknown Rust manifest fields are generally ignored, not rejected.

| Setting or field | Direct runtime manifest | SDK `export-values` path |
| --- | --- | --- |
| Network chain ID | Parsed metadata; CLI still selects runtime chain | Exported as `config.chainId` -> `CHAIN_ID` |
| RPC/fallback URLs | Not used as runtime RPC configuration | Exported to Helm -> `RPC_URL` / `RPC_FALLBACK_URLS` |
| Per-contract start block | Ignored by runtime contract type | Minimum across SDK sources becomes global `START_BLOCK`; no per-contract runtime gating |
| Address / enabled | Address selects logs; enabled skips logic, not RPC address selection | Converted into native contracts; first address also sets CLI target |
| ABI / handler name / event signature | ABI and named function not used for guest dispatch; contract event list is metadata | Used by SDK codegen; host still calls `handle_event`, whose code must route/filter |
| WASM file | Loaded by processor, relative to process working directory | Exporter always bundles `build/mapping.wasm` and points every contract at `/etc/logrix/handlers/mapping.wasm` |
| `chunkSize`, `chainHeadPollMs`, `maxQueueDepth` | Applied by ingester, overriding CLI values | Mapped to Helm config/env, not copied into exported manifest |
| `wasmFuelLimit`, `wasmMaxMemoryMb` | Applied by engine as global manifest defaults | Mapped to Helm config/env |
| Ring/reconciler indexing settings | Parsed but not applied by current processor | Mapped to Helm config/env; interval/ring export defaults 30/128 |
| RPC, DB, queue, API, webhook timeout tuning | Not recognized by native `ManifestIndexing` | Only explicitly mapped keys are forwarded to Helm config/env |
| `webhooks` | Seeded only by dedicated dispatcher with a connected store; active URLs skipped, not updated | Converted to native entries; first URL sets fallback and enables webhook chart component |
| GraphQL schema | Separate `SCHEMA_PATH`, not inferred from manifest | `schema.graphql` embedded as Helm schema content |
| Images, replicas, DB/queue URLs, S3, credentials | Explicit runtime/env/deployment configuration | Separate Helm values/Secrets; not inferred from arbitrary indexing keys |

Exporter tuning allowlist (all become same-named Helm `config` keys):

- Ingestion/reconciliation: `chunkSize`, `chainHeadPollMs`, `maxQueueDepth`,
  `reconcilerChunkSize`, `reconcilerLagThreshold`; ring/interval are handled separately.
- RPC: `rpcTimeoutSecs`, `rpcCbThreshold`, `rpcCooldownSecs`, `cuBudget`, `bulkStreamUrl`.
- Database/queue: `databaseMaxConnections`, `databaseMinConnections`,
  `rabbitmqPrefetch`, `rabbitmqTimeoutSecs`, `sqsWaitTimeSeconds`, `sqsMaxMessages`.
- WASM: `wasmFuelLimit`, `wasmMaxMemoryMb`.
- API: `graphqlMaxDepth`, `graphqlMaxComplexity`, `graphqlDefaultLimit`,
  `graphqlMaxLimit`, `graphqlQueryTimeoutSecs`.
- Webhook: `webhookTimeoutSecs`, `webhookMaxRetries`.

Source of truth: [converter](../packages/logrix-sdk/src/cli/manifest_converter.ts),
[export command](../packages/logrix-sdk/src/cli/commands/export_values.ts),
[Rust manifest](../crates/logrix-handlers/src/manifest.rs), and
[SDK manifest adapter](../crates/logrix-handlers/src/sdk_manifest.rs).

The current Helm ingester receives `MANIFEST_PATH` for inline manifest content
but has no matching mount. Since it reads that file at startup, exported bundles
need a consumer-managed ingester mount patch or equivalent readable path; export
success alone does not make the bundle deployable. See the
[chart file wiring](../deploy/helm/logrix/README.md#manifest-schema-and-handler-files).

## Secrets, placeholders, and AWS discovery

- Use symbolic `${NAME}` references rather than provider-key-bearing literal
  URLs. The exporter preserves recognized references; it does not read `.env`.
  Literal RPC credentials can still be copied into exported values.
- Primary/fallback URLs expand references in the runtime gateway factory.
  Manifest parsing expands YAML string values. Processor configuration expands
  its fallback webhook URL/secret; the dedicated runner uses those CLI strings
  directly and handles manifest seeding separately.
- Missing/invalid references fail on paths that propagate configuration errors.
  The dedicated runner ignores manifest parse/seed errors, so successful startup
  there alone does not prove your manifest was accepted.
- Secret examples contain `CHANGE_ME`, not usable credentials. Applying one
  untouched overrides working ConfigMap URLs. Review exact keys and namespace;
  base64 storage of Secret data does not encrypt it.
- SQS/S3 constructors load the AWS SDK configuration chain. Supply appropriate
  region, identity, endpoint, and permissions through your platform. Wrapper
  defaults do not universally select `us-east-1`. KEDA's `awsRegion` configures
  the scaler, not the application's SDK region. IAM access to Secrets Manager
  does not automatically populate environment variables.
- No comprehensive URL-secret redaction is guaranteed by helper functions.
  Avoid recording raw credentials in logs, terminal output, commands, and reports.

See [deployment secret precedence](../deploy/README.md#secrets-and-configuration-precedence)
and [SDK placeholder handling](../packages/logrix-sdk/README.md#secret-placeholders-and-startup).

## Fixed behavior and controls without the advertised effect

These boundaries cannot be changed merely by inventing a YAML key or env name:

| Area | Current behavior | Details |
| --- | --- | --- |
| RPC bulk streaming | Endpoint can be stored but `stream_logs` returns `None`; RPC is used | [Gateway](../crates/logrix-rpc-gateway/README.md) |
| RPC routing/cost | Priority/round-robin, fixed tip penalty; process-local heuristic CU accounting | [Gateway](../crates/logrix-rpc-gateway/README.md) |
| Retry tuning | No `RPC_MAX_RETRIES` binding; supervised EVM retry policy is fixed | [EVM](../crates/logrix-chain-evm/README.md) |
| Reconciliation | Checkpoint-to-head lag only, not arbitrary missing history below checkpoint | [Reconciler](../crates/logrix-reconciler/README.md) |
| Persistence | Checkpoint namespace is not full pipeline data isolation; rollback cannot restore entity history/engine state | [PostgreSQL](../crates/logrix-store-postgres/README.md) |
| RabbitMQ | Fixed routing suffixes; prefixes alone do not isolate shared-exchange traffic; no automatic retry ceiling | [RabbitMQ](../crates/logrix-queue-rabbitmq/README.md) |
| SQS | Buffered message visibility 43,200 s; `nack(false)` means 300 s; no processing visibility heartbeat | [SQS](../crates/logrix-queue-sqs/README.md) |
| S3 | Generic bytes adapter; runtime archive is best effort on live envelopes, fixed compression level 3 | [S3](../crates/logrix-blob-s3/README.md) |
| Webhooks | Empty event list matches all; `*` is literal in dispatcher; `reorg` registration differs from `event.reverted` producer | [Webhooks](../crates/logrix-webhook/README.md) |
| API | No authentication/mutation root; static/dynamic guardrails differ; subscriptions have no durable replay | [API](../crates/logrix-api/README.md) |
| Leadership | Current elector is not safe active/passive acquisition or renewal | [Core](../crates/logrix-core/README.md) |
| WASM | Fuel is metered; memory checks do not limit allocation growth; state commit is in-memory | [Handlers](../crates/logrix-handlers/README.md) |
| SDK flags | `test --watch` has no effect; `--deploy-profiles` does not create a local profile | [SDK](../packages/logrix-sdk/README.md#cli-reference) |

The interfaces provide extension points, not installed SQLite/Redis/gRPC
adapters or arbitrary-language handler toolchains. Adding these requires
implementation work. Historical plans are not a capability reference.

## Troubleshooting configuration

1. Verify the executable, subcommand help, working directory, and exact spelling.
2. Check the value at the actual consumer: direct manifest versus exported Helm,
   resolved pod env, and constructor/global/per-contract overrides.
3. Confirm manifests/schema/WASM are mounted and readable. A missing API schema
   path silently selects core queries instead of the expected dynamic entities.
4. Check checkpoint state before expecting a new start block. Do not delete
   shared data to work around precedence; plan an explicit bounded backfill.
5. Confirm migrated database and provisioned queues/buckets. Constructors do
   not automatically create all external resources or verify connectivity.
6. Review role-specific logs without exposing credentials. `status` and probes
   do not certify complete coverage, correct chain identity, or healthy failover.

For validation commands and their coverage limits, see
[contribution checks](../CONTRIBUTING.md) and [deployment checks](../deploy/README.md#offline-source-validation).
