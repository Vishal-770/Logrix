# RPC gateway, range sizing, and cost estimates

The runtime factory creates `RpcGateway` over managed `EvmChainClient` providers.
The [gateway README](../crates/logrix-rpc-gateway/README.md) documents every public
setting and the [CLI](../crates/logrix-cli/README.md) documents environment mapping.

## Provider selection

- The CLI assigns primary priority 1 and fallbacks priorities 2, 3, and so on.
  Lowest priority wins among healthy candidates; equal tiers round-robin.
- Known tips more than three blocks behind the highest observed tip receive a
  fixed +10 priority penalty. EWMA latency is measured, not used as routing weight.
- Failover tries untried provider names. Selected range/limit error strings
  return immediately for range handling instead of normal breaker accounting.
- When all providers appear unhealthy, best-effort selection can still call an
  open breaker. Cooldown jitter is not an enforced sleep or availability SLA.
- Default CLI HTTP timeout is 10 seconds. Supervised providers use a fixed
  single-call retry setting of one retry, independent of direct client defaults.
  JSON-RPC batch calls have a separate path.

Provider identity is labeled with the configured chain ID; runtime construction
does not verify remote chain identity. Select compatible providers and history.
Failures are typed, but not every caller applies the same error-class policy.

## Caching and ranges

Only latest head height has the default 500 ms cache and SingleFlight sharing.
There is no general log/header/receipt LRU cache or durable learned provider
range-limit database. Each process has its own provider state and accounting.

The EVM client's adaptive chunker recommends a range using success/error/latency
feedback. The ingester caps that recommendation with `CHUNK_SIZE` (default 10).
The processor can split failed backfill jobs for selected range-error text.
Calling `fetch_logs` directly does not automatically partition an arbitrary range.
This is adaptive range sizing, not database-latency-driven worker-thread scaling.

## Bulk streaming status

`BulkStreamClient` stores known/default or custom endpoint strings, but
`stream_logs` returns `Ok(None)` without archive transport. Runtime backfills
therefore use HTTP JSON-RPC. A configured `BULK_STREAM_URL` does not enable SQD,
HyperSync, zero-CU backfill, or a documented throughput multiplier. Live height
tracking is polling, not a WebSocket `newHeads` subscription with polling fallback.

## CU budget scope

Successful gateway operations add fixed estimates: block height 10 CU, block
header/envelope 20 CU, and logs 75 CU. Cache hits, failed operations and internal
retries are not fully billed by this counter; an envelope can involve multiple
RPC requests. The optional limit is checked before execution without reserving
the next operation's cost. Concurrent/final operations can overshoot it.

Counters reset with the gateway and are not shared across pods. Exhaustion
blocks uncached live calls too; it is not just a backfill throttle. These are
heuristics, not provider-native billing or a strict account spending cap.

## Backfill preview

With backing queue configuration available, from the repository root:

```bash
./target/debug/logrix --chain-id 421614 --chunk-size 10 backfill \
  --from-block 1000000 --to-block 1000099 --dry-run
```

The inclusive range contains 100 blocks/10 chunks. The estimate uses 95 CU per
chunk, $1 per million CU, and two seconds per chunk. It does not probe log density,
verify providers, predict autoscaling, or ask for an interactive confirmation.
The CLI initializes the selected queue before this dry-run branch, so it is not
an external-service-free operation. Omitting `--dry-run` publishes jobs; processors
must be running separately to execute them.

See [configuration](configuration.md) for placeholder expansion, library versus
CLI defaults, and fixed settings; see [gateway tests](../crates/logrix-rpc-gateway/tests/)
for mocked-network coverage rather than provider/throughput certification.
