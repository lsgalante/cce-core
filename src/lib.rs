//! The GUI-free half of the cce toolkit.
//!
//! What a cce process needs whether or not it draws anything: the KDL config
//! (`config`) and input bindings (`input`), the DE-wide animations switch
//! (`motion`), lengths with units and the display metric (`units`), the
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
#[cfg(feature = "config")]
pub mod input;
#[cfg(not(target_arch = "wasm32"))]
pub mod ipc;
pub mod motion;
pub mod ramp;
pub mod relief_spec;
pub mod units;
