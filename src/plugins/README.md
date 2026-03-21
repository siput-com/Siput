# Plugin System Documentation

## Overview

The Siput blockchain plugin system provides a modular architecture for extending blockchain functionality without modifying the core components. This enables developers to add new features like NFT support, DeFi protocols, and custom transaction types.

## Architecture

### Plugin Interface

All plugins must implement the `Plugin` trait defined in `src/plugins/trait_def.rs`:

```rust
pub trait Plugin: Send + Sync + Any {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn init(&mut self, context: &mut PluginContext) -> Result<(), String>;
    fn start(&mut self, context: &mut PluginContext) -> Result<(), String>;
    fn stop(&mut self, context: &mut PluginContext) -> Result<(), String>;
    fn handle_transaction(&mut self, tx: &Transaction, context: &mut PluginContext) -> Result<bool, String>;
    fn process_block(&mut self, block: &Block, context: &mut PluginContext) -> Result<(), String>;
    // ... additional methods
}
```

### Plugin Manager

The `PluginManager` handles plugin lifecycle and coordination:

- **Registration**: Plugins are registered with the manager
- **Lifecycle Management**: init/start/stop operations
- **Transaction Processing**: Routes transactions to appropriate plugins
- **Block Processing**: Allows plugins to process blocks

### Plugin Context

Plugins receive a `PluginContext` providing access to:
- State manager for blockchain state
- Current block being processed
- Other blockchain components

## Lifecycle

1. **Registration**: Plugin is registered with `PluginManager::register_plugin()`
2. **Initialization**: `init()` is called to set up plugin state
3. **Starting**: `start()` begins plugin operations
4. **Runtime**: Plugin processes transactions and blocks
5. **Stopping**: `stop()` cleans up resources

## Example Plugins

### NFT Plugin
- Handles NFT minting, transferring, and burning
- Supports collections and metadata
- Transaction type: 0x01

### DeFi Plugin
- Implements AMM (Automated Market Maker)
- Supports lending and borrowing
- Transaction type: 0x02

### Custom Transaction Plugin
- Allows custom transaction types
- Supports script execution
- Transaction type: 0x03

## Usage

```rust
use siput::PluginManager;
use siput::plugins::examples::{NftPlugin, DefiPlugin, CustomTxPlugin};

// Create plugin manager
let plugin_manager = PluginManager::new(state_manager);

// Register plugins
plugin_manager.register_plugin(Box::new(NftPlugin::new()))?;
plugin_manager.register_plugin(Box::new(DefiPlugin::new()))?;
plugin_manager.register_plugin(Box::new(CustomTxPlugin::new()))?;

// Initialize and start plugins
plugin_manager.init_all_plugins()?;
plugin_manager.start_all_plugins()?;

// Process transactions
plugin_manager.process_transaction(&transaction)?;

// Stop plugins
plugin_manager.stop_all_plugins()?;
```

## Developing Custom Plugins

1. Implement the `Plugin` trait
2. Define custom transaction types in `supported_transaction_types()`
3. Handle transactions in `handle_transaction()`
4. Register with the plugin manager

## Future Extensions

- Dynamic plugin loading from shared libraries
- Plugin marketplace and discovery
- Plugin dependency management
- Hot-swappable plugins