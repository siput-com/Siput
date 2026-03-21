use crate::core::{Block, Transaction, TransactionId};
use crate::state::state_manager::StateManager;
use std::any::Any;

/// Plugin context providing access to blockchain components
pub struct PluginContext<'a> {
    pub state_manager: &'a mut StateManager,
    pub current_block: Option<&'a Block>,
}

/// Plugin trait defining the interface for blockchain plugins
pub trait Plugin: Send + Sync + Any {
    /// Get plugin name
    fn name(&self) -> &str;

    /// Get plugin version
    fn version(&self) -> &str;

    /// Initialize plugin with context
    fn init(&mut self, context: &mut PluginContext) -> Result<(), String>;

    /// Start plugin operations
    fn start(&mut self, context: &mut PluginContext) -> Result<(), String>;

    /// Stop plugin operations
    fn stop(&mut self, context: &mut PluginContext) -> Result<(), String>;

    /// Handle incoming transaction
    /// Return true if transaction was handled by this plugin
    fn handle_transaction(&mut self, tx: &Transaction, context: &mut PluginContext) -> Result<bool, String>;

    /// Handle block processing
    /// Called after block is validated but before state update
    fn process_block(&mut self, block: &Block, context: &mut PluginContext) -> Result<(), String>;

    /// Get plugin-specific transaction types this plugin handles
    fn supported_transaction_types(&self) -> Vec<u8> {
        Vec::new()
    }

    /// Validate plugin-specific transaction
    fn validate_transaction(&self, tx: &Transaction, context: &PluginContext) -> Result<(), String> {
        Ok(())
    }

    /// Execute plugin-specific logic
    fn execute(&mut self, _method: &str, _params: &[u8], _context: &mut PluginContext) -> Result<Vec<u8>, String> {
        Err("Method not implemented".to_string())
    }

    /// Get plugin metadata
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: self.name().to_string(),
            version: self.version().to_string(),
            description: String::new(),
            author: String::new(),
        }
    }
}

/// Plugin metadata
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
}

/// Helper trait for downcasting plugins
pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Plugin> AsAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}