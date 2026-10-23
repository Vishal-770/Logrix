import { BigInt, log, EventLog, allocate } from "@logrix/sdk";
import {
  SupplyEvent,
  WithdrawEvent,
  BorrowEvent,
  RepayEvent,
  LiquidationCallEvent,
  FlashLoanEvent,
} from "./generated/events";
import {
  Protocol,
  User,
  Reserve,
  SupplyAction,
  BorrowAction,
  LiquidationAction,
  FlashLoanAction,
} from "./generated/schema";

// Required export so the Logrix Rust engine can allocate linear memory
export { allocate } from "@logrix/sdk";

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

  if (logObj.topics.length == 0) {
    return 0;
  }

  const topic0 = logObj.topics[0];

  // Supply: 0x2b627736bca15cd5381dcf80b0bf11fd197d01a037c52b927a881a10fb73ba61
  if (topic0.startsWith("0x2b627736")) {
    handleSupply(new SupplyEvent(logObj));
    return 1;
  }
  // Withdraw: 0x3115d1449a7b732c986cba18244e897a450f61e1bb8d589cd2e69e6c892a1b0f
  if (topic0.startsWith("0x3115d144")) {
    handleWithdraw(new WithdrawEvent(logObj));
    return 1;
  }
  // Borrow: 0xb3d081b630faf2286ba829b5050d29bc50463228134ec378ea827860957174fb
  if (topic0.startsWith("0xb3d081b6")) {
    handleBorrow(new BorrowEvent(logObj));
    return 1;
  }
  // Repay: 0xa534c8dbe71f871f9f3530e97a74601fea17b426cae02e1c5aee42c96c784051
  if (topic0.startsWith("0xa534c8db")) {
    handleRepay(new RepayEvent(logObj));
    return 1;
  }
  // LiquidationCall: 0xe413a321e86e37203b87b2046e7f227181c039b23b379965a388bdf20b085144
  if (topic0.startsWith("0xe413a321")) {
    handleLiquidationCall(new LiquidationCallEvent(logObj));
    return 1;
  }
  // FlashLoan: 0xefefdef48797d283b674b8f596d6d7a7ec369533f83441586e2d3848017085a4
  if (topic0.startsWith("0xefefdef4")) {
    handleFlashLoan(new FlashLoanEvent(logObj));
    return 1;
  }

  return 0;
}

const PROTOCOL_ID = "aave-v3-ethereum";

function getOrCreateProtocol(): Protocol {
  let protocol = Protocol.load(PROTOCOL_ID);
  if (protocol == null) {
    protocol = new Protocol(PROTOCOL_ID);
    protocol.totalSuppliedCount = BigInt.zero();
    protocol.totalBorrowedCount = BigInt.zero();
    protocol.totalLiquidatedCount = BigInt.zero();
    protocol.totalFlashLoansCount = BigInt.zero();
    protocol.save();
  }
  return protocol;
}

function getOrCreateUser(userAddress: string): User {
  let user = User.load(userAddress);
  if (user == null) {
    user = new User(userAddress);
    user.address = userAddress;
    user.supplyCount = BigInt.zero();
    user.borrowCount = BigInt.zero();
    user.liquidationCount = BigInt.zero();
    user.save();
  }
  return user;
}

function getOrCreateReserve(assetAddress: string): Reserve {
  let reserve = Reserve.load(assetAddress);
  if (reserve == null) {
    reserve = new Reserve(assetAddress);
    reserve.assetAddress = assetAddress;
    reserve.totalSupplied = BigInt.zero();
    reserve.totalBorrowed = BigInt.zero();
    reserve.totalLiquidated = BigInt.zero();
    reserve.save();
  }
  return reserve;
}

export function handleSupply(event: SupplyEvent): void {
  const reserveAddr = event.params.reserve.toHexString();
  const userAddr = event.params.user.toHexString();
  const onBehalfOfAddr = event.params.onBehalfOf.toHexString();
  const amount = event.params.amount;

  log.info(
    "Aave V3 Supply: user=" +
      userAddr +
      " reserve=" +
      reserveAddr +
      " amount=" +
      amount.toString()
  );

  // Update Protocol metrics
  const protocol = getOrCreateProtocol();
  protocol.totalSuppliedCount = protocol.totalSuppliedCount.plus(BigInt.fromI32(1));
  protocol.save();

  // Update User metrics
  const user = getOrCreateUser(onBehalfOfAddr);
  user.supplyCount = user.supplyCount.plus(BigInt.fromI32(1));
  user.save();

  // Update Reserve metrics
  const reserve = getOrCreateReserve(reserveAddr);
  reserve.totalSupplied = reserve.totalSupplied.plus(amount);
  reserve.save();

  // Record SupplyAction
  const actionId =
    event.transactionHash + "-" + event.logIndex.toString();
  const action = new SupplyAction(actionId);
  action.reserve = reserveAddr;
  action.user = userAddr;
  action.onBehalfOf = onBehalfOfAddr;
  action.amount = amount;
  action.referralCode = I32.parseInt(event.params.referralCode.toString());
  action.blockNumber = event.blockNumber;
  action.blockTimestamp = event.blockTimestamp;
  action.transactionHash = event.transactionHash;
  action.save();
}

export function handleWithdraw(event: WithdrawEvent): void {
  const reserveAddr = event.params.reserve.toHexString();
  const userAddr = event.params.user.toHexString();
  const amount = event.params.amount;

  log.info(
    "Aave V3 Withdraw: user=" +
      userAddr +
      " reserve=" +
      reserveAddr +
      " amount=" +
      amount.toString()
  );

  const reserve = getOrCreateReserve(reserveAddr);
  if (reserve.totalSupplied.gte(amount)) {
    reserve.totalSupplied = reserve.totalSupplied.minus(amount);
  } else {
    reserve.totalSupplied = BigInt.zero();
  }
  reserve.save();
}

export function handleBorrow(event: BorrowEvent): void {
  const reserveAddr = event.params.reserve.toHexString();
  const userAddr = event.params.user.toHexString();
  const onBehalfOfAddr = event.params.onBehalfOf.toHexString();
  const amount = event.params.amount;

  log.info(
    "Aave V3 Borrow: user=" +
      onBehalfOfAddr +
      " reserve=" +
      reserveAddr +
      " amount=" +
      amount.toString()
  );

  const protocol = getOrCreateProtocol();
  protocol.totalBorrowedCount = protocol.totalBorrowedCount.plus(BigInt.fromI32(1));
  protocol.save();

  const user = getOrCreateUser(onBehalfOfAddr);
  user.borrowCount = user.borrowCount.plus(BigInt.fromI32(1));
  user.save();

  const reserve = getOrCreateReserve(reserveAddr);
  reserve.totalBorrowed = reserve.totalBorrowed.plus(amount);
  reserve.save();

  const actionId =
    event.transactionHash + "-" + event.logIndex.toString();
  const action = new BorrowAction(actionId);
  action.reserve = reserveAddr;
  action.user = userAddr;
  action.onBehalfOf = onBehalfOfAddr;
  action.amount = amount;
  action.interestRateMode = 2; // Variable
  action.borrowRate = event.params.borrowRate;
  action.blockNumber = event.blockNumber;
  action.blockTimestamp = event.blockTimestamp;
  action.transactionHash = event.transactionHash;
  action.save();
}

export function handleRepay(event: RepayEvent): void {
  const reserveAddr = event.params.reserve.toHexString();
  const userAddr = event.params.user.toHexString();
  const amount = event.params.amount;

  log.info(
    "Aave V3 Repay: user=" +
      userAddr +
      " reserve=" +
      reserveAddr +
      " amount=" +
      amount.toString()
  );

  const reserve = getOrCreateReserve(reserveAddr);
  if (reserve.totalBorrowed.gte(amount)) {
    reserve.totalBorrowed = reserve.totalBorrowed.minus(amount);
  } else {
    reserve.totalBorrowed = BigInt.zero();
  }
  reserve.save();
}

export function handleLiquidationCall(event: LiquidationCallEvent): void {
  const collateralAsset = event.params.collateralAsset.toHexString();
  const debtAsset = event.params.debtAsset.toHexString();
  const userAddr = event.params.user.toHexString();
  const liquidatorAddr = event.params.liquidator.toHexString();
  const debtToCover = event.params.debtToCover;
  const liquidatedCollateralAmount = event.params.liquidatedCollateralAmount;

  log.info(
    "Aave V3 Liquidation: user=" +
      userAddr +
      " liquidator=" +
      liquidatorAddr +
      " debtCovered=" +
      debtToCover.toString()
  );

  const protocol = getOrCreateProtocol();
  protocol.totalLiquidatedCount = protocol.totalLiquidatedCount.plus(BigInt.fromI32(1));
  protocol.save();

  const user = getOrCreateUser(userAddr);
  user.liquidationCount = user.liquidationCount.plus(BigInt.fromI32(1));
  user.save();

  const collateralReserve = getOrCreateReserve(collateralAsset);
  collateralReserve.totalLiquidated = collateralReserve.totalLiquidated.plus(liquidatedCollateralAmount);
  collateralReserve.save();

  const actionId =
    event.transactionHash + "-" + event.logIndex.toString();
  const action = new LiquidationAction(actionId);
  action.collateralAsset = collateralAsset;
  action.debtAsset = debtAsset;
  action.user = userAddr;
  action.debtToCover = debtToCover;
  action.liquidatedCollateralAmount = liquidatedCollateralAmount;
  action.liquidator = liquidatorAddr;
  action.receiveAToken = event.params.receiveAToken;
  action.blockNumber = event.blockNumber;
  action.blockTimestamp = event.blockTimestamp;
  action.transactionHash = event.transactionHash;
  action.save();
}

export function handleFlashLoan(event: FlashLoanEvent): void {
  const target = event.params.target.toHexString();
  const initiator = event.params.initiator.toHexString();
  const asset = event.params.asset.toHexString();
  const amount = event.params.amount;
  const premium = event.params.premium;

  log.info(
    "Aave V3 FlashLoan: initiator=" +
      initiator +
      " asset=" +
      asset +
      " amount=" +
      amount.toString()
  );

  const protocol = getOrCreateProtocol();
  protocol.totalFlashLoansCount = protocol.totalFlashLoansCount.plus(BigInt.fromI32(1));
  protocol.save();

  const actionId =
    event.transactionHash + "-" + event.logIndex.toString();
  const action = new FlashLoanAction(actionId);
  action.target = target;
  action.initiator = initiator;
  action.asset = asset;
  action.amount = amount;
  action.premium = premium;
  action.blockNumber = event.blockNumber;
  action.blockTimestamp = event.blockTimestamp;
  action.transactionHash = event.transactionHash;
  action.save();
}
