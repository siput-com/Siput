# Transaction Pipeline

Modular transaction processing pipeline untuk Siput blockchain yang dapat dikustomisasi dan diperluas.

## Arsitektur

Pipeline transaksi terdiri dari beberapa stages yang dapat dikonfigurasi:

1. **Validation Stage** - Validasi signature, balance, nonce, dan format transaksi
2. **Mempool Stage** - Penyimpanan transaksi dalam mempool dengan DAG dependencies
3. **Execution Stage** - Eksekusi transaksi menggunakan VM engine
4. **State Update Stage** - Update state blockchain setelah eksekusi
5. **Finality Stage** - Penanganan finality dan konfirmasi transaksi

## Penggunaan

```rust
use siput_core::pipeline::{TransactionPipelineManager, ValidationStage, MempoolStage, ExecutionStage, StateUpdateStage, FinalityStage};

// Buat pipeline manager
let mut pipeline = TransactionPipelineManager::new();

// Tambahkan stages
pipeline.add_stage(Box::new(ValidationStage::new(state_manager.clone())));
pipeline.add_stage(Box::new(MempoolStage::new(mempool.clone())));
pipeline.add_stage(Box::new(ExecutionStage::new(executor.clone())));
pipeline.add_stage(Box::new(StateUpdateStage::new(state_manager.clone())));
pipeline.add_stage(Box::new(FinalityStage::new(finality_engine.clone())));

// Proses transaksi
let result = pipeline.process_transaction(transaction).await?;
```

## Ekstensibilitas

Pipeline dapat diperluas dengan menambahkan custom stages:

```rust
#[async_trait]
impl TransactionPipelineStage for MyCustomStage {
    fn name(&self) -> &'static str {
        "custom_stage"
    }

    async fn process(&self, context: &mut PipelineContext) -> Result<PipelineResult, String> {
        // Custom logic here
        Ok(PipelineResult {
            success: true,
            transaction_hash: Some(context.transaction.hash()),
            error_message: None,
            metadata: HashMap::new(),
        })
    }
}
```

## Konfigurasi

- **Required Stages**: Stages yang harus berhasil untuk melanjutkan pipeline
- **Optional Stages**: Stages yang bisa gagal tanpa menghentikan pipeline
- **Timeouts**: Timeout per stage untuk mencegah hanging
- **Metadata**: Informasi tambahan yang dikumpulkan dari setiap stage

## Error Handling

Pipeline menggunakan error propagation dengan informasi detail:
- Stage yang gagal
- Error message spesifik
- Metadata dari stages yang berhasil

## Testing

Pipeline dapat ditest secara isolated:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pipeline_validation() {
        let mut pipeline = TransactionPipelineManager::new();
        pipeline.add_stage(Box::new(ValidationStage::new(state_manager)));

        let invalid_tx = Transaction::new_transfer(from, to, amount, 0, gas_limit, gas_price);
        // tx tanpa signature

        let result = pipeline.process_transaction(invalid_tx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be signed"));
    }
}
```