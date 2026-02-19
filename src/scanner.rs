use anyhow::Result;
use std::collections::VecDeque;

use crate::rpc::eth::{parse_hex_u64, EthRpcClient};
use crate::types::BlockData;

/// Continuously scans blocks from the chain via Ethereum JSON-RPC.
/// Stores a rolling window of recent blocks.
pub struct BlockScanner {
    eth_client: EthRpcClient,
    blocks: VecDeque<BlockData>,
    max_blocks: usize,
    last_seen_height: u64,
    prev_timestamp: Option<f64>,
}

impl BlockScanner {
    pub fn new(eth_client: EthRpcClient, max_blocks: usize) -> Self {
        Self {
            eth_client,
            blocks: VecDeque::with_capacity(max_blocks),
            max_blocks,
            last_seen_height: 0,
            prev_timestamp: None,
        }
    }

    /// Poll for the latest block. Returns `Some(BlockData)` if a new block was found.
    pub async fn poll_latest_block(&mut self) -> Result<Option<BlockData>> {
        let block = self.eth_client.get_latest_block().await?;
        let height = parse_hex_u64(&block.number)?;

        if height <= self.last_seen_height {
            return Ok(None);
        }

        let timestamp = parse_hex_u64(&block.timestamp)? as f64;
        let gas_used = parse_hex_u64(&block.gas_used)?;
        let gas_limit = parse_hex_u64(&block.gas_limit)?;
        let tx_count = block.transactions.len() as u64;
        let size_bytes = block
            .size
            .as_deref()
            .and_then(|s| parse_hex_u64(s).ok());

        let block_time = match self.prev_timestamp {
            Some(prev) => timestamp - prev,
            None => 0.0,
        };

        self.prev_timestamp = Some(timestamp);
        self.last_seen_height = height;

        let data = BlockData {
            height,
            timestamp,
            tx_count,
            gas_used,
            gas_limit,
            block_time,
            size_bytes,
        };

        self.push_block(data.clone());

        Ok(Some(data))
    }

    /// Fetch a specific block by height
    pub async fn fetch_block(&mut self, height: u64) -> Result<Option<BlockData>> {
        let block = match self.eth_client.get_block_by_number(height, false).await? {
            Some(b) => b,
            None => return Ok(None),
        };

        let timestamp = parse_hex_u64(&block.timestamp)? as f64;
        let gas_used = parse_hex_u64(&block.gas_used)?;
        let gas_limit = parse_hex_u64(&block.gas_limit)?;
        let tx_count = block.transactions.len() as u64;
        let size_bytes = block
            .size
            .as_deref()
            .and_then(|s| parse_hex_u64(s).ok());

        let block_time = match self.prev_timestamp {
            Some(prev) if timestamp > prev => timestamp - prev,
            _ => 0.0,
        };

        self.prev_timestamp = Some(timestamp);

        let data = BlockData {
            height,
            timestamp,
            tx_count,
            gas_used,
            gas_limit,
            block_time,
            size_bytes,
        };

        self.push_block(data.clone());

        Ok(Some(data))
    }

    fn push_block(&mut self, block: BlockData) {
        if self.blocks.len() >= self.max_blocks {
            self.blocks.pop_front();
        }
        self.blocks.push_back(block);
    }

    /// Get all stored blocks
    pub fn blocks(&self) -> Vec<BlockData> {
        self.blocks.iter().cloned().collect()
    }
}
