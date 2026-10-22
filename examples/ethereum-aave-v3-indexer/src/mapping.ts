import { BigInt, log } from "@logrix/sdk";
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
