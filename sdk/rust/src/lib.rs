//! Siput SDK
//!
//! This module provides a lightweight developer SDK for building wallets, dApps,
//! bots, and integrations with Siput blockchain.
//!
//! The SDK is intentionally kept minimal and modular; users can pick and choose
//! the components they need (RPC client, wallet utilities, transaction builders,
//! contract helpers, etc.).

pub mod client;
pub mod contract;
pub mod crypto;
pub mod errors;
pub mod events;
pub mod mobile;
pub mod network;
pub mod sdk;
pub mod transaction;
pub mod wallet;

pub use client::Client;
pub use errors::SdkError;
pub use events::{SdkEventListener, SdkEventEmitter};
pub use mobile::{MobileClient, MobileWallet};
pub use sdk::{SiputSDK, wallet_connect::WalletConnector, transaction_builder::EnhancedTransactionBuilder, event_listener::EnhancedEventListener};
