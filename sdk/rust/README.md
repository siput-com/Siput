# Siput SDK (Rust)

A high-level Rust SDK for building dApps, wallets, and blockchain integrations on the Siput blockchain.

## Features

- 🚀 **Easy to Use**: Abstract away blockchain complexity
- 🔐 **Wallet Connect**: Multiple wallet connection methods
- ⚡ **Transaction Builder**: Fluent API for building transactions
- 📡 **Event Listening**: Real-time blockchain event subscriptions
- 🏗️ **Type Safe**: Full Rust type safety and async/await support
- 📚 **Well Documented**: Comprehensive examples and guides

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
siput-sdk = "0.1.0"
```

Or for local development:

```toml
[dependencies]
siput-sdk = { path = "/path/to/siput/sdk/rust" }
```

## Quick Start

```rust
use siput_sdk::{SiputSDK, WalletConnector};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize SDK
    let sdk = Arc::new(SiputSDK::new("http://localhost:8080"));

    // Connect wallet
    let connector = WalletConnector::new(Arc::clone(&sdk));
    let wallet = connector.create_wallet().await?;
    sdk.connect_wallet(wallet).await?;

    // Send tokens
    let tx_hash = sdk.send_tokens(recipient_address, 1000).await?;
    println!("Transaction sent: {}", tx_hash);

    // Listen for balance changes
    let event_listener = EnhancedEventListener::new(Arc::clone(&sdk));
    event_listener.on_balance_change(wallet.address, |old, new| {
        println!("Balance changed: {} -> {}", old, new);
    }).await?;

    Ok(())
}
```

## Wallet Connection

### Create New Wallet

```rust
let connector = WalletConnector::new(sdk);
let wallet = connector.create_wallet().await?;
```

### Connect with Mnemonic

```rust
connector.connect_with_mnemonic("your mnemonic phrase", Some("password")).await?;
```

### Connect with Private Key

```rust
connector.connect_with_private_key("hex_private_key").await?;
```

## Transactions

### Send Tokens

```rust
let tx_hash = sdk.send_tokens(recipient_address, amount).await?;
```

### Deploy Contract

```rust
let tx_hash = sdk.deploy_contract(wasm_bytecode, init_args).await?;
```

### Call Contract

```rust
let tx_hash = sdk.call_contract(contract_address, "method_name", args).await?;
```

### Transaction Builder (Fluent API)

```rust
use siput_sdk::EnhancedTransactionBuilder;

let tx_hash = EnhancedTransactionBuilder::new(sdk)
    .transfer(recipient_address, 1000)
    .gas_limit(50000).await?
    .build_sign_and_send()
    .await?;
```

## Event Listening

### New Blocks

```rust
sdk.on_new_blocks(|block, height| async move {
    println!("New block: {} - {:?}", height, block.hash);
}).await?;
```

### Transaction Confirmations

```rust
sdk.on_transaction_confirmations(|tx, block_hash| async move {
    println!("Transaction confirmed: {:?}", tx.hash());
}).await?;
```

### Balance Changes

```rust
let event_listener = EnhancedEventListener::new(sdk);
event_listener.on_balance_change(wallet.address, |old, new| {
    println!("Balance changed: {} -> {}", old, new);
}).await?;
```

### Incoming/Outgoing Transactions

```rust
event_listener.on_incoming_transaction(|tx, amount| async move {
    println!("Received {} tokens", amount);
}).await?;
```

## Advanced Usage

### Custom RPC Calls

```rust
// Access underlying client for advanced operations
let client = sdk.client();
let balance_response = client.get_balance(address).await?;
```

### Wallet Management

```rust
// Export wallet data
let mnemonic = connector.export_mnemonic(None).await?;

// Access connected wallet
if let Some(wallet) = sdk.get_wallet().await {
    let address = wallet.address;
    let private_key_hex = wallet.export_private_key_hex();
}
```

### Error Handling

```rust
use siput_sdk::SdkError;

match sdk.send_tokens(address, amount).await {
    Ok(tx_hash) => println!("Success: {}", tx_hash),
    Err(SdkError::WalletError(msg)) if msg.contains("balance") => {
        println!("Insufficient balance");
    }
    Err(e) => eprintln!("Error: {:?}", e),
}
```

## API Reference

### SiputSDK

#### Constructor
- `new(rpc_url: &str) -> Self` - Create SDK instance

#### Wallet Methods
- `connect_wallet(wallet: Wallet) -> Result<(), SdkError>` - Connect wallet
- `disconnect_wallet() -> Result<(), SdkError>` - Disconnect wallet
- `get_address() -> Result<Address, SdkError>` - Get connected wallet address
- `is_wallet_connected() -> bool` - Check connection status
- `get_wallet() -> Option<Wallet>` - Get connected wallet

#### Transaction Methods
- `get_balance(address: Option<Address>) -> Result<u64, SdkError>` - Get account balance
- `send_tokens(to: Address, amount: u64) -> Result<String, SdkError>` - Send tokens
- `deploy_contract(wasm_code: Vec<u8>, init_args: Vec<u8>) -> Result<String, SdkError>` - Deploy contract
- `call_contract(contract_address: Address, method: &str, args: Vec<u8>) -> Result<String, SdkError>` - Call contract method
- `get_transaction(tx_hash: &str) -> Result<Option<Transaction>, SdkError>` - Get transaction by hash
- `get_block(block_hash: &str) -> Result<Option<Block>, SdkError>` - Get block by hash

#### Event Methods
- `on_new_blocks<F>(callback: F) -> Result<(), SdkError>` - Subscribe to new blocks
- `on_transaction_confirmations<F>(callback: F) -> Result<(), SdkError>` - Subscribe to confirmations
- `on_contract_events<F>(callback: F) -> Result<(), SdkError>` - Subscribe to contract events

#### Utility Methods
- `client() -> &Arc<Client>` - Get underlying RPC client
- `events() -> &Arc<SdkEventEmitter>` - Get event emitter

### WalletConnector

#### Methods
- `new(sdk: Arc<SiputSDK>) -> Self` - Create connector
- `create_wallet() -> Result<Wallet, SdkError>` - Create new wallet
- `connect_with_mnemonic(mnemonic: &str, password: Option<&str>) -> Result<(), SdkError>` - Connect with mnemonic
- `connect_with_private_key(private_key_hex: &str) -> Result<(), SdkError>` - Connect with private key
- `export_mnemonic(password: Option<&str>) -> Result<String, SdkError>` - Export mnemonic

### EnhancedTransactionBuilder

#### Chainable Methods
- `new(sdk: Arc<SiputSDK>) -> Self` - Create builder
- `from_connected_wallet() -> Result<Self, SdkError>` - Set sender from connected wallet
- `transfer(to: Address, amount: u64) -> Self` - Transfer tokens
- `deploy_contract(wasm_code: Vec<u8>, init_args: Vec<u8>) -> Self` - Deploy contract
- `call_contract(contract_address: Address, method: String, args: Vec<u8>) -> Self` - Call contract
- `gas_limit(gas_limit: Option<u64>) -> Result<Self, SdkError>` - Set gas limit
- `gas_price(gas_price: u64) -> Self` - Set gas price
- `nonce() -> Result<Self, SdkError>` - Auto-set nonce

#### Action Methods
- `estimate_gas() -> Result<u64, SdkError>` - Estimate gas cost
- `build() -> Result<Transaction, SdkError>` - Build transaction
- `build_and_sign() -> Result<Transaction, SdkError>` - Build and sign
- `build_sign_and_send() -> Result<String, SdkError>` - Build, sign, and send

## Examples

See the `examples/` directory for complete working examples:

- `examples/rust/simple-wallet/` - Basic wallet operations
- `examples/rust/contract-interaction/` - Contract deployment and calls

Run examples:

```bash
# Simple wallet
cargo run --manifest-path examples/rust/simple-wallet/Cargo.toml

# Contract interaction
cargo run --manifest-path examples/rust/contract-interaction/Cargo.toml
```

## Testing

```bash
cargo test
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

MIT License - see LICENSE file for details