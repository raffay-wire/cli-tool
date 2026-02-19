mod bottleneck;
mod config;
mod loadgen;
mod metrics;
mod report;
mod rpc;
mod scanner;
mod tps;
mod types;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use types::{LoadMode, OutputFormat, TxType};

#[derive(Parser)]
#[command(name = "tps-cli")]
#[command(about = "Production-grade TPS measurement tool for Cosmos-based EVM chains")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Passive monitoring - measure real network TPS without sending transactions
    Monitor {
        /// Ethereum JSON-RPC endpoint URL
        #[arg(long)]
        rpc: String,

        /// Tendermint RPC endpoint URL (optional, auto-derived if not set)
        #[arg(long)]
        tendermint_rpc: Option<String>,

        /// Monitoring duration in seconds
        #[arg(long, default_value = "60")]
        duration: u64,

        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        output: OutputFormat,

        /// Output file path for JSON/CSV export
        #[arg(long)]
        output_file: Option<String>,

        /// Rolling window size in blocks for TPS calculation
        #[arg(long, default_value = "10")]
        window: usize,
    },

    /// Active load testing - generate transactions and measure confirmed TPS
    Load {
        /// Ethereum JSON-RPC endpoint URL
        #[arg(long)]
        rpc: String,

        /// Tendermint RPC endpoint URL (optional)
        #[arg(long)]
        tendermint_rpc: Option<String>,

        /// Target transactions per second
        #[arg(long, default_value = "100")]
        rps: u64,

        /// Number of concurrent sender threads
        #[arg(long, default_value = "10")]
        threads: u64,

        /// Test duration in seconds
        #[arg(long, default_value = "60")]
        duration: u64,

        /// Transaction type
        #[arg(long, value_enum, default_value = "transfer")]
        tx_type: TxType,

        /// Gas limit per transaction
        #[arg(long, default_value = "21000")]
        gas_limit: u64,

        /// Gas price in gwei (0 = auto)
        #[arg(long, default_value = "0")]
        gas_price: u64,

        /// Load generation mode
        #[arg(long, value_enum, default_value = "parallel")]
        mode: LoadMode,

        /// Private key for signing transactions (hex, no 0x prefix)
        #[arg(long)]
        key: String,

        /// Chain ID (auto-detected if not set)
        #[arg(long)]
        chain_id: Option<u64>,

        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        output: OutputFormat,

        /// Output file path
        #[arg(long)]
        output_file: Option<String>,
    },

    /// Display chain configuration and parameters
    Config {
        /// Ethereum JSON-RPC endpoint URL
        #[arg(long)]
        rpc: String,

        /// Tendermint RPC endpoint URL (optional)
        #[arg(long)]
        tendermint_rpc: Option<String>,
    },

    /// Run specific TPS tests
    Test {
        #[command(subcommand)]
        test_type: TestType,
    },
}

#[derive(Subcommand)]
enum TestType {
    /// Calculate theoretical maximum TPS from chain parameters
    Theoretical {
        /// Ethereum JSON-RPC endpoint URL
        #[arg(long)]
        rpc: String,

        /// Tendermint RPC endpoint URL (optional)
        #[arg(long)]
        tendermint_rpc: Option<String>,

        /// Average gas per transaction (default: 21000 for simple transfer)
        #[arg(long, default_value = "21000")]
        avg_gas: u64,
    },

    /// Saturation ramp test - find the breaking point
    Saturation {
        /// Ethereum JSON-RPC endpoint URL
        #[arg(long)]
        rpc: String,

        /// Tendermint RPC endpoint URL (optional)
        #[arg(long)]
        tendermint_rpc: Option<String>,

        /// Private key for signing transactions
        #[arg(long)]
        key: String,

        /// Starting TPS rate
        #[arg(long, default_value = "10")]
        start_rps: u64,

        /// TPS increment per step
        #[arg(long, default_value = "10")]
        step: u64,

        /// Duration per step in seconds
        #[arg(long, default_value = "30")]
        step_duration: u64,

        /// Chain ID (auto-detected if not set)
        #[arg(long)]
        chain_id: Option<u64>,

        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        output: OutputFormat,

        /// Output file path
        #[arg(long)]
        output_file: Option<String>,
    },

    /// Sustained stability test - run at 80% of detected max
    Sustained {
        /// Ethereum JSON-RPC endpoint URL
        #[arg(long)]
        rpc: String,

        /// Tendermint RPC endpoint URL (optional)
        #[arg(long)]
        tendermint_rpc: Option<String>,

        /// Private key for signing transactions
        #[arg(long)]
        key: String,

        /// Test duration in seconds (default: 300 = 5 minutes)
        #[arg(long, default_value = "300")]
        duration: u64,

        /// Target TPS (0 = auto-detect 80% of max)
        #[arg(long, default_value = "0")]
        target_tps: u64,

        /// Chain ID (auto-detected if not set)
        #[arg(long)]
        chain_id: Option<u64>,

        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        output: OutputFormat,

        /// Output file path
        #[arg(long)]
        output_file: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Monitor {
            rpc,
            tendermint_rpc,
            duration,
            output,
            output_file,
            window,
        } => {
            let tendermint_rpc = resolve_tendermint_rpc(&rpc, tendermint_rpc);
            cmd_monitor::run(rpc, tendermint_rpc, duration, output, output_file, window).await?;
        }
        Commands::Load {
            rpc,
            tendermint_rpc,
            rps,
            threads,
            duration,
            tx_type,
            gas_limit,
            gas_price,
            mode,
            key,
            chain_id,
            output,
            output_file,
        } => {
            let tendermint_rpc = resolve_tendermint_rpc(&rpc, tendermint_rpc);
            cmd_load::run(
                rpc,
                tendermint_rpc,
                rps,
                threads,
                duration,
                tx_type,
                gas_limit,
                gas_price,
                mode,
                key,
                chain_id,
                output,
                output_file,
            )
            .await?;
        }
        Commands::Config {
            rpc,
            tendermint_rpc,
        } => {
            let tendermint_rpc = resolve_tendermint_rpc(&rpc, tendermint_rpc);
            cmd_config::run(rpc, tendermint_rpc).await?;
        }
        Commands::Test { test_type } => match test_type {
            TestType::Theoretical {
                rpc,
                tendermint_rpc,
                avg_gas,
            } => {
                let tendermint_rpc = resolve_tendermint_rpc(&rpc, tendermint_rpc);
                cmd_tests::theoretical(rpc, tendermint_rpc, avg_gas).await?;
            }
            TestType::Saturation {
                rpc,
                tendermint_rpc,
                key,
                start_rps,
                step,
                step_duration,
                chain_id,
                output,
                output_file,
            } => {
                let tendermint_rpc = resolve_tendermint_rpc(&rpc, tendermint_rpc);
                cmd_tests::saturation(
                    rpc,
                    tendermint_rpc,
                    key,
                    start_rps,
                    step,
                    step_duration,
                    chain_id,
                    output,
                    output_file,
                )
                .await?;
            }
            TestType::Sustained {
                rpc,
                tendermint_rpc,
                key,
                duration,
                target_tps,
                chain_id,
                output,
                output_file,
            } => {
                let tendermint_rpc = resolve_tendermint_rpc(&rpc, tendermint_rpc);
                cmd_tests::sustained(
                    rpc,
                    tendermint_rpc,
                    key,
                    duration,
                    target_tps,
                    chain_id,
                    output,
                    output_file,
                )
                .await?;
            }
        },
    }

    Ok(())
}

/// Try to derive Tendermint RPC URL from Ethereum RPC URL if not explicitly provided
fn resolve_tendermint_rpc(eth_rpc: &str, explicit: Option<String>) -> Option<String> {
    if explicit.is_some() {
        return explicit;
    }
    // Common pattern: if EVM RPC is on port 8545, Tendermint is on 26657
    if let Ok(url) = reqwest::Url::parse(eth_rpc) {
        if url.port() == Some(8545) {
            let mut derived = url.clone();
            let _ = derived.set_port(Some(26657));
            return Some(derived.to_string());
        }
    }
    None
}

// ─── Command handler modules ─────────────────────────────────────────────────

mod cmd_monitor {
    use crate::bottleneck;
    use crate::report;
    use crate::rpc::eth::EthRpcClient;
    use crate::rpc::tendermint::TendermintRpcClient;
    use crate::scanner::BlockScanner;
    use crate::tps::TpsEngine;
    use crate::types::{FinalReport, OutputFormat};
    use std::time::Instant;

    pub async fn run(
        rpc: String,
        tendermint_rpc: Option<String>,
        duration: u64,
        output: OutputFormat,
        output_file: Option<String>,
        window: usize,
    ) -> anyhow::Result<()> {
        println!("=== TPS-CLI Passive Monitor ===");
        println!("RPC: {}", rpc);
        if let Some(ref tm) = tendermint_rpc {
            println!("Tendermint RPC: {}", tm);
        }
        println!("Duration: {}s | Rolling window: {} blocks", duration, window);
        println!();

        let eth_client = EthRpcClient::new(&rpc);
        let tm_client = tendermint_rpc.as_deref().map(TendermintRpcClient::new);

        let chain_config = crate::config::fetch_chain_config(&eth_client, tm_client.as_ref()).await;
        if let Ok(ref cfg) = chain_config {
            println!("Chain: {} | Height: {}", cfg.chain_id, cfg.latest_height);
            println!();
        }

        let mut scanner = BlockScanner::new(eth_client.clone(), window * 2);
        let mut tps_engine = TpsEngine::new(window);
        let mut rpc_latencies: Vec<f64> = Vec::new();
        let mut rpc_timeouts: u64 = 0;

        let start = Instant::now();
        let mut last_height: u64 = 0;
        let mut first_block = true;

        report::print_table_header();

        while start.elapsed().as_secs() < duration {
            let poll_start = Instant::now();
            match scanner.poll_latest_block().await {
                Ok(Some(block)) => {
                    let rpc_latency = poll_start.elapsed().as_secs_f64() * 1000.0;
                    rpc_latencies.push(rpc_latency);

                    if first_block {
                        last_height = block.height;
                        first_block = false;
                        tps_engine.add_block(block.clone());
                        report::print_block_row(&block, &tps_engine.compute());
                        continue;
                    }

                    // Backfill any blocks we missed between polls
                    if block.height > last_height + 1 {
                        for h in (last_height + 1)..block.height {
                            if let Ok(Some(missed)) = scanner.fetch_block(h).await {
                                tps_engine.add_block(missed.clone());
                                report::print_block_row(&missed, &tps_engine.compute());
                            }
                        }
                    }

                    if block.height > last_height {
                        last_height = block.height;
                        tps_engine.add_block(block.clone());
                        let metrics = tps_engine.compute();
                        report::print_block_row(&block, &metrics);
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    rpc_timeouts += 1;
                    tracing::warn!("RPC error: {}", e);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        println!();
        println!("=== Monitoring Complete ===");

        let metrics = tps_engine.compute();
        let blocks = tps_engine.blocks();
        let bottleneck_report = bottleneck::analyze(&blocks, &rpc_latencies, rpc_timeouts);

        let rpc_latency_avg = if rpc_latencies.is_empty() {
            0.0
        } else {
            rpc_latencies.iter().sum::<f64>() / rpc_latencies.len() as f64
        };

        let elapsed = start.elapsed().as_secs_f64();
        let total_tx: u64 = blocks.iter().map(|b| b.tx_count).sum();
        let gas_util_avg = avg_gas_utilization(&blocks);

        let final_report = FinalReport {
            chain_id: chain_config
                .map(|c| c.chain_id)
                .unwrap_or_else(|_| "unknown".into()),
            duration_secs: elapsed,
            blocks_scanned: blocks.len() as u64,
            total_transactions: total_tx,
            average_tps: metrics.rolling_tps,
            peak_tps: metrics.peak_tps,
            sustained_tps: metrics.sustained_tps,
            finalized_tps: metrics.finalized_tps,
            block_time_avg: metrics.block_time_avg,
            block_time_variance: metrics.block_time_variance,
            gas_utilization_avg: gas_util_avg,
            bottleneck: bottleneck_report,
            load_test: None,
            inclusion_latency: None,
            rpc_latency_avg_ms: rpc_latency_avg,
            theoretical_max_tps: None,
            saturation_point_tps: None,
        };

        report::print_final_summary(&final_report);
        report::print_bottleneck_analysis(&final_report.bottleneck);
        report::print_key_takeaways(&final_report);

        if let Some(ref path) = output_file {
            match output {
                OutputFormat::Json | OutputFormat::Table => report::write_json(path, &final_report)?,
                OutputFormat::Csv => report::write_csv(path, &blocks)?,
            }
            println!("Report saved to: {}", path);
        }

        Ok(())
    }

    fn avg_gas_utilization(blocks: &[crate::types::BlockData]) -> f64 {
        let vals: Vec<f64> = blocks
            .iter()
            .filter(|b| b.gas_limit > 0)
            .map(|b| b.gas_used as f64 / b.gas_limit as f64)
            .collect();
        if vals.is_empty() { 0.0 } else { vals.iter().sum::<f64>() / vals.len() as f64 }
    }
}

mod cmd_load {
    use crate::bottleneck;
    use crate::loadgen::LoadGenerator;
    use crate::report;
    use crate::rpc::eth::EthRpcClient;
    use crate::rpc::tendermint::TendermintRpcClient;
    use crate::scanner::BlockScanner;
    use crate::tps::TpsEngine;
    use crate::types::*;
    use std::sync::Arc;
    use std::time::Instant;
    use tokio::sync::Mutex;

    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        rpc: String,
        tendermint_rpc: Option<String>,
        rps: u64,
        threads: u64,
        duration: u64,
        tx_type: TxType,
        gas_limit: u64,
        gas_price: u64,
        mode: LoadMode,
        key: String,
        chain_id: Option<u64>,
        output: OutputFormat,
        output_file: Option<String>,
    ) -> anyhow::Result<()> {
        println!("=== TPS-CLI Active Load Test ===");
        println!("RPC: {}", rpc);
        println!(
            "Target RPS: {} | Threads: {} | Duration: {}s",
            rps, threads, duration
        );
        println!("Mode: {:?} | Tx Type: {:?}", mode, tx_type);
        println!();

        let eth_client = EthRpcClient::new(&rpc);
        let tm_client = tendermint_rpc.as_deref().map(TendermintRpcClient::new);

        let chain_id = match chain_id {
            Some(id) => id,
            None => {
                let id = eth_client.chain_id().await?;
                println!("Auto-detected chain ID: {}", id);
                id
            }
        };

        let gas_price = if gas_price == 0 {
            let price = eth_client.gas_price().await?;
            println!("Auto-detected gas price: {} wei", price);
            price
        } else {
            gas_price * 1_000_000_000
        };

        let chain_config =
            crate::config::fetch_chain_config(&eth_client, tm_client.as_ref()).await;
        if let Ok(ref cfg) = chain_config {
            println!("Chain: {} | Height: {}", cfg.chain_id, cfg.latest_height);
        }
        println!();

        let scanner = Arc::new(Mutex::new(BlockScanner::new(eth_client.clone(), 200)));
        let tps_engine = Arc::new(Mutex::new(TpsEngine::new(20)));
        let rpc_latencies = Arc::new(Mutex::new(Vec::<f64>::new()));
        let rpc_timeouts = Arc::new(Mutex::new(0u64));

        // Background block scanner
        let scan_handle = {
            let scanner = scanner.clone();
            let tps_engine = tps_engine.clone();
            let rpc_latencies = rpc_latencies.clone();
            let rpc_timeouts = rpc_timeouts.clone();
            let scan_duration = duration;

            tokio::spawn(async move {
                scan_blocks(scanner, tps_engine, rpc_latencies, rpc_timeouts, scan_duration + 30)
                    .await;
            })
        };

        // Run load generator
        let load_gen = LoadGenerator::new(
            eth_client.clone(),
            key,
            chain_id,
            gas_limit,
            gas_price,
            tx_type,
        )?;

        let load_metrics = load_gen
            .run(rps, threads, duration, mode, &eth_client)
            .await?;

        let _ = scan_handle.await;

        println!();
        println!("=== Load Test Complete ===");

        let metrics = tps_engine.lock().await.compute();
        let blocks = tps_engine.lock().await.blocks();
        let lats = rpc_latencies.lock().await;
        let timeouts = *rpc_timeouts.lock().await;
        let bottleneck_report = bottleneck::analyze(&blocks, &lats, timeouts);

        let rpc_latency_avg = if lats.is_empty() {
            0.0
        } else {
            lats.iter().sum::<f64>() / lats.len() as f64
        };

        let total_tx: u64 = blocks.iter().map(|b| b.tx_count).sum();
        let gas_util_avg = avg_gas_util(&blocks);

        let final_report = FinalReport {
            chain_id: chain_config
                .map(|c| c.chain_id)
                .unwrap_or_else(|_| "unknown".into()),
            duration_secs: duration as f64,
            blocks_scanned: blocks.len() as u64,
            total_transactions: total_tx,
            average_tps: metrics.rolling_tps,
            peak_tps: metrics.peak_tps,
            sustained_tps: metrics.sustained_tps,
            finalized_tps: metrics.finalized_tps,
            block_time_avg: metrics.block_time_avg,
            block_time_variance: metrics.block_time_variance,
            gas_utilization_avg: gas_util_avg,
            bottleneck: bottleneck_report,
            load_test: Some(load_metrics),
            inclusion_latency: None,
            rpc_latency_avg_ms: rpc_latency_avg,
            theoretical_max_tps: None,
            saturation_point_tps: None,
        };

        report::print_final_summary(&final_report);
        report::print_bottleneck_analysis(&final_report.bottleneck);
        report::print_key_takeaways(&final_report);

        if let Some(ref path) = output_file {
            match output {
                OutputFormat::Json | OutputFormat::Table => {
                    report::write_json(path, &final_report)?
                }
                OutputFormat::Csv => report::write_csv(path, &blocks)?,
            }
            println!("Report saved to: {}", path);
        }

        Ok(())
    }

    async fn scan_blocks(
        scanner: Arc<Mutex<BlockScanner>>,
        tps_engine: Arc<Mutex<TpsEngine>>,
        rpc_latencies: Arc<Mutex<Vec<f64>>>,
        rpc_timeouts: Arc<Mutex<u64>>,
        duration: u64,
    ) {
        use crate::report;

        let start = Instant::now();
        let mut last_height: u64 = 0;

        report::print_table_header();

        while start.elapsed().as_secs() < duration {
            let poll_start = Instant::now();
            let result = {
                let mut s = scanner.lock().await;
                s.poll_latest_block().await
            };

            match result {
                Ok(Some(block)) => {
                    let lat = poll_start.elapsed().as_secs_f64() * 1000.0;
                    rpc_latencies.lock().await.push(lat);

                    if block.height > last_height {
                        // Backfill missed blocks
                        if last_height > 0 && block.height > last_height + 1 {
                            for h in (last_height + 1)..block.height {
                                let missed = {
                                    let mut s = scanner.lock().await;
                                    s.fetch_block(h).await
                                };
                                if let Ok(Some(b)) = missed {
                                    let mut eng = tps_engine.lock().await;
                                    eng.add_block(b.clone());
                                    let m = eng.compute();
                                    report::print_block_row(&b, &m);
                                }
                            }
                        }
                        last_height = block.height;
                        let mut eng = tps_engine.lock().await;
                        eng.add_block(block.clone());
                        let m = eng.compute();
                        report::print_block_row(&block, &m);
                    }
                }
                Ok(None) => {}
                Err(_) => {
                    *rpc_timeouts.lock().await += 1;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    fn avg_gas_util(blocks: &[crate::types::BlockData]) -> f64 {
        let vals: Vec<f64> = blocks
            .iter()
            .filter(|b| b.gas_limit > 0)
            .map(|b| b.gas_used as f64 / b.gas_limit as f64)
            .collect();
        if vals.is_empty() {
            0.0
        } else {
            vals.iter().sum::<f64>() / vals.len() as f64
        }
    }
}

mod cmd_config {
    use crate::config;
    use crate::rpc::eth::EthRpcClient;
    use crate::rpc::tendermint::TendermintRpcClient;

    pub async fn run(rpc: String, tendermint_rpc: Option<String>) -> anyhow::Result<()> {
        println!("=== Chain Configuration ===");

        let eth_client = EthRpcClient::new(&rpc);
        let tm_client = tendermint_rpc.as_deref().map(TendermintRpcClient::new);

        let cfg = config::fetch_chain_config(&eth_client, tm_client.as_ref()).await?;

        println!("Chain ID:       {}", cfg.chain_id);
        println!("Latest Height:  {}", cfg.latest_height);
        println!("Node Version:   {}", cfg.node_version);
        println!(
            "Max Gas:        {}",
            if cfg.max_gas < 0 {
                "unlimited".to_string()
            } else {
                format!("{}", cfg.max_gas)
            }
        );
        println!(
            "Max Bytes:      {}",
            if cfg.max_bytes < 0 {
                "unlimited".to_string()
            } else {
                format!("{}", cfg.max_bytes)
            }
        );
        println!("Block Time Avg: {:.2}s", cfg.block_time_avg);

        Ok(())
    }
}

mod cmd_tests {
    use crate::bottleneck;
    use crate::config;
    use crate::loadgen::LoadGenerator;
    use crate::report;
    use crate::rpc::eth::EthRpcClient;
    use crate::rpc::tendermint::TendermintRpcClient;
    use crate::scanner::BlockScanner;
    use crate::tps::TpsEngine;
    use crate::types::*;
    use std::sync::Arc;
    use std::time::Instant;
    use tokio::sync::Mutex;

    pub async fn theoretical(
        rpc: String,
        tendermint_rpc: Option<String>,
        avg_gas: u64,
    ) -> anyhow::Result<()> {
        println!("=== Theoretical Max TPS Test ===");

        let eth_client = EthRpcClient::new(&rpc);
        let tm_client = tendermint_rpc.as_deref().map(TendermintRpcClient::new);

        let cfg = config::fetch_chain_config(&eth_client, tm_client.as_ref()).await?;

        println!("Chain: {}", cfg.chain_id);
        println!("Max Gas per Block: {}", cfg.max_gas);
        println!("Avg Gas per Tx:    {}", avg_gas);
        println!("Block Time:        {:.2}s", cfg.block_time_avg);

        if cfg.max_gas <= 0 {
            println!();
            println!("Max gas is unlimited. Estimating from recent blocks...");

            let mut scanner = BlockScanner::new(eth_client.clone(), 10);
            let latest = eth_client.block_number().await?;
            let mut gas_limits = Vec::new();

            for h in (latest.saturating_sub(9))..=latest {
                if let Ok(Some(block)) = scanner.fetch_block(h).await {
                    gas_limits.push(block.gas_limit);
                }
            }

            if !gas_limits.is_empty() {
                let avg_limit = gas_limits.iter().sum::<u64>() / gas_limits.len() as u64;
                let theoretical = avg_limit as f64 / avg_gas as f64 / cfg.block_time_avg;
                println!("Avg Block Gas Limit: {}", avg_limit);
                println!();
                println!("Theoretical Max TPS: {:.1}", theoretical);
            }
        } else {
            let theoretical = cfg.max_gas as f64 / avg_gas as f64 / cfg.block_time_avg;
            println!();
            println!("Theoretical Max TPS: {:.1}", theoretical);
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn saturation(
        rpc: String,
        tendermint_rpc: Option<String>,
        key: String,
        start_rps: u64,
        step: u64,
        step_duration: u64,
        chain_id: Option<u64>,
        output: OutputFormat,
        output_file: Option<String>,
    ) -> anyhow::Result<()> {
        println!("=== Saturation Ramp Test ===");

        let eth_client = EthRpcClient::new(&rpc);
        let tm_client = tendermint_rpc.as_deref().map(TendermintRpcClient::new);

        let chain_id = match chain_id {
            Some(id) => id,
            None => eth_client.chain_id().await?,
        };

        let gas_price = eth_client.gas_price().await?;
        let cfg = config::fetch_chain_config(&eth_client, tm_client.as_ref()).await?;
        println!(
            "Chain: {} | Start: {} RPS, step +{}, {}s/step",
            cfg.chain_id, start_rps, step, step_duration
        );
        println!();

        let load_gen = LoadGenerator::new(
            eth_client.clone(),
            key,
            chain_id,
            21000,
            gas_price,
            TxType::Transfer,
        )?;

        let scanner = Arc::new(Mutex::new(BlockScanner::new(eth_client.clone(), 200)));
        let tps_engine = Arc::new(Mutex::new(TpsEngine::new(20)));
        let rpc_latencies = Arc::new(Mutex::new(Vec::<f64>::new()));

        let mut current_rps = start_rps;
        let mut saturation_point: Option<f64> = None;
        let mut all_blocks = Vec::new();

        report::print_table_header();

        loop {
            println!("\n--- Ramp: {} RPS ---", current_rps);

            let scanner_clone = scanner.clone();
            let tps_clone = tps_engine.clone();
            let latencies_clone = rpc_latencies.clone();
            let dur = step_duration;

            let scan_handle = tokio::spawn(async move {
                let start = Instant::now();
                let mut last_height: u64 = 0;

                while start.elapsed().as_secs() < dur + 5 {
                    let result = {
                        let mut s = scanner_clone.lock().await;
                        s.poll_latest_block().await
                    };
                    if let Ok(Some(block)) = result {
                        let lat = start.elapsed().as_secs_f64() * 1000.0;
                        latencies_clone.lock().await.push(lat);
                        if block.height > last_height {
                            last_height = block.height;
                            let mut eng = tps_clone.lock().await;
                            eng.add_block(block.clone());
                            let m = eng.compute();
                            report::print_block_row(&block, &m);
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            });

            let load_metrics = load_gen
                .run(
                    current_rps,
                    current_rps.max(4),
                    step_duration,
                    LoadMode::Parallel,
                    &eth_client,
                )
                .await?;

            let _ = scan_handle.await;

            let failure_rate = if load_metrics.tx_sent > 0 {
                (load_metrics.tx_failed + load_metrics.tx_dropped) as f64
                    / load_metrics.tx_sent as f64
            } else {
                0.0
            };

            let current_blocks = tps_engine.lock().await.blocks();
            all_blocks.extend(current_blocks);

            if failure_rate > 0.05 {
                println!(
                    "\nSaturation detected at {} RPS (failure rate: {:.1}%)",
                    current_rps,
                    failure_rate * 100.0
                );
                saturation_point = Some(current_rps as f64);
                break;
            }

            let metrics = tps_engine.lock().await.compute();
            if metrics.block_time_avg > cfg.block_time_avg * 1.5 {
                println!(
                    "\nSaturation detected at {} RPS (block time: {:.2}s vs {:.2}s normal)",
                    current_rps, metrics.block_time_avg, cfg.block_time_avg
                );
                saturation_point = Some(current_rps as f64);
                break;
            }

            current_rps += step;
            if current_rps > 10000 {
                println!("\nReached maximum ramp limit (10000 RPS)");
                break;
            }
        }

        println!();
        println!("=== Saturation Test Complete ===");
        if let Some(sat) = saturation_point {
            println!("Saturation Point: {:.0} RPS", sat);
            println!("Safe Operating TPS: {:.0} RPS (80%)", sat * 0.8);
        }

        let lats = rpc_latencies.lock().await;
        let bottleneck_report = bottleneck::analyze(&all_blocks, &lats, 0);
        let metrics = tps_engine.lock().await.compute();
        let total_tx: u64 = all_blocks.iter().map(|b| b.tx_count).sum();
        let rpc_latency_avg = if lats.is_empty() {
            0.0
        } else {
            lats.iter().sum::<f64>() / lats.len() as f64
        };

        let final_report = FinalReport {
            chain_id: cfg.chain_id,
            duration_secs: 0.0,
            blocks_scanned: all_blocks.len() as u64,
            total_transactions: total_tx,
            average_tps: metrics.rolling_tps,
            peak_tps: metrics.peak_tps,
            sustained_tps: metrics.sustained_tps,
            finalized_tps: metrics.finalized_tps,
            block_time_avg: metrics.block_time_avg,
            block_time_variance: metrics.block_time_variance,
            gas_utilization_avg: 0.0,
            bottleneck: bottleneck_report,
            load_test: None,
            inclusion_latency: None,
            rpc_latency_avg_ms: rpc_latency_avg,
            theoretical_max_tps: None,
            saturation_point_tps: saturation_point,
        };

        report::print_final_summary(&final_report);
        report::print_bottleneck_analysis(&final_report.bottleneck);
        report::print_key_takeaways(&final_report);

        if let Some(ref path) = output_file {
            match output {
                OutputFormat::Json | OutputFormat::Table => {
                    report::write_json(path, &final_report)?
                }
                OutputFormat::Csv => report::write_csv(path, &all_blocks)?,
            }
            println!("Report saved to: {}", path);
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn sustained(
        rpc: String,
        tendermint_rpc: Option<String>,
        key: String,
        duration: u64,
        target_tps: u64,
        chain_id: Option<u64>,
        output: OutputFormat,
        output_file: Option<String>,
    ) -> anyhow::Result<()> {
        println!("=== Sustained Stability Test ===");

        let eth_client = EthRpcClient::new(&rpc);
        let tm_client = tendermint_rpc.as_deref().map(TendermintRpcClient::new);

        let chain_id = match chain_id {
            Some(id) => id,
            None => eth_client.chain_id().await?,
        };

        let gas_price = eth_client.gas_price().await?;
        let cfg = config::fetch_chain_config(&eth_client, tm_client.as_ref()).await?;

        let target = if target_tps == 0 {
            let gas_limit = if cfg.max_gas > 0 {
                cfg.max_gas as u64
            } else {
                let latest = eth_client.block_number().await?;
                let mut scanner = BlockScanner::new(eth_client.clone(), 1);
                if let Ok(Some(block)) = scanner.fetch_block(latest).await {
                    block.gas_limit
                } else {
                    30_000_000
                }
            };
            let theoretical = gas_limit as f64 / 21000.0 / cfg.block_time_avg;
            let target = (theoretical * 0.8) as u64;
            println!(
                "Auto-detected target: {} RPS (80% of {:.0} theoretical)",
                target, theoretical
            );
            target
        } else {
            target_tps
        };

        println!(
            "Chain: {} | Target: {} RPS | Duration: {}s",
            cfg.chain_id, target, duration
        );
        println!();

        let load_gen = LoadGenerator::new(
            eth_client.clone(),
            key,
            chain_id,
            21000,
            gas_price,
            TxType::Transfer,
        )?;

        let scanner = Arc::new(Mutex::new(BlockScanner::new(eth_client.clone(), 500)));
        let tps_engine = Arc::new(Mutex::new(TpsEngine::new(30)));
        let rpc_latencies = Arc::new(Mutex::new(Vec::<f64>::new()));
        let rpc_timeouts = Arc::new(Mutex::new(0u64));

        // Background scanner
        let scan_handle = {
            let scanner = scanner.clone();
            let tps_engine = tps_engine.clone();
            let rpc_latencies = rpc_latencies.clone();
            let rpc_timeouts = rpc_timeouts.clone();
            let scan_dur = duration;

            tokio::spawn(async move {
                let start = Instant::now();
                let mut last_height: u64 = 0;

                report::print_table_header();

                while start.elapsed().as_secs() < scan_dur + 30 {
                    let poll_start = Instant::now();
                    let result = {
                        let mut s = scanner.lock().await;
                        s.poll_latest_block().await
                    };
                    match result {
                        Ok(Some(block)) => {
                            let lat = poll_start.elapsed().as_secs_f64() * 1000.0;
                            rpc_latencies.lock().await.push(lat);
                            if block.height > last_height {
                                if last_height > 0 && block.height > last_height + 1 {
                                    for h in (last_height + 1)..block.height {
                                        let missed = {
                                            let mut s = scanner.lock().await;
                                            s.fetch_block(h).await
                                        };
                                        if let Ok(Some(b)) = missed {
                                            let mut eng = tps_engine.lock().await;
                                            eng.add_block(b.clone());
                                            let m = eng.compute();
                                            report::print_block_row(&b, &m);
                                        }
                                    }
                                }
                                last_height = block.height;
                                let mut eng = tps_engine.lock().await;
                                eng.add_block(block.clone());
                                let m = eng.compute();
                                report::print_block_row(&block, &m);
                            }
                        }
                        Ok(None) => {}
                        Err(_) => {
                            *rpc_timeouts.lock().await += 1;
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            })
        };

        let load_metrics = load_gen
            .run(target, target.max(4), duration, LoadMode::Parallel, &eth_client)
            .await?;

        let _ = scan_handle.await;

        println!();
        println!("=== Sustained Test Complete ===");

        let metrics = tps_engine.lock().await.compute();
        let blocks = tps_engine.lock().await.blocks();
        let lats = rpc_latencies.lock().await;
        let timeouts = *rpc_timeouts.lock().await;
        let bottleneck_report = bottleneck::analyze(&blocks, &lats, timeouts);

        let total_tx: u64 = blocks.iter().map(|b| b.tx_count).sum();
        let rpc_latency_avg = if lats.is_empty() {
            0.0
        } else {
            lats.iter().sum::<f64>() / lats.len() as f64
        };

        let final_report = FinalReport {
            chain_id: cfg.chain_id,
            duration_secs: duration as f64,
            blocks_scanned: blocks.len() as u64,
            total_transactions: total_tx,
            average_tps: metrics.rolling_tps,
            peak_tps: metrics.peak_tps,
            sustained_tps: metrics.sustained_tps,
            finalized_tps: metrics.finalized_tps,
            block_time_avg: metrics.block_time_avg,
            block_time_variance: metrics.block_time_variance,
            gas_utilization_avg: 0.0,
            bottleneck: bottleneck_report,
            load_test: Some(load_metrics),
            inclusion_latency: None,
            rpc_latency_avg_ms: rpc_latency_avg,
            theoretical_max_tps: None,
            saturation_point_tps: None,
        };

        report::print_final_summary(&final_report);
        report::print_bottleneck_analysis(&final_report.bottleneck);
        report::print_key_takeaways(&final_report);

        if let Some(ref path) = output_file {
            match output {
                OutputFormat::Json | OutputFormat::Table => {
                    report::write_json(path, &final_report)?
                }
                OutputFormat::Csv => report::write_csv(path, &blocks)?,
            }
            println!("Report saved to: {}", path);
        }

        Ok(())
    }
}
