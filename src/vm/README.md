# Multi-VM System Documentation

## Overview

The Siput blockchain supports multiple Virtual Machine (VM) implementations for smart contract execution. This allows nodes to choose the most appropriate VM for different contract types while maintaining a unified execution interface.

## Architecture

### VM Engine Interface

All VM implementations must implement the `VmEngine` trait:

```rust
pub trait VmEngine: Send + Sync {
    fn vm_type(&self) -> VmType;
    fn deploy_contract(&self, ...) -> Result<ContractInstance, VmError>;
    fn execute_contract(&self, ...) -> Result<ExecutionResult, VmError>;
    fn get_contract_code(&self, ...) -> Result<Option<Vec<u8>>, VmError>;
    fn contract_exists(&self, ...) -> bool;
    fn validate_bytecode(&self, ...) -> Result<(), VmError>;
    fn supported_contract_types(&self) -> Vec<String>;
}
```

### VM Engine Manager

The `VmEngineManager` manages multiple VM implementations:

- **Registration**: Register different VM engines
- **Auto-detection**: Automatically select appropriate VM based on bytecode
- **Runtime Selection**: Allow nodes to choose default VM
- **Unified Interface**: Provide consistent API regardless of underlying VM

### Supported VM Types

#### WASM VM (`VmType::Wasm`)
- **Implementation**: `WasmVmEngine`
- **Backend**: Wasmtime
- **Bytecode**: WebAssembly modules
- **Features**: Full WASM support, gas metering, host functions
- **Magic Bytes**: `0x00 0x61 0x73 0x6D`

#### Custom VM (`VmType::Custom`)
- **Implementation**: `CustomVmEngine`
- **Backend**: Custom bytecode interpreter
- **Bytecode**: Custom instruction set
- **Features**: Lightweight, domain-specific operations
- **Magic Bytes**: `0x43 0x56 0x4D` ("CVM")

#### Future Support
- **EVM**: Ethereum Virtual Machine compatibility
- **Other VMs**: Additional VM types can be added

## Usage

### Basic Setup

```rust
use siput::vm::{VmEngineManager, WasmVmEngine, CustomVmEngine, InMemoryCustomStorage};

// Create VM manager
let mut vm_manager = VmEngineManager::new();

// Register WASM VM
let wasm_storage = Arc::new(InMemoryCustomStorage::new()); // Temporary storage
let wasm_vm = WasmVmEngine::new(wasm_storage)?;
vm_manager.register_engine(Box::new(wasm_vm))?;

// Register Custom VM
let custom_storage = Arc::new(InMemoryCustomStorage::new());
let custom_vm = CustomVmEngine::new(custom_storage);
vm_manager.register_engine(Box::new(custom_vm))?;

// Set default VM
vm_manager.set_default_vm(VmType::Wasm)?;
```

### Contract Deployment

```rust
// Auto-detect VM from bytecode
let vm_engine = vm_manager.get_engine_for_bytecode(&bytecode);

// Deploy contract
let block_context = BlockContext {
    block_number: 12345,
    block_hash: current_block_hash,
    timestamp: current_timestamp,
    gas_price: 100,
};

let contract = vm_engine.deploy_contract(
    &bytecode,
    &deployer_address,
    gas_limit,
    &block_context,
)?;
```

### Contract Execution

```rust
// Execute contract method
let result = vm_engine.execute_contract(
    &contract_address,
    "transfer",
    &args,
    &caller_address,
    gas_limit,
    &block_context,
)?;

if result.success {
    println!("Return data: {:?}", result.return_data);
    println!("Gas used: {}", result.gas_used);
}
```

### Node Configuration

Nodes can configure their preferred VM setup:

```rust
// In node configuration
let mut contract_executor = ContractExecutor::new(
    state_manager,
    registry_path,
    storage_path,
)?;

// Set default VM for this node
contract_executor.set_default_vm(VmType::Custom)?;

// Check available VMs
let available_vms = contract_executor.available_vms();
println!("Available VMs: {:?}", available_vms);
```

## VM-Specific Features

### WASM VM Features
- **Gas Metering**: Automatic gas calculation
- **Host Functions**: Access to blockchain state and external data
- **Memory Management**: WASM linear memory with limits
- **Deterministic Execution**: Reproducible results

### Custom VM Features
- **Lightweight**: Minimal overhead for simple contracts
- **Domain-Specific**: Optimized for specific use cases
- **Extensible**: Easy to add new opcodes
- **Fast Execution**: Direct interpretation without compilation

## Development

### Adding New VM Types

1. Implement the `VmEngine` trait
2. Define VM type in `VmType` enum
3. Add bytecode detection logic in `VmEngineManager::get_engine_for_bytecode`
4. Register VM in node initialization

### VM Storage Abstraction

Each VM can have its own storage implementation:

```rust
pub trait CustomStorage: Send + Sync {
    fn store_contract(&self, address: &Address, bytecode: &[u8]) -> Result<(), VmError>;
    fn load_contract(&self, address: &Address) -> Result<Option<Vec<u8>>, VmError>;
    fn store_state(&self, address: &Address, key: &[u8], value: &[u8]) -> Result<(), VmError>;
    fn load_state(&self, address: &Address, key: &[u8]) -> Result<Option<Vec<u8>>, VmError>;
}
```

## Error Handling

VM operations can fail with `VmError`:

- `InvalidBytecode`: Malformed contract code
- `ExecutionFailed`: Runtime execution error
- `OutOfGas`: Gas limit exceeded
- `ContractNotFound`: Contract doesn't exist
- `InvalidMethod`: Method not found in contract
- `StorageError`: Storage operation failed

## Performance Considerations

- **VM Selection**: Choose appropriate VM for contract type
- **Caching**: Cache compiled contracts when possible
- **Gas Limits**: Set appropriate gas limits to prevent abuse
- **Storage Optimization**: Use efficient storage backends

## Future Enhancements

- **Dynamic VM Loading**: Load VMs as plugins at runtime
- **Cross-VM Calls**: Allow contracts on different VMs to interact
- **VM Migration**: Migrate contracts between VM types
- **Parallel Execution**: Execute independent contracts in parallel