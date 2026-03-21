use crate::plugins::trait_def::{Plugin, PluginContext, PluginMetadata};
use crate::core::Transaction;

/// Custom Transaction Logic Plugin
/// Demonstrates how to add custom transaction types and logic
pub struct CustomTxPlugin {
    custom_states: std::collections::HashMap<String, serde_json::Value>,
}

impl CustomTxPlugin {
    pub fn new() -> Self {
        Self {
            custom_states: std::collections::HashMap::new(),
        }
    }
}

impl Plugin for CustomTxPlugin {
    fn name(&self) -> &str {
        "custom_tx"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn init(&mut self, _context: &mut PluginContext) -> Result<(), String> {
        // Initialize custom state
        self.custom_states.insert("counter".to_string(), serde_json::json!(0));
        Ok(())
    }

    fn start(&mut self, _context: &mut PluginContext) -> Result<(), String> {
        // Start custom operations
        Ok(())
    }

    fn stop(&mut self, _context: &mut PluginContext) -> Result<(), String> {
        // Stop custom operations
        Ok(())
    }

    fn handle_transaction(&mut self, tx: &Transaction, context: &mut PluginContext) -> Result<bool, String> {
        // Check if this is a custom transaction
        if let Some(custom_tx) = self.parse_custom_payload(tx) {
            match custom_tx {
                CustomTransaction::IncrementCounter => {
                    self.increment_counter()?;
                }
                CustomTransaction::SetValue { key, value } => {
                    self.set_custom_value(key, value)?;
                }
                CustomTransaction::ExecuteScript { script } => {
                    self.execute_script(script, context)?;
                }
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn process_block(&mut self, block: &crate::core::Block, context: &mut PluginContext) -> Result<(), String> {
        // Process custom logic for each block
        // For example, execute scheduled operations
        Ok(())
    }

    fn supported_transaction_types(&self) -> Vec<u8> {
        vec![0x03] // Custom transaction type
    }

    fn validate_transaction(&self, tx: &Transaction, _context: &PluginContext) -> Result<(), String> {
        if let Some(custom_tx) = self.parse_custom_payload(tx) {
            match custom_tx {
                CustomTransaction::ExecuteScript { script } => {
                    // Validate script syntax, permissions, etc.
                    if script.contains("dangerous_operation") {
                        return Err("Script contains dangerous operations".to_string());
                    }
                }
                _ => {} // Other validations
            }
        }
        Ok(())
    }

    fn execute(&mut self, method: &str, params: &[u8], _context: &mut PluginContext) -> Result<Vec<u8>, String> {
        match method {
            "get_counter" => {
                let counter = self.custom_states.get("counter")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                Ok(counter.to_string().into_bytes())
            }
            "get_value" => {
                let key = String::from_utf8_lossy(params);
                if let Some(value) = self.custom_states.get(&key) {
                    Ok(serde_json::to_vec(value).unwrap_or_default())
                } else {
                    Err("Key not found".to_string())
                }
            }
            _ => Err("Unknown method".to_string())
        }
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "Custom Transaction Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Plugin for custom transaction types and logic".to_string(),
            author: "Siput Team".to_string(),
        }
    }
}

impl CustomTxPlugin {
    fn parse_custom_payload(&self, tx: &Transaction) -> Option<CustomTransaction> {
        // Simplified parsing - in real implementation, parse tx.payload
        None // Placeholder
    }

    fn increment_counter(&mut self) -> Result<(), String> {
        if let Some(counter) = self.custom_states.get_mut("counter") {
            if let Some(num) = counter.as_i64() {
                *counter = serde_json::json!(num + 1);
            }
        }
        Ok(())
    }

    fn set_custom_value(&mut self, key: String, value: serde_json::Value) -> Result<(), String> {
        self.custom_states.insert(key, value);
        Ok(())
    }

    fn execute_script(&mut self, script: String, context: &mut PluginContext) -> Result<(), String> {
        // Simplified script execution
        // In real implementation, this could be a proper scripting engine
        if script == "reset_counter" {
            self.custom_states.insert("counter".to_string(), serde_json::json!(0));
        }
        Ok(())
    }
}

enum CustomTransaction {
    IncrementCounter,
    SetValue { key: String, value: serde_json::Value },
    ExecuteScript { script: String },
}