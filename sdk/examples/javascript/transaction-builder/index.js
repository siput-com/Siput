/**
 * Transaction Builder Example
 *
 * This example demonstrates how to use the TransactionBuilder
 * for creating and sending different types of transactions.
 */

const { SiputSDK, TransactionBuilder } = require('../../../javascript/siput-sdk');

async function main() {
    // Initialize SDK and connect wallet
    const sdk = new SiputSDK('http://localhost:8080');

    console.log('=== Siput SDK Transaction Builder Example ===\n');

    try {
        // Connect wallet
        console.log('1. Connecting wallet...');
        await sdk.connectWallet({ create: true });
        console.log('✅ Wallet connected:', sdk.getAddress(), '\n');

        // Check initial balance
        const initialBalance = await sdk.getBalance();
        console.log('2. Initial balance:', initialBalance, 'tokens\n');

        // Example 1: Simple token transfer using SDK method
        console.log('3. Sending tokens (SDK method)...');
        try {
            // For demo purposes, we'll try to send to ourselves
            // In real usage, you'd send to another address
            const recipient = sdk.getAddress(); // Send to self for demo
            const amount = 100;

            console.log(`   Sending ${amount} tokens to ${recipient}...`);
            const txHash = await sdk.sendTokens(recipient, amount);
            console.log('✅ Transaction sent!');
            console.log('   Hash:', txHash, '\n');
        } catch (error) {
            console.log('ℹ️  Transaction demo skipped (insufficient balance or demo setup)\n');
        }

        // Example 2: Using TransactionBuilder for transfer
        console.log('4. Using TransactionBuilder for transfer...');
        const builder = new TransactionBuilder(sdk);

        try {
            const txHash2 = await builder
                .to(sdk.getAddress()) // Send to self
                .amount(50)
                .gasLimit(25000)
                .gasPrice(1)
                .send();

            console.log('✅ Transaction built and sent with TransactionBuilder!');
            console.log('   Hash:', txHash2, '\n');
        } catch (error) {
            console.log('ℹ️  TransactionBuilder demo skipped:', error.message, '\n');
        }

        // Example 3: Contract deployment (mock example)
        console.log('5. Contract deployment example...');
        try {
            // Mock WASM code (in real usage, this would be compiled WASM)
            const mockWasmCode = new Uint8Array([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);
            const initArgs = new Uint8Array([0x01, 0x02, 0x03]);

            console.log('   Deploying contract...');
            const deployTxHash = await sdk.deployContract(mockWasmCode, initArgs);
            console.log('✅ Contract deployment transaction sent!');
            console.log('   Hash:', deployTxHash, '\n');
        } catch (error) {
            console.log('ℹ️  Contract deployment demo skipped:', error.message, '\n');
        }

        // Example 4: Contract call (mock example)
        console.log('6. Contract call example...');
        try {
            const contractAddress = 'spt1234567890123456789012345678901234567890';
            const method = 'transfer';
            const args = new Uint8Array([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x64]); // 100 in bytes

            console.log(`   Calling contract method '${method}'...`);
            const callTxHash = await sdk.callContract(contractAddress, method, args);
            console.log('✅ Contract call transaction sent!');
            console.log('   Hash:', callTxHash, '\n');
        } catch (error) {
            console.log('ℹ️  Contract call demo skipped:', error.message, '\n');
        }

        // Example 5: Gas estimation
        console.log('7. Gas estimation example...');
        const estimateBuilder = new TransactionBuilder(sdk);
        const estimatedGas = await estimateBuilder
            .to(sdk.getAddress())
            .amount(1)
            .estimateGas();

        console.log('✅ Gas estimated:', estimatedGas, 'units\n');

        // Final balance check
        const finalBalance = await sdk.getBalance();
        console.log('8. Final balance:', finalBalance, 'tokens');
        console.log('   Balance change:', finalBalance - initialBalance, 'tokens\n');

    } catch (error) {
        console.error('❌ Error:', error.message);
    } finally {
        // Clean up
        sdk.cleanup();
        console.log('✅ Example completed and cleaned up!');
    }
}

// Run the example
if (require.main === module) {
    main().catch(console.error);
}

module.exports = { main };