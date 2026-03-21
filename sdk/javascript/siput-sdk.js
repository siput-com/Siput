/**
 * Siput SDK - High-Level JavaScript SDK
 *
 * A developer-friendly SDK for building dApps, wallets, and blockchain integrations
 * on the Siput blockchain with wallet connect, transaction building, and event listening.
 */

class SiputSDK {
    constructor(rpcUrl = 'http://localhost:8080') {
        this.rpcUrl = rpcUrl;
        this.wallet = null;
        this.eventListeners = new Map();
        this.isConnected = false;
    }

    /**
     * Connect a wallet using different methods
     */
    async connectWallet(options = {}) {
        if (options.mnemonic) {
            this.wallet = await Wallet.fromMnemonic(options.mnemonic, options.password);
        } else if (options.privateKey) {
            this.wallet = await Wallet.fromPrivateKey(options.privateKey);
        } else if (options.create) {
            this.wallet = await Wallet.create();
            return {
                address: this.wallet.address,
                mnemonic: this.wallet.mnemonic
            };
        } else {
            throw new Error('Must provide mnemonic, privateKey, or create option');
        }

        this.isConnected = true;
        return { address: this.wallet.address };
    }

    /**
     * Disconnect wallet
     */
    disconnectWallet() {
        this.wallet = null;
        this.isConnected = false;
    }

    /**
     * Get wallet address
     */
    getAddress() {
        if (!this.wallet) throw new Error('No wallet connected');
        return this.wallet.address;
    }

    /**
     * Get account balance
     */
    async getBalance(address = null) {
        const addr = address || this.getAddress();
        const response = await this.rpcCall('get_balance', { address: addr });
        return response.balance;
    }

    /**
     * Send tokens to another address
     */
    async sendTokens(to, amount) {
        if (!this.wallet) throw new Error('No wallet connected');

        const balance = await this.getBalance();
        if (balance < amount) {
            throw new Error('Insufficient balance');
        }

        const nonce = await this.getNonce();
        const tx = {
            from: this.wallet.address,
            to,
            amount,
            nonce,
            gasLimit: 21000,
            gasPrice: 1
        };

        const signedTx = await this.wallet.signTransaction(tx);
        const txHash = await this.rpcCall('send_tx', { transaction: signedTx });

        return txHash;
    }

    /**
     * Get transaction nonce
     */
    async getNonce() {
        const balance = await this.getBalance();
        return balance.nonce;
    }

    /**
     * Deploy a smart contract
     */
    async deployContract(wasmCode, initArgs = []) {
        if (!this.wallet) throw new Error('No wallet connected');

        const nonce = await this.getNonce();
        const tx = {
            from: this.wallet.address,
            wasmCode: Array.from(wasmCode),
            initArgs: Array.from(initArgs),
            nonce,
            gasLimit: 1000000,
            gasPrice: 1
        };

        const signedTx = await this.wallet.signDeployTransaction(tx);
        const txHash = await this.rpcCall('deploy_contract', { transaction: signedTx });

        return txHash;
    }

    /**
     * Call a smart contract method
     */
    async callContract(contractAddress, method, args = []) {
        if (!this.wallet) throw new Error('No wallet connected');

        const nonce = await this.getNonce();
        const tx = {
            from: this.wallet.address,
            contractAddress,
            method,
            args: Array.from(args),
            nonce,
            gasLimit: 100000,
            gasPrice: 1
        };

        const signedTx = await this.wallet.signCallTransaction(tx);
        const txHash = await this.rpcCall('call_contract', { transaction: signedTx });

        return txHash;
    }

    /**
     * Get transaction by hash
     */
    async getTransaction(txHash) {
        return await this.rpcCall('get_tx', { hash: txHash });
    }

    /**
     * Get block by hash
     */
    async getBlock(blockHash) {
        return await this.rpcCall('get_block', { hash: blockHash });
    }

    /**
     * Subscribe to new blocks
     */
    onNewBlocks(callback) {
        return this.subscribe('new_blocks', callback);
    }

    /**
     * Subscribe to transaction confirmations
     */
    onTransactionConfirmations(callback) {
        return this.subscribe('transaction_confirmations', callback);
    }

    /**
     * Subscribe to balance changes
     */
    onBalanceChange(callback) {
        if (!this.wallet) throw new Error('No wallet connected');

        let lastBalance = null;
        return this.onNewBlocks(async (block) => {
            const currentBalance = await this.getBalance();
            if (lastBalance !== null && currentBalance !== lastBalance) {
                callback(lastBalance, currentBalance);
            }
            lastBalance = currentBalance;
        });
    }

    /**
     * Subscribe to incoming transactions
     */
    onIncomingTransactions(callback) {
        if (!this.wallet) throw new Error('No wallet connected');

        return this.onTransactionConfirmations((tx) => {
            if (tx.to === this.wallet.address) {
                const amount = this.extractTransferAmount(tx);
                if (amount) callback(tx, amount);
            }
        });
    }

    /**
     * Subscribe to outgoing transactions
     */
    onOutgoingTransactions(callback) {
        if (!this.wallet) throw new Error('No wallet connected');

        return this.onTransactionConfirmations((tx) => {
            if (tx.from === this.wallet.address) {
                const amount = this.extractTransferAmount(tx);
                if (amount) callback(tx, amount);
            }
        });
    }

    /**
     * Extract transfer amount from transaction
     */
    extractTransferAmount(tx) {
        if (tx.payload && tx.payload.type === 'Transfer') {
            return tx.payload.amount;
        }
        return null;
    }

    /**
     * Low-level RPC call
     */
    async rpcCall(method, params = {}) {
        const response = await fetch(`${this.rpcUrl}/v1/blockchain/${method}`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(params)
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || `RPC call failed: ${response.status}`);
        }

        return await response.json();
    }

    /**
     * WebSocket subscription
     */
    subscribe(event, callback) {
        // For now, use polling. In production, implement WebSocket
        const interval = setInterval(async () => {
            try {
                // Poll for new data based on event type
                const data = await this.pollEvent(event);
                if (data) callback(data);
            } catch (error) {
                console.error('Subscription error:', error);
            }
        }, 5000); // Poll every 5 seconds

        const unsubscribe = () => clearInterval(interval);
        this.eventListeners.set(event, unsubscribe);
        return unsubscribe;
    }

    /**
     * Poll for event data (simplified implementation)
     */
    async pollEvent(event) {
        switch (event) {
            case 'new_blocks':
                const dagInfo = await this.rpcCall('dag');
                return dagInfo;
            case 'transaction_confirmations':
                // This would need more complex logic in real implementation
                return null;
            default:
                return null;
        }
    }

    /**
     * Clean up all subscriptions
     */
    cleanup() {
        for (const unsubscribe of this.eventListeners.values()) {
            unsubscribe();
        }
        this.eventListeners.clear();
    }
}

/**
 * Wallet class for key management
 */
class Wallet {
    constructor(privateKey, publicKey, address, mnemonic = null) {
        this.privateKey = privateKey;
        this.publicKey = publicKey;
        this.address = address;
        this.mnemonic = mnemonic;
    }

    /**
     * Create a new random wallet
     */
    static async create() {
        // This would use crypto.randomBytes in Node.js or Web Crypto API
        const privateKey = crypto.getRandomValues(new Uint8Array(32));
        return this.fromPrivateKey(privateKey);
    }

    /**
     * Create wallet from mnemonic
     */
    static async fromMnemonic(mnemonic, password = '') {
        // Implement BIP39 derivation
        // This is a simplified placeholder
        const seed = await this.mnemonicToSeed(mnemonic, password);
        const privateKey = seed.slice(0, 32);
        return this.fromPrivateKey(privateKey, mnemonic);
    }

    /**
     * Create wallet from private key
     */
    static async fromPrivateKey(privateKey, mnemonic = null) {
        // Derive public key and address from private key
        // This is a placeholder - actual implementation would use secp256k1
        const publicKey = await this.derivePublicKey(privateKey);
        const address = await this.deriveAddress(publicKey);

        return new Wallet(privateKey, publicKey, address, mnemonic);
    }

    /**
     * Sign a transfer transaction
     */
    async signTransaction(tx) {
        const txHash = await this.hashTransaction(tx);
        const signature = await this.sign(txHash);
        return { ...tx, signature };
    }

    /**
     * Sign a contract deployment transaction
     */
    async signDeployTransaction(tx) {
        const txHash = await this.hashDeployTransaction(tx);
        const signature = await this.sign(txHash);
        return { ...tx, signature };
    }

    /**
     * Sign a contract call transaction
     */
    async signCallTransaction(tx) {
        const txHash = await this.hashCallTransaction(tx);
        const signature = await this.sign(txHash);
        return { ...tx, signature };
    }

    /**
     * Sign data with private key
     */
    async sign(data) {
        // This would use secp256k1 signing
        // Placeholder implementation
        return new Uint8Array(65); // 64 bytes + recovery id
    }

    /**
     * Hash transaction for signing
     */
    async hashTransaction(tx) {
        // Implement transaction hashing
        const data = `${tx.from}${tx.to}${tx.amount}${tx.nonce}${tx.gasLimit}${tx.gasPrice}`;
        return await this.hash(data);
    }

    /**
     * Hash deploy transaction
     */
    async hashDeployTransaction(tx) {
        const data = `${tx.from}deploy${tx.wasmCode.join('')}${tx.initArgs.join('')}${tx.nonce}${tx.gasLimit}${tx.gasPrice}`;
        return await this.hash(data);
    }

    /**
     * Hash call transaction
     */
    async hashCallTransaction(tx) {
        const data = `${tx.from}${tx.contractAddress}${tx.method}${tx.args.join('')}${tx.nonce}${tx.gasLimit}${tx.gasPrice}`;
        return await this.hash(data);
    }

    /**
     * Hash data using SHA-256
     */
    async hash(data) {
        const encoder = new TextEncoder();
        const dataBuffer = encoder.encode(data);
        const hashBuffer = await crypto.subtle.digest('SHA-256', dataBuffer);
        return new Uint8Array(hashBuffer);
    }

    /**
     * Derive public key from private key
     */
    static async derivePublicKey(privateKey) {
        // This would use secp256k1 public key derivation
        // Placeholder
        return new Uint8Array(33);
    }

    /**
     * Derive address from public key
     */
    static async deriveAddress(publicKey) {
        // This would hash public key and take last 20 bytes
        // Placeholder
        return Array.from(publicKey.slice(-20));
    }

    /**
     * Convert mnemonic to seed
     */
    static async mnemonicToSeed(mnemonic, password) {
        // Implement BIP39 seed derivation
        // Placeholder
        return new Uint8Array(64);
    }

    /**
     * Export wallet data
     */
    export() {
        return {
            address: this.address,
            mnemonic: this.mnemonic,
            privateKey: Array.from(this.privateKey),
            publicKey: Array.from(this.publicKey)
        };
    }
}

/**
 * Transaction Builder for fluent API
 */
class TransactionBuilder {
    constructor(sdk) {
        this.sdk = sdk;
        this.tx = {};
    }

    /**
     * Set recipient
     */
    to(address) {
        this.tx.to = address;
        return this;
    }

    /**
     * Set amount
     */
    amount(amount) {
        this.tx.amount = amount;
        return this;
    }

    /**
     * Set contract address
     */
    contract(address) {
        this.tx.contractAddress = address;
        return this;
    }

    /**
     * Set contract method
     */
    method(method) {
        this.tx.method = method;
        return this;
    }

    /**
     * Set contract args
     */
    args(args) {
        this.tx.args = args;
        return this;
    }

    /**
     * Set gas limit
     */
    gasLimit(limit) {
        this.tx.gasLimit = limit;
        return this;
    }

    /**
     * Set gas price
     */
    gasPrice(price) {
        this.tx.gasPrice = price;
        return this;
    }

    /**
     * Estimate gas
     */
    async estimateGas() {
        // Simple estimation logic
        if (this.tx.contractAddress) {
            return 100000; // Contract call
        } else if (this.tx.wasmCode) {
            return 1000000; // Contract deploy
        } else {
            return 21000; // Transfer
        }
    }

    /**
     * Build and send transaction
     */
    async send() {
        // Auto-fill missing fields
        if (!this.tx.gasLimit) {
            this.tx.gasLimit = await this.estimateGas();
        }
        if (!this.tx.gasPrice) {
            this.tx.gasPrice = 1;
        }

        // Send based on transaction type
        if (this.tx.contractAddress) {
            return await this.sdk.callContract(
                this.tx.contractAddress,
                this.tx.method,
                this.tx.args
            );
        } else if (this.tx.wasmCode) {
            return await this.sdk.deployContract(
                this.tx.wasmCode,
                this.tx.initArgs
            );
        } else {
            return await this.sdk.sendTokens(
                this.tx.to,
                this.tx.amount
            );
        }
    }
}

// Export for different environments
if (typeof module !== 'undefined' && module.exports) {
    // Node.js
    module.exports = { SiputSDK, Wallet, TransactionBuilder };
} else if (typeof window !== 'undefined') {
    // Browser
    window.SiputSDK = SiputSDK;
    window.Wallet = Wallet;
    window.TransactionBuilder = TransactionBuilder;
}