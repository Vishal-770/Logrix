# Project scope and extension status

This guide replaces earlier product promises with a current capability map.
Historical proposals remain under [plans](plans/README.md); their goals are
not a release contract or implementation checklist marked complete by these docs.

## Available extension points

| Need | Current mechanism | Boundary |
| --- | --- | --- |
| Add an infrastructure adapter | Implement a core Rust port and wire it into an embedding app/CLI | Existing CLI selectors expose RabbitMQ/SQS and PostgreSQL, not arbitrary plugins |
| Map raw log fields to entities | Native declarative manifest rules | Limited expressions/matching, not full ABI expression evaluation |
| Execute custom logic | Wasmtime guest matching host ABI; supplied AssemblyScript SDK | Other languages need independently verified compatible guests/toolchains |
| Define queried entity fields | GraphQL SDL or supported YAML schema | JSON entity storage; relation/list/index metadata is not full resolver/DDL support |
| Consume live entity changes | Optional GraphQL WebSocket subscriptions | Non-durable notifications, no guaranteed replay |
| Deliver HTTP notifications | Signed webhook dispatcher and endpoint REST API | Current producer/filter vocabulary and retry/audit limitations apply |
| Configure deployment resources | Helm/Kustomize plus reference AWS Terraform | Operators, IAM, migrations, durable backing services and upgrades remain operator work |

## Not shipped as working capabilities

- Embedded SQLite, Redis/BullMQ, local filesystem archive adapters, or generic
  production in-memory queue mode.
- Remote gRPC plugins, JavaScript-in-WASM execution, dynamic Rust library plugins,
  factory discovery or automatic ABI/proxy-upgrade registration.
- SQD/HyperSync bulk transport, archive-first zero-RPC replay, durable learned
  provider limits, or a latency-weighted global request scheduler.
- A singleton Controller that coordinates database-aware worker budgets,
  cross-pod handler state, arbitrary gap audits, or atomic cross-system rollback.
- A `logrix dev` command, automatic cluster provisioning, hot reload, or a verified
  under-60-second installation workflow.
- An out-of-the-box security/HA guarantee, strict provider billing cap, universal
  memory bound, exactly-once processing, or guaranteed shutdown drain.

The Rust ports and data fields leave room for implementation changes, but a
type, endpoint string, configuration key or aspirational test name alone does
not establish working behavior.

## Evidence for changes

Document a capability only after tracing its public input to the actual consumer
and checking the relevant failure paths. Separate direct library defaults from
runtime wiring and deployment translation. Quantitative results need a workload,
environment, command, raw measurements and reproducible comparison; old reports
do not establish current performance.

Useful starting points: [architecture](01-architecture.md),
[configuration](configuration.md), [all crate guides](README.md#crate-guides),
and [contribution checks](../CONTRIBUTING.md).
