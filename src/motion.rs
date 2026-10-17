//! The DE-wide animations switch.
//!
//! One flag, followed by every cce-ui widget that eases and by the
//! compositor: when it is off, anything that would glide, fade or slide
//! lands on its target in the same frame instead. Disabled means
//! "snap", never "freeze" — a dropdown still opens, a scroll still moves.
//! A trackpad flick's coast is NOT under it (`widget::scroll_motion`): that
//! is the hand's gesture carried on, not an animation the toolkit adds.
//!
//! The switch is a file, [`STATE_PATH`], holding `on` or `off`. The System
//! Interface's Power page sets it per power mode and `cce-power-apply`
//! writes it as root whenever the mode changes (plug, unplug, boot), which
//! is why it lives under /run rather than in `~/.config/cce`: the writer has
//! no session and no `$HOME`. No file means on.
//!
//! [`enabled`] is cheap enough for per-frame use: it re-reads the file at
//! most every [`RECHECK`], so a running client follows a mode change within
//! half a second and nothing needs restarting or reloading. `CCE_ANIMATIONS`
//! (`0`/`off` or `1`/`on`) overrides the file for one process, for testing.
//!
//! Beside the switch, the DE's animation **speed** ([`speed`]): a multiplier
//! on the rhythm of whatever animates for as long as it runs — the activity
//! indicators, which step their cycle by `dt * speed()`. It is a preference,
//! not a power-mode lever, so it lives in the shared config as
//! `style { animation_speed 1.0 }` ([`SPEED_KEY`]), set from the System
//! Interface's Appearance page, and like the switch it is re-read at most
//! every [`RECHECK`], so a running client follows a change without a reload.
//! Transitions (a dropdown opening, a toggle sliding) keep their own timing.

use std::time::Duration;

/// Where the switch lives. Shared with `cce-power-apply`, the writer.
pub const STATE_PATH: &str = crate::plan::ANIMATIONS_PATH;

/// How stale [`enabled`] may be. A mode change is a plug or an unplug, so
/// half a second is instant to a person, and the stat stays off the frame.
pub const RECHECK: Duration = Duration::from_millis(500);

/// `on` / `off` (surrounding whitespace ignored); anything else says nothing.
pub fn parse(text: &str) -> Option<bool> {
    match text.trim() {
        "on" | "1" | "true" => Some(true),
        "off" | "0" | "false" => Some(false),
        _ => None,
    }
}

/// What the state file says right now, uncached. `None` when there is no
/// file or it holds nothing recognizable — both of which mean "animate".
pub fn read_state() -> Option<bool> {
    parse(&std::fs::read_to_string(STATE_PATH).ok()?)
}

// Under `cfg(test)` the switch is the SUITE's, not the machine's: on,
// unless a test forces it with [`force_for_test`]. Until 2026-09-28
// `enabled` read `/run/cce/animations` in the test binary too, so three
// glide and fade tests passed or failed with the laptop's power mode —
// off on battery, on when plugged in — and read as a broken glide rather
// than a borrowed switch. Thread-local rather than the shared cache,
// because libtest runs tests in parallel and a process-wide override set
// by one test would race every other test's read; a test that forces it
// does so for its own thread only, and the value resets with the thread.
#[cfg(any(test, feature = "test-isolation"))]
thread_local! {
    static FORCED: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Set what [`enabled`] answers on this thread, for a test that exercises
/// the snap-instead-of-ease path. Tests never set `CCE_ANIMATIONS`, since
/// an environment variable is process-wide.
#[cfg(any(test, feature = "test-isolation"))]
pub fn force_for_test(value: bool) {
    FORCED.with(|f| f.set(value));
}

/// Whether to animate. Every easing in the toolkit asks this before it
/// steps, and snaps to its target when the answer is no.
pub fn enabled() -> bool {
    #[cfg(any(test, feature = "test-isolation"))]
    {
        FORCED.with(|f| f.get())
    }
    #[cfg(not(any(test, feature = "test-isolation")))]
    enabled_on_this_machine()
}

/// [`enabled`] as the shipped binary answers it: the `CCE_ANIMATIONS`
/// override for this process, else the state file, re-read at most every
/// [`RECHECK`].
#[cfg(not(any(test, feature = "test-isolation")))]
fn enabled_on_this_machine() -> bool {
    static ENV: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
    if let Some(forced) = *ENV.get_or_init(|| std::env::var("CCE_ANIMATIONS").ok().and_then(|v| parse(&v))) {
        return forced;
    }
    use std::sync::Mutex;
    use web_time::Instant;
    static CACHE: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    match *cache {
        Some((at, value)) if now.duration_since(at) < RECHECK => value,
        _ => {
            let value = read_state().unwrap_or(true);
            *cache = Some((now, value));
            value
        }
    }
}

/// The speed's key in the shared config, as `config::write_config_value`
/// spells it.
pub const SPEED_KEY: &str = "style.animation_speed";

/// The slowest and fastest [`speed`]; a configured value outside is clamped.
pub const MIN_SPEED: f32 = 0.25;
pub const MAX_SPEED: f32 = 4.0;

/// A configured speed as [`speed`] answers it: clamped to
/// [`MIN_SPEED`]..=[`MAX_SPEED`], and normal (1) when it is not a positive
/// finite number.
pub fn clamp_speed(value: f64) -> f32 {
    let v = value as f32;
    if v.is_finite() && v > 0.0 {
        v.clamp(MIN_SPEED, MAX_SPEED)
    } else {
        1.0
    }
}

/// What the shared config says right now about the speed ([`SPEED_KEY`]),
/// clamped; `None` when it says nothing, which means normal. Mtime-cached
/// by the config module, not by [`RECHECK`] — for the page that sets it.
#[cfg(feature = "config")]
pub fn read_speed() -> Option<f32> {
    crate::config::cached_shared_config_arc().pointer("/style/animation_speed").and_then(|v| v.as_f64()).map(clamp_speed)
}

#[cfg(any(test, feature = "test-isolation"))]
thread_local! {
    static FORCED_SPEED: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}

/// Set what [`speed`] answers on this thread, as [`force_for_test`] does
/// for the switch. Normal (1) unless forced.
#[cfg(any(test, feature = "test-isolation"))]
pub fn force_speed_for_test(value: f32) {
    FORCED_SPEED.with(|f| f.set(value));
}

/// How fast the DE animates, as a multiplier on an animation's own rhythm:
/// 1 is normal, 0.5 half as fast. Cheap enough for per-frame use. Without
/// the `config` feature there is nothing to read, and it is 1.
pub fn speed() -> f32 {
    #[cfg(any(test, feature = "test-isolation"))]
    {
        FORCED_SPEED.with(|f| f.get())
    }
    #[cfg(not(any(test, feature = "test-isolation")))]
    speed_on_this_machine()
}

/// [`speed`] as the shipped binary answers it: the shared config's value,
/// re-read at most every [`RECHECK`].
#[cfg(all(feature = "config", not(any(test, feature = "test-isolation"))))]
fn speed_on_this_machine() -> f32 {
    use std::sync::Mutex;
    use web_time::Instant;
    static CACHE: Mutex<Option<(Instant, f32)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    match *cache {
        Some((at, value)) if now.duration_since(at) < RECHECK => value,
        _ => {
            let value = read_speed().unwrap_or(1.0);
            *cache = Some((now, value));
            value
        }
    }
}

#[cfg(all(not(feature = "config"), not(any(test, feature = "test-isolation"))))]
fn speed_on_this_machine() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configured_speed_is_clamped_and_nonsense_is_normal() {
        assert_eq!(clamp_speed(0.5), 0.5);
        assert_eq!(clamp_speed(100.0), MAX_SPEED);
        assert_eq!(clamp_speed(0.01), MIN_SPEED);
        assert_eq!(clamp_speed(0.0), 1.0);
        assert_eq!(clamp_speed(-2.0), 1.0);
        assert_eq!(clamp_speed(f64::NAN), 1.0);
    }

    /// The key as the writer stores it is where the reader looks: a float
    /// child of `style`, beside the rest of the block.
    #[cfg(feature = "config")]
    #[test]
    fn the_speed_key_is_read_where_it_is_written() {
        let mut doc: kdl::KdlDocument = "style {\n    color_scheme \"dark\"\n}\n".parse().unwrap();
        assert!(crate::config::update_kdl_in_memory(&mut doc, SPEED_KEY, "0.75", "style"));
        let text = doc.to_string();
        let json = crate::config::parse_kdl_to_json(&text);
        assert_eq!(json.pointer("/style/animation_speed").and_then(|v| v.as_f64()), Some(0.75), "{text}");
        assert_eq!(json.pointer("/style/color_scheme").and_then(|v| v.as_str()), Some("dark"), "{text}");
    }

    #[test]
    fn the_suite_runs_at_normal_speed_unless_a_test_forces_it() {
        assert_eq!(speed(), 1.0);
        force_speed_for_test(0.5);
        assert_eq!(speed(), 0.5);
        assert_eq!(std::thread::spawn(speed).join().unwrap(), 1.0, "the force is this thread's alone");
        force_speed_for_test(1.0);
    }

    #[test]
    fn parse_reads_the_helpers_spelling_and_nothing_else() {
        assert_eq!(parse("off\n"), Some(false));
        assert_eq!(parse("on"), Some(true));
        assert_eq!(parse(" 0 "), Some(false));
        assert_eq!(parse(""), None);
        assert_eq!(parse("disabled"), None);
    }

    /// The suite's switch is on whatever the machine's file says, a test
    /// can force it off for its own thread only, and another thread still
    /// sees it on.
    #[test]
    fn the_suite_animates_whatever_the_machine_says() {
        assert!(enabled(), "on by default, not read off {STATE_PATH}");
        force_for_test(false);
        assert!(!enabled(), "a test can force the snap path");
        let elsewhere = std::thread::spawn(enabled).join().unwrap();
        assert!(elsewhere, "the force is this thread's alone");
        force_for_test(true);
        assert!(enabled());
    }
}
