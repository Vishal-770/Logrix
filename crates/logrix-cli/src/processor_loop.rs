use logrix_core::domain::{QueueMessage, QueueType};
use std::time::Duration;
use tracing::info;

use crate::handlers::{handle_backfill_range, handle_live_block};
use crate::processor_types::ProcessorContext;

type DynError = Box<dyn std::error::Error + Send + Sync>;

pub async fn run_consumer_loop(
    ctx: &ProcessorContext<'_>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
) -> Result<(), DynError> {
    info!("Starting processor consumer loop; listening for shutdown signals");
    let mut backoff = Duration::from_millis(500);
    let max_backoff = Duration::from_secs(5);

    let mut check_live_first = true;

    loop {
        let (first_q, second_q) = if check_live_first {
            (QueueType::Live, QueueType::Backfill)
        } else {
            (QueueType::Backfill, QueueType::Live)
        };
        check_live_first = !check_live_first;

        let msg_res = tokio::select! {
            _ = crate::shutdown::wait_for_shutdown_signal() => {
                info!("Shutdown signal received; draining processor consumer loop");
                let _ = shutdown_tx.send(true);
                break;
            }
            res = ctx.queue.consume(first_q) => match res {
                Ok(Some(h)) => {
                    backoff = Duration::from_millis(500);
                    Some((first_q, h))
                }
                Ok(None) => match ctx.queue.consume(second_q).await {
                    Ok(opt) => {
                        backoff = Duration::from_millis(500);
                        opt.map(|h| (second_q, h))
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, queue = ?second_q, "Error consuming from secondary queue; backing off");
                        tokio::time::sleep(backoff).await;
                        backoff = (backoff * 2).min(max_backoff);
                        None
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, queue = ?first_q, "Error consuming from primary queue; checking secondary");
                    match ctx.queue.consume(second_q).await {
                        Ok(opt) => {
                            backoff = Duration::from_millis(500);
                            opt.map(|h| (second_q, h))
                        }
                        Err(e2) => {
                            tracing::warn!(error = %e2, queue = ?second_q, "Both queues errored; backing off");
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(max_backoff);
                            None
                        }
                    }
                }
            }
        };

        let Some((_q_type, handle)) = msg_res else {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        };

        match &handle.message {
            QueueMessage::LiveBlock(job) => {
                handle_live_block(ctx, job.block_number, &handle)
                    .await
                    .map_err(|e| Box::new(std::io::Error::other(e.to_string())) as DynError)?;
            }
            QueueMessage::BackfillRange(job) => {
                handle_backfill_range(ctx, job, &handle)
                    .await
                    .map_err(|e| Box::new(std::io::Error::other(e.to_string())) as DynError)?;
            }
            _ => {
                let _ = ctx.queue.ack(&handle).await;
            }
        }
    }
    let _ = shutdown_tx.send(true);
    info!("Processor consumer loop exited cleanly");
    Ok(())
}
