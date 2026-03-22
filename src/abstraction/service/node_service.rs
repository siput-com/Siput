use crate::network::p2p_node::P2PNode;
use crate::rpc::interfaces::*;
use std::sync::Arc;

pub struct NodeService {
    p2p_node: Option<Arc<tokio::sync::RwLock<P2PNode>>>,
}

impl NodeService {
    pub fn new(p2p_node: Option<Arc<tokio::sync::RwLock<P2PNode>>>) -> Self {
        NodeService { p2p_node }
    }
}

#[async_trait::async_trait]
impl NetworkInterface for NodeService {
    async fn get_connected_peers(&self) -> Result<Vec<String>, RpcError> {
        if let Some(node_arc) = &self.p2p_node {
            let node = node_arc.read().await;
            let peers: Vec<String> = node
                .get_connected_peers()
                .into_iter()
                .map(|p| p.to_string())
                .collect();
            Ok(peers)
        } else {
            Ok(vec![])
        }
    }

    async fn get_network_stats(&self) -> Result<NetworkStats, RpcError> {
        Ok(NetworkStats {
            connected_peers: 0,
            bytes_sent: 0,
            bytes_received: 0,
            messages_sent: 0,
            messages_received: 0,
        })
    }
}
