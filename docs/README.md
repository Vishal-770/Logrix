# Documentation index

The maintained guides below describe the implementation in this checkout.
The Rust workspace is version `0.7.3`; the SDK package is version `0.7.0`.
These version declarations do not prove matching artifacts are published.

## Start here

| Guide | Contents |
| --- | --- |
| [Project README](../README.md) | Runtime quickstart, component map, operational limits |
| [Configuration](configuration.md) | Consumers, defaults, override rules, startup versus live changes, fixed behavior |
| [Architecture](01-architecture.md) | Actual processes, ports, pipeline and persistence boundaries |
| [RPC gateway](02-rpc-gateway.md) | Routing, caching, adaptive ranges, cost estimates and unimplemented bulk path |
| [Resilience and reorgs](03-resilience-reorgs.md) | Failure policies, rollback, reconciliation, backpressure and limitations |
| [Developer workflow](04-developer-experience.md) | Two CLIs, local services, AssemblyScript, queries and validation |
| [Scope and extension status](05-open-source-strategy.md) | Implemented extension points versus historical design goals |
| [Contribution guide](../CONTRIBUTING.md) | Reproducible tool versions, offline tests, lint and format checks |
| [Security](../SECURITY.md) | Disclosure and operator responsibilities |

## Crate guides

Every Cargo workspace member has a README describing its public controls and
fixed behavior. Environment/CLI mappings belong to the runtime wiring unless a
crate explicitly parses them itself.

| Crate | Responsibility |
| --- | --- |
| [logrix-core](../crates/logrix-core/README.md) | Domain types, errors, ports and leadership implementations |
| [logrix-resilience](../crates/logrix-resilience/README.md) | Breakers, rate limiters, retry policies and budgets |
| [logrix-testkit](../crates/logrix-testkit/README.md) | In-memory chain, queue and store mocks with documented realism limits |
| [logrix-store-postgres](../crates/logrix-store-postgres/README.md) | Migrations, raw events/checkpoints, entities, transfers and rollback |
| [logrix-queue-rabbitmq](../crates/logrix-queue-rabbitmq/README.md) | AMQP topology, publishing, pull consumption and acknowledgments |
| [logrix-queue-sqs](../crates/logrix-queue-sqs/README.md) | AWS/SQS-compatible queue transport and visibility semantics |
| [logrix-blob-s3](../crates/logrix-blob-s3/README.md) | Object bytes, optional compression and AWS client controls |
| [logrix-chain-evm](../crates/logrix-chain-evm/README.md) | HTTP JSON-RPC transport, log/envelope parsing and adaptive ranges |
| [logrix-rpc-gateway](../crates/logrix-rpc-gateway/README.md) | Provider pool, head cache and estimated CU tracking |
| [logrix-reconciler](../crates/logrix-reconciler/README.md) | Lag scheduling, ring-buffer continuity and rollback orchestration |
| [logrix-handlers](../crates/logrix-handlers/README.md) | Manifest formats, declarative expressions, WASM ABI and state staging |
| [logrix-webhook](../crates/logrix-webhook/README.md) | HTTP signatures, endpoint storage, dispatch, filters and audit limits |
| [logrix-api](../crates/logrix-api/README.md) | Core/dynamic GraphQL, subscriptions, probes and webhook REST routes |
| [logrix-cli](../crates/logrix-cli/README.md) | Full runtime command/flag/env/default reference and role wiring |

## SDK, examples, and deployment

- [SDK reference and compilation workarounds](../packages/logrix-sdk/README.md)
- [Standalone AssemblyScript starter](../templates/typescript-starter/README.md)
- [Uniswap example and known correctness limits](../arbitrum-uniswap-indexer/README.md)
- [Deployment overview and shared prerequisites](../deploy/README.md)
- [Helm values and rendered resources](../deploy/helm/logrix/README.md)
- [Kustomize base and local overlay](../deploy/k8s/README.md)
- [AWS Terraform variables and output handoff](../deploy/terraform/aws/README.md)
- [AWS operations](AWS_EKS_OPERATIONS_GUIDE.md)

## Historical material

[Plans](plans/README.md) and the [academic report](reports/README.md) preserve
earlier proposals and historical claims. Their feature lists, commands, numeric
results, and completion labels are not current capability or benchmark evidence.
Use the maintained guides and source for operational decisions.
