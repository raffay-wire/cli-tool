use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use tokio::sync::Mutex;

use crate::metrics;
use crate::rpc::eth::EthRpcClient;
use crate::types::*;

/// Load generator that creates and sends transactions to measure confirmed TPS.
/// Uses one wallet per thread to eliminate nonce contention.
pub struct LoadGenerator {
    master_signer: alloy_signer_local::PrivateKeySigner,
    master_address: String,
    chain_id: u64,
    gas_limit: u64,
    gas_price: u64,
    tx_type: TxType,
}

/// Per-thread worker wallet
struct WorkerWallet {
    signer: alloy_signer_local::PrivateKeySigner,
    address: String,
    nonce: AtomicU64,
}

impl LoadGenerator {
    pub fn new(
        _eth_client: EthRpcClient,
        key: String,
        chain_id: u64,
        gas_limit: u64,
        gas_price: u64,
        tx_type: TxType,
    ) -> Result<Self> {
        let key_hex = key.strip_prefix("0x").unwrap_or(&key);
        let signer: alloy_signer_local::PrivateKeySigner = key_hex
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid private key: {}", e))?;
        let address = format!("{:?}", signer.address());
        println!("Master address: {}", address);

        Ok(Self {
            master_signer: signer,
            master_address: address,
            chain_id,
            gas_limit,
            gas_price,
            tx_type,
        })
    }

    /// Generate N random worker wallets
    fn generate_workers(&self, count: u64) -> Vec<WorkerWallet> {
        let mut workers = Vec::with_capacity(count as usize);
        for i in 0..count {
            let signer = alloy_signer_local::PrivateKeySigner::random();
            let address = format!("{:?}", signer.address());
            println!("  Worker {} address: {}", i, address);
            workers.push(WorkerWallet {
                signer,
                address,
                nonce: AtomicU64::new(0),
            });
        }
        workers
    }

    /// Fund worker wallets from the master key.
    /// Sends enough ETH for each worker to cover `tx_per_worker` transactions.
    /// Uses a higher gas price for funding to avoid "replacement underpriced" errors.
    async fn fund_workers(
        &self,
        workers: &[WorkerWallet],
        tx_per_worker: u64,
        eth_client: &EthRpcClient,
    ) -> Result<()> {
        use alloy_consensus::TxLegacy;
        use alloy_consensus::transaction::RlpEcdsaTx;
        use alloy_network::TxSignerSync;
        use alloy_primitives::{hex, Address, TxKind, U256};

        // Re-fetch gas price fresh and use 2x premium for funding txs to avoid
        // "replacement underpriced" if old pending txs exist in the mempool
        let funding_gas_price = eth_client.gas_price().await?.max(self.gas_price) * 2;

        // Wait a moment for any pending txs from previous runs to clear
        println!("\nWaiting 10s for mempool to settle...");
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

        let mut master_nonce = eth_client
            .get_transaction_count(&self.master_address)
            .await?;

        // Each worker needs: gas_price * gas_limit * tx_per_worker (plus margin)
        let fund_per_worker =
            U256::from(self.gas_price) * U256::from(self.gas_limit) * U256::from(tx_per_worker + 10);

        println!(
            "Funding {} workers with ~{} wei each (gas price: {} wei)...",
            workers.len(),
            fund_per_worker,
            funding_gas_price,
        );

        // Send funding txs in batches to avoid overwhelming the RPC
        let batch_size = 5;
        let mut fund_hashes = Vec::new();

        for (i, worker) in workers.iter().enumerate() {
            let to_bytes: [u8; 20] = worker.signer.address().into();
            let to = Address::from(to_bytes);

            let mut tx = TxLegacy {
                chain_id: Some(self.chain_id),
                nonce: master_nonce,
                gas_price: funding_gas_price as u128,
                gas_limit: 21000,
                to: TxKind::Call(to),
                value: fund_per_worker,
                input: Default::default(),
            };

            let sig = self.master_signer.sign_transaction_sync(&mut tx)?;
            let mut buf = Vec::new();
            tx.rlp_encode_signed(&sig, &mut buf);
            let raw = format!("0x{}", hex::encode(&buf));

            match eth_client.send_raw_transaction(&raw).await {
                Ok(hash) => {
                    fund_hashes.push((hash, worker.address.clone()));
                    master_nonce += 1;
                }
                Err(e) => {
                    // Try once more with an even higher nonce (skip the stuck one)
                    tracing::warn!("Funding attempt failed for worker {}: {}, retrying...", i, e);
                    master_nonce += 1;

                    let mut tx2 = TxLegacy {
                        chain_id: Some(self.chain_id),
                        nonce: master_nonce,
                        gas_price: funding_gas_price as u128 * 2,
                        gas_limit: 21000,
                        to: TxKind::Call(to),
                        value: fund_per_worker,
                        input: Default::default(),
                    };
                    let sig2 = self.master_signer.sign_transaction_sync(&mut tx2)?;
                    let mut buf2 = Vec::new();
                    tx2.rlp_encode_signed(&sig2, &mut buf2);
                    let raw2 = format!("0x{}", hex::encode(&buf2));

                    match eth_client.send_raw_transaction(&raw2).await {
                        Ok(hash) => {
                            fund_hashes.push((hash, worker.address.clone()));
                            master_nonce += 1;
                        }
                        Err(e2) => {
                            anyhow::bail!("Failed to fund worker {} after retry: {}", worker.address, e2);
                        }
                    }
                }
            }

            // Pace the sends to avoid RPC saturation
            if (i + 1) % batch_size == 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }
        }

        // Wait for all funding transactions to confirm
        println!("Waiting for {} funding transactions to confirm...", fund_hashes.len());
        let mut confirmed = 0;
        for (hash, addr) in &fund_hashes {
            match wait_receipt(eth_client, hash, 120).await {
                Ok(true) => {
                    confirmed += 1;
                }
                Ok(false) => {
                    anyhow::bail!("Funding tx reverted for worker {}", addr);
                }
                Err(e) => {
                    anyhow::bail!("Funding tx timeout for worker {}: {}", addr, e);
                }
            }
        }
        println!("All {} workers funded successfully.\n", confirmed);

        Ok(())
    }

    /// Run the load generator and return metrics.
    /// Generates one wallet per thread, funds them, then each thread sends independently.
    pub async fn run(
        &self,
        target_rps: u64,
        threads: u64,
        duration: u64,
        mode: LoadMode,
        eth_client: &EthRpcClient,
    ) -> Result<LoadTestMetrics> {
        let tx_sent = Arc::new(AtomicU64::new(0));
        let tx_confirmed = Arc::new(AtomicU64::new(0));
        let tx_failed = Arc::new(AtomicU64::new(0));
        let tx_reverted = Arc::new(AtomicU64::new(0));
        let inclusion_latencies = Arc::new(Mutex::new(Vec::<f64>::new()));

        // Generate and fund worker wallets
        let effective_threads = match mode {
            LoadMode::Sequential => 1,
            _ => threads,
        };

        let tx_per_worker = (target_rps * duration / effective_threads).max(100);
        println!("Generating {} worker wallets...", effective_threads);
        let workers = self.generate_workers(effective_threads);
        self.fund_workers(&workers, tx_per_worker, eth_client).await?;

        let workers: Vec<Arc<WorkerWallet>> = workers.into_iter().map(Arc::new).collect();

        let start = Instant::now();

        match mode {
            LoadMode::Sequential => {
                self.run_sequential_multi(
                    target_rps,
                    duration,
                    eth_client,
                    &workers[0],
                    &tx_sent,
                    &tx_confirmed,
                    &tx_failed,
                    &tx_reverted,
                    &inclusion_latencies,
                )
                .await?;
            }
            LoadMode::Parallel | LoadMode::Batch => {
                self.run_parallel_multi(
                    target_rps,
                    duration,
                    eth_client,
                    &workers,
                    &tx_sent,
                    &tx_confirmed,
                    &tx_failed,
                    &tx_reverted,
                    &inclusion_latencies,
                )
                .await?;
            }
            LoadMode::Adaptive => {
                self.run_adaptive_multi(
                    target_rps,
                    duration,
                    eth_client,
                    &workers,
                    &tx_sent,
                    &tx_confirmed,
                    &tx_failed,
                    &tx_reverted,
                    &inclusion_latencies,
                )
                .await?;
            }
        }

        let elapsed = start.elapsed().as_secs_f64();
        let sent = tx_sent.load(Ordering::Relaxed);
        let confirmed = tx_confirmed.load(Ordering::Relaxed);
        let failed = tx_failed.load(Ordering::Relaxed);
        let reverted = tx_reverted.load(Ordering::Relaxed);
        let dropped = sent.saturating_sub(confirmed + failed + reverted);

        let lats = inclusion_latencies.lock().await;
        let inclusion = metrics::compute_inclusion_latency(&lats);

        let drop_rate = if sent > 0 {
            dropped as f64 / sent as f64
        } else {
            0.0
        };
        let revert_rate = if confirmed > 0 {
            reverted as f64 / confirmed as f64
        } else {
            0.0
        };

        println!(
            "\nLoad test complete: {:.1}s | sent={} confirmed={} failed={} dropped={} reverted={}",
            elapsed, sent, confirmed, failed, dropped, reverted
        );

        Ok(LoadTestMetrics {
            tx_sent: sent,
            tx_confirmed: confirmed,
            tx_failed: failed,
            tx_dropped: dropped,
            tx_reverted: reverted,
            revert_rate,
            drop_rate,
            inclusion_latency: inclusion,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_sequential_multi(
        &self,
        target_rps: u64,
        duration: u64,
        eth_client: &EthRpcClient,
        worker: &Arc<WorkerWallet>,
        tx_sent: &Arc<AtomicU64>,
        tx_confirmed: &Arc<AtomicU64>,
        tx_failed: &Arc<AtomicU64>,
        tx_reverted: &Arc<AtomicU64>,
        inclusion_latencies: &Arc<Mutex<Vec<f64>>>,
    ) -> Result<()> {
        let interval = std::time::Duration::from_secs_f64(1.0 / target_rps as f64);
        let start = Instant::now();

        while start.elapsed().as_secs() < duration {
            let nonce = worker.nonce.fetch_add(1, Ordering::SeqCst);
            let send_time = Instant::now();

            let raw_tx = build_transfer_tx(
                &worker.signer,
                nonce,
                self.chain_id,
                self.gas_limit,
                self.gas_price,
            );

            match raw_tx {
                Ok(raw) => match eth_client.send_raw_transaction(&raw).await {
                    Ok(tx_hash) => {
                        tx_sent.fetch_add(1, Ordering::Relaxed);

                        let confirmed_result = wait_receipt(eth_client, &tx_hash, 30).await;
                        let latency = send_time.elapsed().as_secs_f64();

                        match confirmed_result {
                            Ok(true) => {
                                tx_confirmed.fetch_add(1, Ordering::Relaxed);
                                inclusion_latencies.lock().await.push(latency);
                            }
                            Ok(false) => {
                                tx_reverted.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(_) => {
                                tx_failed.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                    Err(_) => {
                        tx_failed.fetch_add(1, Ordering::Relaxed);
                    }
                },
                Err(_) => {
                    tx_failed.fetch_add(1, Ordering::Relaxed);
                }
            }

            tokio::time::sleep(interval).await;
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_parallel_multi(
        &self,
        target_rps: u64,
        duration: u64,
        eth_client: &EthRpcClient,
        workers: &[Arc<WorkerWallet>],
        tx_sent: &Arc<AtomicU64>,
        tx_confirmed: &Arc<AtomicU64>,
        tx_failed: &Arc<AtomicU64>,
        tx_reverted: &Arc<AtomicU64>,
        inclusion_latencies: &Arc<Mutex<Vec<f64>>>,
    ) -> Result<()> {
        let threads = workers.len() as u64;
        let rps_per_thread = (target_rps as f64 / threads as f64).ceil() as u64;

        let mut handles = Vec::new();

        for worker in workers {
            let client = eth_client.clone();
            let worker = worker.clone();
            let sent = tx_sent.clone();
            let confirmed = tx_confirmed.clone();
            let failed = tx_failed.clone();
            let reverted = tx_reverted.clone();
            let latencies = inclusion_latencies.clone();
            let chain_id = self.chain_id;
            let gas_limit = self.gas_limit;
            let gas_price = self.gas_price;

            let handle = tokio::spawn(async move {
                let interval =
                    std::time::Duration::from_secs_f64(1.0 / rps_per_thread.max(1) as f64);
                let start = Instant::now();

                while start.elapsed().as_secs() < duration {
                    let nonce = worker.nonce.fetch_add(1, Ordering::SeqCst);
                    let send_time = Instant::now();

                    let raw_tx = build_transfer_tx(
                        &worker.signer,
                        nonce,
                        chain_id,
                        gas_limit,
                        gas_price,
                    );

                    match raw_tx {
                        Ok(raw) => match client.send_raw_transaction(&raw).await {
                            Ok(tx_hash) => {
                                sent.fetch_add(1, Ordering::Relaxed);

                                let client2 = client.clone();
                                let confirmed2 = confirmed.clone();
                                let failed2 = failed.clone();
                                let reverted2 = reverted.clone();
                                let latencies2 = latencies.clone();

                                tokio::spawn(async move {
                                    match wait_receipt(&client2, &tx_hash, 60).await {
                                        Ok(true) => {
                                            confirmed2.fetch_add(1, Ordering::Relaxed);
                                            let lat = send_time.elapsed().as_secs_f64();
                                            latencies2.lock().await.push(lat);
                                        }
                                        Ok(false) => {
                                            reverted2.fetch_add(1, Ordering::Relaxed);
                                        }
                                        Err(_) => {
                                            failed2.fetch_add(1, Ordering::Relaxed);
                                        }
                                    }
                                });
                            }
                            Err(_) => {
                                failed.fetch_add(1, Ordering::Relaxed);
                            }
                        },
                        Err(_) => {
                            failed.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    tokio::time::sleep(interval).await;
                }
            });

            handles.push(handle);
        }

        for h in handles {
            let _ = h.await;
        }

        // Wait for trailing receipts
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_adaptive_multi(
        &self,
        target_rps: u64,
        duration: u64,
        eth_client: &EthRpcClient,
        workers: &[Arc<WorkerWallet>],
        tx_sent: &Arc<AtomicU64>,
        tx_confirmed: &Arc<AtomicU64>,
        tx_failed: &Arc<AtomicU64>,
        tx_reverted: &Arc<AtomicU64>,
        inclusion_latencies: &Arc<Mutex<Vec<f64>>>,
    ) -> Result<()> {
        let start_rps = target_rps / 2;
        let ramp_duration = duration / 4;

        println!(
            "Adaptive mode: ramping from {} to {} RPS over {}s",
            start_rps, target_rps, ramp_duration
        );

        let steps = 5u64;
        let step_dur = ramp_duration / steps;
        for i in 0..steps {
            let current_rps = start_rps + (target_rps - start_rps) * (i + 1) / steps;
            println!("  Ramp step {}: {} RPS for {}s", i + 1, current_rps, step_dur);

            self.run_parallel_multi(
                current_rps,
                step_dur,
                eth_client,
                workers,
                tx_sent,
                tx_confirmed,
                tx_failed,
                tx_reverted,
                inclusion_latencies,
            )
            .await?;
        }

        let sustain_dur = duration - ramp_duration;
        println!("  Sustain phase: {} RPS for {}s", target_rps, sustain_dur);
        self.run_parallel_multi(
            target_rps,
            sustain_dur,
            eth_client,
            workers,
            tx_sent,
            tx_confirmed,
            tx_failed,
            tx_reverted,
            inclusion_latencies,
        )
        .await?;

        Ok(())
    }
}

/// Build and sign a simple ETH transfer transaction to a random recipient
fn build_transfer_tx(
    signer: &alloy_signer_local::PrivateKeySigner,
    nonce: u64,
    chain_id: u64,
    gas_limit: u64,
    gas_price: u64,
) -> Result<String> {
    use alloy_consensus::TxLegacy;
    use alloy_consensus::transaction::RlpEcdsaTx;
    use alloy_network::TxSignerSync;
    use alloy_primitives::{hex, Address, TxKind, U256};

    // Generate a random recipient address for each transaction
    let mut addr_bytes = [0u8; 20];
    rand::Rng::fill(&mut rand::thread_rng(), &mut addr_bytes);
    let to = Address::from(addr_bytes);

    let mut tx = TxLegacy {
        chain_id: Some(chain_id),
        nonce,
        gas_price: gas_price as u128,
        gas_limit,
        to: TxKind::Call(to),
        value: U256::ZERO,
        input: Default::default(),
    };

    let sig = signer.sign_transaction_sync(&mut tx)?;
    let mut buf = Vec::new();
    tx.rlp_encode_signed(&sig, &mut buf);

    Ok(format!("0x{}", hex::encode(&buf)))
}

/// Wait for a transaction receipt with timeout
async fn wait_receipt(
    eth_client: &EthRpcClient,
    tx_hash: &str,
    timeout_secs: u64,
) -> Result<bool> {
    let start = Instant::now();
    let poll_interval = tokio::time::Duration::from_millis(500);

    while start.elapsed().as_secs() < timeout_secs {
        if let Ok(Some(receipt)) = eth_client.get_transaction_receipt(tx_hash).await {
            let status = receipt
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("0x1");

            return Ok(status == "0x1");
        }
        tokio::time::sleep(poll_interval).await;
    }

    anyhow::bail!("Receipt timeout for {}", tx_hash)
}
