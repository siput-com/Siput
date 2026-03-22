use crate::core::{Block, Transaction};
use crate::utils::validator::PreTransactionValidator as InnerValidator;

// Re-export for compatibility
pub use crate::utils::validator::{PreValidationResult, ValidationError};

/// Stateless transaction pre-validator (wrapper for compatibility)
pub struct TransactionPreValidator {
    inner: InnerValidator,
}

impl TransactionPreValidator {
    /// Create new pre-validator
    pub fn new(max_gas_limit: u64, min_gas_price: u64, max_tx_size: usize) -> Self {
        Self {
            inner: InnerValidator::new(max_gas_limit, min_gas_price, max_tx_size),
        }
    }

    /// Pre-validate a single transaction
    pub fn pre_validate_transaction(&self, tx: &Transaction) -> PreValidationResult {
        self.inner.pre_validate_transaction(tx)
    }

    /// Pre-validate multiple transactions
    pub fn pre_validate_transactions(
        &self,
        transactions: &[Transaction],
    ) -> Vec<PreValidationResult> {
        self.inner.pre_validate_transactions(transactions)
    }

    /// Validate block transactions (basic checks)
    pub fn validate_block_transactions(&self, block: &Block) -> PreValidationResult {
        self.inner.validate_block_transactions(block)
    }

    /// Estimate gas usage for transaction (rough estimate)
    pub fn estimate_gas_usage(&self, tx: &Transaction) -> u64 {
        self.inner.estimate_gas_usage(tx)
    }

    /// Add transaction hash to known set (for duplicate detection)
    pub fn add_known_transaction(&mut self, tx_hash: [u8; 32]) {
        self.inner.add_known_transaction(tx_hash);
    }

    /// Remove transaction hash from known set
    pub fn remove_known_transaction(&mut self, tx_hash: &[u8; 32]) {
        self.inner.remove_known_transaction(tx_hash);
    }

    /// Clear known transactions (periodic cleanup)
    pub fn clear_known_transactions(&mut self) {
        self.inner.clear_known_transactions();
    }

    /// Add address to blacklist
    pub fn blacklist_address(&mut self, address: [u8; 20]) {
        self.inner.blacklist_address(address);
    }

    /// Remove address from blacklist
    pub fn unblacklist_address(&mut self, address: &[u8; 20]) {
        self.inner.unblacklist_address(address);
    }

    /// Get current blacklist size
    pub fn blacklist_size(&self) -> usize {
        self.inner.blacklist_size()
    }

    /// Get known transactions count
    pub fn known_transactions_count(&self) -> usize {
        self.inner.known_transactions_count()
    }
}

impl Default for TransactionPreValidator {
    fn default() -> Self {
        Self::new(
            8_000_000,     // max gas limit
            1_000_000_000, // min gas price (1 gwei)
            128 * 1024,    // max tx size (128KB)
        )
    }
}

    #[test]
    fn test_valid_transaction() {
        let validator = TransactionPreValidator::default();
        let tx = create_test_transaction(100, 0);

        let result = validator.pre_validate_transaction(&tx);
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_zero_amount() {
        let validator = TransactionPreValidator::default();
        let tx = create_test_transaction(0, 0);

        let result = validator.pre_validate_transaction(&tx);
        assert!(!result.is_valid);
        assert!(matches!(
            result.errors[0],
            ValidationError::InvalidFormat(_)
        ));
    }

    #[test]
    fn test_self_transfer() {
        let validator = TransactionPreValidator::default();
        let mut tx = create_test_transaction(100, 0);
        // Modify the payload to make it a self-transfer
        if let TxPayload::Transfer { to, .. } = &mut tx.payload {
            *to = tx.from;
        }

        let result = validator.pre_validate_transaction(&tx);
        assert!(!result.is_valid);
        assert!(matches!(
            result.errors[0],
            ValidationError::InvalidFormat(_)
        ));
    }

    #[test]
    fn test_gas_limit_exceeded() {
        let validator = TransactionPreValidator::new(1000, 1_000_000_000, 128 * 1024);
        let mut tx = create_test_transaction(100, 0);
        tx.gas_limit = 2000; // Exceeds limit

        let result = validator.pre_validate_transaction(&tx);
        assert!(!result.is_valid);
        assert!(matches!(
            result.errors[0],
            ValidationError::GasLimitExceeded
        ));
    }

    #[test]
    fn test_duplicate_detection() {
        let mut validator = TransactionPreValidator::default();
        let tx = create_test_transaction(100, 0);
        let tx_hash = tx.hash();

        // First time should be valid
        let result1 = validator.pre_validate_transaction(&tx);
        assert!(result1.is_valid);

        // Add to known transactions
        validator.add_known_transaction(tx_hash);

        // Second time should be duplicate
        let result2 = validator.pre_validate_transaction(&tx);
        assert!(!result2.is_valid);
        assert!(matches!(
            result2.errors[0],
            ValidationError::DuplicateTransaction
        ));
    }

    #[test]
    fn test_blacklist() {
        let mut validator = TransactionPreValidator::default();
        let tx = create_test_transaction(100, 0);

        // Add sender to blacklist
        validator.blacklist_address(tx.from);

        let result = validator.pre_validate_transaction(&tx);
        assert!(!result.is_valid);
        assert!(matches!(
            result.errors[0],
            ValidationError::BlacklistedAddress
        ));
    }

    #[test]
    fn test_gas_estimation() {
        let validator = TransactionPreValidator::default();
        let mut tx = create_test_transaction(100, 0);
        tx.payload = TxPayload::ContractDeploy {
            wasm_code: vec![1, 2, 3, 0, 0],
            init_args: vec![],
        };

        let gas = validator.estimate_gas_usage(&tx);
        assert!(gas >= 21000); // Base gas
        assert!(gas <= tx.gas_limit);
    }
