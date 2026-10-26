//! .NET detection and commands: processes, files, the dotnet CLI.
//!
//! Everything here that touches the outside world, so [`crate::logic`]
//! stays pure. Split into modules as it grows; this file then only does
//! `pub use`.
