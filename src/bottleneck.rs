use crate::types::{BlockData, BottleneckReport};

/// Analyze block data and RPC metrics to detect bottlenecks.
///
/// Detects three types:
/// 1. Gas bottleneck: gas_used/gas_limit > 0.95 consistently
/// 2. Consensus bottleneck: blocks full but block time increasing
/// 3. RPC bottleneck: RPC latency spikes while on-chain TPS is stable
pub fn analyze(
    blocks: &[BlockData],
    rpc_latencies: &[f64],
    rpc_timeouts: u64,
) -> BottleneckReport {
    let mut details = Vec::new();

    // --- Gas utilization analysis ---
    let gas_utilizations: Vec<f64> = blocks
        .iter()
        .filter(|b| b.gas_limit > 0)
        .map(|b| b.gas_used as f64 / b.gas_limit as f64)
        .collect();

    let gas_utilization = if gas_utilizations.is_empty() {
        0.0
    } else {
        gas_utilizations.iter().sum::<f64>() / gas_utilizations.len() as f64
    };

    let high_gas_blocks = gas_utilizations
        .iter()
        .filter(|&&g| g > 0.95)
        .count();

    let gas_bottleneck = if gas_utilizations.is_empty() {
        false
    } else {
        high_gas_blocks as f64 / gas_utilizations.len() as f64 > 0.5
    };

    if gas_bottleneck {
        details.push(format!(
            "GAS BOTTLENECK: {:.1}% avg utilization, {}/{} blocks above 95%",
            gas_utilization * 100.0,
            high_gas_blocks,
            gas_utilizations.len()
        ));
    }

    // --- Block time trend analysis (consensus bottleneck) ---
    let block_times: Vec<f64> = blocks
        .iter()
        .filter(|b| b.block_time > 0.0)
        .map(|b| b.block_time)
        .collect();

    let block_time_trend = compute_trend(&block_times);

    // Consensus bottleneck: blocks are gas-full AND block time is increasing
    let consensus_bottleneck = gas_bottleneck && block_time_trend > 0.1;

    if consensus_bottleneck {
        details.push(format!(
            "CONSENSUS BOTTLENECK: Block time trending up ({:+.3}s/block) while gas is full",
            block_time_trend
        ));
    } else if block_time_trend > 0.2 {
        details.push(format!(
            "WARNING: Block time increasing ({:+.3}s/block), possible validator lag",
            block_time_trend
        ));
    }

    // --- RPC latency analysis ---
    let rpc_latency_avg = if rpc_latencies.is_empty() {
        0.0
    } else {
        rpc_latencies.iter().sum::<f64>() / rpc_latencies.len() as f64
    };

    let rpc_latency_p95 = percentile(rpc_latencies, 0.95);

    // RPC bottleneck: high latency or many timeouts, but chain TPS is stable
    let rpc_bottleneck = (rpc_latency_avg > 1000.0 || rpc_timeouts > 5)
        && !gas_bottleneck
        && block_time_trend < 0.1;

    if rpc_bottleneck {
        details.push(format!(
            "RPC BOTTLENECK: Avg latency {:.0}ms, P95 {:.0}ms, {} timeouts. \
             Chain is fine but RPC is saturated.",
            rpc_latency_avg, rpc_latency_p95, rpc_timeouts
        ));
    }

    if rpc_timeouts > 0 {
        details.push(format!("RPC timeouts: {}", rpc_timeouts));
    }

    // Additional insights
    if !gas_bottleneck && gas_utilization < 0.1 && !blocks.is_empty() {
        let has_txs = blocks.iter().any(|b| b.tx_count > 0);
        if !has_txs {
            details.push("No transactions observed in scanned blocks.".to_string());
        } else {
            details.push(format!(
                "Low gas utilization ({:.1}%): chain has capacity for more throughput.",
                gas_utilization * 100.0
            ));
        }
    }

    BottleneckReport {
        gas_bottleneck,
        gas_utilization,
        consensus_bottleneck,
        block_time_trend,
        rpc_bottleneck,
        rpc_latency_avg_ms: rpc_latency_avg,
        rpc_timeout_count: rpc_timeouts,
        details,
    }
}

/// Compute a simple linear trend (slope) from a time series.
/// Positive = increasing, negative = decreasing.
fn compute_trend(values: &[f64]) -> f64 {
    if values.len() < 3 {
        return 0.0;
    }

    let n = values.len() as f64;
    let x_mean = (n - 1.0) / 2.0;
    let y_mean = values.iter().sum::<f64>() / n;

    let mut numerator = 0.0;
    let mut denominator = 0.0;

    for (i, &y) in values.iter().enumerate() {
        let x = i as f64;
        numerator += (x - x_mean) * (y - y_mean);
        denominator += (x - x_mean).powi(2);
    }

    if denominator.abs() < f64::EPSILON {
        0.0
    } else {
        numerator / denominator
    }
}

/// Compute a percentile from a slice of values
fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let idx = (p * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}
