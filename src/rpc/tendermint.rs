use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

/// Tendermint/CometBFT RPC client
#[derive(Debug, Clone)]
pub struct TendermintRpcClient {
    url: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct TmRpcResponse<T> {
    result: Option<T>,
    error: Option<TmRpcError>,
}

#[derive(Debug, Deserialize)]
struct TmRpcError {
    message: String,
    data: Option<String>,
}

impl std::fmt::Display for TmRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tendermint RPC error: {}", self.message)?;
        if let Some(ref data) = self.data {
            write!(f, " ({})", data)?;
        }
        Ok(())
    }
}

/// Status response from /status endpoint
#[derive(Debug, Clone)]
pub struct TmStatus {
    pub chain_id: String,
    pub latest_height: u64,
    pub latest_block_time: String,
    pub node_version: String,
}

/// Consensus parameters from /consensus_params endpoint
#[derive(Debug, Clone)]
pub struct ConsensusParams {
    pub max_gas: i64,
    pub max_bytes: i64,
}

/// Block info from /block endpoint
#[derive(Debug, Clone)]
pub struct TmBlock {
    pub height: u64,
    pub time: String,
    pub num_txs: u64,
}

impl TendermintRpcClient {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let url = format!("{}{}", self.url, path);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to call {}", url))?;

        let text = resp.text().await?;
        let parsed: Value = serde_json::from_str(&text)
            .with_context(|| format!("Failed to parse Tendermint response: {}", &text[..text.len().min(200)]))?;

        // Check for error
        if let Some(err) = parsed.get("error") {
            if !err.is_null() {
                let msg = err
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                anyhow::bail!("Tendermint RPC error: {}", msg);
            }
        }

        parsed
            .get("result")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No result in Tendermint response"))
    }

    /// Fetch node status
    pub async fn status(&self) -> Result<TmStatus> {
        let result = self.get("/status").await?;

        let node_info = result
            .get("node_info")
            .ok_or_else(|| anyhow::anyhow!("Missing node_info in status"))?;

        let sync_info = result
            .get("sync_info")
            .ok_or_else(|| anyhow::anyhow!("Missing sync_info in status"))?;

        let chain_id = node_info
            .get("network")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let node_version = node_info
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let latest_height = sync_info
            .get("latest_block_height")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let latest_block_time = sync_info
            .get("latest_block_time")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(TmStatus {
            chain_id,
            latest_height,
            latest_block_time,
            node_version,
        })
    }

    /// Fetch consensus parameters
    pub async fn consensus_params(&self, height: Option<u64>) -> Result<ConsensusParams> {
        let path = match height {
            Some(h) => format!("/consensus_params?height={}", h),
            None => "/consensus_params".to_string(),
        };

        let result = self.get(&path).await?;

        let params = result
            .get("consensus_params")
            .ok_or_else(|| anyhow::anyhow!("Missing consensus_params"))?;

        let block_params = params
            .get("block")
            .ok_or_else(|| anyhow::anyhow!("Missing block params"))?;

        let max_gas = block_params
            .get("max_gas")
            .and_then(|v| v.as_str().or_else(|| v.as_i64().map(|_| "")).and_then(|s| if s.is_empty() { v.as_i64() } else { s.parse().ok() }))
            .unwrap_or(-1);

        let max_bytes = block_params
            .get("max_bytes")
            .and_then(|v| v.as_str().or_else(|| v.as_i64().map(|_| "")).and_then(|s| if s.is_empty() { v.as_i64() } else { s.parse().ok() }))
            .unwrap_or(-1);

        Ok(ConsensusParams { max_gas, max_bytes })
    }

    /// Fetch block at a given height
    pub async fn block(&self, height: Option<u64>) -> Result<TmBlock> {
        let path = match height {
            Some(h) => format!("/block?height={}", h),
            None => "/block".to_string(),
        };

        let result = self.get(&path).await?;

        let block = result
            .get("block")
            .ok_or_else(|| anyhow::anyhow!("Missing block in response"))?;

        let header = block
            .get("header")
            .ok_or_else(|| anyhow::anyhow!("Missing header in block"))?;

        let height = header
            .get("height")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let time = header
            .get("time")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let num_txs = block
            .get("data")
            .and_then(|d| d.get("txs"))
            .and_then(|t| t.as_array())
            .map(|a| a.len() as u64)
            .unwrap_or(0);

        Ok(TmBlock {
            height,
            time,
            num_txs,
        })
    }

    /// Fetch block results (for gas info from Cosmos side)
    pub async fn block_results(&self, height: u64) -> Result<Value> {
        let path = format!("/block_results?height={}", height);
        self.get(&path).await
    }
}
