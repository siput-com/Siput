# Siput SDK (JavaScript)

A high-level JavaScript SDK for building dApps, wallets, and blockchain integrations on the Siput blockchain.

## Features

- 🚀 **Easy to Use**: Abstract away blockchain complexity
- 🔐 **Wallet Connect**: Multiple wallet connection methods
- ⚡ **Transaction Builder**: Fluent API for building transactions
- 📡 **Event Listening**: Real-time blockchain event subscriptions
- 🌐 **Universal**: Works in Node.js and browsers
- 📚 **Well Documented**: Comprehensive examples and guides

## Installation

### NPM

```bash
npm install siput-sdk
```

### CDN (Browser)

```html
<script src="https://cdn.jsdelivr.net/npm/siput-sdk/dist/siput-sdk.js"></script>
```

### Local Development

```bash
git clone https://github.com/siput-com/Siput.git
cd Siput/sdk/javascript
npm install
```

## Quick Start

### Node.js

```javascript
const { SiputSDK } = require('siput-sdk');

async function main() {
    // Initialize SDK
    const sdk = new SiputSDK('http://localhost:8080');

    // Connect wallet
    const walletInfo = await sdk.connectWallet({ create: true });
    console.log('Wallet created:', walletInfo.address);

    // Send tokens
    const txHash = await sdk.sendTokens(recipientAddress, 1000);
    console.log('Transaction sent:', txHash);

    // Listen for balance changes
    sdk.onBalanceChange((oldBalance, newBalance) => {
        console.log(`Balance: ${oldBalance} -> ${newBalance}`);
    });

    // Clean up when done
    sdk.cleanup();
}

main().catch(console.error);
```

### Browser

```html
<!DOCTYPE html>
<html>
<head>
    <title>Siput dApp</title>
    <script src="https://cdn.jsdelivr.net/npm/siput-sdk/dist/siput-sdk.js"></script>
</head>
<body>
    <h1>My Siput dApp</h1>
    <button onclick="connectWallet()">Connect Wallet</button>
    <div id="status"></div>

    <script>
        const sdk = new SiputSDK('http://localhost:8080');

        async function connectWallet() {
            try {
                const walletInfo = await sdk.connectWallet({ create: true });
                document.getElementById('status').textContent =
                    `Connected: ${walletInfo.address}`;
            } catch (error) {
                document.getElementById('status').textContent =
                    `Error: ${error.message}`;
            }
        }
    </script>
</body>
</html>
```

## Wallet Connection

### Create New Wallet

```javascript
const walletInfo = await sdk.connectWallet({ create: true });
console.log('Address:', walletInfo.address);
console.log('Mnemonic:', walletInfo.mnemonic);
```

### Connect with Mnemonic

```javascript
await sdk.connectWallet({
    mnemonic: "your twelve word mnemonic phrase here",
    password: "optional password"
});
```

### Connect with Private Key

```javascript
await sdk.connectWallet({
    privateKey: "0x1234567890abcdef..."
});
```

## Transactions

### Send Tokens

```javascript
const txHash = await sdk.sendTokens(recipientAddress, amount);
```

### Deploy Contract

```javascript
const wasmCode = new Uint8Array([...]); // Your compiled WASM
const initArgs = new Uint8Array([...]); // Initialization arguments

const txHash = await sdk.deployContract(wasmCode, initArgs);
```

### Call Contract

```javascript
const contractAddress = "spt1234567890123456789012345678901234567890";
const txHash = await sdk.callContract(contractAddress, "transfer", args);
```

### Transaction Builder (Fluent API)

```javascript
const { TransactionBuilder } = require('siput-sdk');

const builder = new TransactionBuilder(sdk);

const txHash = await builder
    .to(recipientAddress)
    .amount(1000)
    .gasLimit(50000)
    .send();
```

## Event Listening

### New Blocks

```javascript
const unsubscribe = sdk.onNewBlocks((block, height) => {
    console.log('New block:', height, block.hash);
});

// Later: unsubscribe();
```

### Balance Changes

```javascript
sdk.onBalanceChange((oldBalance, newBalance) => {
    console.log(`Balance: ${oldBalance} -> ${newBalance}`);
});
```

### Incoming Transactions

```javascript
sdk.onIncomingTransactions((tx, amount) => {
    console.log(`Received ${amount} tokens`);
});
```

### Outgoing Transactions

```javascript
sdk.onOutgoingTransactions((tx, amount) => {
    console.log(`Sent ${amount} tokens`);
});
```

### Transaction Confirmations

```javascript
sdk.onTransactionConfirmations((tx, blockHash) => {
    console.log('Transaction confirmed:', tx.hash);
});
```

## Advanced Usage

### Custom RPC Calls

```javascript
const balance = await sdk.rpcCall('get_balance', {
    address: 'spt1234567890123456789012345678901234567890'
});
```

### Wallet Management

```javascript
const wallet = sdk.wallet; // Access underlying wallet
const exported = wallet.export();
```

### Error Handling

```javascript
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

Run examples:

```bash
# Wallet connect
node examples/javascript/wallet-connect/index.js

# Transaction builder
node examples/javascript/transaction-builder/index.js

# Event listening
node examples/javascript/event-listening/index.js
```

## Browser Compatibility

- Chrome 60+
- Firefox 55+
- Safari 12+
- Edge 79+

## Node.js Compatibility

- Node.js 14+

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

MIT License - see LICENSE file for details