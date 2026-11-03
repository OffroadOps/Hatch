/// Product version surfaced by the UI, the update checker and diagnostics.
///
/// `npga-rust/Cargo.toml` is the single source of truth; hard-coding the
/// number anywhere else lets the About pane and the release tag drift apart.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod config;
pub mod core_config;
pub mod geoip;
pub mod links;
pub mod mode_wizard;
pub mod model;
pub mod modes;
pub mod nat;
pub mod platform;
pub mod runtime;
pub mod selector;
pub mod speedtest;
pub mod stats;
pub mod subscription;
pub mod update;
