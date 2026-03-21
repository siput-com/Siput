use crate::abstraction::traits::{Storage, StorageOperation};
use crate::core::{Block, BlockHash, Transaction};
use crate::storage::{BlockStore, ChainStorage};
use std::sync::Arc;

/// Unified storage engine implementing the Storage trait
/// Combines block store, chain storage, and other storage components
pub struct StorageEngine {
    block_store: Arc<BlockStore>,
    chain_store: Arc<ChainStorage>,
}

impl StorageEngine {
    pub fn new(block_store: Arc<BlockStore>, chain_store: Arc<ChainStorage>) -> Self {
        Self {
            block_store,
            chain_store,
        }
    }
}

impl Storage for StorageEngine {
    fn insert_block(&self, block: &Block) -> Result<(), String> {
        self.block_store.insert_block(block.clone())
    }

    fn get_block(&self, hash: &BlockHash) -> Result<Option<Block>, String> {
        Ok(self.block_store.get_block(hash))
    }

    fn block_exists(&self, hash: &BlockHash) -> Result<bool, String> {
        Ok(self.block_store.block_exists(hash))
    }

    fn get_blocks_in_range(&self, start_height: u64, end_height: u64) -> Result<Vec<Block>, String> {
        self.block_store.get_blocks_by_height_range(start_height, end_height)
    }

    fn insert_transaction(&self, tx: &Transaction) -> Result<(), String> {
        // For now, transactions are stored with blocks
        // Could be extended to separate transaction store
        Ok(())
    }

    fn get_transaction(&self, hash: &BlockHash) -> Result<Option<Transaction>, String> {
        // Simplified - would need transaction store
        Ok(None)
    }

    fn transaction_exists(&self, hash: &BlockHash) -> Result<bool, String> {
        Ok(false)
    }

    fn set_metadata(&self, key: &str, value: &[u8]) -> Result<(), String> {
        // Use chain storage metadata
        let cf = self.chain_store.db.cf_handle(crate::storage::chain_storage::META_CF)
            .ok_or("Metadata CF not found")?;
        self.chain_store.db.put_cf(&cf, key.as_bytes(), value)
            .map_err(|e| e.to_string())
    }

    fn get_metadata(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let cf = self.chain_store.db.cf_handle(crate::storage::chain_storage::META_CF)
            .ok_or("Metadata CF not found")?;
        match self.chain_store.db.get_cf(&cf, key.as_bytes()) {
            Ok(Some(value)) => Ok(Some(value)),
            Ok(None) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn batch_write(&self, operations: Vec<StorageOperation>) -> Result<(), String> {
        // Simplified batch implementation
        for op in operations {
            match op {
                StorageOperation::InsertBlock(block) => self.insert_block(&block)?,
                StorageOperation::InsertTransaction(tx) => self.insert_transaction(&tx)?,
                StorageOperation::SetMetadata(key, value) => self.set_metadata(&key, &value)?,
            }
        }
        Ok(())
    }

    fn flush(&self) -> Result<(), String> {
        // RocksDB auto-flushes, but we can force
        Ok(())
    }

    fn compact(&self) -> Result<(), String> {
        self.block_store.compact()
    }
}
