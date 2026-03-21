/**
 * Wallet Connect Example
 *
 * This example demonstrates how to connect different types of wallets
 * using the Siput SDK.
 */

const { SiputSDK } = require('../../../javascript/siput-sdk');

async function main() {
    // Initialize SDK
    const sdk = new SiputSDK('http://localhost:8080');

    console.log('=== Siput SDK Wallet Connect Example ===\n');

    try {
        // Example 1: Create a new wallet
        console.log('1. Creating a new wallet...');
        const walletInfo = await sdk.connectWallet({ create: true });
        console.log('✅ Wallet created!');
        console.log('   Address:', walletInfo.address);
        console.log('   Mnemonic:', walletInfo.mnemonic);
        console.log('   ⚠️  Save this mnemonic securely!\n');

        // Example 2: Connect with mnemonic
        console.log('2. Connecting with mnemonic...');
        await sdk.disconnectWallet();
        await sdk.connectWallet({
            mnemonic: walletInfo.mnemonic,
            password: '' // optional
        });
        console.log('✅ Connected with mnemonic!');
        console.log('   Address:', sdk.getAddress(), '\n');

        // Example 3: Connect with private key (if you have one)
        console.log('3. Connecting with private key...');
        // Note: In real usage, you'd get the private key from secure storage
        const exported = sdk.wallet.export();
        await sdk.disconnectWallet();
        await sdk.connectWallet({
            privateKey: '0x' + Buffer.from(exported.privateKey).toString('hex')
        });
        console.log('✅ Connected with private key!');
        console.log('   Address:', sdk.getAddress(), '\n');

        // Example 4: Check balance
        console.log('4. Checking balance...');
        const balance = await sdk.getBalance();
        console.log('✅ Balance:', balance, 'tokens\n');

        // Example 5: Export wallet data
        console.log('5. Exporting wallet data...');
        const walletData = sdk.wallet.export();
        console.log('✅ Wallet exported:');
        console.log('   Address:', walletData.address);
        console.log('   Has Mnemonic:', !!walletData.mnemonic);
        console.log('   Private Key Length:', walletData.privateKey.length, 'bytes\n');

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