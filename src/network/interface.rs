use async_trait::async_trait;

/// Interface for network communication across node components.
#[async_trait]
pub trait NetworkInterface: Send + Sync {
    async fn broadcast_block(&self, block: crate::core::Block) -> Result<(), String>;
    async fn broadcast_transaction(&self, tx: crate::core::Transaction) -> Result<(), String>;
}
