# backend-rust-memory

Rust-based gRPC Memory/Vector Database service for the PAGI Digital Twin platform.

## Overview

This service provides high-performance memory operations (query and commit) via gRPC, serving as the VectorDBClient Bridge for the Digital Twin architecture.

## Features

- **gRPC Interface**: Tonic-based gRPC server for efficient inter-service communication
- **Semantic Search**: Query memory using semantic search (currently mock implementation)
- **Memory Storage**: Commit/store memory blocks with metadata
- **Namespace Support**: Scoped memory operations by namespace
- **Mock Implementation**: In-memory storage for development/testing

## Protocol

The service implements the `MemoryService` gRPC interface:

- `QueryMemory`: Semantic search with query text, namespace, and filters
- `CommitMemory`: Store new memory blocks with metadata
- `HealthCheck`: Service health status

See `proto/memory.proto` for the complete protocol definition.

## Configuration

Environment variables:

- `MEMORY_GRPC_PORT`: gRPC server port (default: `50052`)
- `LOG_LEVEL`: Logging level (default: `info`)

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run
```

Or with environment variables:

```bash
MEMORY_GRPC_PORT=50052 cargo run
```

## Future Integration

The current implementation uses in-memory mock storage. Future versions will integrate with:

- **Qdrant** or **Milvus** for vector storage
- **ChromaDB** for local development
- **Embedding models** for semantic search (e.g., sentence-transformers)

## Development

The service is designed to be a drop-in replacement for the Python memory service, providing better performance and type safety through Rust's type system.
