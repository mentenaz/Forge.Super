//! Node.js/npm detection and commands: processes, files, the npm CLI.
//!
//! Everything here that touches the outside world, so [`crate::logic`]
//! stays pure. Split into modules as it grows; this file then only does
//! `pub use`.
pub mod node_backend;
pub mod npm_backend;
