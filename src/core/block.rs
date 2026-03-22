use crate::core::transaction::Transaction;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;

pub type BlockHash = [u8; 32];
pub type TransactionId = String;

/// Interface abstraction for consensus to avoid hard core dependency details
pub trait ConsensusBlock {
    fn hash(&self) -> BlockHash;
    fn parent_hashes(&self) -> &[BlockHash];
    fn blue_score(&self) -> u64;
    fn chain_height(&self) -> u64;
    fn apply_consensus_metadata(
        &mut self,
        selected_parent: Option<BlockHash>,
        blue_score: u64,
        chain_height: u64,
        topo_index: u64,
    );
}

/// Header portion of a block which is used for hashing and PoW
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BlockHeader {
    pub parent_hashes: Vec<BlockHash>,
    pub timestamp: u64,
    pub nonce: u64,
    pub difficulty: u32,      // expressed as leading-zero bits requirement
    pub base_fee: u64,        // EIP-1559 style base fee burned per gas unit
    pub state_root: [u8; 32], // merkle root of state after applying this block
    pub version: u32,

    // GhostDAG metrics
    pub blue_score: u64,
    pub selected_parent: Option<BlockHash>,
    pub chain_height: u64,
    pub topo_index: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub hash: BlockHash,
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub producer: [u8; 20],
    pub reward: u64,
    pub height: u64,
}

impl Block {
    /// Create new block with given parameters
    /// Create new block with given parameters.  The `difficulty` field is
    /// typically supplied by the miner and affects the PoW target; for regular
    /// construction it can be zero.
    pub fn new(
        parent_hashes: Vec<BlockHash>,
        timestamp: u64,
        transactions: Vec<Transaction>,
        nonce: u64,
        difficulty: u32,
        base_fee: u64,
        producer: [u8; 20],
        state_root: [u8; 32],
    ) -> Self {
        // Base mining reward plus transaction fee contributions.
        let base = crate::consensus::calculate_expected_block_reward(
            0,
            0.0,
            &[],
            transactions.len(),
            &crate::consensus::RewardConfig::default(),
        );
        let tx_fee: u64 = transactions.iter().map(|tx| tx.gas_price).sum();
        let reward = base.saturating_add(tx_fee);
        let header = BlockHeader {
            parent_hashes,
            timestamp,
            nonce,
            difficulty,
            base_fee,
            state_root,
            version: 1,
            blue_score: 0,
            selected_parent: None,
            chain_height: 0,
            topo_index: 0,
        };
        let mut block = Block {
            hash: [0u8; 32],
            header,
            transactions,
            producer,
            reward,
            height: 0,
        };
        block.hash = block.calculate_hash();
        block
    }

    /// Calculate SHA256 hash of entire block content
    pub fn calculate_hash(&self) -> BlockHash {
        let mut hasher = Sha256::new();

        // Hash parents
        for parent in &self.header.parent_hashes {
            hasher.update(parent);
        }

        // Hash transactions
        for tx in &self.transactions {
            hasher.update(tx.hash());
        }

        // Hash header metadata
        hasher.update(self.header.timestamp.to_le_bytes());
        hasher.update(self.header.nonce.to_le_bytes());
        hasher.update(self.header.difficulty.to_le_bytes());
        hasher.update(self.header.base_fee.to_le_bytes());
        hasher.update(&self.header.state_root);
        hasher.update(self.header.version.to_le_bytes());
        // GhostDAG metadata should not affect block ID for topology consistency.
        // We keep producer in hash to avoid easy collisions by different miners.
        hasher.update(&self.producer);

        hasher.finalize().into()
    }

    /// Validate basic block structure
    pub fn validate_basic(&self) -> Result<(), String> {
        crate::utils::validator::block_validation::validate_basic(self)
    }

    /// Validate block has all references to parent blocks
    pub fn validate_references(&self) -> Result<(), String> {
        crate::utils::validator::block_validation::validate_references(self)
    }

    /// Update GhostDAG-specific metadata and recompute hash for consistency
    pub fn set_consensus_metadata(
        &mut self,
        selected_parent: Option<BlockHash>,
        blue_score: u64,
        chain_height: u64,
        topo_index: u64,
    ) {
        self.header.selected_parent = selected_parent;
        self.header.blue_score = blue_score;
        self.header.chain_height = chain_height;
        self.header.topo_index = topo_index;
        self.height = chain_height;
        self.hash = self.calculate_hash();
    }

    /// Check if this is a genesis block (no parents or explicit genesis)
    pub fn is_genesis(&self) -> bool {
        self.header.parent_hashes.is_empty()
    }

    /// Get all transaction hashes in this block
    pub fn transaction_hashes(&self) -> Vec<String> {
        self.transactions
            .iter()
            .map(|t| hex::encode(t.hash()))
            .collect()
    }

    /// Get size estimate in bytes
    pub fn size_estimate(&self) -> usize {
        std::mem::size_of::<BlockHash>()
            + self.header.parent_hashes.len() * std::mem::size_of::<BlockHash>()
            + self.transactions.len() * (std::mem::size_of::<TransactionId>() + 256 + 8)
            + 8  // timestamp
            + 8  // nonce
            + 4  // version
            + 4  // difficulty
            + 20 // producer address
            + 8 // reward
    }
}

impl ConsensusBlock for Block {
    fn hash(&self) -> BlockHash {
        self.hash
    }

    fn parent_hashes(&self) -> &[BlockHash] {
        &self.header.parent_hashes
    }

    fn blue_score(&self) -> u64 {
        self.header.blue_score
    }

    fn chain_height(&self) -> u64 {
        self.height
    }

    fn apply_consensus_metadata(&mut self,
        selected_parent: Option<BlockHash>,
        blue_score: u64,
        chain_height: u64,
        topo_index: u64,
    ) {
        Block::set_consensus_metadata(self, selected_parent, blue_score, chain_height, topo_index);
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Block {{ hash: {}, parents: {:?}, ts: {}, txs: {}, nonce: {}, diff: {}, producer: {}, reward: {} }}",
            hex::encode(&self.hash[..std::cmp::min(16, self.hash.len())]),
            &self.header.parent_hashes,
            self.header.timestamp,
            self.transactions.len(),
            self.header.nonce,
            self.header.difficulty,
            hex::encode(self.producer),
            self.reward
        )
    }
}

impl Default for Block {
    fn default() -> Self {
        Block::new(vec![], 0, vec![], 0, 0, 0, [0; 20], [0; 32])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_creation() {
        let from: [u8; 20] = [1; 20];
        let to: [u8; 20] = [2; 20];
        let tx = Transaction::new_transfer(from, to, 100, 1, 21000, 1);
        if let crate::core::transaction::TxPayload::Transfer { amount, .. } = tx.payload {
            assert_eq!(amount, 100);
        } else {
            panic!("Expected transfer payload, got {:?}", tx.payload);
        }
        assert_eq!(tx.nonce, 1);
    }

    #[test]
    fn test_block_hash_calculation() {
        let from: [u8; 20] = [1; 20];
        let to: [u8; 20] = [2; 20];
        let tx = Transaction::new(from, to, 100, 0, 21000, 1);
        let block = Block::new(vec![], 1000, vec![tx], 42, 0, 0, [0; 20], [0; 32]);

        assert_eq!(block.hash.len(), 32); // SHA256 produces 32 bytes
    }

    #[test]
    fn test_block_validation_basic() {
        let from: [u8; 20] = [1; 20];
        let to: [u8; 20] = [2; 20];
        let tx = Transaction::new(from, to, 100, 0, 21000, 1);
        let block = Block::new(vec![], 1000, vec![tx], 42, 0, 0, [0; 20], [0; 32]);

        assert!(block.validate_basic().is_ok());
    }

    #[test]
    fn test_block_genesis_detection() {
        let block = Block::new(vec![], 1000, vec![], 0, 0, 0, [0; 20], [0; 32]);
        assert!(block.is_genesis());
    }

    #[test]
    fn test_block_with_multiple_parents() {
        let parent1: BlockHash = [1; 32];
        let parent2: BlockHash = [2; 32];
        let from: [u8; 20] = [1; 20];
        let to: [u8; 20] = [2; 20];
        let tx = Transaction::new(from, to, 100, 0, 21000, 1);

        let block = Block::new(
            vec![parent1, parent2],
            1000,
            vec![tx],
            42,
            0,
            0,
            [0; 20],
            [0; 32],
        );
        assert_eq!(block.header.parent_hashes.len(), 2);
        assert!(block.validate_basic().is_ok());
    }

    #[test]
    fn test_duplicate_transaction_detection() {
        let from: [u8; 20] = [1; 20];
        let to: [u8; 20] = [2; 20];
        let tx1 = Transaction::new(from, to, 100, 0, 21000, 1);
        let tx2 = Transaction::new(from, to, 100, 0, 21000, 1); // Exact duplicate

        let block = Block::new(vec![], 1000, vec![tx1, tx2], 42, 0, 0, [0; 20], [0; 32]);
        assert!(block.validate_basic().is_err());
    }

    #[test]
    fn test_hash_consistency() {
        let from: [u8; 20] = [1; 20];
        let to: [u8; 20] = [2; 20];
        let tx = Transaction::new(from, to, 100, 0, 21000, 1);
        let parent: BlockHash = [4; 32];
        let block1 = Block::new(
            vec![parent],
            1000,
            vec![tx.clone()],
            42,
            0,
            0,
            [0; 20],
            [0; 32],
        );
        let block2 = Block::new(vec![parent], 1000, vec![tx], 42, 0, 0, [0; 20], [0; 32]);

        assert_eq!(block1.hash, block2.hash); // same inputs including difficulty 0
    }

    #[test]
    fn test_reward_calculation_matches_consensus() {
        let from: [u8; 20] = [1; 20];
        let to: [u8; 20] = [2; 20];
        // empty block
        let b0 = Block::new(vec![], 1000, vec![], 0, 0, 0, [0; 20], [0; 32]);
        assert_eq!(b0.reward, crate::consensus::calculate_expected_block_reward(0, 0.0, &[], 0, &crate::consensus::mining::RewardConfig::default()));
        // moderate tx count
        let mut txs = Vec::new();
        for i in 0..50 {
            txs.push(Transaction::new(from, to, 100, i, 21000, 1));
        }
        let b1 = Block::new(vec![], 1000, txs.clone(), 0, 0, 0, [0; 20], [0; 32]);
        // Reward should include base reward + total tip fees (gas_price - base_fee) per tx.
        let expected = crate::consensus::calculate_expected_block_reward(0, 0.0, &[], 50, &crate::consensus::mining::RewardConfig::default()) + 50u64;
        assert_eq!(b1.reward, expected);
    }

    #[test]
    fn test_pow_check_in_validation() {
        // build a block with difficulty 1 (very easy)
        let from: [u8; 20] = [1; 20];
        let to: [u8; 20] = [2; 20];
        let tx = Transaction::new(from, to, 100, 0, 21000, 1);
        let mut block = Block::new(vec![], 1000, vec![tx], 0, 1, 0, [0; 20], [0; 32]);
        // we may need to adjust nonce until satisfies
        let mut nonce = 0;
        while !crate::consensus::meets_difficulty(&block.hash, block.header.difficulty)
            && nonce < 1000
        {
            nonce += 1;
            block.header.nonce = nonce;
            block.hash = block.calculate_hash();
        }
        assert!(crate::consensus::meets_difficulty(&block.hash, 1));
        assert!(block.validate_basic().is_ok());
    }
}
