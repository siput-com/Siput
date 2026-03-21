use crate::core::{Block, Transaction};
use crate::core::transaction::TxPayload;
use std::collections::HashSet;

/// Validation error types
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    InvalidFormat(String),
    InvalidSignature,
    InsufficientBalance,
    InvalidNonce,
    GasLimitExceeded,
    InvalidGasPrice,
    DuplicateTransaction,
    BlacklistedAddress,
}

/// Pre-validation result
#[derive(Debug, Clone)]
pub struct PreValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
}

/// Transaction Validator trait
pub trait TransactionValidator {
    fn validate_format(&self, tx: &Transaction) -> Result<(), ValidationError>;
    fn validate_signature(&self, tx: &Transaction) -> Result<(), ValidationError>;
    fn validate_gas(&self, tx: &Transaction) -> Result<(), ValidationError>;
    fn validate_addresses(&self, tx: &Transaction) -> Result<(), ValidationError>;
    fn check_duplicate(&self, tx: &Transaction) -> Result<(), ValidationError>;
    fn validate_block_transactions(&self, block: &Block) -> PreValidationResult;
}

/// Stateless transaction pre-validator
/// Performs fast validation checks without accessing full blockchain state
pub struct PreTransactionValidator {
    /// Maximum gas limit per transaction
    max_gas_limit: u64,
    /// Minimum gas price
    min_gas_price: u64,
    /// Maximum transaction size in bytes
    max_tx_size: usize,
    /// Blacklisted addresses
    blacklisted_addresses: HashSet<[u8; 20]>,
    /// Known transaction hashes (for duplicate detection in mempool)
    known_tx_hashes: HashSet<[u8; 32]>,
}

impl PreTransactionValidator {
    /// Create new pre-validator
    pub fn new(max_gas_limit: u64, min_gas_price: u64, max_tx_size: usize) -> Self {
        Self {
            max_gas_limit,
            min_gas_price,
            max_tx_size,
            blacklisted_addresses: HashSet::new(),
            known_tx_hashes: HashSet::new(),
        }
    }

    /// Pre-validate a single transaction
    pub fn pre_validate_transaction(&self, tx: &Transaction) -> PreValidationResult {
        let mut errors = Vec::new();

        // Format validation
        if let Err(e) = self.validate_format(tx) {
            errors.push(e);
        }

        // Signature validation
        if let Err(e) = self.validate_signature(tx) {
            errors.push(e);
        }

        // Gas validation
        if let Err(e) = self.validate_gas(tx) {
            errors.push(e);
        }

        // Address validation
        if let Err(e) = self.validate_addresses(tx) {
            errors.push(e);
        }

        // Duplicate check (if we have known hashes)
        if let Err(e) = self.check_duplicate(tx) {
            errors.push(e);
        }

        PreValidationResult {
            is_valid: errors.is_empty(),
            errors,
        }
    }

    /// Pre-validate multiple transactions
    pub fn pre_validate_transactions(
        &self,
        transactions: &[Transaction],
    ) -> Vec<PreValidationResult> {
        transactions
            .iter()
            .map(|tx| self.pre_validate_transaction(tx))
            .collect()
    }

    /// Add transaction hash to known set (for duplicate detection)
    pub fn add_known_transaction(&mut self, tx_hash: [u8; 32]) {
        self.known_tx_hashes.insert(tx_hash);
    }

    /// Remove transaction hash from known set
    pub fn remove_known_transaction(&mut self, tx_hash: &[u8; 32]) {
        self.known_tx_hashes.remove(tx_hash);
    }

    /// Clear known transactions (periodic cleanup)
    pub fn clear_known_transactions(&mut self) {
        self.known_tx_hashes.clear();
    }

    /// Add address to blacklist
    pub fn blacklist_address(&mut self, address: [u8; 20]) {
        self.blacklisted_addresses.insert(address);
    }

    /// Remove address from blacklist
    pub fn unblacklist_address(&mut self, address: &[u8; 20]) {
        self.blacklisted_addresses.remove(address);
    }

    /// Get current blacklist size
    pub fn blacklist_size(&self) -> usize {
        self.blacklisted_addresses.len()
    }

    /// Get known transactions count
    pub fn known_transactions_count(&self) -> usize {
        self.known_tx_hashes.len()
    }

    /// Estimate gas usage for transaction (rough estimate)
    pub fn estimate_gas_usage(&self, tx: &Transaction) -> u64 {
        // Base gas for transaction
        let mut gas = 21000;

        // Additional gas based on payload
        match &tx.payload {
            TxPayload::Transfer { .. } => {
                // Basic transfer, no additional cost
            }
            TxPayload::Coinbase { .. } => {
                // Coinbase transactions use no gas
                gas = 0;
            }
            TxPayload::ContractDeploy {
                wasm_code,
                init_args,
                ..
            } => {
                // Gas for code deployment (16 gas per byte for code, 4 for init args)
                gas += (wasm_code.len() as u64) * 16;
                gas += (init_args.len() as u64) * 4;
            }
            TxPayload::ContractCall { args, .. } => {
                // Gas for contract call arguments
                gas += (args.len() as u64) * 4;
            }
        }

        gas.min(tx.gas_limit)
    }
}

impl TransactionValidator for PreTransactionValidator {
    fn validate_format(&self, tx: &Transaction) -> Result<(), ValidationError> {
        // Check transaction size
        let tx_size = std::mem::size_of_val(tx);
        if tx_size > self.max_tx_size {
            return Err(ValidationError::InvalidFormat(format!(
                "Transaction size {} exceeds maximum {}",
                tx_size, self.max_tx_size
            )));
        }

        // Check amount based on payload type
        let amount = match &tx.payload {
            TxPayload::Transfer { amount, .. } => *amount,
            TxPayload::Coinbase { amount: _, .. } => {
                return Err(ValidationError::InvalidFormat(
                    "Coinbase tx not allowed in mempool".to_string(),
                ));
            }
            TxPayload::ContractDeploy { .. } => 0, // Deploy can have 0 amount
            TxPayload::ContractCall { .. } => 0,   // Call can have 0 amount
        };

        if amount == 0 && matches!(tx.payload, TxPayload::Transfer { .. }) {
            return Err(ValidationError::InvalidFormat(
                "Transfer transaction amount cannot be zero".to_string(),
            ));
        }

        // Check gas limit is reasonable
        if tx.gas_limit == 0 {
            return Err(ValidationError::InvalidFormat(
                "Gas limit cannot be zero".to_string(),
            ));
        }

        // Check addresses are not zero
        if tx.from == [0u8; 20] {
            return Err(ValidationError::InvalidFormat(
                "From address cannot be zero".to_string(),
            ));
        }

        // Check payload-specific validation
        match &tx.payload {
            TxPayload::Transfer { to, .. } => {
                if *to == [0u8; 20] {
                    return Err(ValidationError::InvalidFormat(
                        "To address cannot be zero".to_string(),
                    ));
                }
                // Check from != to (prevent self-transfers that might be used for DoS)
                if tx.from == *to {
                    return Err(ValidationError::InvalidFormat(
                        "From and to addresses cannot be the same".to_string(),
                    ));
                }
            }
            TxPayload::Coinbase {
                amount: _,
                to: _,
                height: _,
                blue_score: _,
            } => {
                // Coinbase should not reach pre-validation as it's not in mempool
                return Err(ValidationError::InvalidFormat(
                    "Coinbase transactions are not allowed in mempool".to_string(),
                ));
            }
            TxPayload::ContractDeploy { wasm_code, .. } => {
                if wasm_code.is_empty() {
                    return Err(ValidationError::InvalidFormat(
                        "Contract deploy must include WASM code".to_string(),
                    ));
                }
            }
            TxPayload::ContractCall {
                contract_address,
                method,
                ..
            } => {
                if *contract_address == [0u8; 20] {
                    return Err(ValidationError::InvalidFormat(
                        "Contract address cannot be zero".to_string(),
                    ));
                }
                if method.is_empty() {
                    return Err(ValidationError::InvalidFormat(
                        "Contract method cannot be empty".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    fn validate_signature(&self, tx: &Transaction) -> Result<(), ValidationError> {
        // Check if signature exists
        // Basic signature format check (full verification happens in batch verifier)
        if tx.signature.data.len() != 65 {
            return Err(ValidationError::InvalidSignature);
        }

        Ok(())
    }

    fn validate_gas(&self, tx: &Transaction) -> Result<(), ValidationError> {
        // Check gas limit
        if tx.gas_limit > self.max_gas_limit {
            return Err(ValidationError::GasLimitExceeded);
        }

        // Check gas price
        if tx.gas_price < self.min_gas_price {
            return Err(ValidationError::InvalidGasPrice);
        }

        // Check gas price is not unreasonably high (potential DoS)
        if tx.gas_price > self.min_gas_price * 1000 {
            return Err(ValidationError::InvalidGasPrice);
        }

        Ok(())
    }

    fn validate_addresses(&self, tx: &Transaction) -> Result<(), ValidationError> {
        // Check if addresses are blacklisted
        if self.blacklisted_addresses.contains(&tx.from) {
            return Err(ValidationError::BlacklistedAddress);
        }

        match &tx.payload {
            TxPayload::Transfer { to, .. }
            | TxPayload::ContractCall {
                contract_address: to,
                ..
            } => {
                if self.blacklisted_addresses.contains(to) {
                    return Err(ValidationError::BlacklistedAddress);
                }
            }
            TxPayload::Coinbase { to, .. } => {
                if self.blacklisted_addresses.contains(to) {
                    return Err(ValidationError::BlacklistedAddress);
                }
            }
            TxPayload::ContractDeploy { .. } => {
                // No additional address to check for deploy
            }
        }

        Ok(())
    }

    fn check_duplicate(&self, tx: &Transaction) -> Result<(), ValidationError> {
        let tx_hash = tx.hash();
        if self.known_tx_hashes.contains(&tx_hash) {
            return Err(ValidationError::DuplicateTransaction);
        }

        Ok(())
    }

    fn validate_block_transactions(&self, block: &Block) -> PreValidationResult {
        let mut all_errors = Vec::new();
        let mut seen_hashes = HashSet::new();

        for (idx, tx) in block.transactions.iter().enumerate() {
            // Individual validation
            let result = self.pre_validate_transaction(tx);
            if !result.is_valid {
                for error in result.errors {
                    all_errors.push(ValidationError::InvalidFormat(format!(
                        "Transaction {}: {:?}",
                        idx, error
                    )));
                }
            }

            // Check for duplicates within block
            let tx_hash = tx.hash();
            if seen_hashes.contains(&tx_hash) {
                all_errors.push(ValidationError::InvalidFormat(format!(
                    "Duplicate transaction in block at index {}",
                    idx
                )));
            }
            seen_hashes.insert(tx_hash);
        }

        PreValidationResult {
            is_valid: all_errors.is_empty(),
            errors: all_errors,
        }
    }
}

impl Default for PreTransactionValidator {
    fn default() -> Self {
        Self::new(
            8_000_000,     // max gas limit
            1_000_000_000, // min gas price (1 gwei)
            128 * 1024,    // max tx size (128KB)
        )
    }
}

/// Block validator functions
pub mod block_validator {
    use super::*;

    pub fn validate_basic(block: &Block) -> Result<(), String> {
        // Check if block has at least one parent
        if block.parents.is_empty() {
            return Err("Block must have at least one parent".to_string());
        }

        // Check timestamp is not too far in the future
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if block.timestamp > now + 300 {
            return Err("Block timestamp is too far in the future".to_string());
        }

        // Check timestamp is not too old
        if block.timestamp < now.saturating_sub(3600 * 24 * 7) {
            return Err("Block timestamp is too old".to_string());
        }

        Ok(())
    }

    pub fn validate_references(block: &Block) -> Result<(), String> {
        // Check for duplicate parents
        let mut seen_parents = std::collections::HashSet::new();
        for parent in &block.parents {
            if !seen_parents.insert(parent) {
                return Err("Block has duplicate parents".to_string());
            }
        }

        Ok(())
    }
}

/// Block validation functions (consolidated)
pub mod block_validation {
    use super::*;

    pub fn validate_basic(block: &Block) -> Result<(), String> {
        // Check if hash matches content
        // verify header/hash consistency
        let calculated_hash = block.calculate_hash();
        if block.hash != calculated_hash {
            return Err("Block hash mismatch".to_string());
        }

        // verify PoW meets difficulty (difficulty of 0 is treated as no-PoW)
        // Skip PoW validation for bootstrap blocks
        if block.header.difficulty > 0
            && !crate::consensus::meets_difficulty(&block.hash, block.header.difficulty)
        {
            return Err("Block does not satisfy PoW difficulty".to_string());
        }
        // state_root validity is checked elsewhere (during sync)

        // verify reward matches expectation (base mining reward only; fees credited separately)
        // For bootstrap blocks (height 0), use a simple calculation
        let tx_count = block.transactions.len().saturating_sub(1); // Exclude coinbase transaction
        let expected = if block.header.height <= 1 {
            block.reward // Accept any reward for bootstrap blocks
        } else {
            crate::utils::calculations::calculate_expected_block_reward(0, 0.0, &[], tx_count, &crate::utils::calculations::RewardConfig::default())
        };
        if block.header.height > 1 && block.reward != expected {
            return Err(format!(
                "Unexpected block reward: {} vs {}",
                block.reward, expected
            ));
        }

        // Check for duplicate transactions
        let tx_hashes: std::collections::HashSet<_> = block.transactions.iter().map(|t| t.hash()).collect();
        if tx_hashes.len() != block.transactions.len() {
            return Err("Duplicate transactions found".to_string());
        }

        // Check timestamp is reasonable (not in far future)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if block.header.timestamp > now + 3600 {
            return Err("Block timestamp too far in future".to_string());
        }

        Ok(())
    }

    pub fn validate_references(block: &Block) -> Result<(), String> {
        // Genesis may have no parents; non-genesis blocks should have at least one.
        if block.parents.is_empty() {
            return Ok(());
        }

        // Check for duplicate parents
        let mut seen_parents = std::collections::HashSet::new();
        for parent in &block.parents {
            if !seen_parents.insert(parent) {
                return Err("Block has duplicate parents".to_string());
            }
        }

        Ok(())
    }
}

/// Transaction validation functions (consolidated)
pub mod transaction_validation {
    use super::*;

    pub fn validate_basic(tx: &Transaction) -> Result<(), String> {
        // gas limit must be >0 (except for coinbase, where it can be 0)
        if tx.gas_limit == 0 {
            if !matches!(tx.payload, TxPayload::Coinbase { .. }) {
                return Err("Gas limit must be greater than 0".to_string());
            }
        }

        // gas price must be >0 (except for coinbase)
        if tx.gas_price == 0 {
            if !matches!(tx.payload, TxPayload::Coinbase { .. }) {
                return Err("Gas price must be greater than 0".to_string());
            }
        }

        // note: transfer must have amount >0
        if let TxPayload::Transfer { amount, .. } = &tx.payload {
            if *amount == 0 {
                return Err("Amount must be greater than 0".to_string());
            }
        }

        Ok(())
    }
}