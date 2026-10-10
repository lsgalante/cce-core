//! The GUI-free half of the cce toolkit.
//!
//! What a cce process needs whether or not it draws anything: the KDL config
//! (`config`) and input bindings (`input`), the DE-wide animations switch
//! (`motion`), the user's locale (`locale`) and message catalogues in it (`l10n`, a
//! feature), lengths with units and the display
//! metric (`units`), the
//! Unix-socket IPC convention (`ipc`), and the parsers for the specs the DE
//! writes in its config — colours (`color`), ramps (`ramp`), relief
//! (`relief_spec`) and droplets (`droplet`).
//!
//! `cce-ui` re-exports every module here at its old path (`cce_ui::config`,
//! `cce_ui::motion`, …), so an app never names this crate. A process that does
//! not draw — the compositor, a sync daemon, a CLI helper — depends on it
//! directly and links none of the toolkit's Wayland, Vulkan or text stack.
pub mod color;
#[cfg(feature = "config")]
pub mod config;
pub mod droplet;
pub mod fmt;
#[cfg(feature = "config")]
pub mod input;
#[cfg(not(target_arch = "wasm32"))]
pub mod ipc;
#[cfg(feature = "l10n")]
pub mod l10n;
pub mod locale;
pub mod motion;
pub mod plan;
#[cfg(not(target_arch = "wasm32"))]
pub mod process;
pub mod ramp;
pub mod relief_spec;
pub mod units;
