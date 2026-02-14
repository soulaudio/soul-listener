//! Application configuration and constants
//!
//! This module defines central configuration values used across the application.
//! All branding, naming, and configuration should reference these constants
//! rather than hardcoding values.

/// The application name
pub const APP_NAME: &str = "Soul Listener";

/// The application short name (used in compact displays)
pub const APP_NAME_SHORT: &str = "Soul Listener";

/// The application type/category
pub const APP_TYPE: &str = "DAP"; // Digital Audio Player

/// Application version (synchronized with Cargo.toml)
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Full application title (name + type)
pub const fn app_title() -> &'static str {
    APP_NAME
}

/// Development mode banner
pub const fn dev_banner() -> &'static str {
    "Soul Listener - Development Mode"
}
