//! Abstraction layer for common data access patterns
//!
//! This module provides unified interfaces for accessing data across different components,
//! reducing duplication and providing consistent APIs.

use crate::core::{Block, BlockHash, Transaction};
use crate::network::PeerId;

/// Common data access traits
pub mod traits {
    use super::*;

    /// Trait for storage operations - all database access must go through this abstraction
    pub trait Storage: Send + Sync {
        /// Block operations
        fn insert_block(&self, block: &Block) -> Result<(), String>;
        fn get_block(&self, hash: &BlockHash) -> Result<Option<Block>, String>;
        fn block_exists(&self, hash: &BlockHash) -> Result<bool, String>;
        fn get_blocks_in_range(&self, start_height: u64, end_height: u64) -> Result<Vec<Block>, String>;

        /// Transaction operations
        fn insert_transaction(&self, tx: &Transaction) -> Result<(), String>;
        fn get_transaction(&self, hash: &BlockHash) -> Result<Option<Transaction>, String>;
        fn transaction_exists(&self, hash: &BlockHash) -> Result<bool, String>;

        /// Metadata operations
        fn set_metadata(&self, key: &str, value: &[u8]) -> Result<(), String>;
        fn get_metadata(&self, key: &str) -> Result<Option<Vec<u8>>, String>;

        /// Batch operations
        fn batch_write(&self, operations: Vec<StorageOperation>) -> Result<(), String>;

        /// Maintenance
        fn flush(&self) -> Result<(), String>;
        fn compact(&self) -> Result<(), String>;
    }

    /// Storage operation for batch writes
    pub enum StorageOperation {
        InsertBlock(Block),
        InsertTransaction(Transaction),
        SetMetadata(String, Vec<u8>),
    }

    /// Trait for components that can provide peer information
    pub trait PeerProvider {
        fn get_connected_peers(&self) -> Vec<String>;
        fn get_all_peers(&self) -> Vec<String>;
        fn get_peer_info(&self, peer_id: &PeerId) -> Option<crate::network::PeerInfo>;
    }

    /// Trait for components that can provide transaction information
    pub trait TransactionProvider {
        fn get_pending_transactions(&self) -> Vec<crate::core::Transaction>;
        fn get_transaction_count(&self) -> usize;
    }

    /// Trait for components that can provide block information
    pub trait BlockProvider {
        fn get_block_hash(&self, height: u64) -> Option<BlockHash>;
        fn get_block_height(&self, hash: &BlockHash) -> Option<u64>;
    }
}

    /// Trait for components that can provide peer information
    pub trait PeerProvider {
        fn get_connected_peers(&self) -> Vec<String>;
        fn get_all_peers(&self) -> Vec<String>;
        fn get_peer_info(&self, peer_id: &PeerId) -> Option<crate::network::PeerInfo>;
    }

    /// Trait for components that can provide transaction information
    pub trait TransactionProvider {
        fn get_pending_transactions(&self) -> Vec<crate::core::Transaction>;
        fn get_transaction_count(&self) -> usize;
    }

    /// Trait for components that can provide block information
    pub trait BlockProvider {
        fn get_block_hash(&self, height: u64) -> Option<BlockHash>;
        fn get_block_height(&self, hash: &BlockHash) -> Option<u64>;
    }
}

/// Shared utility functions for data access
pub mod utils {
    use super::*;

    /// Get the total number of active connections across providers
    pub fn get_total_connections(providers: &[&dyn traits::PeerProvider]) -> usize {
        providers.iter().map(|p| p.get_connected_peers().len()).sum()
    }

    /// Get all unique peers from multiple providers
    pub fn get_all_unique_peers(providers: &[&dyn traits::PeerProvider]) -> Vec<String> {
        let mut all_peers = std::collections::HashSet::new();
        for provider in providers {
            all_peers.extend(provider.get_all_peers());
        }
        all_peers.into_iter().collect()
    }
}