/**
 * Event Listening Example
 *
 * This example demonstrates how to subscribe to various blockchain events
 * using the Siput SDK.
 */

const { SiputSDK } = require('../../../javascript/siput-sdk');

async function main() {
    // Initialize SDK and connect wallet
    const sdk = new SiputSDK('http://localhost:8080');

    console.log('=== Siput SDK Event Listening Example ===\n');

    try {
        // Connect wallet
        console.log('1. Connecting wallet...');
        await sdk.connectWallet({ create: true });
        console.log('✅ Wallet connected:', sdk.getAddress(), '\n');

        // Example 1: Listen for new blocks
        console.log('2. Listening for new blocks...');
        const unsubscribeBlocks = sdk.onNewBlocks((block, height) => {
            console.log('📦 New block detected:');
            console.log('   Height:', height);
            console.log('   Hash:', block.hash);
            console.log('   Transactions:', block.transactions?.length || 0);
            console.log('');
        });

        // Example 2: Listen for balance changes
        console.log('3. Listening for balance changes...');
        const unsubscribeBalance = sdk.onBalanceChange((oldBalance, newBalance) => {
            console.log('💰 Balance changed:');
            console.log('   From:', oldBalance, 'tokens');
            console.log('   To:', newBalance, 'tokens');
            console.log('   Change:', newBalance - oldBalance, 'tokens');
            console.log('');
        });

        // Example 3: Listen for incoming transactions
        console.log('4. Listening for incoming transactions...');
        const unsubscribeIncoming = sdk.onIncomingTransactions((tx, amount) => {
            console.log('📥 Incoming transaction:');
            console.log('   From:', tx.from);
            console.log('   Amount:', amount, 'tokens');
            console.log('   Hash:', tx.hash);
            console.log('');
        });

        // Example 4: Listen for outgoing transactions
        console.log('5. Listening for outgoing transactions...');
        const unsubscribeOutgoing = sdk.onOutgoingTransactions((tx, amount) => {
            console.log('📤 Outgoing transaction:');
            console.log('   To:', tx.to);
            console.log('   Amount:', amount, 'tokens');
            console.log('   Hash:', tx.hash);
            console.log('');
        });

        // Example 5: Listen for transaction confirmations
        console.log('6. Listening for transaction confirmations...');
        const unsubscribeConfirmations = sdk.onTransactionConfirmations((tx, blockHash) => {
            console.log('✅ Transaction confirmed:');
            console.log('   Hash:', tx.hash);
            console.log('   Block:', blockHash);
            console.log('   Type:', tx.payload?.type || 'Unknown');
            console.log('');
        });

        // Keep the example running for a while to demonstrate events
        console.log('7. Listening for events... (press Ctrl+C to stop)\n');

        // In a real application, you'd keep the process running
        // For demo purposes, we'll wait a bit then clean up
        await new Promise(resolve => setTimeout(resolve, 30000)); // Wait 30 seconds

        console.log('8. Cleaning up subscriptions...');

        // Clean up all subscriptions
        unsubscribeBlocks();
        unsubscribeBalance();
        unsubscribeIncoming();
        unsubscribeOutgoing();
        unsubscribeConfirmations();

        console.log('✅ All subscriptions cleaned up!\n');

    } catch (error) {
        console.error('❌ Error:', error.message);
    } finally {
        // Clean up SDK
        sdk.cleanup();
        console.log('✅ Example completed and cleaned up!');
    }
}

// Utility function to demonstrate manual event polling
async function demonstratePolling(sdk) {
    console.log('Manual polling example:');

    try {
        // Get current DAG info
        const dagInfo = await sdk.rpcCall('dag');
        console.log('Current DAG tips:', dagInfo.tips?.length || 0);
        console.log('Total blocks:', dagInfo.total_blocks || 0);

        // Get mempool info
        const mempoolInfo = await sdk.rpcCall('mempool');
        console.log('Mempool transactions:', mempoolInfo.tx_count || 0);

    } catch (error) {
        console.log('Polling failed:', error.message);
    }
}

// Run the example
if (require.main === module) {
    main().catch(console.error);
}

module.exports = { main, demonstratePolling };