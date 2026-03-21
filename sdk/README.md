# Siput SDK

A high-level, developer-friendly SDK for building dApps, wallets, and blockchain integrations on the Siput blockchain.

## Features

- 🚀 **Easy to Use**: Abstract away blockchain complexity
- 🔐 **Wallet Connect**: Multiple wallet connection methods
- ⚡ **Transaction Builder**: Fluent API for building transactions
- 📡 **Event Listening**: Real-time blockchain event subscriptions
- 🌐 **Multi-Language**: Rust and JavaScript implementations
- 📚 **Well Documented**: Comprehensive examples and guides

## Quick Start

### JavaScript

```javascript
import { SiputSDK, Wallet, TransactionBuilder } from 'siput-sdk';

// Initialize SDK
const sdk = new SiputSDK('http://localhost:8080');

// Connect a wallet
const walletInfo = await sdk.connectWallet({ create: true });
console.log('Wallet created:', walletInfo.address);

// Send tokens
const txHash = await sdk.sendTokens('spt1234567890123456789012345678901234567890', 1000);
console.log('Transaction sent:', txHash);

// Listen for balance changes
sdk.onBalanceChange((oldBalance, newBalance) => {
    console.log(`Balance changed: ${oldBalance} -> ${newBalance}`);
});

// Clean up when done
sdk.cleanup();
```

### Rust

```rust
use siput_sdk::{SiputSDK, WalletConnector, EnhancedTransactionBuilder, EnhancedEventListener};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize SDK
    let sdk = Arc::new(SiputSDK::new("http://localhost:8080"));

    // Connect wallet
    let connector = WalletConnector::new(Arc::clone(&sdk));
    let wallet = connector.create_wallet().await?;
    println!("Wallet created: {:?}", wallet.address);

    // Send tokens
    let tx_hash = sdk.send_tokens(wallet.address, 1000).await?;
    println!("Transaction sent: {}", tx_hash);

    // Listen for balance changes
    let event_listener = EnhancedEventListener::new(Arc::clone(&sdk));
    event_listener.on_balance_change(wallet.address, |old, new| {
        println!("Balance changed: {} -> {}", old, new);
    }).await?;

    Ok(())
}
```

## Installation

### JavaScript (NPM)

```bash
npm install siput-sdk
```

### JavaScript (Browser)

```html
<script src="https://cdn.jsdelivr.net/npm/siput-sdk/dist/siput-sdk.js"></script>
```

### Rust

Add to your `Cargo.toml`:

```toml
[dependencies]
siput-sdk = "0.1.0"
```

## Wallet Connection

### Create New Wallet

```javascript
// JavaScript
const walletInfo = await sdk.connectWallet({ create: true });
console.log('Address:', walletInfo.address);
console.log('Mnemonic:', walletInfo.mnemonic);
```

```rust
// Rust
let connector = WalletConnector::new(sdk);
let wallet = connector.create_wallet().await?;
```

### Connect with Mnemonic

```javascript
// JavaScript
await sdk.connectWallet({
    mnemonic: "your twelve word mnemonic phrase here",
    password: "optional password"
});
```

```rust
// Rust
connector.connect_with_mnemonic("mnemonic phrase", Some("password")).await?;
```

### Connect with Private Key

```javascript
// JavaScript
await sdk.connectWallet({
    privateKey: "0x1234567890abcdef..."
});
```

```rust
// Rust
connector.connect_with_private_key("hex_private_key").await?;
```

## Transactions

### Send Tokens

```javascript
// JavaScript
const txHash = await sdk.sendTokens(recipientAddress, amount);
```

```rust
// Rust
let tx_hash = sdk.send_tokens(recipient_address, amount).await?;
```

### Deploy Contract

```javascript
// JavaScript
const wasmCode = new Uint8Array([...]); // Your compiled WASM
const initArgs = new Uint8Array([...]); // Initialization arguments

const txHash = await sdk.deployContract(wasmCode, initArgs);
```

```rust
// Rust
let tx_hash = sdk.deploy_contract(wasm_code, init_args).await?;
```

### Call Contract

```javascript
// JavaScript
const contractAddress = "spt1234567890123456789012345678901234567890";
const txHash = await sdk.callContract(contractAddress, "transfer", args);
```

```rust
// Rust
let tx_hash = sdk.call_contract(contract_address, "transfer", args).await?;
```

### Transaction Builder (Fluent API)

```javascript
// JavaScript
const builder = new TransactionBuilder(sdk);

const txHash = await builder
    .to(recipientAddress)
    .amount(1000)
    .gasLimit(50000)
    .send();
```

```rust
// Rust
let tx_hash = EnhancedTransactionBuilder::new(sdk)
    .transfer(recipient_address, 1000)
    .gas_limit(50000).await?
    .build_sign_and_send()
    .await?;
```

## Event Listening

### New Blocks

```javascript
// JavaScript
const unsubscribe = sdk.onNewBlocks((block, height) => {
    console.log('New block:', height, block.hash);
});

// Later: unsubscribe();
```

```rust
// Rust
sdk.on_new_blocks(|block, height| {
    async move {
        println!("New block: {} - {:?}", height, block.hash);
    }
}).await?;
```

### Transaction Confirmations

```javascript
// JavaScript
sdk.onTransactionConfirmations((tx, blockHash) => {
    console.log('Transaction confirmed:', tx.hash);
});
```

```rust
// Rust
sdk.on_transaction_confirmations(|tx, block_hash| {
    async move {
        println!("Transaction confirmed: {:?}", tx.hash());
    }
}).await?;
```

### Balance Changes

```javascript
// JavaScript
sdk.onBalanceChange((oldBalance, newBalance) => {
    console.log(`Balance: ${oldBalance} -> ${newBalance}`);
});
```

```rust
// Rust
let event_listener = EnhancedEventListener::new(sdk);
event_listener.on_balance_change(wallet.address, |old, new| {
    println!("Balance changed: {} -> {}", old, new);
}).await?;
```

### Incoming/Outgoing Transactions

```javascript
// JavaScript
// Incoming transactions
sdk.onIncomingTransactions((tx, amount) => {
    console.log(`Received ${amount} tokens`);
});

// Outgoing transactions
sdk.onOutgoingTransactions((tx, amount) => {
    console.log(`Sent ${amount} tokens`);
});
```

```rust
// Rust
event_listener.on_incoming_transaction(|tx, amount| {
    async move {
        println!("Received {} tokens", amount);
    }
}).await?;
```

## Advanced Usage

### Custom RPC Calls

```javascript
// JavaScript
const balance = await sdk.rpcCall('get_balance', {
    address: 'spt1234567890123456789012345678901234567890'
});
```

### Wallet Management

```javascript
// JavaScript
const wallet = sdk.wallet; // Access underlying wallet
const exported = wallet.export();
```

```rust
// Rust
let wallet = sdk.get_wallet().await.unwrap();
let mnemonic = wallet.to_mnemonic(None)?;
```

### Error Handling

```javascript
// JavaScript
try {
    await sdk.sendTokens(address, amount);
} catch (error) {
    if (error.message.includes('Insufficient balance')) {
        console.log('Not enough funds');
    } else {
        console.error('Transaction failed:', error);
    }
}
```

```rust
// Rust
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
- `new(rpcUrl?: string)` - Create SDK instance

#### Wallet Methods
- `connectWallet(options)` - Connect wallet
- `disconnectWallet()` - Disconnect wallet
- `getAddress()` - Get connected wallet address
- `isWalletConnected()` - Check connection status

#### Transaction Methods
- `getBalance(address?)` - Get account balance
- `sendTokens(to, amount)` - Send tokens
- `deployContract(wasmCode, initArgs?)` - Deploy contract
- `callContract(address, method, args?)` - Call contract method
- `getTransaction(txHash)` - Get transaction by hash
- `getBlock(blockHash)` - Get block by hash

#### Event Methods
- `onNewBlocks(callback)` - Subscribe to new blocks
- `onTransactionConfirmations(callback)` - Subscribe to confirmations
- `onBalanceChange(callback)` - Subscribe to balance changes
- `onIncomingTransactions(callback)` - Subscribe to incoming txs
- `onOutgoingTransactions(callback)` - Subscribe to outgoing txs

#### Utility Methods
- `rpcCall(method, params?)` - Low-level RPC call
- `cleanup()` - Clean up subscriptions

### Wallet

#### Static Methods
- `create()` - Create new random wallet
- `fromMnemonic(mnemonic, password?)` - Create from mnemonic
- `fromPrivateKey(privateKey)` - Create from private key

#### Instance Methods
- `signTransaction(tx)` - Sign transfer transaction
- `signDeployTransaction(tx)` - Sign deploy transaction
- `signCallTransaction(tx)` - Sign call transaction
- `export()` - Export wallet data

### TransactionBuilder

#### Chainable Methods
- `to(address)` - Set recipient
- `amount(amount)` - Set transfer amount
- `contract(address)` - Set contract address
- `method(method)` - Set contract method
- `args(args)` - Set contract arguments
- `gasLimit(limit)` - Set gas limit
- `gasPrice(price)` - Set gas price

#### Action Methods
- `estimateGas()` - Estimate gas cost
- `send()` - Build and send transaction

## Examples

See the `examples/` directory for complete working examples:

- `examples/javascript/wallet-connect/` - Wallet connection demo
- `examples/javascript/transaction-builder/` - Transaction building
- `examples/javascript/event-listening/` - Event subscription
- `examples/rust/simple-wallet/` - Basic Rust wallet
- `examples/rust/contract-interaction/` - Contract interaction

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

MIT License - see LICENSE file for details