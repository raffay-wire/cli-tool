use anyhow::Result;
use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};

use crate::types::{BlockData, BottleneckReport, FinalReport, TpsMetrics};

/// Print the live monitoring table header
pub fn print_table_header() {
    println!(
        "{:>10} {:>6} {:>10} {:>10} {:>10} {:>10} {:>12}",
        "Height", "Tx", "BlockTime", "InstTPS", "RollTPS", "GasUsed", "GasLimit"
    );
    println!("{}", "-".repeat(78));
}

/// Print a single block row in the live monitoring table
pub fn print_block_row(block: &BlockData, metrics: &TpsMetrics) {
    let gas_pct = if block.gas_limit > 0 {
        format!(
            "{:.1}%",
            block.gas_used as f64 / block.gas_limit as f64 * 100.0
        )
    } else {
        "N/A".to_string()
    };

    println!(
        "{:>10} {:>6} {:>9.2}s {:>10.1} {:>10.1} {:>10} {:>12} ({})",
        block.height,
        block.tx_count,
        block.block_time,
        metrics.instant_tps,
        metrics.rolling_tps,
        format_gas(block.gas_used),
        format_gas(block.gas_limit),
        gas_pct,
    );
}

/// Print the final summary report
pub fn print_final_summary(report: &FinalReport) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("Metric").fg(Color::Cyan),
        Cell::new("Value").fg(Color::Cyan),
    ]);

    table.add_row(vec!["Chain ID", &report.chain_id]);
    table.add_row(vec![
        "Duration",
        &format!("{:.1}s", report.duration_secs),
    ]);
    table.add_row(vec![
        "Blocks Scanned",
        &report.blocks_scanned.to_string(),
    ]);
    table.add_row(vec![
        "Total Transactions",
        &report.total_transactions.to_string(),
    ]);
    table.add_row(vec!["", ""]);
    table.add_row(vec![
        "Average TPS",
        &format!("{:.2}", report.average_tps),
    ]);
    table.add_row(vec!["Peak TPS", &format!("{:.2}", report.peak_tps)]);
    table.add_row(vec![
        "Sustained TPS",
        &format!("{:.2}", report.sustained_tps),
    ]);
    table.add_row(vec![
        "Finalized TPS",
        &format!("{:.2}", report.finalized_tps),
    ]);
    table.add_row(vec!["", ""]);
    table.add_row(vec![
        "Block Time Avg",
        &format!("{:.3}s", report.block_time_avg),
    ]);
    table.add_row(vec![
        "Block Time Variance",
        &format!("{:.4}s\u{00B2}", report.block_time_variance),
    ]);
    table.add_row(vec![
        "Gas Utilization Avg",
        &format!("{:.1}%", report.gas_utilization_avg * 100.0),
    ]);
    table.add_row(vec![
        "RPC Latency Avg",
        &format!("{:.1}ms", report.rpc_latency_avg_ms),
    ]);

    if let Some(ref load) = report.load_test {
        table.add_row(vec!["", ""]);
        table.add_row(vec!["--- Load Test ---", ""]);
        table.add_row(vec!["Tx Sent", &load.tx_sent.to_string()]);
        table.add_row(vec!["Tx Confirmed", &load.tx_confirmed.to_string()]);
        table.add_row(vec!["Tx Failed", &load.tx_failed.to_string()]);
        table.add_row(vec!["Tx Dropped", &load.tx_dropped.to_string()]);
        table.add_row(vec!["Tx Reverted", &load.tx_reverted.to_string()]);
        table.add_row(vec![
            "Drop Rate",
            &format!("{:.2}%", load.drop_rate * 100.0),
        ]);
        table.add_row(vec![
            "Revert Rate",
            &format!("{:.2}%", load.revert_rate * 100.0),
        ]);
        table.add_row(vec![
            "Inclusion P50",
            &format!("{:.2}s", load.inclusion_latency.p50),
        ]);
        table.add_row(vec![
            "Inclusion P95",
            &format!("{:.2}s", load.inclusion_latency.p95),
        ]);
        table.add_row(vec![
            "Inclusion P99",
            &format!("{:.2}s", load.inclusion_latency.p99),
        ]);
    }

    if let Some(theoretical) = report.theoretical_max_tps {
        table.add_row(vec![
            "Theoretical Max TPS",
            &format!("{:.1}", theoretical),
        ]);
    }

    if let Some(saturation) = report.saturation_point_tps {
        table.add_row(vec![
            "Saturation Point",
            &format!("{:.0} RPS", saturation),
        ]);
    }

    println!("\n{table}");
}

/// Print bottleneck analysis
pub fn print_bottleneck_analysis(report: &BottleneckReport) {
    if report.details.is_empty() {
        println!("\nBottleneck Analysis: No bottlenecks detected.");
        return;
    }

    println!("\n=== Bottleneck Analysis ===");
    for detail in &report.details {
        let prefix = if detail.starts_with("GAS") || detail.starts_with("CONSENSUS") || detail.starts_with("RPC") {
            "[!]"
        } else if detail.starts_with("WARNING") {
            "[~]"
        } else {
            "[i]"
        };
        println!("  {} {}", prefix, detail);
    }
}

/// Print key takeaways derived from the final report
pub fn print_key_takeaways(report: &FinalReport) {
    println!("\n=== Key Takeaways ===");

    let mut takeaways: Vec<String> = Vec::new();

    // 1. Confirmation rate (load test only)
    if let Some(ref load) = report.load_test {
        if load.tx_sent > 0 {
            let confirm_rate = load.tx_confirmed as f64 / load.tx_sent as f64 * 100.0;
            if confirm_rate >= 99.0 {
                takeaways.push(format!(
                    "EXCELLENT confirmation rate: {:.1}% ({}/{} tx confirmed, 0 drops)",
                    confirm_rate, load.tx_confirmed, load.tx_sent
                ));
            } else if confirm_rate >= 90.0 {
                takeaways.push(format!(
                    "GOOD confirmation rate: {:.1}% ({}/{} tx). {} dropped, {} failed.",
                    confirm_rate, load.tx_confirmed, load.tx_sent, load.tx_dropped, load.tx_failed
                ));
            } else {
                takeaways.push(format!(
                    "LOW confirmation rate: {:.1}% ({}/{} tx). {} dropped, {} failed. Consider using more sender keys or reducing RPS.",
                    confirm_rate, load.tx_confirmed, load.tx_sent, load.tx_dropped, load.tx_failed
                ));
            }
        }
    }

    // 2. TPS assessment
    if report.sustained_tps > 0.0 {
        takeaways.push(format!(
            "Sustained TPS: {:.1} | Peak: {:.1} | Finalized: {:.1}",
            report.sustained_tps, report.peak_tps, report.finalized_tps
        ));
    }

    // 3. Theoretical max vs actual
    if let Some(theoretical) = report.theoretical_max_tps {
        if theoretical > 0.0 && report.sustained_tps > 0.0 {
            let utilization = report.sustained_tps / theoretical * 100.0;
            takeaways.push(format!(
                "Chain utilization: {:.1}% of theoretical max ({:.0} TPS).",
                utilization, theoretical
            ));
        }
    }

    // 4. Gas utilization & headroom
    if report.gas_utilization_avg > 0.0 {
        let headroom = (1.0 - report.gas_utilization_avg) * 100.0;
        if report.gas_utilization_avg > 0.95 {
            takeaways.push(format!(
                "GAS SATURATED: {:.1}% avg utilization. Chain is at max throughput.",
                report.gas_utilization_avg * 100.0
            ));
        } else if report.gas_utilization_avg > 0.70 {
            takeaways.push(format!(
                "High gas utilization: {:.1}%. Only {:.0}% headroom remaining.",
                report.gas_utilization_avg * 100.0, headroom
            ));
        } else {
            takeaways.push(format!(
                "Gas utilization: {:.1}%. Chain has {:.0}% headroom for more throughput.",
                report.gas_utilization_avg * 100.0, headroom
            ));
        }
    }

    // 5. Block time stability
    if report.block_time_avg > 0.0 {
        let stddev = report.block_time_variance.sqrt();
        let cv = stddev / report.block_time_avg; // coefficient of variation
        if cv < 0.1 {
            takeaways.push(format!(
                "Block time STABLE: {:.2}s avg (stddev {:.2}s). Consensus healthy.",
                report.block_time_avg, stddev
            ));
        } else if cv < 0.3 {
            takeaways.push(format!(
                "Block time VARIABLE: {:.2}s avg (stddev {:.2}s). Some consensus jitter.",
                report.block_time_avg, stddev
            ));
        } else {
            takeaways.push(format!(
                "Block time UNSTABLE: {:.2}s avg (stddev {:.2}s). Possible validator issues.",
                report.block_time_avg, stddev
            ));
        }
    }

    // 6. Inclusion latency
    if let Some(ref load) = report.load_test {
        if load.inclusion_latency.p50 > 0.0 {
            let blocks_to_include = load.inclusion_latency.p50 / report.block_time_avg.max(1.0);
            if blocks_to_include <= 1.5 {
                takeaways.push(format!(
                    "FAST inclusion: P50={:.1}s P95={:.1}s P99={:.1}s (tx land in ~{:.0} block).",
                    load.inclusion_latency.p50,
                    load.inclusion_latency.p95,
                    load.inclusion_latency.p99,
                    blocks_to_include.ceil()
                ));
            } else {
                takeaways.push(format!(
                    "Inclusion latency: P50={:.1}s P95={:.1}s P99={:.1}s (~{:.0} blocks to confirm).",
                    load.inclusion_latency.p50,
                    load.inclusion_latency.p95,
                    load.inclusion_latency.p99,
                    blocks_to_include.ceil()
                ));
            }
        }
    }

    // 7. RPC health
    if report.rpc_latency_avg_ms > 0.0 {
        if report.rpc_latency_avg_ms < 200.0 {
            takeaways.push(format!(
                "RPC healthy: {:.0}ms avg latency.",
                report.rpc_latency_avg_ms
            ));
        } else if report.rpc_latency_avg_ms < 1000.0 {
            takeaways.push(format!(
                "RPC moderate load: {:.0}ms avg latency. Consider scaling RPC if testing higher TPS.",
                report.rpc_latency_avg_ms
            ));
        } else {
            takeaways.push(format!(
                "RPC under pressure: {:.0}ms avg latency. RPC may be limiting measured throughput.",
                report.rpc_latency_avg_ms
            ));
        }
    }

    // 8. Bottleneck summary
    let bottlenecks: Vec<&str> = [
        (report.bottleneck.gas_bottleneck, "Gas limit"),
        (report.bottleneck.consensus_bottleneck, "Consensus/validators"),
        (report.bottleneck.rpc_bottleneck, "RPC node"),
    ]
    .iter()
    .filter(|(active, _)| *active)
    .map(|(_, name)| *name)
    .collect();

    if bottlenecks.is_empty() {
        takeaways.push("No bottlenecks detected. Chain operating within safe limits.".to_string());
    } else {
        takeaways.push(format!(
            "BOTTLENECK(S) DETECTED: {}.",
            bottlenecks.join(", ")
        ));
    }

    // 9. Saturation point
    if let Some(sat) = report.saturation_point_tps {
        takeaways.push(format!(
            "Saturation point: {:.0} RPS. Safe operating limit: {:.0} RPS (80%).",
            sat, sat * 0.8
        ));
    }

    // Print all takeaways
    for (i, takeaway) in takeaways.iter().enumerate() {
        println!("  {}. {}", i + 1, takeaway);
    }
    println!();
}

/// Write final report as JSON
pub fn write_json(path: &str, report: &FinalReport) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Write block data as CSV
pub fn write_csv(path: &str, blocks: &[BlockData]) -> Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record([
        "height",
        "timestamp",
        "tx_count",
        "gas_used",
        "gas_limit",
        "block_time",
        "size_bytes",
    ])?;

    for block in blocks {
        writer.write_record(&[
            block.height.to_string(),
            format!("{:.3}", block.timestamp),
            block.tx_count.to_string(),
            block.gas_used.to_string(),
            block.gas_limit.to_string(),
            format!("{:.3}", block.block_time),
            block.size_bytes.map(|s| s.to_string()).unwrap_or_default(),
        ])?;
    }

    writer.flush()?;
    Ok(())
}

/// Format gas values for display (e.g., 10000000 -> 10.0M)
fn format_gas(gas: u64) -> String {
    if gas >= 1_000_000_000 {
        format!("{:.1}B", gas as f64 / 1_000_000_000.0)
    } else if gas >= 1_000_000 {
        format!("{:.1}M", gas as f64 / 1_000_000.0)
    } else if gas >= 1_000 {
        format!("{:.1}K", gas as f64 / 1_000.0)
    } else {
        gas.to_string()
    }
}
