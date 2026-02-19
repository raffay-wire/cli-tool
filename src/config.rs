use anyhow::Result;

use crate::rpc::eth::EthRpcClient;
use crate::rpc::tendermint::TendermintRpcClient;
use crate::types::ChainConfig;

/// Fetch chain configuration from both Ethereum and Tendermint RPC endpoints.
/// Falls back gracefully if Tendermint RPC is unavailable.
pub async fn fetch_chain_config(
    eth_client: &EthRpcClient,
    tm_client: Option<&TendermintRpcClient>,
) -> Result<ChainConfig> {
    // Always available via EVM RPC
    let chain_id_num = eth_client.chain_id().await?;
    let latest_height = eth_client.block_number().await?;

    // Try Tendermint RPC for consensus params
    let (chain_id_str, max_gas, max_bytes, node_version, block_time_avg) =
        if let Some(tm) = tm_client {
            match tm.status().await {
                Ok(status) => {
                    let consensus = tm.consensus_params(None).await.ok();

                    // Estimate block time from recent blocks
                    let block_time = estimate_block_time_tm(tm, status.latest_height).await;

                    (
                        status.chain_id,
                        consensus.as_ref().map(|c| c.max_gas).unwrap_or(-1),
                        consensus.as_ref().map(|c| c.max_bytes).unwrap_or(-1),
                        status.node_version,
                        block_time,
                    )
                }
                Err(_) => {
                    // Tendermint RPC failed, fall back to EVM-only
                    let block_time = estimate_block_time_eth(eth_client, latest_height).await;
                    (
                        format!("{}", chain_id_num),
                        -1i64,
                        -1i64,
                        "unknown".to_string(),
                        block_time,
                    )
                }
            }
        } else {
            // No Tendermint RPC configured
            let block_time = estimate_block_time_eth(eth_client, latest_height).await;
            (
                format!("{}", chain_id_num),
                -1i64,
                -1i64,
                "unknown".to_string(),
                block_time,
            )
        };

    Ok(ChainConfig {
        chain_id: chain_id_str,
        max_gas,
        max_bytes,
        block_time_avg,
        latest_height,
        node_version,
    })
}

/// Estimate average block time from recent EVM blocks
async fn estimate_block_time_eth(eth_client: &EthRpcClient, latest: u64) -> f64 {
    let sample_size = 10u64;
    let start_height = latest.saturating_sub(sample_size);

    let start_block = eth_client.get_block_by_number(start_height, false).await;
    let end_block = eth_client.get_block_by_number(latest, false).await;

    match (start_block, end_block) {
        (Ok(Some(start)), Ok(Some(end))) => {
            let start_ts = crate::rpc::eth::parse_hex_u64(&start.timestamp).unwrap_or(0);
            let end_ts = crate::rpc::eth::parse_hex_u64(&end.timestamp).unwrap_or(0);
            if end_ts > start_ts && sample_size > 0 {
                (end_ts - start_ts) as f64 / sample_size as f64
            } else {
                2.0 // Default fallback
            }
        }
        _ => 2.0,
    }
}

/// Estimate average block time from Tendermint RPC
async fn estimate_block_time_tm(tm_client: &TendermintRpcClient, latest: u64) -> f64 {
    let sample_size = 10u64;
    let start_height = latest.saturating_sub(sample_size);

    let start_block = tm_client.block(Some(start_height)).await;
    let end_block = tm_client.block(Some(latest)).await;

    match (start_block, end_block) {
        (Ok(start), Ok(end)) => {
            let start_ts = parse_rfc3339_to_unix(&start.time);
            let end_ts = parse_rfc3339_to_unix(&end.time);
            if end_ts > start_ts && sample_size > 0 {
                (end_ts - start_ts) / sample_size as f64
            } else {
                2.0
            }
        }
        _ => 2.0,
    }
}

/// Parse an RFC3339 timestamp string to Unix seconds
fn parse_rfc3339_to_unix(ts: &str) -> f64 {
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.timestamp() as f64 + dt.timestamp_subsec_nanos() as f64 / 1e9)
        .unwrap_or(0.0)
}
