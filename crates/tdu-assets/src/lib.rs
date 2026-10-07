//! Higher-level TDU asset assembly.
//!
//! Keep binary parsing in tdu-formats. This crate owns semantic asset
//! relationships and deterministic material/texture generation shared by the
//! viewer, future asset host and runtime integration.

pub mod procedural;

pub use tdu_formats;
