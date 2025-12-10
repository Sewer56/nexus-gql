# nexus-gql

[![Crates.io](https://img.shields.io/crates/v/nexus-gql.svg)](https://crates.io/crates/nexus-gql)
[![Docs.rs](https://docs.rs/nexus-gql/badge.svg)](https://docs.rs/nexus-gql)
[![CI](https://github.com/Sewer56/nexus-gql/actions/workflows/rust.yml/badge.svg)](https://github.com/Sewer56/nexus-gql/actions)

GraphQL API client for Nexus Mods. Built for batch downloading operations.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
nexus-gql = "0.1.0"
```

```rust
use nexus_gql::{NexusClient, NexusConfig};

let config = NexusConfig::new("your-api-key");
let client = NexusClient::new(config)?;
```

See the [nexus-gql crate documentation](./src/nexus-gql/README.MD) for detailed usage.

## Crates

- [nexus-gql](./src/nexus-gql/README.MD): GraphQL client library for Nexus Mods API
- [batch-downloader-cli](./src/batch-downloader-cli/): Command-line batch downloader (not published)

## Developer Manual

For step-by-step development guidance, see the [Developer Manual](https://reloaded-project.github.io/reloaded-templates-rust/manual/).

## Contributing

We welcome contributions! See the [Contributing Guide](https://reloaded-project.github.io/reloaded-templates-rust/manual/#contributing) for details.

## License

Licensed under [GPL v3 (with Reloaded FAQ)](./LICENSE).

[codecov]: https://about.codecov.io/
