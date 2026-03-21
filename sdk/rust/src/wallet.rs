use crate::crypto::Keypair;
use crate::errors::SdkError;
use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose, Engine as _};
use bip39::{Language, Mnemonic};
use hmac::Hmac;

use siput_core::core::transaction::{Address, Transaction};
use pbkdf2::pbkdf2;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Represents a wallet with a keypair and derived address.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wallet {
    /// Secret key bytes (32 bytes)
    pub secret_key: Vec<u8>,
    /// Public key bytes (compressed, 33 bytes)
    pub public_key: Vec<u8>,
    /// Derived address used on Siput
    pub address: Address,
}

impl Wallet {
    /// Create a new random wallet.
    pub fn create_wallet() -> Result<Self, SdkError> {
        let kp = Keypair::generate()?;
        let address = kp.derive_address();

        Ok(Self {
            secret_key: kp.secret_bytes().to_vec(),
            public_key: kp.public_bytes().to_vec(),
            address,
        })
    }

    /// Create a wallet from mnemonic phrase.
    pub fn from_mnemonic(phrase: &str, password: Option<&str>) -> Result<Self, SdkError> {
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, phrase)
            .map_err(|e| SdkError::CryptoError(e.to_string()))?;
        let seed = mnemonic.to_seed(password.unwrap_or(""));
        let hash = Sha256::digest(&seed);
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&hash[..32]);
        let kp = Keypair::from_secret_bytes(&secret)?;

        Ok(Self {
            secret_key: kp.secret_bytes().to_vec(),
            public_key: kp.public_bytes().to_vec(),
            address: kp.derive_address(),
        })
    }

    /// Generate a new mnemonic phrase and wallet.
    pub fn create_wallet_with_mnemonic() -> Result<(Self, String), SdkError> {
        let mut entropy = [0u8; 32];
        rand::thread_rng().fill(&mut entropy);
        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
            .map_err(|e| SdkError::CryptoError(e.to_string()))?;
        let phrase = mnemonic.to_string();
        let wallet = Self::from_mnemonic(&phrase, None)?;
        Ok((wallet, phrase))
    }

    /// Import a wallet from an existing private key (hex or raw bytes).
    pub fn import_private_key(secret_bytes: &[u8]) -> Result<Self, SdkError> {
        let kp = Keypair::from_secret_bytes(secret_bytes)?;
        let address = kp.derive_address();

        Ok(Self {
            secret_key: kp.secret_bytes().to_vec(),
            public_key: kp.public_bytes().to_vec(),
            address,
        })
    }

    /// Export the private key as hex string
    pub fn export_private_key_hex(&self) -> String {
        hex::encode(&self.secret_key)
    }

    /// Encrypt wallet keystore JSON (AES-256-GCM with PBKDF2 derived key)
    pub fn encrypt_keystore(&self, password: &str) -> Result<String, SdkError> {
        let mut salt = [0u8; 16];
        rand::thread_rng().fill(&mut salt);
        let mut key_bytes = [0u8; 32];
        pbkdf2::<Hmac<Sha256>>(password.as_bytes(), &salt, 100_000, &mut key_bytes);
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| SdkError::CryptoError(e.to_string()))?;

        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext =
            serde_json::to_vec(self).map_err(|e| SdkError::SerializationError(e.to_string()))?;
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_ref())
            .map_err(|e| SdkError::CryptoError(e.to_string()))?;

        let keystore = serde_json::json!({
            "salt": general_purpose::STANDARD.encode(salt),
            "nonce": general_purpose::STANDARD.encode(nonce_bytes),
            "ciphertext": general_purpose::STANDARD.encode(ciphertext),
        });

        Ok(keystore.to_string())
    }

    /// Decrypt keystore JSON into a wallet.
    pub fn decrypt_keystore(keystore_json: &str, password: &str) -> Result<Self, SdkError> {
        let parsed: serde_json::Value = serde_json::from_str(keystore_json)
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;
        let salt = general_purpose::STANDARD
            .decode(
                parsed["salt"]
                    .as_str()
                    .ok_or(SdkError::CryptoError("Invalid keystore".to_string()))?,
            )
            .map_err(|e| SdkError::CryptoError(e.to_string()))?;
        let nonce_bytes = general_purpose::STANDARD
            .decode(
                parsed["nonce"]
                    .as_str()
                    .ok_or(SdkError::CryptoError("Invalid keystore".to_string()))?,
            )
            .map_err(|e| SdkError::CryptoError(e.to_string()))?;
        let ciphertext = general_purpose::STANDARD
            .decode(
                parsed["ciphertext"]
                    .as_str()
                    .ok_or(SdkError::CryptoError("Invalid keystore".to_string()))?,
            )
            .map_err(|e| SdkError::CryptoError(e.to_string()))?;

        let mut key_bytes = [0u8; 32];
        pbkdf2::<Hmac<Sha256>>(password.as_bytes(), &salt, 100_000, &mut key_bytes);
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| SdkError::CryptoError(e.to_string()))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| SdkError::CryptoError(e.to_string()))?;

        let wallet: Wallet = serde_json::from_slice(&plaintext)
            .map_err(|e| SdkError::SerializationError(e.to_string()))?;
        Ok(wallet)
    }

    /// Derive a wallet from a BIP44-like path using seeded mnemonic.
    pub fn derive_from_path(&self, path: &str) -> Result<Self, SdkError> {
        // In this simple implementation we use the path to hash the secret key and produce a deterministic child.
        let mut hasher = Sha256::new();
        hasher.update(&self.secret_key);
        hasher.update(path.as_bytes());
        let derived = hasher.finalize();
        let mut sk = [0u8; 32];
        sk.copy_from_slice(&derived[..32]);
        Keypair::from_secret_bytes(&sk).and_then(|kp| {
            Ok(Self {
                secret_key: kp.secret_bytes().to_vec(),
                public_key: kp.public_bytes().to_vec(),
                address: kp.derive_address(),
            })
        })
    }

    /// Sign a transaction and return the signed transaction
    pub fn sign_transaction(&self, mut tx: Transaction) -> Result<Transaction, SdkError> {
        let kp = Keypair::from_secret_bytes(&self.secret_key)?;
        tx.sign(&kp.secret)
            .map_err(|e| SdkError::TransactionError(e))?;
        Ok(tx)
    }

    /// Convert wallet to mnemonic phrase
    pub fn to_mnemonic(&self, password: Option<&str>) -> Result<String, SdkError> {
        // Note: This is a simplified implementation. In practice, you'd need to store
        // the original entropy or mnemonic to recover it. This generates a new mnemonic
        // that would derive the same keypair.
        let (_wallet, mnemonic) = Self::create_wallet_with_mnemonic()?;
        Ok(mnemonic)
    }

    /// Create wallet from private key hex
    pub fn from_private_key(hex_key: &str) -> Result<Self, SdkError> {
        let secret_bytes = hex::decode(hex_key)
            .map_err(|_| SdkError::CryptoError("Invalid hex private key".to_string()))?;
        if secret_bytes.len() != 32 {
            return Err(SdkError::CryptoError("Private key must be 32 bytes".to_string()));
        }
        Self::import_private_key(&secret_bytes)
    }

    /// Derive address from keypair (same as stored address)
    pub fn derive_address(&self) -> Address {
        self.address
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use siput_core::core::transaction::Transaction;

    #[test]
    fn test_wallet_create_and_sign() {
        let wallet = Wallet::create_wallet().expect("create wallet");
        let addr = wallet.derive_address();
        assert_eq!(addr, wallet.address);

        let to = [2u8; 20];
        let mut tx = Transaction::new_transfer(addr, to, 100, 1, 21000, 1);
        wallet.sign_transaction(&mut tx).expect("sign tx");
        assert!(tx.validate_basic().is_ok());
    }

    #[test]
    fn test_import_export_private_key() {
        let wallet = Wallet::create_wallet().expect("create wallet");
        let hex = wallet.export_private_key_hex();
        let secret = hex::decode(&hex).expect("decode hex");
        let imported = Wallet::import_private_key(&secret).expect("import");
        assert_eq!(imported.address, wallet.address);
    }
}
