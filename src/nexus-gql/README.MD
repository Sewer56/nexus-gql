# nexus-gql

[![Crates.io](https://img.shields.io/crates/v/nexus-gql.svg)](https://crates.io/crates/nexus-gql)
[![Docs.rs](https://docs.rs/nexus-gql/badge.svg)](https://docs.rs/nexus-gql)
[![CI](https://github.com/Sewer56/nexus-gql/actions/workflows/rust.yml/badge.svg)](https://github.com/Sewer56/nexus-gql/actions)

GraphQL API client for Nexus Mods. Built for batch downloading operations.

## Features

- GraphQL-based API client for Nexus Mods
- Support for querying games, mods, and mod files
- Download link generation
- Built-in retry middleware for resilient requests

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
nexus-gql = "0.1.0"
```

### Feature Flags

| Feature | Description                                          |
| ------- | ---------------------------------------------------- |
| `std`   | Enable standard library support (enabled by default) |

## Usage

### Basic Example

```rust
use nexus_gql::{NexusClient, NexusConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = NexusConfig::new("your-api-key");
    let client = NexusClient::new(config)?;
    
    // Use client to query Nexus Mods API
    Ok(())
}
```

## License

Licensed under [GPL v3 (with Reloaded FAQ)](https://github.com/Sewer56/nexus-gql/blob/main/LICENSE).
