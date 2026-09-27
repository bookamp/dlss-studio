pub mod pe;
pub mod gpu;
pub mod scan;
pub mod journal;
pub mod optiscaler;
pub mod mfg_unlock;
pub mod state;
pub mod install_guards;
pub mod compatibility;
pub mod steamart;
pub mod install_routes;

pub mod i18n;
pub mod tray;
pub mod logger;
pub mod single_instance;
pub mod downloader;
pub mod vulkan_layer;
pub mod vibepollo;
pub mod display;
 
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

