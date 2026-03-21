use crate::plugins::trait_def::{Plugin, PluginContext, PluginMetadata};
use crate::core::{Transaction, TransactionId};
use std::collections::HashMap;

/// NFT Plugin for Non-Fungible Token functionality
pub struct NftPlugin {
    nfts: HashMap<TransactionId, NftMetadata>,
    collections: HashMap<String, NftCollection>,
}

#[derive(Clone, Debug)]
struct NftMetadata {
    token_id: u64,
    collection: String,
    owner: [u8; 20],
    metadata_uri: String,
}

#[derive(Clone, Debug)]
struct NftCollection {
    name: String,
    creator: [u8; 20],
    total_supply: u64,
}

impl NftPlugin {
    pub fn new() -> Self {
        Self {
            nfts: HashMap::new(),
            collections: HashMap::new(),
        }
    }
}

impl Plugin for NftPlugin {
    fn name(&self) -> &str {
        "nft"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn init(&mut self, _context: &mut PluginContext) -> Result<(), String> {
        // Initialize NFT storage
        Ok(())
    }

    fn start(&mut self, _context: &mut PluginContext) -> Result<(), String> {
        // Start NFT operations
        Ok(())
    }

    fn stop(&mut self, _context: &mut PluginContext) -> Result<(), String> {
        // Stop NFT operations
        Ok(())
    }

    fn handle_transaction(&mut self, tx: &Transaction, context: &mut PluginContext) -> Result<bool, String> {
        // Check if this is an NFT transaction
        if let Some(payload) = self.parse_nft_payload(tx) {
            match payload {
                NftTransaction::Mint { collection, token_id, metadata_uri } => {
                    self.mint_nft(collection, token_id, tx.from, metadata_uri)?;
                }
                NftTransaction::Transfer { token_id, to } => {
                    self.transfer_nft(token_id, tx.from, to)?;
                }
                NftTransaction::Burn { token_id } => {
                    self.burn_nft(token_id, tx.from)?;
                }
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn process_block(&mut self, _block: &crate::core::Block, _context: &mut PluginContext) -> Result<(), String> {
        // Process NFT-related block data
        Ok(())
    }

    fn supported_transaction_types(&self) -> Vec<u8> {
        vec![0x01] // NFT transaction type
    }

    fn validate_transaction(&self, tx: &Transaction, _context: &PluginContext) -> Result<(), String> {
        if let Some(payload) = self.parse_nft_payload(tx) {
            match payload {
                NftTransaction::Mint { collection, token_id, .. } => {
                    if self.nfts.contains_key(&TransactionId::from(tx.hash())) {
                        return Err("NFT already exists".to_string());
                    }
                    if let Some(coll) = self.collections.get(&collection) {
                        if token_id >= coll.total_supply {
                            return Err("Token ID exceeds collection supply".to_string());
                        }
                    }
                }
                NftTransaction::Transfer { token_id, .. } => {
                    if let Some(nft) = self.nfts.get(&TransactionId::from(tx.hash())) {
                        if nft.owner != tx.from {
                            return Err("Not the owner of this NFT".to_string());
                        }
                    } else {
                        return Err("NFT does not exist".to_string());
                    }
                }
                NftTransaction::Burn { token_id } => {
                    if let Some(nft) = self.nfts.get(&TransactionId::from(tx.hash())) {
                        if nft.owner != tx.from {
                            return Err("Not the owner of this NFT".to_string());
                        }
                    } else {
                        return Err("NFT does not exist".to_string());
                    }
                }
            }
        }
        Ok(())
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "NFT Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Non-Fungible Token functionality for the blockchain".to_string(),
            author: "Siput Team".to_string(),
        }
    }
}

impl NftPlugin {
    fn parse_nft_payload(&self, tx: &Transaction) -> Option<NftTransaction> {
        // Simplified parsing - in real implementation, parse tx.payload
        // For demo, assume payload contains NFT data
        None // Placeholder
    }

    fn mint_nft(&mut self, collection: String, token_id: u64, owner: [u8; 20], metadata_uri: String) -> Result<(), String> {
        let tx_id = TransactionId::default(); // Would use actual tx hash
        let nft = NftMetadata {
            token_id,
            collection: collection.clone(),
            owner,
            metadata_uri,
        };
        self.nfts.insert(tx_id, nft);

        if let Some(coll) = self.collections.get_mut(&collection) {
            coll.total_supply += 1;
        }
        Ok(())
    }

    fn transfer_nft(&mut self, token_id: u64, from: [u8; 20], to: [u8; 20]) -> Result<(), String> {
        // Find and update NFT ownership
        for nft in self.nfts.values_mut() {
            if nft.token_id == token_id && nft.owner == from {
                nft.owner = to;
                return Ok(());
            }
        }
        Err("NFT not found or not owned by sender".to_string())
    }

    fn burn_nft(&mut self, token_id: u64, owner: [u8; 20]) -> Result<(), String> {
        // Remove NFT from circulation
        let mut to_remove = None;
        for (tx_id, nft) in &self.nfts {
            if nft.token_id == token_id && nft.owner == owner {
                to_remove = Some(*tx_id);
                break;
            }
        }

        if let Some(tx_id) = to_remove {
            self.nfts.remove(&tx_id);
            Ok(())
        } else {
            Err("NFT not found or not owned by sender".to_string())
        }
    }
}

enum NftTransaction {
    Mint { collection: String, token_id: u64, metadata_uri: String },
    Transfer { token_id: u64, to: [u8; 20] },
    Burn { token_id: u64 },
}