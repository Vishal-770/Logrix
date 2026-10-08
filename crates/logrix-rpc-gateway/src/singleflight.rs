use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::debug;

type FlightChannel = broadcast::Sender<Result<String, String>>;
type FlightMap = Arc<Mutex<HashMap<String, FlightChannel>>>;

/// In-flight singleflight request coordinator.
///
/// Merges duplicate concurrent requests into a single network execution,
/// broadcasting the result or error to all awaiting tasks.
#[derive(Clone, Default)]
pub struct SingleFlight {
    in_flight: FlightMap,
}

impl SingleFlight {
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute or join an in-flight asynchronous computation identified by `key`.
    ///
    /// If an identical key is already executing, the caller waits for the existing
    /// task's broadcast rather than making a redundant network call.
    pub async fn execute<F, Fut>(&self, key: &str, task_fn: F) -> LogrixResult<String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = LogrixResult<String>>,
    {
        let mut rx = {
            let mut map = self.in_flight.lock().await;
            if let Some(tx) = map.get(key) {
                debug!(key, "SingleFlight joining existing in-flight request");
                tx.subscribe()
            } else {
                let (tx, _rx) = broadcast::channel(1);
                map.insert(key.to_string(), tx);
                drop(map);

                // RAII guard ensures key is removed if task_fn is cancelled
                let mut guard = FlightGuard {
                    key: key.to_string(),
                    in_flight: self.in_flight.clone(),
                    completed: false,
                };

                // We are the leader for this key. Execute task.
                let outcome = task_fn().await;
                guard.completed = true;

                // Clean up map and broadcast to followers
                let mut map = self.in_flight.lock().await;
                if let Some(tx) = map.remove(key) {
                    let broadcast_payload = outcome.clone().map_err(|e| e.to_string());
                    let _ = tx.send(broadcast_payload);
                }
                return outcome;
            }
        };

        // We are a follower. Await broadcast result.
        match rx.recv().await {
            Ok(Ok(val)) => Ok(val),
            Ok(Err(err_msg)) => Err(LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::ChainRpc,
                format!("SingleFlight broadcast error: {err_msg}"),
            )),
            Err(e) => Err(LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::ChainRpc,
                format!("SingleFlight subscriber missed broadcast: {e}"),
            )),
        }
    }
}

struct FlightGuard {
    key: String,
    in_flight: FlightMap,
    completed: bool,
}

impl Drop for FlightGuard {
    fn drop(&mut self) {
        if !self.completed {
            let key = self.key.clone();
            let in_flight = self.in_flight.clone();
            tokio::spawn(async move {
                let mut map = in_flight.lock().await;
                if let Some(tx) = map.remove(&key) {
                    let _ = tx.send(Err("SingleFlight leader task cancelled".to_string()));
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn test_singleflight_deduplication() {
        let sf = SingleFlight::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..10 {
            let sf_clone = sf.clone();
            let counter_clone = counter.clone();
            handles.push(tokio::spawn(async move {
                sf_clone
                    .execute("block-header-100", || async move {
                        // simulate network latency
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        counter_clone.fetch_add(1, Ordering::SeqCst);
                        Ok("0xdeadbeef".to_string())
                    })
                    .await
            }));
        }

        for h in handles {
            let res = h.await.unwrap().unwrap();
            assert_eq!(res, "0xdeadbeef");
        }

        // 10 concurrent requests, but the actual task only ran once!
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
