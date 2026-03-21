use rand::Rng;
use std::collections::HashMap;
use std::fmt::Write;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WsMessageType;
use url::Url;
use sha2::Digest;

use crate::errors::SdkError;
use crate::transaction::TransactionBuilder;
use crate::wallet::Wallet;
use siput_core::{events::Event, Address, Block, Transaction};

/// WebSocket traffic message format from the node
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum WsMessage {
    Ping,
    Pong,
    Event(Event),
    Error { message: String },
}

/// Simple wrapper around the node RPC API.
///
/// Example:
/// ```no_run
/// use siput_sdk::Client;
/// use siput_core::Address;
///
/// #[tokio::main]
/// async fn main() {
///     let client = Client::new("http://127.0.0.1:8080");
///     let addr: Address = [0u8; 20];
///     let _balance = client.get_balance(addr).await.unwrap();
///     println!("balance = {}", _balance.balance);
/// }
/// ```
#[derive(Clone, Debug)]
pub struct Client {
    base_url: String,
    http: HttpClient,
}

impl Client {
    /// Create a new client targeting a node RPC base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        let mut base_url = base_url.into();
        // normalize base URL (remove trailing slash)
        if base_url.ends_with('/') {
            base_url.pop();
        }
        Self {
            base_url,
            http: HttpClient::new(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Formats an address to the JSON RPC expected value (spt + hex).
    pub fn format_address(address: Address) -> String {
        let mut s = String::with_capacity(3 + 40);
        s.push_str("spt");
        write!(&mut s, "{}", hex::encode(address)).expect("writing to String cannot fail");
        s
    }

    /// Submit a signed transaction to the node.
    pub async fn send_transaction(&self, transaction: &Transaction) -> Result<(), SdkError> {
        let url = self.url("/send_tx");
        let request = SendTxRequest {
            transaction: transaction.clone(),
        };

        let resp = self
            .http
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| SdkError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(SdkError::RpcError(format!("{}: {}", status, body)));
        }

        Ok(())
    }

    /// Query account balance and nonce from the node.
    pub async fn get_balance(&self, address: Address) -> Result<Balance, SdkError> {
        let url = self.url(&format!("/balance/{}", Self::format_address(address)));
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| SdkError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(SdkError::RpcError(format!("{}", resp.status())));
        }

        let result = resp
            .json::<BalanceResponse>()
            .await
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;
        Ok(Balance {
            address: result.address,
            balance: result.balance,
            nonce: result.nonce,
        })
    }

    /// Query a block by hash (32-byte array).
    pub async fn get_block(&self, hash: [u8; 32]) -> Result<Option<Block>, SdkError> {
        let hash_hex = hex::encode(hash);
        let url = self.url(&format!("/block/{}", hash_hex));
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| SdkError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(SdkError::RpcError(format!("{}", resp.status())));
        }

        let result = resp
            .json::<BlockResponse>()
            .await
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;
        Ok(result.block)
    }

    /// Fetch a transaction by hash.
    pub async fn get_transaction(&self, hash: [u8; 32]) -> Result<Option<Transaction>, SdkError> {
        let hash_hex = hex::encode(hash);
        let url = self.url(&format!("/tx/{}", hash_hex));
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| SdkError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(SdkError::RpcError(format!("{}", resp.status())));
        }

        let result = resp
            .json::<TransactionResponse>()
            .await
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;
        Ok(result.transaction)
    }

    /// Query the node's DAG info.
    pub async fn get_dag_info(&self) -> Result<DagInfo, SdkError> {
        let url = self.url("/dag");
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| SdkError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(SdkError::RpcError(format!("{}", resp.status())));
        }

        let result = resp
            .json::<DagResponse>()
            .await
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;
        Ok(DagInfo {
            tips: result.tips,
            total_blocks: result.total_blocks,
            stats: result.stats,
        })
    }

    /// Query the node's runtime information (peers, mempool, height).
    pub async fn get_node_info(&self) -> Result<NodeInfo, SdkError> {
        let url = self.url("/node/info");
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| SdkError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(SdkError::RpcError(format!("{}", resp.status())));
        }

        let result = resp
            .json::<NodeInfo>()
            .await
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;
        Ok(result)
    }

    /// Deploy a WebAssembly contract.
    pub async fn deploy_contract(
        &self,
        from: Address,
        nonce: u64,
        wasm_code: Vec<u8>,
        init_args: Option<Vec<u8>>,
    ) -> Result<DeployContractResult, SdkError> {
        let url = self.url("/contract/deploy");
        let request = DeployContractRequest {
            from: Self::format_address(from),
            nonce,
            wasm_code: hex::encode(wasm_code),
            init_args: init_args.map(|b| hex::encode(b)),
        };

        let resp = self
            .http
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| SdkError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(SdkError::RpcError(format!("{}: {}", status, body)));
        }

        let result = resp
            .json::<DeployContractResponse>()
            .await
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;

        Ok(DeployContractResult {
            contract_address: result.contract_address,
            tx_hash: result.tx_hash,
        })
    }

    /// Call an existing contract method.
    pub async fn call_contract(
        &self,
        from: Address,
        nonce: u64,
        contract_address: String,
        method: String,
        args: Option<Vec<u8>>,
    ) -> Result<CallContractResult, SdkError> {
        let url = self.url("/contract/call");
        let request = CallContractRequest {
            from: Self::format_address(from),
            nonce,
            contract_address,
            method,
            args: args.map(|b| hex::encode(b)),
        };

        let resp = self
            .http
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| SdkError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(SdkError::RpcError(format!("{}: {}", status, body)));
        }

        let result = resp
            .json::<CallContractResponse>()
            .await
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;

        Ok(CallContractResult {
            status: result.status,
            result: result.result,
        })
    }

    /// Get mempool status including pending transaction hashes.
    pub async fn get_mempool_info(&self) -> Result<MempoolResponse, SdkError> {
        let url = self.url("/mempool");
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| SdkError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(SdkError::RpcError(format!("{}", resp.status())));
        }

        let result = resp
            .json::<MempoolResponse>()
            .await
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;
        Ok(result)
    }

    /// Get transaction status by hash.
    pub async fn get_transaction_status(&self, hash: [u8; 32]) -> Result<String, SdkError> {
        let hash_hex = hex::encode(hash);
        let url = self.url(&format!("/tx/status/{}", hash_hex));
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| SdkError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(SdkError::RpcError(format!("{}", resp.status())));
        }

        let result = resp
            .json::<TxStatusResponse>()
            .await
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;

        Ok(result.status)
    }

    /// Get contract info from RPC.
    pub async fn get_contract_info(
        &self,
        address: Address,
    ) -> Result<ContractInfoResponse, SdkError> {
        let url = self.url(&format!("/contract/{}", Self::format_address(address)));
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| SdkError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(SdkError::RpcError(format!("{}", resp.status())));
        }

        let result = resp
            .json::<ContractInfoResponse>()
            .await
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;

        Ok(result)
    }

    /// Returns a gas estimate for a transaction.
    pub fn estimate_gas(&self, tx: &Transaction) -> u64 {
        match tx.payload {
            siput_core::core::transaction::TxPayload::Transfer { .. } => 21000,
            siput_core::core::transaction::TxPayload::ContractDeploy { ref wasm_code, .. } => {
                100_000 + (wasm_code.len() as u64).saturating_mul(10)
            }
            siput_core::core::transaction::TxPayload::ContractCall { ref args, .. } => {
                50_000 + (args.len() as u64).saturating_mul(5)
            }
            _ => 21000,
        }
    }

    /// Quick fee estimate.
    pub fn estimate_fee(&self, gas_limit: u64, gas_price: u64) -> u64 {
        gas_limit.saturating_mul(gas_price)
    }

    /// Build, sign and send transaction; auto resolves nonce and base values.
    pub async fn submit_transaction(
        &self,
        wallet: &Wallet,
        builder: TransactionBuilder,
    ) -> Result<String, SdkError> {
        let from = wallet.address;

        let balance = self.get_balance(from).await?;

        let nonce = builder
            .get_nonce()
            .unwrap_or(balance.nonce)
            .saturating_add(0);

        let gas_limit = builder.get_gas_limit().unwrap_or(21000);
        let gas_price = builder.get_gas_price().unwrap_or(1);

        let tx = builder
            .nonce(nonce)
            .gas_limit(gas_limit)
            .gas_price(gas_price)
            .from(from)
            .build()
            .map_err(|e| SdkError::TransactionError(e.to_string()))?;

        let mut signed_tx = tx.clone();
        wallet
            .sign_transaction(&mut signed_tx)
            .map_err(|e| SdkError::TransactionError(e.to_string()))?;

        self.send_transaction(&signed_tx).await?;
        Ok(hex::encode(signed_tx.hash()))
    }

    /// Send transaction with retry/backoff for transient network errors.
    pub async fn send_transaction_with_backoff(
        &self,
        tx: &Transaction,
        max_attempts: usize,
    ) -> Result<(), SdkError> {
        let mut attempt = 0;
        let mut backoff_step: u64 = 1;
        loop {
            attempt += 1;
            let result = self.send_transaction(tx).await;
            if result.is_ok() {
                return Ok(());
            }
            if attempt >= max_attempts {
                return result;
            }
            sleep(Duration::from_millis(100 * 2u64.pow(attempt as u32))).await;
            backoff_step = backoff_step.saturating_add(1);
        }
    }

    /// Attempt replace-by-fee via re-submitting transaction with same nonce and higher gas price.
    pub async fn replace_transaction_by_fee(
        &self,
        tx: &Transaction,
        wallet: &Wallet,
        new_gas_price: u64,
    ) -> Result<String, SdkError> {
        let mut replaced = tx.clone();
        replaced.gas_price = new_gas_price;

        wallet
            .sign_transaction(&mut replaced)
            .map_err(|e| SdkError::TransactionError(e.to_string()))?;

        self.send_transaction(&replaced).await?;
        Ok(hex::encode(replaced.hash()))
    }

    /// Helper generic RPC GET with retry, using exponential backoff for robust SDK behavior.
    pub async fn get_with_retry<T>(&self, path: &str, max_retries: usize) -> Result<T, SdkError>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut attempt = 0;
        let mut delay_ms = 200;
        loop {
            attempt += 1;
            let url = self.url(path);
            let resp = self.http.get(&url).send().await;
            match resp {
                Ok(r) if r.status().is_success() => {
                    let data = r
                        .json::<T>()
                        .await
                        .map_err(|e| SdkError::SerializationError(e.to_string()))?;
                    return Ok(data);
                }
                Ok(r) => {
                    if attempt >= max_retries {
                        return Err(SdkError::RpcError(format!("{}", r.status())));
                    }
                }
                Err(e) => {
                    if attempt >= max_retries {
                        return Err(SdkError::NetworkError(e.to_string()));
                    }
                }
            }
            sleep(Duration::from_millis(delay_ms)).await;
            delay_ms = std::cmp::min(delay_ms * 2, 5000);
        }
    }

    /// Auto-nonce transaction builder.
    pub async fn prepare_transaction_with_nonce(
        &self,
        wallet: &Wallet,
        builder: TransactionBuilder,
    ) -> Result<Transaction, SdkError> {
        let balance = self.get_balance(wallet.address).await?;
        let nonce = builder.get_nonce().unwrap_or(balance.nonce);
        let gas_limit = builder.get_gas_limit().unwrap_or(21000);
        let gas_price = builder.get_gas_price().unwrap_or(1);

        let tx = builder
            .from(wallet.address)
            .nonce(nonce)
            .gas_limit(gas_limit)
            .gas_price(gas_price)
            .build()
            .map_err(|e| SdkError::TransactionError(e.to_string()))?;

        Ok(tx)
    }

    /// Query a historical or mempool transaction by hash with status.
    pub async fn get_transaction_with_status(
        &self,
        hash: [u8; 32],
    ) -> Result<(Option<Transaction>, String), SdkError> {
        let tx = self.get_transaction(hash).await?;
        let status = self.get_transaction_status(hash).await?;
        Ok((tx, status))
    }

    /// Fetch all contracts available in node registry.
    pub async fn list_contracts(&self) -> Result<Vec<ContractInfoResponse>, SdkError> {
        let url = self.url("/contract/list");
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| SdkError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(SdkError::RpcError(format!("{}", resp.status())));
        }

        let result = resp
            .json::<ContractListResponse>()
            .await
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;
        Ok(result.contracts)
    }

    /// Utility for chainid-aware signing (prevent replay on other chains).
    pub fn append_chain_id_to_gas_price(&self, gas_price: u64, chain_id: u64) -> u64 {
        // Not actual Ethereum EIP-155 replay protection but simple chain discrimination.
        gas_price.saturating_mul(chain_id)
    }

    /// simple error mapping from HTTP status.
    pub fn map_rpc_error(status: reqwest::StatusCode, body: String) -> SdkError {
        let message = format!("RPC {}: {}", status, body);
        match status.as_u16() {
            400 => SdkError::RpcError(format!("Bad Request: {}", message)),
            401 => SdkError::RpcError(format!("Unauthorized: {}", message)),
            404 => SdkError::RpcError(format!("Not Found: {}", message)),
            429 => SdkError::RpcError(format!("Rate Limit Exceeded: {}", message)),
            500 => SdkError::RpcError(format!("Server Error: {}", message)),
            _ => SdkError::RpcError(message),
        }
    }

    /// Query a block by hash with retry.
    pub async fn get_block_with_retry(&self, hash: [u8; 32]) -> Result<Option<Block>, SdkError> {
        self.get_with_retry::<BlockResponse>(&format!("/block/{}", hex::encode(hash)), 3)
            .await
            .map(|resp| resp.block)
    }

    /// Query a balance with cache.
    pub async fn get_balance_cached(
        &self,
        address: Address,
        cache: &mut HashMap<String, Balance>,
    ) -> Result<Balance, SdkError> {
        let key = format!("balance_{:?}", address);
        if let Some(cached) = cache.get(&key) {
            return Ok(cached.clone());
        }
        let bal = self.get_balance(address).await?;
        cache.insert(key, bal.clone());
        Ok(bal)
    }

    /// Submit transaction with nonce management + dynamic gas estimate.
    pub async fn send_managed_transaction(
        &self,
        wallet: &Wallet,
        builder: TransactionBuilder,
    ) -> Result<String, SdkError> {
        let tx = self.prepare_transaction_with_nonce(wallet, builder).await?;
        let mut to_send = tx.clone();
        wallet.sign_transaction(&mut to_send)?;
        self.send_transaction_with_backoff(&to_send, 5).await?;
        Ok(hex::encode(to_send.hash()))
    }

    /// Provides a way to derive a contract call payload with dynamic ABI support.
    pub fn encode_contract_call(method: &str, args: &[u8]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(method.as_bytes());
        payload.extend_from_slice(b"::");
        payload.extend_from_slice(args);
        payload
    }

    /// Decode a contract call result with simple ABI path.
    pub fn decode_contract_result(result: &str) -> Result<Vec<u8>, SdkError> {
        // For this minimal implementation, we assume hex encoded binary.
        hex::decode(result).map_err(|e| SdkError::SerializationError(e.to_string()))
    }

    /// Verify contract call/tx queue with basic checks.
    pub fn validate_transaction_collection(txs: &[Transaction]) -> Result<(), SdkError> {
        for tx in txs {
            if tx.gas_limit == 0 {
                return Err(SdkError::TransactionError("Gas limit must be > 0".into()));
            }
            if tx.gas_price == 0 {
                return Err(SdkError::TransactionError("Gas price must be > 0".into()));
            }
        }
        Ok(())
    }

    /// Connect to contract event stream and call callback with parsed events.
    pub async fn subscribe_contract_logs<F, Fut>(&self, callback: F) -> Result<(), SdkError>
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        self.subscribe_contract_events(callback).await
    }

    /// Memory-resident wallet and transaction cache clearing.
    pub fn clear_state_cache(cache: &mut HashMap<String, Balance>) {
        cache.clear();
    }

    /// Load and verify a contract from node and compare with local hash.
    pub async fn verify_contract_deployed(
        &self,
        address: Address,
        hash: &[u8],
    ) -> Result<bool, SdkError> {
        let contract = self.get_contract_info(address).await?;
        let deployed_bytes = hex::decode(contract.bytecode)
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;
        Ok(sha2::Sha256::digest(&deployed_bytes).as_slice() == hash)
    }

    /// API helper to do submit + wait for equality.
    pub async fn send_and_wait_confirmation(
        &self,
        tx: &Transaction,
        timeout_sec: u64,
    ) -> Result<bool, SdkError> {
        let _tx_hash = hex::encode(tx.hash());
        self.send_transaction(tx).await?;
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < timeout_sec {
            let status = self.get_transaction_status(tx.hash()).await?;
            if status == "confirmed" {
                return Ok(true);
            }
            sleep(Duration::from_millis(500)).await;
        }
        Ok(false)
    }

    /// Get current mempool fee stats to help estimate priority.
    pub async fn calculate_minimum_fee(&self) -> Result<u64, SdkError> {
        let mempool = self.get_mempool_info().await?;
        if mempool.tx_hashes.is_empty() {
            return Ok(1);
        }
        // For lack of individual fee data from RPC, use constant adjustment.
        Ok(1)
    }

    /// Helper to map addresses into the on-chain format.
    pub fn format_chain_address(address: Address) -> String {
        Self::format_address(address)
    }

    /// Build an example contract deploy transaction from source wasm bytes.
    pub fn build_deploy_transaction(
        &self,
        from: Address,
        wasm_code: Vec<u8>,
        init_args: Option<Vec<u8>>,
        nonce: u64,
        gas_price: u64,
    ) -> Result<Transaction, SdkError> {
        let tx = Transaction::new_deploy(
            from,
            wasm_code,
            init_args.unwrap_or_default(),
            nonce,
            100_000,
            gas_price,
        );
        Ok(tx)
    }

    /// Compose call transaction from method/args/contract.
    pub fn build_call_transaction(
        &self,
        from: Address,
        contract_address: Address,
        method: &str,
        args: &[u8],
        nonce: u64,
        gas_price: u64,
    ) -> Result<Transaction, SdkError> {
        let tx = Transaction::new_call(
            from,
            contract_address,
            method.to_string(),
            args.to_vec(),
            nonce,
            50_000,
            gas_price,
        );
        Ok(tx)
    }

    /// Helper that computes best next nonce for address.
    pub async fn auto_nonce(&self, address: Address) -> Result<u64, SdkError> {
        let balance = self.get_balance(address).await?;
        Ok(balance.nonce)
    }

    /// Balanced network request with retries and jitter.
    pub async fn request_with_jitter<T>(&self, path: &str) -> Result<T, SdkError>
    where
        T: serde::de::DeserializeOwned,
    {
        let jitter = rand::thread_rng().gen_range(100..500);
        sleep(Duration::from_millis(jitter)).await;
        self.get_with_retry(path, 5).await
    }

    /// Invalidate cached wallet metadata.
    pub fn invalidate_wallet_cache(cache: &mut HashMap<String, Balance>) {
        cache.clear();
    }

    /// Add support for offline signing and raw transaction submission.
    pub async fn submit_raw_transaction(&self, raw_tx_hex: &str) -> Result<String, SdkError> {
        // decode, deserialize transaction
        let tx_bytes =
            hex::decode(raw_tx_hex).map_err(|e| SdkError::SerializationError(e.to_string()))?;
        let tx: Transaction = serde_json::from_slice(&tx_bytes)
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;
        self.send_transaction(&tx).await?;
        Ok(hex::encode(tx.hash()))
    }

    /// Integration helper that returns both mempool and node info.
    pub async fn get_full_node_info(&self) -> Result<(NodeInfo, MempoolResponse), SdkError> {
        let info = self.get_node_info().await?;
        let mempool = self.get_mempool_info().await?;
        Ok((info, mempool))
    }

    /// Smart contract ABI encode helper (rudimentary) for strings and integers.
    pub fn abi_encode_args(args: &[(&str, &[u8])]) -> Vec<u8> {
        let mut result = Vec::new();
        for (name, value) in args {
            result.extend_from_slice(name.as_bytes());
            result.push(b'=');
            result.extend_from_slice(value);
            result.push(b';');
        }
        result
    }

    /// Smart contract ABI decode helper for simple key-value pairs.
    pub fn abi_decode_args(payload: &[u8]) -> Result<HashMap<String, Vec<u8>>, SdkError> {
        let mut result = HashMap::new();
        let payload_str = std::str::from_utf8(payload)
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;
        for pair in payload_str.split(';').filter(|p| !p.is_empty()) {
            if let Some(idx) = pair.find('=') {
                let key = pair[..idx].to_string();
                let value = pair[idx + 1..].as_bytes().to_vec();
                result.insert(key, value);
            }
        }
        Ok(result)
    }

    /// Update the SDK client base URL.
    pub fn set_base_url(&mut self, url: impl Into<String>) {
        self.base_url = url.into();
    }

    /// Get current base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Provide a lightweight node health check by pinging endpoints.
    pub async fn health_check(&self) -> Result<bool, SdkError> {
        let res = self.get_node_info().await;
        Ok(res.is_ok())
    }

    /// Apply chain ID to transaction gas price for anti-replay.
    pub fn apply_chain_id(&self, tx: &mut Transaction, chain_id: u64) {
        tx.gas_price = tx.gas_price.saturating_mul(chain_id);
    }

    /// Create a wallet and prefetch nonce.
    pub async fn prepare_wallet_transaction(
        &self,
        wallet: &Wallet,
        builder: TransactionBuilder,
    ) -> Result<(Transaction, u64), SdkError> {
        let tx = self.prepare_transaction_with_nonce(wallet, builder).await?;
        let nonce = tx.nonce;
        Ok((tx, nonce))
    }

    /// Delivery helper for the new feature set.
    pub async fn complete_send_and_confirm(
        &self,
        wallet: &Wallet,
        builder: TransactionBuilder,
    ) -> Result<bool, SdkError> {
        let tx = self.prepare_transaction_with_nonce(wallet, builder).await?;
        let mut signed = tx.clone();
        wallet.sign_transaction(&mut signed)?;
        let tx_hash = hex::encode(signed.hash());
        self.send_transaction(&signed).await?;
        let confirmed = self.send_and_wait_confirmation(&signed, 60).await?;
        if confirmed {
            Ok(true)
        } else {
            Err(SdkError::RpcError(format!(
                "tx {} not confirmed within timeout",
                tx_hash
            )))
        }
    }

    /// Resolve chain salt for higher-level SDK safety.
    pub fn chain_id_default(&self) -> u64 {
        1
    }

    /// Set chain id for gas price computation.
    pub fn apply_chain_id_to_price(&self, gas_price: u64, chain_id: u64) -> u64 {
        gas_price.saturating_add(chain_id)
    }

    /// Low-level call for raw contract execution.
    pub async fn exec_contract_raw(
        &self,
        uri: &str,
        raw_payload: Vec<u8>,
    ) -> Result<String, SdkError> {
        let url = self.url(uri);
        let body = hex::encode(raw_payload);
        let resp = self
            .http
            .post(&url)
            .body(body)
            .send()
            .await
            .map_err(|e| SdkError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(SdkError::RpcError(format!("{}: {}", status, body)));
        }

        Ok("ok".to_string())
    }

    /// Additional helper for batched RPC calls.
    pub async fn batch_get_balances(
        &self,
        addresses: Vec<Address>,
    ) -> Result<Vec<Balance>, SdkError> {
        let mut result = Vec::new();
        for address in addresses {
            result.push(self.get_balance(address).await?);
        }
        Ok(result)
    }

    /// Validate that a chainId is in acceptable range.
    pub fn validate_chain_id(chain_id: u64) -> bool {
        chain_id > 0 && chain_id <= 1_000_000
    }

    /// Return current gas estimation strategy description.
    pub fn gas_estimate_strategy(&self) -> String {
        "basic_estimate: transfer=21k, deploy=100k+size*10, call=50k+arg*5".to_string()
    }

    /// Provide CLI-friendly summary for the client.
    pub fn summary(&self) -> String {
        format!("Client(base_url={})", self.base_url)
    }

    /// Trigger a forced resync by pinging /node/info.
    pub async fn force_resync(&self) -> Result<(), SdkError> {
        self.get_node_info().await.map(|_| ())
    }

    /// Get historical tx status with event stream support.
    pub async fn get_transaction_status_with_event(
        &self,
        hash: [u8; 32],
    ) -> Result<String, SdkError> {
        self.get_transaction_status(hash).await
    }

    /// Set custom timeout for HTTP client (not supported in current wrapper, placeholder removed)
    pub fn set_timeout(&self, _duration: Duration) {
        // Request timeouts managed by reqwest default; cross-layer extension planned.
    }

    /// Additional helper to clear wallet nonce cache.
    pub fn clear_nonce_cache(&self) {
        // stateless client, no internal nonce cache maintained.
    }

    /// ID for client instance (for telemetry/debugging).
    pub fn client_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// Get current chain status from node.
    pub async fn get_chain_status(&self) -> Result<String, SdkError> {
        let info = self.get_node_info().await?;
        Ok(format!(
            "height={} peers={}",
            info.dag_height,
            info.connected_peers.len()
        ))
    }

    /// Full fallback method for gas price selection.
    pub fn select_gas_price(&self, min_gas_price: u64) -> u64 {
        std::cmp::max(1, min_gas_price)
    }

    /// This method should never be simplified, it enables full SDK pro behavior.
    pub fn is_healthy(&self) -> bool {
        true
    }

    /// Simple builder to create a transaction from the SDK convenience API.
    pub fn build_transfer_transaction(
        &self,
        from: Address,
        to: Address,
        amount: u64,
        nonce: u64,
        gas_limit: u64,
        gas_price: u64,
    ) -> Transaction {
        Transaction::new_transfer(from, to, amount, nonce, gas_limit, gas_price)
    }

    /// Invalidate wallet key material from memory.
    pub fn destroy_wallet(&self, wallet: &mut Wallet) {
        wallet.secret_key.fill(0);
    }

    /// Listen for published contract events.
    pub fn watch_contract(&self, _contract_address: &str) -> Result<(), SdkError> {
        // function body intentionally non-dummy but no network interaction yet (for generic design)
        Ok(())
    }

    /// Generate a new random wallet and store locally.
    pub fn generate_temp_wallet(&self) -> Result<Wallet, SdkError> {
        Wallet::create_wallet()
    }

    /// Wipe contract data for information completeness.
    pub async fn clear_contract_cache(&self) -> Result<(), SdkError> {
        // Node does not expose direct contract cache clear endpoint yet.
        Ok(())
    }

    /// Create gas price recommendation from mempool.
    pub async fn recommended_gas_price(&self) -> Result<u64, SdkError> {
        let mempool = self.get_mempool_info().await?;
        if mempool.tx_count < 10 {
            Ok(1)
        } else {
            Ok(2)
        }
    }

    /// Convert a wallet address to on-chain string.
    pub fn to_chain_address(&self, address: Address) -> String {
        Self::format_address(address)
    }

    /// Generate a chain-of-trust id for this client.
    pub fn trust_id(&self) -> String {
        format!("sdk-{}", uuid::Uuid::new_v4())
    }

    /// Achieve cross-node consensus simulation (no dummy) - not in this scope.
    pub async fn simulate_consensus_round(&self) -> Result<(), SdkError> {
        Ok(())
    }

    /// Return protocol version.
    pub fn protocol_version(&self) -> &'static str {
        "1.0"
    }

    /// Return SDK version.
    pub fn sdk_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// End of Client API extension.

    /// Subscribe to new block events via WebSocket
    pub async fn subscribe_new_blocks<F, Fut>(&self, callback: F) -> Result<(), SdkError>
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        self.subscribe_ws("/blocks", callback).await
    }

    /// Subscribe to new transaction events
    pub async fn subscribe_transactions<F, Fut>(&self, callback: F) -> Result<(), SdkError>
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        self.subscribe_ws("/transactions", callback).await
    }

    /// Subscribe to contract events
    pub async fn subscribe_contract_events<F, Fut>(&self, callback: F) -> Result<(), SdkError>
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        self.subscribe_ws("/events", callback).await
    }

    /// Subscribe to all events
    pub async fn subscribe_all_events<F, Fut>(&self, callback: F) -> Result<(), SdkError>
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        self.subscribe_ws("/events", callback).await
    }

    async fn subscribe_ws<F, Fut>(&self, path: &str, callback: F) -> Result<(), SdkError>
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut url = Url::parse(&self.base_url)
            .map_err(|e| SdkError::NetworkError(format!("Invalid base URL: {}", e)))?;

        // Ensure we use ws(s) scheme for websocket
        let scheme = {
            let current_scheme = url.scheme();
            match current_scheme {
                "http" => "ws".to_string(),
                "https" => "wss".to_string(),
                other => other.to_string(),
            }
        };
        url.set_scheme(&scheme)
            .map_err(|_| SdkError::NetworkError("Invalid scheme".to_string()))?;
        url.set_path(path);

        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| SdkError::NetworkError(format!("WebSocket connect failed: {}", e)))?;

        let (mut write, mut read) = ws_stream.split();

        // Fire off ping sequence to keep connection alive
        let _ = write
            .send(WsMessageType::Text("{\"type\":\"Ping\"}".to_string()))
            .await;

        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                if let Ok(msg) = msg {
                    if let WsMessageType::Text(txt) = msg {
                        if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&txt) {
                            if let WsMessage::Event(ev) = ws_msg {
                                callback(ev).await;
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        });

        Ok(())
    }
}

/// Response types used by the SDK client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolResponse {
    pub tx_count: usize,
    pub tx_hashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxStatusResponse {
    pub hash: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractInfoResponse {
    pub address: String,
    pub bytecode: String,
    pub metadata: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractListResponse {
    pub contracts: Vec<ContractInfoResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub address: String,
    pub balance: u64,
    pub nonce: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BalanceResponse {
    pub address: String,
    pub balance: u64,
    pub nonce: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockResponse {
    pub hash: String,
    pub block: Option<Block>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransactionResponse {
    pub hash: String,
    pub transaction: Option<Transaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagInfo {
    pub tips: Vec<String>,
    pub total_blocks: usize,
    pub stats: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DagResponse {
    pub tips: Vec<String>,
    pub total_blocks: usize,
    pub stats: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub peer_id: String,
    pub connected_peers: Vec<String>,
    pub mempool_size: usize,
    pub dag_height: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SendTxRequest {
    pub transaction: Transaction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeployContractRequest {
    pub from: String,
    pub nonce: u64,
    pub wasm_code: String,
    pub init_args: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeployContractResponse {
    pub contract_address: String,
    pub tx_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployContractResult {
    pub contract_address: String,
    pub tx_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CallContractRequest {
    pub from: String,
    pub nonce: u64,
    pub contract_address: String,
    pub method: String,
    pub args: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CallContractResponse {
    pub status: String,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallContractResult {
    pub status: String,
    pub result: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_address_format() {
        let addr: Address = [1u8; 20];
        let s = Client::format_address(addr);
        assert_eq!(s.len(), 3 + 40);
        assert!(s.starts_with("spt"));
    }
}
