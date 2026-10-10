//! The power plan's session-wide files under `/run/cce`.
//!
//! `cce-power-apply` (cce-system-interface) writes them as root on every
//! plug, unplug and wake, from udev, with no session and no `$HOME` — which
//! is why they live under /run and not `~/.config`. Readers poll them: the
//! animations switch ([`crate::motion`]) in every client and the compositor,
//! the idle-timeout overrides in the compositor's idle manager, the
//! reduced-motion setting in the desktop portal. The paths are spelled only
//! here; until 2026-10-10 each of those crates spelled its own, because the
//! writer could not depend on the compositor that reads them.

/// The directory every plan file lives in.
pub const DIR: &str = "/run/cce";

/// The animations switch: `on` / `off`, missing means on.
pub const ANIMATIONS_FILE: &str = "animations";
/// Seconds until the display goes off, `0` = never; missing means the config's value.
pub const IDLE_DISPLAY_OFF_FILE: &str = "idle_display_off";
/// Seconds until sleep, `0` = never; missing means the config's value.
pub const IDLE_SLEEP_FILE: &str = "idle_sleep";

/// [`DIR`]/[`ANIMATIONS_FILE`].
pub const ANIMATIONS_PATH: &str = "/run/cce/animations";
/// [`DIR`]/[`IDLE_DISPLAY_OFF_FILE`].
pub const IDLE_DISPLAY_OFF_PATH: &str = "/run/cce/idle_display_off";
/// [`DIR`]/[`IDLE_SLEEP_FILE`].
pub const IDLE_SLEEP_PATH: &str = "/run/cce/idle_sleep";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_path_is_its_directory_and_file() {
        for (path, file) in [
            (ANIMATIONS_PATH, ANIMATIONS_FILE),
            (IDLE_DISPLAY_OFF_PATH, IDLE_DISPLAY_OFF_FILE),
            (IDLE_SLEEP_PATH, IDLE_SLEEP_FILE),
        ] {
            assert_eq!(path, format!("{DIR}/{file}"));
        }
        assert_eq!(crate::motion::STATE_PATH, ANIMATIONS_PATH);
    }
}
