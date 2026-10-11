use tokio::sync::watch;
use tracing::info;

/// Unified graceful shutdown listener.
///
/// On Unix/Linux environments (including Kubernetes pods), listens concurrently for
/// both `SIGINT` (Ctrl+C) and `SIGTERM` (Kubernetes container termination / pod eviction).
/// On other platforms, listens for `SIGINT`.
pub async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let sigterm = match signal(SignalKind::terminate()) {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to register SIGTERM listener; falling back to SIGINT");
                None
            }
        };

        if let Some(mut term) = sigterm {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("Received SIGINT (Ctrl+C). Initiating graceful shutdown...");
                }
                _ = term.recv() => {
                    info!("Received SIGTERM (termination signal). Initiating graceful shutdown...");
                }
            }
        } else {
            let _ = tokio::signal::ctrl_c().await;
            info!("Received SIGINT (Ctrl+C). Initiating graceful shutdown...");
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        info!("Received shutdown signal. Initiating graceful shutdown...");
    }
}

/// Create a watch channel pair for coordinating graceful shutdown across internal tasks.
pub fn shutdown_channel() -> (watch::Sender<bool>, watch::Receiver<bool>) {
    watch::channel(false)
}

/// Awaits until the shutdown channel receiver indicates a termination event.
pub async fn wait_shutdown(mut rx: watch::Receiver<bool>) {
    while !*rx.borrow() {
        if rx.changed().await.is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shutdown_channel_triggers() {
        let (tx, rx) = shutdown_channel();
        let handle = tokio::spawn(async move {
            wait_shutdown(rx).await;
        });
        tx.send(true).expect("Send shutdown signal");
        handle.await.expect("Task completes after shutdown signal");
    }

    #[tokio::test]
    async fn test_shutdown_channel_closed_sender() {
        let (tx, rx) = shutdown_channel();
        let handle = tokio::spawn(async move {
            wait_shutdown(rx).await;
        });
        drop(tx);
        handle.await.expect("Task completes when sender is dropped");
    }
}
