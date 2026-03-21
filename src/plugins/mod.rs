//! Plugin system for blockchain extensibility
//!
//! This module provides a plugin architecture that allows adding new functionality
//! to the blockchain without modifying the core components.

pub mod manager;
pub mod trait_def;
pub mod examples;

pub use manager::PluginManager;
pub use trait_def::Plugin;