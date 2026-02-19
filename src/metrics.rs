use crate::types::InclusionLatency;

/// Compute inclusion latency percentiles from a list of latency values (in seconds)
pub fn compute_inclusion_latency(latencies: &[f64]) -> InclusionLatency {
    if latencies.is_empty() {
        return InclusionLatency {
            p50: 0.0,
            p95: 0.0,
            p99: 0.0,
        };
    }

    let mut sorted = latencies.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    InclusionLatency {
        p50: percentile_sorted(&sorted, 0.50),
        p95: percentile_sorted(&sorted, 0.95),
        p99: percentile_sorted(&sorted, 0.99),
    }
}

fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (p * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}
