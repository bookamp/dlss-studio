//! Pure Rust Vibepollo / Apollo / Sunshine Integration Engine.
//!
//! Provides presence detection, apps.json parsing, and registration of DLSS Studio
//! in Big Picture mode with cover art and `--big-picture` launch arguments.

pub mod apps_json;
pub mod cover;
pub mod detect;
pub mod session;

pub use apps_json::*;
pub use cover::*;
pub use detect::*;
pub use session::*;
