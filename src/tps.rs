use std::collections::VecDeque;

use crate::types::{BlockData, TpsMetrics};

/// TPS calculation engine.
/// Computes instant, rolling, peak, sustained, and finalized TPS
/// from confirmed block data only.
pub struct TpsEngine {
    blocks: VecDeque<BlockData>,
    window_size: usize,
    peak_tps: f64,
    all_blocks: Vec<BlockData>, // Keep all blocks for sustained/finalized calculations
}

impl TpsEngine {
    pub fn new(window_size: usize) -> Self {
        Self {
            blocks: VecDeque::with_capacity(window_size + 1),
            window_size,
            peak_tps: 0.0,
            all_blocks: Vec::new(),
        }
    }

    /// Add a new confirmed block
    pub fn add_block(&mut self, block: BlockData) {
        if self.blocks.len() >= self.window_size {
            self.blocks.pop_front();
        }
        self.blocks.push_back(block.clone());
        self.all_blocks.push(block);
    }

    /// Compute all TPS metrics from confirmed block data.
    ///
    /// Core formula: TPS = total_confirmed_tx / time_window
    /// where time_window = last_block_time - first_block_time
    pub fn compute(&self) -> TpsMetrics {
        let instant_tps = self.instant_tps();
        let rolling_tps = self.rolling_tps();
        let peak = if instant_tps > self.peak_tps {
            instant_tps
        } else {
            self.peak_tps
        };
        let sustained_tps = self.sustained_tps();
        let finalized_tps = self.finalized_tps();
        let (block_time_avg, block_time_variance) = self.block_time_stats();

        TpsMetrics {
            instant_tps,
            rolling_tps,
            peak_tps: peak,
            sustained_tps,
            finalized_tps,
            block_time_avg,
            block_time_variance,
        }
    }

    /// Update peak (called after compute to persist)
    pub fn update_peak(&mut self, metrics: &TpsMetrics) {
        if metrics.instant_tps > self.peak_tps {
            self.peak_tps = metrics.instant_tps;
        }
    }

    /// Instant TPS: transactions in the latest block / block time
    fn instant_tps(&self) -> f64 {
        if let Some(block) = self.blocks.back() {
            if block.block_time > 0.0 {
                return block.tx_count as f64 / block.block_time;
            }
        }
        0.0
    }

    /// Rolling TPS over the sliding window.
    /// TPS = sum(tx_count) / (last_timestamp - first_timestamp)
    fn rolling_tps(&self) -> f64 {
        if self.blocks.len() < 2 {
            return self.instant_tps();
        }

        let first = self.blocks.front().unwrap();
        let last = self.blocks.back().unwrap();

        let time_window = last.timestamp - first.timestamp;
        if time_window <= 0.0 {
            return 0.0;
        }

        // Sum tx from all blocks except the first (the first block's txs
        // were confirmed before our time window started)
        let total_tx: u64 = self
            .blocks
            .iter()
            .skip(1)
            .map(|b| b.tx_count)
            .sum();

        total_tx as f64 / time_window
    }

    /// Sustained TPS: average over all blocks seen (long window, 60s+)
    fn sustained_tps(&self) -> f64 {
        if self.all_blocks.len() < 2 {
            return 0.0;
        }

        let first = &self.all_blocks[0];
        let last = self.all_blocks.last().unwrap();

        let time_window = last.timestamp - first.timestamp;
        if time_window < 1.0 {
            return 0.0;
        }

        let total_tx: u64 = self
            .all_blocks
            .iter()
            .skip(1)
            .map(|b| b.tx_count)
            .sum();

        total_tx as f64 / time_window
    }

    /// Finalized TPS: only count blocks at height <= latest - 2
    /// This ensures we only measure truly finalized transactions.
    fn finalized_tps(&self) -> f64 {
        if self.all_blocks.len() < 4 {
            return 0.0;
        }

        let latest_height = self.all_blocks.last().unwrap().height;
        let finality_cutoff = latest_height.saturating_sub(2);

        let finalized: Vec<&BlockData> = self
            .all_blocks
            .iter()
            .filter(|b| b.height <= finality_cutoff)
            .collect();

        if finalized.len() < 2 {
            return 0.0;
        }

        let first = finalized[0];
        let last = finalized[finalized.len() - 1];
        let time_window = last.timestamp - first.timestamp;

        if time_window <= 0.0 {
            return 0.0;
        }

        let total_tx: u64 = finalized.iter().skip(1).map(|b| b.tx_count).sum();

        total_tx as f64 / time_window
    }

    /// Compute average and variance of block times
    fn block_time_stats(&self) -> (f64, f64) {
        let times: Vec<f64> = self
            .all_blocks
            .iter()
            .filter(|b| b.block_time > 0.0)
            .map(|b| b.block_time)
            .collect();

        if times.is_empty() {
            return (0.0, 0.0);
        }

        let avg = times.iter().sum::<f64>() / times.len() as f64;
        let variance = times.iter().map(|t| (t - avg).powi(2)).sum::<f64>() / times.len() as f64;

        (avg, variance)
    }

    /// Get all blocks stored
    pub fn blocks(&self) -> Vec<BlockData> {
        self.all_blocks.clone()
    }
}
