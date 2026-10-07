# logrix-reconciler

Continuous gap reconciliation and chain reorganization detection engine for Logrix.

## Overview
Ensures deterministic data integrity in the face of blockchain forks, temporary node desyncs, and RPC rate-limit dropouts.

## Key Subsystems
1. **Parent-Hash Ring Buffer (`buffer.rs`)**:
   - In-memory rolling buffer tracking recent block hashes and parent continuity.
2. **Reorg Detector (`detector.rs`)**:
   - Compares incoming block parent hashes against the local chain state to detect fork points and common ancestors.
3. **Rollback Handler (`rollback.rs`)**:
   - Executes atomic SQL rollbacks by marking affected entities as soft-deleted (`is_reverted = TRUE`).
   - Publishes `event.reverted` webhook jobs into `QueueType::Webhook`.
4. **Gap Reconciler (`reconciler.rs`)**:
   - Periodically checks indexed block ranges against PostgreSQL checkpoints and schedules backfill fill-in jobs for missing ranges.
