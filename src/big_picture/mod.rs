//! DLSS Studio Big Picture Subsystem (100% Pure Rust, Isolated & Downstream).
//!
//! Exposes the 10-foot console-style television interface, native Win32 XInput
//! gamepad controller engine, display/virtual monitor detection, and game
//! process supervisor for seamless Vibepollo / Moonlight couch streaming.

pub mod gamepad;
pub mod tv;
pub mod supervisor;
pub mod logger;
pub mod ui;

pub use ui::{BigPictureOverlay, BigPictureProps};

/// Returns true if command-line arguments explicitly request launching directly in Big Picture mode.
pub fn launch_args_request_big_picture(args: &[String]) -> bool {
    args.iter().any(|a| a == "--big-picture" || a == "--tv" || a == "-bp")
}

