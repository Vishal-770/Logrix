-- Migration: Reorganization Soft-Delete and Audit Trail
-- Adds is_reverted and reverted_at columns to preserve data provenance on reorgs

ALTER TABLE logrix_events 
    ADD COLUMN IF NOT EXISTS is_reverted BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS reverted_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_events_reverted 
    ON logrix_events (chain_id, is_reverted, block_number DESC);

ALTER TABLE token_transfers 
    ADD COLUMN IF NOT EXISTS is_reverted BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS reverted_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_transfers_reverted 
    ON token_transfers (chain_id, is_reverted, block_number DESC);
