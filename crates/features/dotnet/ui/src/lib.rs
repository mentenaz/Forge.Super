//! .NET views: the dock panel ([`DotnetPanel`]) and the main-area
//! view ([`DotnetWorkspace`]). Both get their data from `dotnet-core`.
//!
//! Not yet registered with the shell; that happens through the host API
//! (design doc §12.6, phase 1).

mod panel;
mod workspace;

pub use panel::DotnetPanel;
pub use workspace::DotnetWorkspace;
