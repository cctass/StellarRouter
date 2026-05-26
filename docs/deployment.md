# Deployment Guide

Step-by-step instructions for deploying the stellar-router suite to Stellar testnet or mainnet.

---

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | stable | `rustup install stable` |
| wasm32 target | — | `rustup target add wasm32-unknown-unknown` |
| Stellar CLI | latest | `cargo install --locked stellar-cli` |
| Funded account | — | See [Friendbot](#funding-a-testnet-account) |

---

## Testnet vs Mainnet

| | Testnet | Mainnet |
|---|---|---|
| Network passphrase | `Test SDF Network ; September 2015` | `Public Global Stellar Network ; September 2015` |
| RPC URL | `https://soroban-testnet.stellar.org` | `https://mainnet.stellar.validationcloud.io/v1/<key>` |
| Fund account | Friendbot (free) | Real XLM required |
| Risk | None | Real funds at stake |
| Recommended for | Development, testing | Production only |

**Always deploy and test on testnet before mainnet.**

---

## Funding a Testnet Account

```bash
# Generate a new keypair
stellar keys generate --global admin --network testnet

# Fund via Friendbot
stellar keys fund admin --network testnet

# Verify balance
stellar account show admin --network testnet
```

---

## Build WASM Artifacts

```bash
cargo build --target wasm32-unknown-unknown --release
```

Artifacts will be at:
```
target/wasm32-unknown-unknown/release/router_core.wasm
target/wasm32-unknown-unknown/release/router_registry.wasm
target/wasm32-unknown-unknown/release/router_access.wasm
target/wasm32-unknown-unknown/release/router_middleware.wasm
target/wasm32-unknown-unknown/release/router_timelock.wasm
target/wasm32-unknown-unknown/release/router_multicall.wasm
```

---

## Deployment Order

Deploy in this order. Each contract is independent but the initialization
order matters for your integration:

```
1. router-registry   (no dependencies)
2. router-access     (no dependencies)
3. router-middleware (no dependencies)
4. router-timelock   (no dependencies)
5. router-multicall  (no dependencies)
6. router-core       (logically depends on the others, deploy last)
```

---

## Step-by-Step Deployment

Replace `<NETWORK>` with `testnet` or `mainnet` and `<ACCOUNT>` with your key name.

### 1. Deploy router-registry

```bash
REGISTRY_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/router_registry.wasm \
  --network <NETWORK> --source <ACCOUNT>)
echo "registry: $REGISTRY_ID"

stellar contract invoke --id $REGISTRY_ID --network <NETWORK> --source <ACCOUNT> \
  -- initialize --admin <ADMIN_ADDRESS>
```

### 2. Deploy router-access

```bash
ACCESS_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/router_access.wasm \
  --network <NETWORK> --source <ACCOUNT>)
echo "access: $ACCESS_ID"

stellar contract invoke --id $ACCESS_ID --network <NETWORK> --source <ACCOUNT> \
  -- initialize --super_admin <ADMIN_ADDRESS>
```

### 3. Deploy router-middleware

```bash
MIDDLEWARE_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/router_middleware.wasm \
  --network <NETWORK> --source <ACCOUNT>)
echo "middleware: $MIDDLEWARE_ID"

stellar contract invoke --id $MIDDLEWARE_ID --network <NETWORK> --source <ACCOUNT> \
  -- initialize --admin <ADMIN_ADDRESS>
```

### 4. Deploy router-timelock

```bash
TIMELOCK_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/router_timelock.wasm \
  --network <NETWORK> --source <ACCOUNT>)
echo "timelock: $TIMELOCK_ID"

# min_delay in seconds — use 86400 (24h) for mainnet, 3600 (1h) for testnet
stellar contract invoke --id $TIMELOCK_ID --network <NETWORK> --source <ACCOUNT> \
  -- initialize --admin <ADMIN_ADDRESS> --min_delay 86400
```

### 5. Deploy router-multicall

```bash
MULTICALL_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/router_multicall.wasm \
  --network <NETWORK> --source <ACCOUNT>)
echo "multicall: $MULTICALL_ID"

stellar contract invoke --id $MULTICALL_ID --network <NETWORK> --source <ACCOUNT> \
  -- initialize --admin <ADMIN_ADDRESS> --max_batch_size 10
```

### 6. Deploy router-core

```bash
CORE_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/router_core.wasm \
  --network <NETWORK> --source <ACCOUNT>)
echo "core: $CORE_ID"

stellar contract invoke --id $CORE_ID --network <NETWORK> --source <ACCOUNT> \
  -- initialize --admin <ADMIN_ADDRESS>
```

---

## Post-Deployment Verification

```bash
# Verify router-core is initialized
stellar contract invoke --id $CORE_ID --network <NETWORK> --source <ACCOUNT> \
  -- admin

# Register a test route
stellar contract invoke --id $CORE_ID --network <NETWORK> --source <ACCOUNT> \
  -- register_route \
  --caller <ADMIN_ADDRESS> --name test --address $REGISTRY_ID

# Resolve it
stellar contract invoke --id $CORE_ID --network <NETWORK> --source <ACCOUNT> \
  -- resolve --name test
```

---

## Environment Variables Reference

For the metrics exporter and api-server:

| Variable | Default | Description |
|---|---|---|
| `SOROBAN_RPC_URL` | `https://soroban-testnet.stellar.org` | Soroban RPC endpoint |
| `ROUTER_CORE_CONTRACT_ID` | — | Deployed router-core contract ID |
| `ROUTER_REGISTRY_CONTRACT_ID` | — | Deployed router-registry contract ID |
| `ROUTER_ACCESS_CONTRACT_ID` | — | Deployed router-access contract ID |
| `ROUTER_MIDDLEWARE_CONTRACT_ID` | — | Deployed router-middleware contract ID |
| `ROUTER_TIMELOCK_CONTRACT_ID` | — | Deployed router-timelock contract ID |
| `ROUTER_MULTICALL_CONTRACT_ID` | — | Deployed router-multicall contract ID |
| `ROUTER_AUTH_ENABLED` | `false` | Enable API key auth on the api-server |
| `ROUTER_API_KEY` | — | API key (required if auth enabled) |
| `ROUTER_REPLAY_PROTECTION_ENABLED` | `false` | Enable nonce-based replay protection |
| `LISTEN_ADDR` | `127.0.0.1:8080` | api-server listen address |
| `RUST_LOG` | `info` | Log level |

---

## Docker Compose (Local Development)

```bash
# Start metrics exporter + Prometheus + Grafana
docker compose up

# Run tests only
docker compose run tests

# Build WASM artifacts
docker compose run wasm
```

Prometheus: http://localhost:9091  
Grafana: http://localhost:3000

---

## Mainnet Checklist

Before deploying to mainnet:

- [ ] All contracts tested on testnet with production-like data
- [ ] Admin keypair is a hardware wallet or multi-sig account
- [ ] `min_delay` in router-timelock set to at least 24 hours (86400)
- [ ] All contract IDs recorded and backed up
- [ ] Monitoring set up (metrics exporter + alerting rules)
- [ ] `initialize()` called on every contract before registering routes
- [ ] Test route registered and resolved successfully

---

## Troubleshooting

**`Error: contract not found`**  
The contract ID is wrong or the contract was not deployed to this network. Verify with `stellar contract inspect --id <ID> --network <NETWORK>`.

**`Error: not initialized`**  
`initialize()` was not called after deployment. Call it before any other function.

**`Error: unauthorized`**  
The `--source` account does not match the admin address set during `initialize()`. Use the same account that initialized the contract.

**`Error: insufficient funds`**  
The source account does not have enough XLM to pay transaction fees. Fund it via Friendbot (testnet) or transfer XLM (mainnet).

**`Error: simulation failed`**  
The transaction would fail on-chain. Check that all arguments are correct and the contract is initialized. Run with `--verbose` for more detail.

**Contract ID starts with `G` instead of `C`**  
You are using an account address instead of a contract ID. Contract IDs always start with `C`.
