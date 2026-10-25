# Aave V3 Indexer GraphQL Queries Reference

This guide provides ready-to-run GraphQL queries for querying the indexed Aave V3 Lending Pool data from the Logrix API service (`http://localhost:4000/graphql` or interactive Studio at `http://localhost:4000/`).

---

## 1. Protocol Level Statistics

### Query Overall Protocol Totals
Query total supplies, borrows, liquidations, and flash loans across the entire protocol:
```graphql
query GetProtocolOverview {
  protocol(id: "aave-v3-ethereum") {
    id
    totalSuppliedCount
    totalBorrowedCount
    totalLiquidatedCount
    totalFlashLoansCount
  }
}
```

---

## 2. Supply Actions (Deposits)

### Query Most Recent Supply Events
Fetch the 10 most recent deposit/supply actions on Ethereum Mainnet:
```graphql
query GetRecentSupplies {
  supplyactions(first: 10, orderBy: "blockNumber", orderDirection: "desc") {
    id
    reserve
    user
    onBehalfOf
    amount
    referralCode
    blockNumber
    blockTimestamp
    transactionHash
  }
}
```

### Query Supplies for a Specific User Address
Find all deposits made by a specific wallet address:
```graphql
query GetUserSupplies {
  supplyactions(
    first: 10
    where: { user: "0x7a250d5630b4cf539739df2c5dacb4c659f2488d" }
    orderBy: "blockNumber"
    orderDirection: "desc"
  ) {
    id
    reserve
    amount
    blockNumber
    transactionHash
  }
}
```

---

## 3. Borrow Actions (Debt Generation)

### Query Recent Borrows with Interest Rates
Retrieve variable debt borrows, borrow rate modes, and raw amounts:
```graphql
query GetRecentBorrows {
  borrowactions(first: 10, orderBy: "blockNumber", orderDirection: "desc") {
    id
    reserve
    user
    onBehalfOf
    amount
    interestRateMode
    borrowRate
    blockNumber
    blockTimestamp
    transactionHash
  }
}
```

---

## 4. Liquidation Actions (Collateral Auctions)

### Query Recent Liquidations and Bonus Rewards
Inspect bad-debt liquidations, covered debt, and seized collateral amounts:
```graphql
query GetRecentLiquidations {
  liquidationactions(first: 10, orderBy: "blockNumber", orderDirection: "desc") {
    id
    collateralAsset
    debtAsset
    user
    liquidator
    debtToCover
    liquidatedCollateralAmount
    receiveAToken
    blockNumber
    transactionHash
  }
}
```

---

## 5. FlashLoan Actions

### Query Recent Flash Loans and Protocol Premiums
Inspect uncollateralized flashloans, borrower initiator, target contracts, and fee premiums:
```graphql
query GetRecentFlashLoans {
  flashloanactions(first: 10, orderBy: "blockNumber", orderDirection: "desc") {
    id
    target
    initiator
    asset
    amount
    premium
    blockNumber
    blockTimestamp
    transactionHash
  }
}
```

---

## 6. User Portfolio & Activity

### Query Top Active Users by Supply Count
Fetch active wallets and their cumulative interaction counts:
```graphql
query GetTopSuppliers {
  users(first: 10, orderBy: "supplyCount", orderDirection: "desc") {
    id
    address
    supplyCount
    borrowCount
    liquidationCount
  }
}
```

---

## 7. Asset Reserves Overview

### Query Asset Liquidity Overview
Query aggregate lending and borrow metrics per underlying reserve token:
```graphql
query GetReserveStats {
  reserves(first: 20) {
    id
    assetAddress
    totalSupplied
    totalBorrowed
    totalLiquidated
  }
}
```

---

## 8. Testing Queries from Command Line (cURL)

You can run any of these queries directly via `curl` through port-forwarding:

```bash
# 1. Forward port from the cluster
kubectl port-forward svc/logrix-api 4000:4000 -n logrix

# 2. Query in terminal
curl -X POST http://localhost:4000/graphql \
  -H "Content-Type: application/json" \
  -d '{"query":"{ protocol(id: \"aave-v3-ethereum\") { totalSuppliedCount totalLiquidatedCount } }"}'
```
