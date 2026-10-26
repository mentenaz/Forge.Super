//! Running one shell command and streaming its output: the Script Runner
//! panel's backend, and what other panels use to build the commands they
//! send it.
//!
//! - [`run`]: start a command through the platform's shell, with the IDE's
//!   own runtimes first on its `PATH`, and read its output line by line
//!   until it exits or is stopped.
//! - [`output`]: clean up and read a line of output (ANSI escapes, links,
//!   the SPFx debug query string).
//! - [`command`]: checks for the pieces panels splice into a command line.

pub mod command;
pub mod output;
mod run;

pub use run::{RunEvent, Stop, run};
