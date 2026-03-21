use crate::plugins::trait_def::{Plugin, PluginContext};
use crate::state::state_manager::StateManager;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Plugin manager for loading, managing, and coordinating plugins
pub struct PluginManager {
    plugins: RwLock<HashMap<String, Box<dyn Plugin>>>,
    state_manager: Arc<RwLock<StateManager>>,
}

impl PluginManager {
    /// Create new plugin manager
    pub fn new(state_manager: Arc<RwLock<StateManager>>) -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
            state_manager,
        }
    }

    /// Register a plugin
    pub fn register_plugin(&self, plugin: Box<dyn Plugin>) -> Result<(), String> {
        let name = plugin.name().to_string();
        let mut plugins = self.plugins.write();

        if plugins.contains_key(&name) {
            return Err(format!("Plugin '{}' already registered", name));
        }

        plugins.insert(name, plugin);
        Ok(())
    }

    /// Unregister a plugin
    pub fn unregister_plugin(&self, name: &str) -> Result<(), String> {
        let mut plugins = self.plugins.write();
        plugins.remove(name).ok_or_else(|| format!("Plugin '{}' not found", name))?;
        Ok(())
    }

    /// Get plugin by name (immutable reference)
    pub fn get_plugin(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins.read().get(name).map(|p| p.as_ref())
    }

    /// Get plugin by name (mutable reference)
    pub fn get_plugin_mut(&self, name: &str) -> Option<&mut dyn Plugin> {
        self.plugins.write().get_mut(name).map(|p| p.as_mut())
    }

    /// Initialize all registered plugins
    pub fn init_all_plugins(&self) -> Result<(), String> {
        let mut state_manager = self.state_manager.write();
        let mut context = PluginContext {
            state_manager: &mut *state_manager,
            current_block: None,
        };

        let plugin_names: Vec<String> = self.plugins.read().keys().cloned().collect();

        for name in plugin_names {
            if let Some(plugin) = self.plugins.write().get_mut(&name) {
                plugin.init(&mut context)?;
            }
        }

        Ok(())
    }

    /// Start all registered plugins
    pub fn start_all_plugins(&self) -> Result<(), String> {
        let mut state_manager = self.state_manager.write();
        let mut context = PluginContext {
            state_manager: &mut *state_manager,
            current_block: None,
        };

        let plugin_names: Vec<String> = self.plugins.read().keys().cloned().collect();

        for name in plugin_names {
            if let Some(plugin) = self.plugins.write().get_mut(&name) {
                plugin.start(&mut context)?;
            }
        }

        Ok(())
    }

    /// Stop all registered plugins
    pub fn stop_all_plugins(&self) -> Result<(), String> {
        let mut state_manager = self.state_manager.write();
        let mut context = PluginContext {
            state_manager: &mut *state_manager,
            current_block: None,
        };

        let plugin_names: Vec<String> = self.plugins.read().keys().cloned().collect();

        for name in plugin_names {
            if let Some(plugin) = self.plugins.write().get_mut(&name) {
                plugin.stop(&mut context)?;
            }
        }

        Ok(())
    }

    /// Process transaction through all plugins
    pub fn process_transaction(&self, tx: &crate::core::Transaction) -> Result<(), String> {
        let mut state_manager = self.state_manager.write();
        let mut context = PluginContext {
            state_manager: &mut *state_manager,
            current_block: None,
        };

        let plugin_names: Vec<String> = self.plugins.read().keys().cloned().collect();

        for name in plugin_names {
            if let Some(plugin) = self.plugins.write().get_mut(&name) {
                if plugin.handle_transaction(tx, &mut context)? {
                    // Transaction was handled by this plugin
                    break;
                }
            }
        }

        Ok(())
    }

    /// Process block through all plugins
    pub fn process_block(&self, block: &crate::core::Block) -> Result<(), String> {
        let mut state_manager = self.state_manager.write();
        let mut context = PluginContext {
            state_manager: &mut *state_manager,
            current_block: Some(block),
        };

        let plugin_names: Vec<String> = self.plugins.read().keys().cloned().collect();

        for name in plugin_names {
            if let Some(plugin) = self.plugins.write().get_mut(&name) {
                plugin.process_block(block, &mut context)?;
            }
        }

        Ok(())
    }

    /// List all registered plugins
    pub fn list_plugins(&self) -> Vec<String> {
        self.plugins.read().keys().cloned().collect()
    }

    /// Get plugin metadata
    pub fn get_plugin_metadata(&self, name: &str) -> Option<crate::plugins::trait_def::PluginMetadata> {
        self.plugins.read().get(name).map(|p| p.metadata())
    }
}