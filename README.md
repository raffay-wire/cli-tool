# TPS-CLI

Production-grade TPS measurement tool for Cosmos-based EVM chains (CometBFT consensus + EVM module).

TPS is always calculated from **confirmed blocks only** — never from sent transaction counts — ensuring accurate, trustworthy measurements.

## Features

- **Passive Monitoring** — measure real network TPS without sending any transactions
- **Active Load Testing** — generate transactions with multi-key architecture (one wallet per thread, zero nonce contention)
- **Bottleneck Detection** — automatically identifies gas, consensus, and RPC bottlenecks
- **Multiple Test Modes** — theoretical max, saturation ramp, and sustained stability tests
- **Key Takeaways** — auto-generated human-readable insights after every run
- **Export** — JSON and CSV output formats
- **Dual RPC** — supports both Ethereum JSON-RPC and Tendermint RPC

## Quickstart

### Prerequisites

- [Rust](https://rustup.rs/) (1.70+)

### Build

```bash
git clone https://github.com/raffay-wire/cli-tool.git
cd cli-tool
cargo build --release
```

The binary will be at `target/release/tps-cli`.

### Quick Test

```bash
# Check chain configuration
./target/release/tps-cli config --rpc https://your-evm-rpc.com

# Monitor TPS for 30 seconds
./target/release/tps-cli monitor --rpc https://your-evm-rpc.com --duration 30

# Calculate theoretical max TPS
./target/release/tps-cli test theoretical --rpc https://your-evm-rpc.com
```

## Commands

### `monitor` — Passive Monitoring

Polls blocks and calculates TPS in real-time without sending transactions.

```bash
tps-cli monitor --rpc <URL> [OPTIONS]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--rpc` | *required* | Ethereum JSON-RPC endpoint URL |
| `--tendermint-rpc` | auto-derived | Tendermint RPC endpoint URL |
| `--duration` | `60` | Monitoring duration in seconds |
| `--output` | `table` | Output format: `table`, `json`, `csv` |
| `--output-file` | — | File path for JSON/CSV export |
| `--window` | `10` | Rolling window size in blocks |

**Example:**

```bash
tps-cli monitor --rpc https://evm.example.com --duration 120 --output json --output-file report.json
```

### `load` — Active Load Testing

Generates transactions from multiple wallets and measures confirmed TPS from blocks.

```bash
tps-cli load --rpc <URL> --key <PRIVATE_KEY> [OPTIONS]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--rpc` | *required* | Ethereum JSON-RPC endpoint URL |
| `--key` | *required* | Private key (hex, no 0x prefix) |
| `--rps` | `100` | Target transactions per second |
| `--threads` | `10` | Concurrent sender threads (one wallet per thread) |
| `--duration` | `60` | Test duration in seconds |
| `--mode` | `parallel` | Load mode: `sequential`, `parallel`, `batch`, `adaptive` |
| `--tx-type` | `transfer` | Transaction type: `transfer`, `contract` |
| `--gas-limit` | `21000` | Gas limit per transaction |
| `--gas-price` | `0` (auto) | Gas price in gwei (0 = auto-detect) |
| `--chain-id` | auto | Chain ID (auto-detected if not set) |
| `--output` | `table` | Output format: `table`, `json`, `csv` |
| `--output-file` | — | File path for export |

**Example:**

```bash
tps-cli load \
  --rpc https://evm.example.com \
  --key abc123...def \
  --rps 50 \
  --threads 10 \
  --duration 60
```

The tool automatically:
1. Creates one wallet per thread from random keys
2. Funds each wallet from your master key
3. Runs load with zero nonce contention
4. Measures confirmed TPS from blocks only

### `config` — Chain Configuration

Displays chain parameters fetched from RPC.

```bash
tps-cli config --rpc <URL> [--tendermint-rpc <URL>]
```

**Output includes:** Chain ID, latest height, node version, max gas, max bytes, average block time.

### `test theoretical` — Theoretical Max TPS

Calculates the maximum possible TPS from chain parameters.

```bash
tps-cli test theoretical --rpc <URL> [--avg-gas <GAS>]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--avg-gas` | `21000` | Average gas per transaction |

**Formula:** `max_gas_per_block / avg_gas_per_tx / block_time`

### `test saturation` — Saturation Ramp Test

Ramps up load from a starting RPS until the chain breaks, finding the saturation point.

```bash
tps-cli test saturation --rpc <URL> --key <PRIVATE_KEY> [OPTIONS]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--start-rps` | `10` | Starting RPS |
| `--step` | `10` | RPS increment per step |
| `--step-duration` | `30` | Seconds per step |

**Example:**

```bash
tps-cli test saturation \
  --rpc https://evm.example.com \
  --key abc123...def \
  --start-rps 10 \
  --step 20 \
  --step-duration 30
```

### `test sustained` — Sustained Stability Test

Runs at 80% of theoretical max (or a custom target) for an extended period to test stability.

```bash
tps-cli test sustained --rpc <URL> --key <PRIVATE_KEY> [OPTIONS]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--duration` | `300` | Test duration in seconds |
| `--target-tps` | `0` (auto) | Target TPS (0 = auto-detect 80% of max) |

## Output

Every command prints:

1. **Live Block Table** — height, tx count, block time, instant/rolling TPS, gas usage
2. **Summary Table** — aggregated metrics (avg/peak/sustained/finalized TPS, block time stats, gas utilization, RPC latency)
3. **Bottleneck Analysis** — detected gas, consensus, or RPC bottlenecks
4. **Key Takeaways** — plain-English insights about chain performance

### Example Output (Key Takeaways)

```
=== Key Takeaways ===
  1. EXCELLENT confirmation rate: 100.0% (4033/4033 tx confirmed, 0 drops)
  2. Sustained TPS: 24.6 | Peak: 44.8 | Finalized: 22.3
  3. GAS SATURATED: 100.0% avg utilization. Chain is at max throughput.
  4. Block time STABLE: 5.50s avg (stddev 0.50s). Consensus healthy.
  5. FAST inclusion: P50=5.2s P95=8.1s P99=10.3s (tx land in ~1 block).
  6. RPC healthy: 122ms avg latency.
  7. BOTTLENECK(S) DETECTED: Gas limit.
```

## TPS Methodology

| Metric | Formula |
|--------|---------|
| **Instant TPS** | `tx_in_block / block_time` |
| **Rolling TPS** | `sum(tx) / (last_timestamp - first_timestamp)` over N blocks |
| **Sustained TPS** | Same formula over all observed blocks |
| **Finalized TPS** | Only counts blocks at `height <= latest - 2` |
| **Peak TPS** | Maximum instant TPS observed |

## License

MIT
