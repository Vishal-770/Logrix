-- Logrix Primary Schema Initial Migration
-- Tables: checkpoints, raw event log stream, and token transfers domain entity

CREATE TABLE IF NOT EXISTS logrix_checkpoints (
    chain_id BIGINT NOT NULL,
    pipeline_id VARCHAR(64) NOT NULL DEFAULT 'default',
    last_indexed_block BIGINT NOT NULL,
    last_block_hash VARCHAR(66) NOT NULL,
    is_finalized BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (chain_id, pipeline_id)
);

CREATE TABLE IF NOT EXISTS logrix_events (
    chain_id BIGINT NOT NULL,
    block_number BIGINT NOT NULL,
    block_hash VARCHAR(66) NOT NULL,
    tx_hash VARCHAR(66) NOT NULL,
    tx_index BIGINT NOT NULL,
    log_index BIGINT NOT NULL,
    contract_address VARCHAR(42) NOT NULL,
    topic0 VARCHAR(66),
    topics JSONB NOT NULL DEFAULT '[]'::jsonb,
    data BYTEA NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (chain_id, tx_hash, log_index)
);

CREATE INDEX IF NOT EXISTS idx_events_lookup 
    ON logrix_events (chain_id, contract_address, block_number DESC);

CREATE INDEX IF NOT EXISTS idx_events_topic0 
    ON logrix_events (chain_id, topic0, block_number DESC);

CREATE INDEX IF NOT EXISTS idx_events_block 
    ON logrix_events (chain_id, block_number DESC);

CREATE TABLE IF NOT EXISTS token_transfers (
    chain_id BIGINT NOT NULL,
    block_number BIGINT NOT NULL,
    block_hash VARCHAR(66) NOT NULL,
    tx_hash VARCHAR(66) NOT NULL,
    log_index BIGINT NOT NULL,
    contract_address VARCHAR(42) NOT NULL,
    from_address VARCHAR(42) NOT NULL,
    to_address VARCHAR(42) NOT NULL,
    amount NUMERIC(78, 0) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (chain_id, tx_hash, log_index)
);

CREATE INDEX IF NOT EXISTS idx_transfers_from 
    ON token_transfers (chain_id, contract_address, from_address, block_number DESC);

CREATE INDEX IF NOT EXISTS idx_transfers_to 
    ON token_transfers (chain_id, contract_address, to_address, block_number DESC);

CREATE INDEX IF NOT EXISTS idx_transfers_block 
    ON token_transfers (chain_id, block_number DESC);
