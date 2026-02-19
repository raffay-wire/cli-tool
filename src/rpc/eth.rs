use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

/// Ethereum JSON-RPC client for Cosmos EVM chains
#[derive(Debug, Clone)]
pub struct EthRpcClient {
    url: String,
    client: reqwest::Client,
    request_id: std::sync::Arc<AtomicU64>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
    #[allow(dead_code)]
    id: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RPC error {}: {}", self.code, self.message)
    }
}

/// Block data returned from eth_getBlockByNumber
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthBlock {
    pub number: String,
    pub timestamp: String,
    pub transactions: Vec<Value>, // Can be tx hashes or full tx objects
    #[serde(rename = "gasUsed")]
    pub gas_used: String,
    #[serde(rename = "gasLimit")]
    pub gas_limit: String,
    pub size: Option<String>,
    pub hash: Option<String>,
}

impl EthRpcClient {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
            request_id: std::sync::Arc::new(AtomicU64::new(1)),
        }
    }

    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::Relaxed)
    }

    async fn call<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T> {
        let body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": self.next_id(),
        });

        let resp = self
            .client
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("Failed to call {}", method))?;

        let text = resp.text().await?;
        let parsed: JsonRpcResponse<T> =
            serde_json::from_str(&text).with_context(|| {
                format!("Failed to parse response for {}: {}", method, &text[..text.len().min(200)])
            })?;

        if let Some(err) = parsed.error {
            anyhow::bail!("{}", err);
        }

        parsed
            .result
            .ok_or_else(|| anyhow::anyhow!("No result in response for {}", method))
    }

    /// Get the latest block number
    pub async fn block_number(&self) -> Result<u64> {
        let hex: String = self.call("eth_blockNumber", json!([])).await?;
        parse_hex_u64(&hex)
    }

    /// Get block by number. If `full_txs` is true, returns full transaction objects.
    pub async fn get_block_by_number(&self, height: u64, full_txs: bool) -> Result<Option<EthBlock>> {
        let hex_height = format!("0x{:x}", height);
        let result: Option<EthBlock> = self
            .call("eth_getBlockByNumber", json!([hex_height, full_txs]))
            .await
            .ok();
        Ok(result)
    }

    /// Get the latest block
    pub async fn get_latest_block(&self) -> Result<EthBlock> {
        self.call("eth_getBlockByNumber", json!(["latest", false]))
            .await
    }

    /// Get chain ID
    pub async fn chain_id(&self) -> Result<u64> {
        let hex: String = self.call("eth_chainId", json!([])).await?;
        parse_hex_u64(&hex)
    }

    /// Get current gas price in wei
    pub async fn gas_price(&self) -> Result<u64> {
        let hex: String = self.call("eth_gasPrice", json!([])).await?;
        parse_hex_u64(&hex)
    }

    /// Get transaction count (nonce) for an address
    pub async fn get_transaction_count(&self, address: &str) -> Result<u64> {
        let hex: String = self
            .call("eth_getTransactionCount", json!([address, "pending"]))
            .await?;
        parse_hex_u64(&hex)
    }

    /// Send a raw signed transaction
    pub async fn send_raw_transaction(&self, raw_tx: &str) -> Result<String> {
        self.call("eth_sendRawTransaction", json!([raw_tx])).await
    }

    /// Get transaction receipt
    pub async fn get_transaction_receipt(&self, tx_hash: &str) -> Result<Option<Value>> {
        let result: Option<Value> = self
            .call("eth_getTransactionReceipt", json!([tx_hash]))
            .await
            .ok();
        Ok(result)
    }

    /// Get account balance
    pub async fn get_balance(&self, address: &str) -> Result<u64> {
        let hex: String = self
            .call("eth_getBalance", json!([address, "latest"]))
            .await?;
        parse_hex_u64(&hex)
    }

    /// Try to get txpool status (may not be supported)
    pub async fn txpool_status(&self) -> Result<TxPoolStatus> {
        let result: Value = self.call("txpool_status", json!([])).await?;

        let pending = result
            .get("pending")
            .and_then(|v| v.as_str())
            .map(|s| parse_hex_u64(s).unwrap_or(0))
            .unwrap_or(0);

        let queued = result
            .get("queued")
            .and_then(|v| v.as_str())
            .map(|s| parse_hex_u64(s).unwrap_or(0))
            .unwrap_or(0);

        Ok(TxPoolStatus { pending, queued })
    }
}

#[derive(Debug, Clone)]
pub struct TxPoolStatus {
    pub pending: u64,
    pub queued: u64,
}

/// Parse a hex string (with or without 0x prefix) to u64
pub fn parse_hex_u64(hex: &str) -> Result<u64> {
    let stripped = hex.strip_prefix("0x").unwrap_or(hex);
    u64::from_str_radix(stripped, 16).with_context(|| format!("Failed to parse hex: {}", hex))
}
