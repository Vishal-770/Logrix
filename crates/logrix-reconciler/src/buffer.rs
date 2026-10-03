use alloy_primitives::B256;
use logrix_core::domain::BlockRef;
use std::collections::VecDeque;

/// Ring buffer holding the last `capacity` blocks (default 128) for $O(1)$ reorg detection.
#[derive(Debug, Clone)]
pub struct RollingBlockBuffer {
    capacity: usize,
    buffer: VecDeque<BlockRef>,
}

impl RollingBlockBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            buffer: VecDeque::with_capacity(capacity),
        }
    }

    /// Default buffer sized for standard EVM finality depth (128 blocks).
    pub fn default_depth() -> Self {
        Self::new(128)
    }

    /// Push a new block header reference into the buffer.
    pub fn push(&mut self, block: BlockRef) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(block);
    }

    /// Get the most recently appended block reference.
    pub fn latest(&self) -> Option<&BlockRef> {
        self.buffer.back()
    }

    /// Find block reference by block number.
    pub fn get_by_number(&self, number: u64) -> Option<&BlockRef> {
        self.buffer.iter().find(|b| b.number == number)
    }

    /// Check if the incoming block's parent hash matches the tip of our buffer.
    pub fn is_parent_matching(&self, block_number: u64, parent_hash: B256) -> bool {
        match self.latest() {
            Some(tip) => {
                if block_number == tip.number + 1 {
                    tip.hash == parent_hash
                } else {
                    // Non-contiguous block: gap or jump
                    false
                }
            }
            None => true, // First block in buffer always accepts
        }
    }

    /// Rewind in-memory buffer to fork point after a reorg.
    pub fn rewind_to(&mut self, fork_block: u64) {
        while let Some(tip) = self.buffer.back() {
            if tip.number > fork_block {
                self.buffer.pop_back();
            } else {
                break;
            }
        }
    }

    /// Number of blocks currently in buffer.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Clear all buffer entries.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_sliding_and_parent_continuity() {
        let mut buf = RollingBlockBuffer::new(3);
        let h1 = B256::repeat_byte(0x01);
        let h2 = B256::repeat_byte(0x02);
        let h3 = B256::repeat_byte(0x03);
        let h4 = B256::repeat_byte(0x04);

        buf.push(BlockRef::new(100, h1));
        assert!(buf.is_parent_matching(101, h1));
        assert!(!buf.is_parent_matching(101, h2));

        buf.push(BlockRef::new(101, h2));
        buf.push(BlockRef::new(102, h3));

        // Buffer is at capacity (3 items: 100, 101, 102)
        assert_eq!(buf.len(), 3);

        // Push 103: drops 100
        buf.push(BlockRef::new(103, h4));
        assert_eq!(buf.len(), 3);
        assert!(buf.get_by_number(100).is_none());
        assert!(buf.get_by_number(101).is_some());

        // Rewind to 101
        buf.rewind_to(101);
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.latest().unwrap().number, 101);
    }
}
