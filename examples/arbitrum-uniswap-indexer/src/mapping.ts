import { EventLog, log, allocate, BigInt } from "@logrix/sdk/src/runtime/index";
import { Swap, PoolDayData, UserVolumeStats } from "./generated/schema";

// Required export so the Logrix Rust engine can allocate linear memory
export { allocate } from "@logrix/sdk/src/runtime/index";

// ---------------------------------------------------------------------------
// Entry point called by the Logrix engine for matched event logs.
// ---------------------------------------------------------------------------
export function handle_event(ptr: i32, len: i32): i32 {
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = load<u8>(ptr + i);
  }
  const rawJson = String.UTF8.decode(bytes.buffer);
  const logObj = EventLog.fromJson(rawJson);

  // Uniswap V3 Swap event signature:
  // Swap(address,address,int256,int256,uint160,uint128,int24)
  // topic0 = 0xc42079f94a6350d7e6235f29174924f9b50abce77680fdd4e4f0239f98b5e648
  if (logObj.topics.length > 0 && logObj.topics[0].startsWith("0xc42079f9")) {
    return handleSwap(logObj);
  }

  log.info("handle_event: unhandled topic0, skipping");
  return 0;
}

export function handleSwap(logObj: EventLog): i32 {
  const txHash: string = logObj.transaction_hash;
  const logIndex: u32 = logObj.log_index;
  const entityId: string = txHash + "-" + logIndex.toString();

  // 1. Create Individual Raw Swap Entity
  let swap = Swap.load(entityId);
  if (!swap) {
    swap = new Swap(entityId);
  }

  swap.blockNumber = BigInt.fromU64(logObj.block_number);
  swap.timestamp = BigInt.fromU64(logObj.block_timestamp);
  swap.transactionHash = txHash;
  swap.sender = logObj.topics.length > 1 ? logObj.topics[1] : "";
  swap.recipient = logObj.topics.length > 2 ? logObj.topics[2] : "";

  swap.amount0 = BigInt.zero();
  swap.amount1 = BigInt.zero();
  swap.sqrtPriceX96 = BigInt.zero();
  swap.liquidity = BigInt.zero();
  swap.tick = BigInt.zero();
  swap.save();

  // -------------------------------------------------------------------------
  // 2. Custom Aggregator: PoolDayData (Daily Aggregated Pool Metrics)
  // -------------------------------------------------------------------------
  const dayIndex: string = (logObj.block_timestamp / 86400).toString();
  let dayData = PoolDayData.load(dayIndex);
  if (!dayData) {
    dayData = new PoolDayData(dayIndex);
    dayData.date = BigInt.fromU64(logObj.block_timestamp);
    dayData.poolAddress = logObj.address;
    dayData.txCount = BigInt.zero();
    dayData.totalVolumeToken0 = BigInt.zero();
    dayData.totalVolumeToken1 = BigInt.zero();
    dayData.latestPriceSqrtX96 = BigInt.zero();
    dayData.latestTick = BigInt.zero();
  }

  dayData.txCount = dayData.txCount.plus(BigInt.fromI32(1));
  dayData.save();

  // -------------------------------------------------------------------------
  // 3. Custom Aggregator: UserVolumeStats (Cumulative User Activity)
  // -------------------------------------------------------------------------
  const user = swap.recipient.length > 0 ? swap.recipient : swap.sender;
  if (user.length > 0) {
    let userStats = UserVolumeStats.load(user);
    if (!userStats) {
      userStats = new UserVolumeStats(user);
      userStats.userAddress = user;
      userStats.totalSwaps = BigInt.zero();
      userStats.totalVolumeToken0 = BigInt.zero();
      userStats.totalVolumeToken1 = BigInt.zero();
    }

    userStats.totalSwaps = userStats.totalSwaps.plus(BigInt.fromI32(1));
    userStats.lastActiveBlock = BigInt.fromU64(logObj.block_number);
    userStats.lastActiveTimestamp = BigInt.fromU64(logObj.block_timestamp);
    userStats.save();
  }

  log.info("Indexed Swap and updated aggregators for tx: " + txHash);
  return 0;
}

