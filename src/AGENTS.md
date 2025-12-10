# nexus-gql

GraphQL API client for Nexus Mods, providing typed queries for games, mods, mod files, and download links.

# Project Structure

- `nexus-gql/` - Main library crate
  - `src/` - Library source code
  - `src/schema/` - GraphQL schema and query definitions
- `batch-downloader-cli/` - CLI executable for batch downloading

# Code Guidelines

- Optimize for performance; use zero-cost abstractions, avoid allocations.
- Keep modules under 500 lines (excluding tests); split if larger.
- Place `use` inside functions only for `#[cfg]` conditional compilation.

# Documentation Standards

- Document public items with `///`
- Add examples in docs where helpful
- Use `//!` for module-level docs
- Focus comments on "why" not "what"
- Use [`TypeName`] rustdoc links, not backticks.

# Post-Change Verification

```bash
cargo test --workspace --all-features
cargo clippy --workspace --all-features -- -D warnings
cargo doc --workspace --all-features
cargo fmt --all
cargo publish --dry-run -p nexus-gql
```

All must pass before submitting.
