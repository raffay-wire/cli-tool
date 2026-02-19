use serde::{Deserialize, Serialize};

/// Data collected from a single block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockData {
    pub height: u64,
    pub timestamp: f64, // Unix timestamp as seconds (with fractional)
    pub tx_count: u64,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub block_time: f64, // Seconds since previous block
    pub size_bytes: Option<u64>,
}

/// Chain configuration fetched from the node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub chain_id: String,
    pub max_gas: i64,     // -1 means unlimited
    pub max_bytes: i64,   // -1 means unlimited
    pub block_time_avg: f64,
    pub latest_height: u64,
    pub node_version: String,
}

/// TPS metrics computed from block data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TpsMetrics {
    pub instant_tps: f64,
    pub rolling_tps: f64,
    pub peak_tps: f64,
    pub sustained_tps: f64,
    pub finalized_tps: f64,
    pub block_time_avg: f64,
    pub block_time_variance: f64,
}

/// Bottleneck detection results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleneckReport {
    pub gas_bottleneck: bool,
    pub gas_utilization: f64,
    pub consensus_bottleneck: bool,
    pub block_time_trend: f64, // positive = increasing
    pub rpc_bottleneck: bool,
    pub rpc_latency_avg_ms: f64,
    pub rpc_timeout_count: u64,
    pub details: Vec<String>,
}

/// Inclusion latency percentiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InclusionLatency {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
}

/// Load test tracking for sent transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestMetrics {
    pub tx_sent: u64,
    pub tx_confirmed: u64,
    pub tx_failed: u64,
    pub tx_dropped: u64,
    pub tx_reverted: u64,
    pub revert_rate: f64,
    pub drop_rate: f64,
    pub inclusion_latency: InclusionLatency,
}

/// Final report combining all metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalReport {
    pub chain_id: String,
    pub duration_secs: f64,
    pub blocks_scanned: u64,
    pub total_transactions: u64,
    pub average_tps: f64,
    pub peak_tps: f64,
    pub sustained_tps: f64,
    pub finalized_tps: f64,
    pub block_time_avg: f64,
    pub block_time_variance: f64,
    pub gas_utilization_avg: f64,
    pub bottleneck: BottleneckReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_test: Option<LoadTestMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inclusion_latency: Option<InclusionLatency>,
    pub rpc_latency_avg_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theoretical_max_tps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saturation_point_tps: Option<f64>,
}

/// Tracks a sent transaction for inclusion latency measurement
#[derive(Debug, Clone)]
pub struct TrackedTx {
    pub tx_hash: String,
    pub sent_at: f64,      // Unix timestamp
    pub nonce: u64,
    pub confirmed_at: Option<f64>,
    pub block_height: Option<u64>,
    pub status: TxStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TxStatus {
    Pending,
    Confirmed,
    Failed,
    Dropped,
    Reverted,
}

/// Output format selection
#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
    Csv,
}

/// Load test mode
#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum LoadMode {
    Sequential,
    Parallel,
    Batch,
    Adaptive,
}

/// Transaction type for load testing
#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum TxType {
    Transfer,
    Contract,
}
