//! Simple Wallet Example
//!
//! This example demonstrates basic wallet operations using the Siput SDK.

use siput_sdk::{SiputSDK, WalletConnector};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Siput SDK Simple Wallet Example ===\n");

    // Initialize SDK
    let sdk = Arc::new(SiputSDK::new("http://localhost:8080"));
    let connector = WalletConnector::new(Arc::clone(&sdk));

    println!("1. Creating a new wallet...");
    let wallet = connector.create_wallet().await?;
    println!("✅ Wallet created!");
    println!("   Address: {:?}", wallet.address);
    println!("   Mnemonic: {}", wallet.to_mnemonic(None)?);
    println!();

    println!("2. Connecting wallet to SDK...");
    sdk.connect_wallet(wallet.clone()).await?;
    println!("✅ Wallet connected!");
    println!("   Connected address: {:?}", sdk.get_address().await?);
    println!();

    println!("3. Checking balance...");
    match sdk.get_balance(None).await {
        Ok(balance) => println!("✅ Balance: {} tokens", balance),
        Err(e) => println!("ℹ️  Could not get balance: {:?}", e),
    }
    println!();

    println!("4. Attempting to send tokens (demo)...");
    // For demo purposes, try to send to self (will fail due to insufficient balance, but shows the API)
    match sdk.send_tokens(wallet.address, 100).await {
        Ok(tx_hash) => {
            println!("✅ Transaction sent!");
            println!("   Hash: {}", tx_hash);
        }
        Err(e) => {
            println!("ℹ️  Transaction demo: {:?}", e);
            println!("   (This is expected in a demo environment)");
        }
    }
    println!();

    println!("5. Exporting wallet mnemonic...");
    match connector.export_mnemonic(None).await {
        Ok(mnemonic) => {
            println!("✅ Mnemonic exported (first 4 words): {}", mnemonic.split_whitespace().take(4).collect::<Vec<&str>>().join(" "));
            println!("   ⚠️  Never share your mnemonic in production!");
        }
        Err(e) => println!("❌ Failed to export mnemonic: {:?}", e),
    }
    println!();

    println!("6. Disconnecting wallet...");
    sdk.disconnect_wallet().await?;
    println!("✅ Wallet disconnected!");
    println!();

    println!("7. Reconnecting with mnemonic...");
    let mnemonic = wallet.to_mnemonic(None)?;
    connector.connect_with_mnemonic(&mnemonic, None).await?;
    println!("✅ Reconnected with mnemonic!");
    println!("   Address: {:?}", sdk.get_address().await?);
    println!();

    println!("✅ Example completed successfully!");

    Ok(())
}