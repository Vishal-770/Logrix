-- Migration: Dynamic Schema Entity Storage
-- Unified JSONB table for custom user-defined indexer entities with GIN and composite B-tree indexing

CREATE TABLE IF NOT EXISTS logrix_entities (
    chain_id BIGINT NOT NULL,
    entity_type VARCHAR(128) NOT NULL,
    entity_id VARCHAR(256) NOT NULL,
    data JSONB NOT NULL,
    block_number BIGINT NOT NULL,
    is_reverted BOOLEAN NOT NULL DEFAULT FALSE,
    reverted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (chain_id, entity_type, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_entities_lookup 
    ON logrix_entities (chain_id, entity_type, is_reverted, block_number DESC);

CREATE INDEX IF NOT EXISTS idx_entities_data_gin 
    ON logrix_entities USING GIN (data jsonb_path_ops);
