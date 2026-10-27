//! .NET domain code, free of gpui so a CLI, Tauri or tests can use it
//! too.
//!
//! - [`logic`]: pure domain: types, state, rules, commands.
//! - [`backend`]: IO: processes, files, network, CLI wrappers.
//!
//! The Zed edition's equivalents (`dotnet_backend` and friends in
//! `E:\zed_Source\crates`) move in here when this feature is ported.

pub mod backend;
pub mod logic;
