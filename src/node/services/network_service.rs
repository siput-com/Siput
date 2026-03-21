use std::sync::Arc;
use parking_lot::Mutex;

use crate::network::{DiscoveryManager, StateSyncManager};

/// Peer information structure
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub id: String,
    pub address: String,
    pub status: String,
}

/// Service untuk menangani network operations
pub struct NetworkService {
    /// Discovery manager for network peer orchestration
    discovery_manager: Arc<DiscoveryManager>,
    /// State sync manager
    state_sync_manager: Option<Arc<parking_lot::Mutex<StateSyncManager>>>,
}

impl NetworkService {
    /// Buat network service baru
    pub fn new(
        discovery_manager: Arc<DiscoveryManager>,
        state_sync_manager: Option<Arc<parking_lot::Mutex<StateSyncManager>>>,
    ) -> Self {
        Self {
            discovery_manager,
            state_sync_manager,
        }
    }

    /// Get connected peers
    pub fn get_connected_peers(&self) -> Vec<String> {
        // If active P2P node exists, prefer it
        if let Some(peer_ids) = self.get_connected_peers_from_p2p() {
            return peer_ids;
        }

        // Fallback to discovery manager known peers
        self.discovery_manager.get_connected_peer_addresses()
    }

    /// Get peer count
    pub fn get_peer_count(&self) -> usize {
        self.get_connected_peers().len()
    }

    /// Get connection count alias
    pub fn get_connection_count(&self) -> usize {
        self.get_peer_count()
    }

    /// If P2P component is active, return peer list
    fn get_connected_peers_from_p2p(&self) -> Option<Vec<String>> {
        // In simplified implementation, return None
        // Full implementation would query active P2P node
        None
    }

    /// Get detailed peer information
    pub async fn get_detailed_peers(&self) -> Result<Vec<PeerInfo>, String> {
        // In this simplified runtime, return empty list
        // Full implementation would query P2P network
        Ok(Vec::new())
    }
}