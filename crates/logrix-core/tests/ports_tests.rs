use alloy_primitives::{b256, Address};
use async_trait::async_trait;
use logrix_core::domain::{
    BlockEnvelope, BlockRef, ChainId, MessageHandle, QueueMessage, QueueType, WebhookPayload,
};
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use logrix_core::ports::{BlobPort, ChainPort, QueuePort};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct MockQueue {
    published: Arc<AtomicUsize>,
}

#[async_trait]
impl QueuePort for MockQueue {
    async fn publish(&self, _queue: QueueType, _msg: &QueueMessage) -> LogrixResult<()> {
        self.published.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn consume(&self, _queue: QueueType) -> LogrixResult<Option<MessageHandle>> {
        Ok(None)
    }

    async fn ack(&self, _handle: &MessageHandle) -> LogrixResult<()> {
        Ok(())
    }

    async fn nack(&self, _handle: &MessageHandle, _requeue: bool) -> LogrixResult<()> {
        Ok(())
    }

    async fn dead_letter(&self, _handle: &MessageHandle, _reason: &str) -> LogrixResult<()> {
        Ok(())
    }

    async fn depth(&self, _queue: QueueType) -> LogrixResult<u64> {
        Ok(0)
    }
}

struct MockBlob {
    stored: Option<Vec<u8>>,
}

#[async_trait]
impl BlobPort for MockBlob {
    async fn put(&self, _key: &str, _data: &[u8]) -> LogrixResult<()> {
        Ok(())
    }

    async fn get(&self, _key: &str) -> LogrixResult<Option<Vec<u8>>> {
        Ok(self.stored.clone())
    }

    async fn delete(&self, _key: &str) -> LogrixResult<()> {
        Ok(())
    }
}

struct MockChain {
    chain_id: ChainId,
}

#[async_trait]
impl ChainPort for MockChain {
    fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    async fn get_latest_block_number(&self) -> LogrixResult<u64> {
        Ok(100)
    }

    async fn get_block_by_number(&self, _number: u64) -> LogrixResult<Option<BlockRef>> {
        Ok(None)
    }

    async fn fetch_logs(
        &self,
        _from_block: u64,
        _to_block: u64,
        _addresses: &[Address],
    ) -> LogrixResult<Vec<logrix_core::domain::EventLog>> {
        Ok(vec![])
    }

    async fn fetch_block_envelope(
        &self,
        _number: u64,
        _addresses: &[Address],
    ) -> LogrixResult<Option<BlockEnvelope>> {
        Ok(None)
    }
}

#[tokio::test]
async fn test_queue_port_publish_batch_default() {
    let published = Arc::new(AtomicUsize::new(0));
    let queue = MockQueue {
        published: published.clone(),
    };

    let msgs = vec![
        QueueMessage::LiveBlock(logrix_core::domain::LiveBlockJob::new(
            ChainId::ETHEREUM,
            1,
            b256!("0000000000000000000000000000000000000000000000000000000000000001"),
            b256!("0000000000000000000000000000000000000000000000000000000000000000"),
        )),
        QueueMessage::LiveBlock(logrix_core::domain::LiveBlockJob::new(
            ChainId::ETHEREUM,
            2,
            b256!("0000000000000000000000000000000000000000000000000000000000000002"),
            b256!("0000000000000000000000000000000000000000000000000000000000000001"),
        )),
    ];

    queue
        .publish_batch(QueueType::Live, &msgs)
        .await
        .expect("batch publish succeeds");
    assert_eq!(published.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_blob_port_exists_default() {
    let blob_found = MockBlob {
        stored: Some(vec![1, 2, 3]),
    };
    assert!(blob_found.exists("test_key").await.expect("exists check"));

    let blob_missing = MockBlob { stored: None };
    assert!(!blob_missing.exists("test_key").await.expect("exists check"));
}

#[tokio::test]
async fn test_chain_port_chain_id() {
    let chain = MockChain {
        chain_id: ChainId::POLYGON,
    };
    assert_eq!(chain.chain_id(), ChainId::POLYGON);
}

#[test]
fn test_logrix_error_partial_eq_and_sources() {
    let err1 = LogrixError::new(ErrorClass::Transient, ErrorSource::Webhook, "timeout")
        .with_chain_id(ChainId::ETHEREUM);
    let err2 = LogrixError::new(ErrorClass::Transient, ErrorSource::Webhook, "timeout")
        .with_chain_id(1u64);
    assert_eq!(err1, err2);

    let handler_err = LogrixError::new(
        ErrorClass::Permanent,
        ErrorSource::Handler,
        "custom handler err",
    );
    assert_eq!(handler_err.source, ErrorSource::Handler);
}

#[test]
fn test_webhook_payload_with_chain_id() {
    let payload = WebhookPayload::new(
        "transfer",
        ChainId::BASE,
        serde_json::json!({"amount": 100}),
    );
    assert_eq!(payload.chain_id, ChainId::BASE);
    assert_eq!(payload.event, "transfer");

    let payload2 = WebhookPayload::new("approval", 8453, serde_json::json!({}));
    assert_eq!(payload2.chain_id, ChainId::BASE);
}
