pub mod addons;
pub mod advisories;
pub mod compatibility;
pub mod downloader;
pub mod i18n;
pub mod journal;
pub mod payloads;
pub mod platform;
pub mod routes;
pub mod scan;
pub mod state;
pub mod steamart;
pub mod utils;
pub mod vibepollo;

// Top-level aliases for seamless module access
pub use platform::{display, gpu, tray};
pub use platform::process as install_guards;
pub use platform::vulkan as vulkan_layer;
pub use utils::{ini, logger, pe, single_instance};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

