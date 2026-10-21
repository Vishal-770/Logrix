use logrix_core::error::{ErrorClass, ErrorSource, LogrixError};
use logrix_rpc_gateway::SingleFlight;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_singleflight_preserves_error_class_fidelity() {
    let sf = SingleFlight::new();
    let counter = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..5 {
        let sf_clone = sf.clone();
        let counter_clone = counter.clone();
        handles.push(tokio::spawn(async move {
            sf_clone
                .execute("rate-limited-query", || async move {
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    Err(LogrixError::rate_limited(
                        ErrorSource::ChainRpc,
                        "Provider quota exceeded",
                        Some(Duration::from_secs(5)),
                    ))
                })
                .await
        }));
    }

    for h in handles {
        let res = h.await.unwrap();
        assert!(res.is_err());
        let err = res.unwrap_err();
        // Crucial test: Followers must receive exact ErrorClass::RateLimited!
        assert!(
            matches!(err.class, ErrorClass::RateLimited { retry_after } if retry_after == Some(Duration::from_secs(5)))
        );
        assert_eq!(err.source, ErrorSource::ChainRpc);
    }

    // Only executed once across all 5 concurrent tasks
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}
