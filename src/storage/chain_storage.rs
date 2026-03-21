use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, Options, DB};

use crate::core::{Block, BlockHash};
use crate::state::state_manager::StateManager;

/// Column family names for persistent chain storage.
pub const BLOCKS_CF: &str = "blocks";
pub const STATE_CF: &str = "state";
pub const UTXO_CF: &str = "utxo";
pub const TIPS_CF: &str = "tips";
pub const META_CF: &str = "meta";

/// Persistent chain storage using RocksDB.
#[derive(Clone)]
pub struct ChainStorage {
    db: Arc<DB>,
}

impl ChainStorage {
    /// Open or create the chain storage database at the given path.
    pub fn open(path: &str) -> Result<Self, String> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_max_open_files(1000);

        let cfs = vec![
            ColumnFamilyDescriptor::new(BLOCKS_CF, Options::default()),
            ColumnFamilyDescriptor::new(STATE_CF, Options::default()),
            ColumnFamilyDescriptor::new(UTXO_CF, Options::default()),
            ColumnFamilyDescriptor::new(TIPS_CF, Options::default()),
            ColumnFamilyDescriptor::new(META_CF, Options::default()),
        ];

        let db = DB::open_cf_descriptors(&opts, path, cfs)
            .map_err(|e| format!("Failed to open chain storage: {}", e))?;

        Ok(ChainStorage { db: Arc::new(db) })
    }

    /// Check whether the chain storage is empty (no blocks stored).
    pub fn is_empty(&self) -> bool {
        let cf = match self.db.cf_handle(BLOCKS_CF) {
            Some(cf) => cf,
            None => return true,
        };

        self.db
            .iterator_cf(cf, rocksdb::IteratorMode::Start)
            .next()
            .is_none()
    }

    /// Store a block in the chain storage.
    pub fn put_block(&self, block: &Block) -> Result<(), String> {
        let cf = self
            .db
            .cf_handle(BLOCKS_CF)
            .ok_or("Blocks column family missing")?;
        let data =
            bincode::serialize(block).map_err(|e| format!("Failed to serialize block: {}", e))?;
        self.db
            .put_cf(cf, &block.hash, data)
            .map_err(|e| format!("Failed to write block: {}", e))
    }

    /// Retrieve a block by hash.
    pub fn get_block(&self, hash: &BlockHash) -> Option<Block> {
        let cf = self.db.cf_handle(BLOCKS_CF)?;
        match self.db.get_cf(cf, hash) {
            Ok(Some(bytes)) => bincode::deserialize(&bytes).ok(),
            _ => None,
        }
    }

    /// Load all blocks from storage.
    pub fn get_all_blocks(&self) -> Vec<Block> {
        let cf = match self.db.cf_handle(BLOCKS_CF) {
            Some(cf) => cf,
            None => return vec![],
        };

        let mut blocks = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            if let Ok((_key, value)) = item {
                if let Ok(block) = bincode::deserialize::<Block>(&value) {
                    blocks.push(block);
                }
            }
        }
        blocks
    }

    /// Persist the full state manager.
    pub fn persist_state(&self, state: &StateManager) -> Result<(), String> {
        let cf = self
            .db
            .cf_handle(STATE_CF)
            .ok_or("State column family missing")?;
        let data =
            bincode::serialize(state).map_err(|e| format!("Failed to serialize state: {}", e))?;
        self.db
            .put_cf(cf, b"state", data)
            .map_err(|e| format!("Failed to write state: {}", e))
    }

    /// Load state manager from storage.
    pub fn load_state(&self) -> Option<StateManager> {
        let cf = self.db.cf_handle(STATE_CF)?;
        match self.db.get_cf(cf, b"state") {
            Ok(Some(bytes)) => bincode::deserialize(&bytes).ok(),
            _ => None,
        }
    }

    /// Persist tips (vector of tip hashes).
    pub fn persist_tips(&self, tips: &[BlockHash]) -> Result<(), String> {
        let cf = self
            .db
            .cf_handle(TIPS_CF)
            .ok_or("Tips column family missing")?;
        let data =
            bincode::serialize(tips).map_err(|e| format!("Failed to serialize tips: {}", e))?;
        self.db
            .put_cf(cf, b"tips", data)
            .map_err(|e| format!("Failed to write tips: {}", e))
    }

    /// Load tips stored in DB.
    pub fn load_tips(&self) -> Option<Vec<BlockHash>> {
        let cf = self.db.cf_handle(TIPS_CF)?;
        match self.db.get_cf(cf, b"tips") {
            Ok(Some(bytes)) => bincode::deserialize(&bytes).ok(),
            _ => None,
        }
    }

    /// Persist metadata key/value.
    pub fn put_meta(&self, key: &str, value: &[u8]) -> Result<(), String> {
        let cf = self
            .db
            .cf_handle(META_CF)
            .ok_or("Meta column family missing")?;
        self.db
            .put_cf(cf, key.as_bytes(), value)
            .map_err(|e| format!("Failed to write meta: {}", e))
    }

    /// Retrieve metadata.
    pub fn get_meta(&self, key: &str) -> Option<Vec<u8>> {
        let cf = self.db.cf_handle(META_CF)?;
        match self.db.get_cf(cf, key.as_bytes()) {
            Ok(Some(bytes)) => Some(bytes.to_vec()),
            _ => None,
        }
    }

    /// Flush the underlying database.
    pub fn flush(&self) -> Result<(), String> {
        self.db
            .flush()
            .map_err(|e| format!("Failed to flush RocksDB: {}", e))
    }
}
