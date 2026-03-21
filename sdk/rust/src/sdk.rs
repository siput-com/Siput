//! High-Level SDK Interface
//!
//! This module provides a simplified, developer-friendly interface to the Siput blockchain.
//! It abstracts away complex blockchain operations and provides easy-to-use APIs for
//! common use cases like wallet connections, transaction building, and event listening.

use crate::client::Client;
use crate::errors::SdkError;
use crate::events::{SdkEventListener, SdkEventEmitter};
use crate::transaction::TransactionBuilder;
use crate::wallet::Wallet;
use siput_core::{Address, Transaction, Block};
use std::sync::Arc;
use tokio::sync::RwLock;

/// High-level SDK for easy blockchain interaction
#[derive(Clone)]
pub struct SiputSDK {
    client: Arc<Client>,
    wallet: Arc<RwLock<Option<Wallet>>>,
    event_listener: Arc<SdkEventListener>,
    event_emitter: Arc<SdkEventEmitter>,
}

impl SiputSDK {
    /// Create a new SDK instance
    pub fn new(rpc_url: &str) -> Self {
        let client = Arc::new(Client::new(rpc_url));
        let event_listener = Arc::new(SdkEventListener::new("siput-sdk".to_string()));
        let event_emitter = Arc::new(SdkEventEmitter::new());

        Self {
            client,
            wallet: Arc::new(RwLock::new(None)),
            event_listener,
            event_emitter,
        }
    }

    /// Connect a wallet to the SDK
    pub async fn connect_wallet(&self, wallet: Wallet) -> Result<(), SdkError> {
        let mut current_wallet = self.wallet.write().await;
        *current_wallet = Some(wallet);
        Ok(())
    }

    /// Disconnect the current wallet
    pub async fn disconnect_wallet(&self) -> Result<(), SdkError> {
        let mut current_wallet = self.wallet.write().await;
        *current_wallet = None;
        Ok(())
    }

    /// Get the current connected wallet
    pub async fn get_wallet(&self) -> Option<Wallet> {
        self.wallet.read().await.clone()
    }

    /// Check if wallet is connected
    pub async fn is_wallet_connected(&self) -> bool {
        self.wallet.read().await.is_some()
    }

    /// Get wallet address (returns error if no wallet connected)
    pub async fn get_address(&self) -> Result<Address, SdkError> {
        let wallet = self.wallet.read().await;
        wallet.as_ref()
            .map(|w| w.address)
            .ok_or_else(|| SdkError::WalletError("No wallet connected".to_string()))
    }

    /// Get account balance
    pub async fn get_balance(&self, address: Option<Address>) -> Result<u64, SdkError> {
        let addr = match address {
            Some(addr) => addr,
            None => self.get_address().await?,
        };

        let balance_response = self.client.get_balance(addr).await?;
        Ok(balance_response.balance)
    }

    /// Send tokens to another address
    pub async fn send_tokens(&self, to: Address, amount: u64) -> Result<String, SdkError> {
        let wallet = self.wallet.read().await;
        let wallet = wallet.as_ref()
            .ok_or_else(|| SdkError::WalletError("No wallet connected".to_string()))?;

        let balance = self.get_balance(None).await?;
        if balance < amount {
            return Err(SdkError::WalletError("Insufficient balance".to_string()));
        }

        let nonce = self.get_nonce().await?;

        let tx = TransactionBuilder::new()
            .from(wallet.address)
            .transfer(to, amount)
            .nonce(nonce)
            .gas_limit(21_000)
            .gas_price(1)
            .build()?;

        let signed_tx = wallet.sign_transaction(tx)?;
        let tx_hash = self.client.send_transaction(signed_tx).await?;

        Ok(tx_hash)
    }

    /// Get transaction nonce for the connected wallet
    async fn get_nonce(&self) -> Result<u64, SdkError> {
        let address = self.get_address().await?;
        let balance_response = self.client.get_balance(address).await?;
        Ok(balance_response.nonce)
    }

    /// Deploy a smart contract
    pub async fn deploy_contract(&self, wasm_code: Vec<u8>, init_args: Vec<u8>) -> Result<String, SdkError> {
        let wallet = self.wallet.read().await;
        let wallet = wallet.as_ref()
            .ok_or_else(|| SdkError::WalletError("No wallet connected".to_string()))?;

        let nonce = self.get_nonce().await?;

        let tx = TransactionBuilder::new()
            .from(wallet.address)
            .deploy_contract(wasm_code, init_args)
            .nonce(nonce)
            .gas_limit(1_000_000)
            .gas_price(1)
            .build()?;

        let signed_tx = wallet.sign_transaction(tx)?;
        let tx_hash = self.client.send_transaction(signed_tx).await?;

        Ok(tx_hash)
    }

    /// Call a smart contract method
    pub async fn call_contract(&self, contract_address: Address, method: &str, args: Vec<u8>) -> Result<String, SdkError> {
        let wallet = self.wallet.read().await;
        let wallet = wallet.as_ref()
            .ok_or_else(|| SdkError::WalletError("No wallet connected".to_string()))?;

        let nonce = self.get_nonce().await?;

        let tx = TransactionBuilder::new()
            .from(wallet.address)
            .call_contract(contract_address, method.to_string(), args)
            .nonce(nonce)
            .gas_limit(100_000)
            .gas_price(1)
            .build()?;

        let signed_tx = wallet.sign_transaction(tx)?;
        let tx_hash = self.client.send_transaction(signed_tx).await?;

        Ok(tx_hash)
    }

    /// Get transaction by hash
    pub async fn get_transaction(&self, tx_hash: &str) -> Result<Option<Transaction>, SdkError> {
        let hash_bytes = hex::decode(tx_hash)
            .map_err(|_| SdkError::InvalidInput("Invalid transaction hash".to_string()))?;
        if hash_bytes.len() != 32 {
            return Err(SdkError::InvalidInput("Transaction hash must be 32 bytes".to_string()));
        }

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hash_bytes);

        let tx_response = self.client.get_transaction(hash).await?;
        Ok(tx_response.transaction)
    }

    /// Get block by hash
    pub async fn get_block(&self, block_hash: &str) -> Result<Option<Block>, SdkError> {
        let hash_bytes = hex::decode(block_hash)
            .map_err(|_| SdkError::InvalidInput("Invalid block hash".to_string()))?;
        if hash_bytes.len() != 32 {
            return Err(SdkError::InvalidInput("Block hash must be 32 bytes".to_string()));
        }

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hash_bytes);

        let block_response = self.client.get_block(hash).await?;
        Ok(block_response.block)
    }

    /// Subscribe to new blocks
    pub async fn on_new_blocks<F>(&self, callback: F) -> Result<(), SdkError>
    where
        F: Fn(Block, u64) + Send + Sync + 'static,
    {
        self.event_listener.on_new_blocks(callback).await
            .map_err(|e| SdkError::EventError(e))
    }

    /// Subscribe to transaction confirmations
    pub async fn on_transaction_confirmations<F>(&self, callback: F) -> Result<(), SdkError>
    where
        F: Fn(Transaction, [u8; 32]) + Send + Sync + 'static,
    {
        self.event_listener.on_transaction_confirmations(callback).await
            .map_err(|e| SdkError::EventError(e))
    }

    /// Subscribe to contract events
    pub async fn on_contract_events<F>(&self, callback: F) -> Result<(), SdkError>
    where
        F: Fn(String, Vec<u8>) + Send + Sync + 'static,
    {
        self.event_listener.on_contract_events(callback).await
            .map_err(|e| SdkError::EventError(e))
    }

    /// Get client reference for advanced operations
    pub fn client(&self) -> &Arc<Client> {
        &self.client
    }

    /// Get event emitter for custom events
    pub fn events(&self) -> &Arc<SdkEventEmitter> {
        &self.event_emitter
    }
}

/// Wallet Connect functionality
pub mod wallet_connect {
    use super::*;

    /// Wallet connection manager
    pub struct WalletConnector {
        sdk: Arc<SiputSDK>,
    }

    impl WalletConnector {
        pub fn new(sdk: Arc<SiputSDK>) -> Self {
            Self { sdk }
        }

        /// Connect using mnemonic phrase
        pub async fn connect_with_mnemonic(&self, mnemonic: &str, password: Option<&str>) -> Result<(), SdkError> {
            let wallet = Wallet::from_mnemonic(mnemonic, password)?;
            self.sdk.connect_wallet(wallet).await
        }

        /// Connect using private key
        pub async fn connect_with_private_key(&self, private_key_hex: &str) -> Result<(), SdkError> {
            let wallet = Wallet::from_private_key(private_key_hex)?;
            self.sdk.connect_wallet(wallet).await
        }

        /// Create new wallet
        pub async fn create_wallet(&self) -> Result<Wallet, SdkError> {
            let wallet = Wallet::create_wallet()?;
            self.sdk.connect_wallet(wallet.clone()).await?;
            Ok(wallet)
        }

        /// Export wallet mnemonic
        pub async fn export_mnemonic(&self, password: Option<&str>) -> Result<String, SdkError> {
            let wallet = self.sdk.get_wallet().await
                .ok_or_else(|| SdkError::WalletError("No wallet connected".to_string()))?;
            wallet.to_mnemonic(password)
        }
    }
}

/// Transaction Builder with fluent API
pub mod transaction_builder {
    use super::*;

    /// Enhanced transaction builder with validation
    pub struct EnhancedTransactionBuilder {
        builder: TransactionBuilder,
        sdk: Arc<SiputSDK>,
    }

    impl EnhancedTransactionBuilder {
        pub fn new(sdk: Arc<SiputSDK>) -> Self {
            Self {
                builder: TransactionBuilder::new(),
                sdk,
            }
        }

        /// Set sender (auto-filled from connected wallet)
        pub async fn from_connected_wallet(mut self) -> Result<Self, SdkError> {
            let address = self.sdk.get_address().await?;
            self.builder = self.builder.from(address);
            Ok(self)
        }

        /// Transfer tokens
        pub fn transfer(mut self, to: Address, amount: u64) -> Self {
            self.builder = self.builder.transfer(to, amount);
            self
        }

        /// Deploy contract
        pub fn deploy_contract(mut self, wasm_code: Vec<u8>, init_args: Vec<u8>) -> Self {
            self.builder = self.builder.deploy_contract(wasm_code, init_args);
            self
        }

        /// Call contract
        pub fn call_contract(mut self, contract_address: Address, method: &str, args: Vec<u8>) -> Self {
            self.builder = self.builder.call_contract(contract_address, method.to_string(), args);
            self
        }

        /// Set gas limit (with estimation if not provided)
        pub async fn gas_limit(mut self, gas_limit: Option<u64>) -> Result<Self, SdkError> {
            let gas = gas_limit.unwrap_or_else(|| self.estimate_gas().await?);
            self.builder = self.builder.gas_limit(gas);
            Ok(self)
        }

        /// Set gas price
        pub fn gas_price(mut self, gas_price: u64) -> Self {
            self.builder = self.builder.gas_price(gas_price);
            self
        }

        /// Estimate gas for the transaction
        pub async fn estimate_gas(&self) -> Result<u64, SdkError> {
            // Simple gas estimation - can be enhanced with actual estimation logic
            Ok(21_000) // Default transfer gas
        }

        /// Set nonce (auto-filled from wallet)
        pub async fn nonce(mut self) -> Result<Self, SdkError> {
            let nonce = self.sdk.get_nonce().await?;
            self.builder = self.builder.nonce(nonce);
            Ok(self)
        }

        /// Build the transaction
        pub fn build(self) -> Result<Transaction, SdkError> {
            self.builder.build()
        }

        /// Build and sign the transaction
        pub async fn build_and_sign(self) -> Result<Transaction, SdkError> {
            let tx = self.builder.build()?;
            let wallet = self.sdk.get_wallet().await
                .ok_or_else(|| SdkError::WalletError("No wallet connected".to_string()))?;
            wallet.sign_transaction(tx)
        }

        /// Build, sign, and send the transaction
        pub async fn build_sign_and_send(self) -> Result<String, SdkError> {
            let signed_tx = self.build_and_sign().await?;
            self.sdk.client.send_transaction(signed_tx).await
        }
    }
}

/// Event listener with high-level APIs
pub mod event_listener {
    use super::*;

    /// Enhanced event listener
    pub struct EnhancedEventListener {
        sdk: Arc<SiputSDK>,
    }

    impl EnhancedEventListener {
        pub fn new(sdk: Arc<SiputSDK>) -> Self {
            Self { sdk }
        }

        /// Listen for balance changes
        pub async fn on_balance_change<F>(&self, address: Address, callback: F) -> Result<(), SdkError>
        where
            F: Fn(u64, u64) + Send + Sync + 'static, // (old_balance, new_balance)
        {
            let sdk = Arc::clone(&self.sdk);
            let mut last_balance = sdk.get_balance(Some(address)).await.unwrap_or(0);

            self.sdk.on_new_blocks(move |block, _height| {
                let callback = callback.clone();
                let sdk = Arc::clone(&sdk);
                let mut last_balance = last_balance;
                async move {
                    // Check if any transaction in this block affects the address
                    for tx in &block.transactions {
                        if tx.from == address || tx.to() == Some(address) {
                            // Balance might have changed, fetch new balance
                            if let Ok(new_balance) = sdk.get_balance(Some(address)).await {
                                if new_balance != last_balance {
                                    callback(last_balance, new_balance);
                                    last_balance = new_balance;
                                }
                            }
                        }
                    }
                }
            }).await
        }

        /// Listen for incoming transactions
        pub async fn on_incoming_transaction<F>(&self, callback: F) -> Result<(), SdkError>
        where
            F: Fn(Transaction, u64) + Send + Sync + 'static, // (transaction, amount)
        {
            let sdk = Arc::clone(&self.sdk);
            self.sdk.on_transaction_confirmations(move |tx, _block_hash| {
                let callback = callback.clone();
                let sdk = Arc::clone(&sdk);
                async move {
                    if let Some(wallet) = sdk.get_wallet().await {
                        if tx.to() == Some(wallet.address) {
                            if let siput_core::TxPayload::Transfer { amount, .. } = &tx.payload {
                                callback(tx, *amount);
                            }
                        }
                    }
                }
            }).await
        }

        /// Listen for outgoing transactions
        pub async fn on_outgoing_transaction<F>(&self, callback: F) -> Result<(), SdkError>
        where
            F: Fn(Transaction, u64) + Send + Sync + 'static, // (transaction, amount)
        {
            let sdk = Arc::clone(&self.sdk);
            self.sdk.on_transaction_confirmations(move |tx, _block_hash| {
                let callback = callback.clone();
                let sdk = Arc::clone(&sdk);
                async move {
                    if let Some(wallet) = sdk.get_wallet().await {
                        if tx.from == wallet.address {
                            if let siput_core::TxPayload::Transfer { amount, .. } = &tx.payload {
                                callback(tx, *amount);
                            }
                        }
                    }
                }
            }).await
        }
    }
}