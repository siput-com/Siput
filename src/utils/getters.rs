use crate::core::transaction::Address;

/// Abstraction layer for getter operations
pub trait StateGetter {
    fn get_balance(&self, address: &Address) -> u64;
    fn get_nonce(&self, address: &Address) -> u64;
    fn get_state_root(&self) -> [u8; 32];
}

/// Network getter abstraction
pub trait NetworkGetter {
    fn get_connected_peers(&self) -> Vec<String>;
    fn get_peer_count(&self) -> usize;
    fn get_connection_count(&self) -> usize;
}

/// Consensus getter abstraction
pub trait ConsensusGetter {
    fn get_finality_height(&self) -> Option<u64>;
    fn get_current_height(&self) -> u64;
    fn get_hash_rate(&self) -> f64;
    fn get_tips(&self) -> Vec<crate::core::BlockHash>;
}

/// Mempool getter abstraction
pub trait MempoolGetter {
    fn get_mempool_size(&self) -> usize;
}

/// Shared utility functions for getters
pub mod utils {
    use crate::core::transaction::Address;

    /// Get default balance (0) for non-existent addresses
    pub fn default_balance() -> u64 {
        0
    }

    /// Get default nonce (0) for non-existent addresses
    pub fn default_nonce() -> u64 {
        0
    }

    /// Check if address is valid (not zero)
    pub fn is_valid_address(address: &Address) -> bool {
        *address != [0u8; 20]
    }

    /// Format peer info for display
    pub fn format_peer_info(peer: &str, connected: bool) -> String {
        format!("{} ({})", peer, if connected { "connected" } else { "disconnected" })
    }

    /// Calculate connection percentage
    pub fn connection_percentage(connected: usize, total: usize) -> f64 {
        if total == 0 {
            0.0
        } else {
            (connected as f64 / total as f64) * 100.0
        }
    }
}