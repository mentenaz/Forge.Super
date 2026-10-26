//! Node.js views: the dock panel ([`NodePanel`]) and the main-area
//! view ([`NodeWorkspace`]). Both get their data from `node-core`.
//!
//! Not yet registered with the shell; that happens through the host API
//! (design doc §12.6, phase 1).

mod panel;
mod workspace;

pub use panel::NodePanel;
pub use workspace::NodeWorkspace;
