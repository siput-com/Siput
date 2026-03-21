# RPC Layer Refactoring

## Overview

The RPC layer has been refactored to provide better organization, interface-based design, and API versioning for SDK stability. The refactoring separates handlers by domain, introduces stable interfaces, and ensures backward compatibility.

## Architecture

### Directory Structure
```
src/rpc/
├── handlers/
│   ├── mod.rs           # Handler module exports
│   ├── blockchain.rs    # Blockchain operations (tx, blocks, balances)
│   ├── contracts.rs     # Smart contract operations
│   └── node.rs          # Node and network operations
├── interfaces.rs        # RPC service trait definitions
├── services.rs          # Concrete service implementations
├── versioning.rs        # API versioning utilities
├── server.rs            # Main server with routing
└── mod.rs               # Module exports
```

### Key Components

#### 1. Interfaces (`interfaces.rs`)
- `BlockchainService`: Transaction submission, balance queries, block retrieval
- `ContractService`: Contract deployment, calling, and information
- `NodeService`: Node information and network status
- `RpcService`: Composite service containing all domain services

#### 2. Services (`services.rs`)
- `BlockchainServiceImpl`: Wraps DAG, mempool, and state manager
- `ContractServiceImpl`: Wraps contract registry and state manager
- `NodeServiceImpl`: Wraps P2P node, mempool, and DAG

#### 3. Handlers (Domain-separated)
- **Blockchain**: `/v1/blockchain/*` - Transaction and block operations
- **Contracts**: `/v1/contracts/*` - Smart contract operations
- **Node**: `/v1/node/*` - Network and node information

#### 4. Versioning (`versioning.rs`)
- API version management with backward compatibility
- Version middleware for request validation
- Version information endpoint

## API Endpoints

### Version v1 (`/v1/`)

#### Blockchain Operations (`/v1/blockchain/`)
- `POST /send_tx` - Submit transaction
- `GET /balance/:address` - Get account balance
- `GET /block/:hash` - Get block by hash
- `GET /tx/:hash` - Get transaction by hash
- `GET /tx/status/:hash` - Get transaction status
- `GET /dag` - Get DAG information

#### Contract Operations (`/v1/contracts/`)
- `POST /deploy` - Deploy smart contract
- `POST /call` - Call contract method
- `GET /:address` - Get contract information
- `GET /list` - List all contracts

#### Node Operations (`/v1/node/`)
- `GET /info` - Get node information
- `GET /mempool` - Get mempool status

#### Version Information
- `GET /v1/version` - Get API version information

## Benefits

### 1. Domain Separation
- Handlers are organized by business domain
- Easier maintenance and testing
- Clear separation of concerns

### 2. Interface-Based Design
- Stable APIs through trait definitions
- Easy mocking for testing
- Decoupling from core implementation details

### 3. API Versioning
- Explicit version paths (`/v1/`)
- Backward compatibility support
- Clear upgrade paths for SDKs

### 4. SDK Stability
- Interfaces provide stable contracts
- Versioning prevents breaking changes
- Domain separation reduces coupling

## Migration Guide

### For SDK Developers
- Update base URLs to include version: `http://node:port/v1/`
- Use domain-specific paths: `/v1/blockchain/send_tx`
- API responses remain compatible within versions

### For Node Operators
- Existing endpoints redirect to v1 (future implementation)
- New installations use versioned endpoints
- Backward compatibility maintained

## Future Enhancements

1. **Rate Limiting**: Per-endpoint rate limits
2. **Authentication**: API key authentication
3. **WebSocket Support**: Real-time subscriptions
4. **Batch Requests**: Multiple operations in single request
5. **OpenAPI Documentation**: Auto-generated API docs

## Testing

Run the RPC tests:
```bash
cargo test rpc::
```

Test specific domains:
```bash
cargo test blockchain_handlers
cargo test contract_handlers
cargo test node_handlers
```