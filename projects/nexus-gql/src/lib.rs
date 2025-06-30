#![doc = include_str!("../../../README.MD")]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

// Re-export essential types for users
pub use client::{NexusClient, NexusConfig};
pub use errors::{NexusError, Result};
pub use queries::*;

// Core modules
pub mod client;
pub mod errors;
pub mod queries;
