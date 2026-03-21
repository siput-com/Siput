//! Contract Interaction Example
//!
//! This example demonstrates how to deploy and interact with smart contracts
//! using the Siput SDK.

use siput_sdk::{SiputSDK, WalletConnector, EnhancedTransactionBuilder};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Siput SDK Contract Interaction Example ===\n");

    // Initialize SDK and create wallet
    let sdk = Arc::new(SiputSDK::new("http://localhost:8080"));
    let connector = WalletConnector::new(Arc::clone(&sdk));

    println!("1. Setting up wallet...");
    let wallet = connector.create_wallet().await?;
    sdk.connect_wallet(wallet.clone()).await?;
    println!("✅ Wallet ready: {:?}", sdk.get_address().await?);
    println!();

    // Mock WASM bytecode (in real usage, this would be compiled from source)
    let mock_wasm_bytecode = vec![
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // WASM magic and version
        0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // Type section
        0x03, 0x02, 0x01, 0x00, // Function section
        0x07, 0x0A, 0x01, 0x06, 0x65, 0x78, 0x65, 0x63, 0x75, 0x74, 0x65, 0x00, 0x00, // Export section
        0x0A, 0x04, 0x01, 0x02, 0x00, 0x0B, // Code section
    ];

    println!("2. Attempting contract deployment...");
    match sdk.deploy_contract(mock_wasm_bytecode.clone(), vec![]).await {
        Ok(tx_hash) => {
            println!("✅ Contract deployment transaction sent!");
            println!("   Transaction Hash: {}", tx_hash);
            println!("   Note: Contract address can be derived from transaction hash");
        }
        Err(e) => {
            println!("ℹ️  Contract deployment demo: {:?}", e);
            println!("   (This is expected in a demo environment without funds)");
        }
    }
    println!();

    // Mock contract address (in real usage, this would be derived from deployment)
    let mock_contract_address = [0u8; 20]; // All zeros for demo

    println!("3. Attempting contract call...");
    let method_name = "execute";
    let call_args = vec![1, 2, 3, 4]; // Mock arguments

    match sdk.call_contract(mock_contract_address, method_name, call_args).await {
        Ok(tx_hash) => {
            println!("✅ Contract call transaction sent!");
            println!("   Transaction Hash: {}", tx_hash);
            println!("   Method: {}", method_name);
        }
        Err(e) => {
            println!("ℹ️  Contract call demo: {:?}", e);
            println!("   (This is expected in a demo environment)");
        }
    }
    println!();

    println!("4. Using Enhanced Transaction Builder for contracts...");
    let builder = EnhancedTransactionBuilder::new(Arc::clone(&sdk));

    // Example contract deployment with builder
    match builder
        .from_connected_wallet().await?
        .deploy_contract(mock_wasm_bytecode, vec![])
        .gas_limit(Some(1_000_000)).await?
        .build_sign_and_send()
        .await
    {
        Ok(tx_hash) => {
            println!("✅ Contract deployed via TransactionBuilder!");
            println!("   Transaction Hash: {}", tx_hash);
        }
        Err(e) => {
            println!("ℹ️  TransactionBuilder deployment demo: {:?}", e);
        }
    }
    println!();

    // Example contract call with builder
    match builder
        .from_connected_wallet().await?
        .call_contract(mock_contract_address, "execute".to_string(), vec![42])
        .gas_limit(Some(100_000)).await?
        .build_sign_and_send()
        .await
    {
        Ok(tx_hash) => {
            println!("✅ Contract called via TransactionBuilder!");
            println!("   Transaction Hash: {}", tx_hash);
        }
        Err(e) => {
            println!("ℹ️  TransactionBuilder call demo: {:?}", e);
        }
    }
    println!();

    println!("5. Demonstrating gas estimation...");
    let estimate_builder = EnhancedTransactionBuilder::new(Arc::clone(&sdk));

    match estimate_builder
        .call_contract(mock_contract_address, "execute".to_string(), vec![123])
        .estimate_gas()
        .await
    {
        Ok(gas_estimate) => {
            println!("✅ Gas estimated for contract call: {} units", gas_estimate);
        }
        Err(e) => {
            println!("ℹ️  Gas estimation demo: {:?}", e);
        }
    }
    println!();

    println!("=== Contract Interaction Concepts ===");
    println!("• Contract Deployment:");
    println!("  - Compile source code to WASM bytecode");
    println!("  - Use deploy_contract() or TransactionBuilder");
    println!("  - Contract address derived from deployment tx hash");
    println!("");
    println!("• Contract Calls:");
    println!("  - Know contract address and method signature");
    println!("  - Serialize arguments appropriately");
    println!("  - Use call_contract() or TransactionBuilder");
    println!("");
    println!("• Gas Estimation:");
    println!("  - Use estimate_gas() for automatic estimation");
    println!("  - Or set manual gas limits for complex operations");
    println!("");
    println!("• Error Handling:");
    println!("  - Check for sufficient balance before operations");
    println!("  - Handle network errors and reverts gracefully");
    println!("");

    println!("✅ Contract interaction example completed!");

    Ok(())
}