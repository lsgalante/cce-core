// ~/.config/cce/input.kdl — domain-scoped keybindings and pointer input
// settings for the whole desktop.
//
// Top-level nodes are DOMAINS; their children are bindings. One top-level
// node is special: `input { }` holds the global hardware pointer defaults
// (accel, scroll factors per device class), consumed by the compositor.
// Inside a domain, an `input { }` child holds that app's behavior
// overrides, consumed client-side by this module:
//
//     input {                          // global hardware defaults (compositor)
//         accel_profile "flat"
//         accel_speed 1.0
//         mouse { scroll_factor 1.0 }
//         trackpad { tap_to_click true; natural_scroll true; scroll_factor 1.5 }
//         trackpoint { accel_speed 0.5 }
//     }
//     cce-window-manager {
//         close_window "super+q"
//         spawn "super+d" command="cce-cloud --apps"
//         focus_left "swipe3_left"     // a touchpad gesture chord: swipe|pinch,
//                                      // optional finger count, direction
//     }
//     cce-ui {
//         open_search "ctrl+f"         // toolkit-wide widget defaults
//         input {
//             scroll_factor 1.0        // toolkit-wide scroll default
//             smooth_scroll true       // wheel notches glide (widget::scroll_motion)
//             scroll_ease 12.0         // glide rate, 1/s
//             kinetic_scroll true      // trackpad flicks coast after the lift
//             scroll_friction 6.0      // coast decay, 1/s
//         }
//     }
//     cce-files {
//         open_search "/"              // per-app override of the cce-ui default
//         input {
//             scroll_factor 0.8        // both device kinds
//             trackpad { scroll_factor 0.6 }
//         }
//     }
//
// The chord is the first string argument (a `(keybind)` or `(gesture)` type
// annotation is accepted and ignored); a `key="..."` property works too.
// Extra properties (e.g. `command=` for spawn) ride along on the entry, and
// `label="..."` says what the binding does in words, for the key-help
// overlay and the Keyboard page (`InputConfig::label`).
//
// Resolution order for an app is `<app>.<name>` → `cce-ui.<name>`; widgets
// match the resolved chord string with `widget::match_key_shortcut`. The
// `cce-window-manager` domain is consumed by the compositor, which maps
// names to policy `Action`s — chords never get interpreted here.
//
// Per-app scroll factors compose with the compositor's device scaling: the
// compositor applies the global `input` block at the event source; a client
// then scales its own wheel deltas by the resolved app factor (pixel deltas
// count as `trackpad`, discrete wheel clicks as `mouse`).

use std::collections::BTreeMap;

/// Domain holding toolkit-wide default widget bindings.
pub const UI_DOMAIN: &str = "cce-ui";
/// Domain holding compositor / window-management bindings.
pub const WINDOW_MANAGER_DOMAIN: &str = "cce-window-manager";

/// `~/.config/cce/input.kdl` (honoring `XDG_CONFIG_HOME`).
pub fn get_input_path() -> std::path::PathBuf {
    crate::config::cce_config_dir().join("input.kdl")
}

/// One binding line inside a domain block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingEntry {
    /// Node name, e.g. `open_search`. Names may repeat (several `spawn`s).
    pub name: String,
    /// The chord string, e.g. `"super+shift+h"`.
    pub chord: String,
    /// `command="..."` property, for entries that launch something.
    pub command: Option<String>,
}

/// A typed value inside an `input { }` settings block.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingValue {
    Float(f64),
    Bool(bool),
    Str(String),
}

impl SettingValue {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            SettingValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            SettingValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            SettingValue::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// Device classes an `input { }` block may scope settings to.
pub const DEVICE_CLASSES: [&str; 3] = ["mouse", "trackpad", "trackpoint"];

/// One `input { }` block: generic `name value` settings plus per-device-class
/// sub-blocks (`mouse` / `trackpad` / `trackpoint`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputSettings {
    values: BTreeMap<String, SettingValue>,
    classes: BTreeMap<String, BTreeMap<String, SettingValue>>,
}

impl InputSettings {
    fn parse(node: &kdl::KdlNode) -> InputSettings {
        let mut settings = InputSettings::default();
        let Some(children) = node.children() else { return settings };
        for child in children.nodes() {
            let name = child.name().value();
            if DEVICE_CLASSES.contains(&name) {
                let class = settings.classes.entry(name.to_string()).or_default();
                if let Some(class_children) = child.children() {
                    for leaf in class_children.nodes() {
                        if let Some(v) = setting_value(leaf) {
                            class.insert(leaf.name().value().to_string(), v);
                        }
                    }
                }
            } else if let Some(v) = setting_value(child) {
                settings.values.insert(name.to_string(), v);
            }
        }
        settings
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.classes.values().all(|c| c.is_empty())
    }

    /// A generic (class-independent) setting.
    pub fn get(&self, key: &str) -> Option<&SettingValue> {
        self.values.get(key)
    }

    /// A setting for one device class, falling back to the generic value.
    pub fn get_class(&self, class: &str, key: &str) -> Option<&SettingValue> {
        self.classes.get(class).and_then(|c| c.get(key)).or_else(|| self.get(key))
    }
}

/// First-argument value of a settings leaf node, if it is a scalar.
fn setting_value(node: &kdl::KdlNode) -> Option<SettingValue> {
    let entry = node.entries().iter().find(|e| e.name().is_none())?;
    match entry.value() {
        kdl::KdlValue::Base10Float(f) => Some(SettingValue::Float(*f)),
        kdl::KdlValue::Base2(i) | kdl::KdlValue::Base8(i) | kdl::KdlValue::Base10(i) | kdl::KdlValue::Base16(i) => {
            Some(SettingValue::Float(*i as f64))
        }
        kdl::KdlValue::Bool(b) => Some(SettingValue::Bool(*b)),
        kdl::KdlValue::String(s) | kdl::KdlValue::RawString(s) => Some(SettingValue::Str(s.clone())),
        kdl::KdlValue::Null => None,
    }
}

#[derive(Debug, Clone, Default)]
pub struct InputConfig {
    domains: BTreeMap<String, Vec<BindingEntry>>,
    /// Per-domain `input { }` behavior overrides.
    settings: BTreeMap<String, InputSettings>,
    /// The top-level `input { }` block: global hardware defaults, consumed
    /// by the compositor.
    global: InputSettings,
    /// Each domain's `label=` properties, index-aligned with its entries.
    /// Beside [`BindingEntry`] rather than in it, so code that builds
    /// entries by struct literal (the compositor's `migrate-input`) is
    /// untouched.
    labels: BTreeMap<String, Vec<Option<String>>>,
}

impl InputConfig {
    /// Parse the file content. Domain blocks with no children are ignored;
    /// a child with no chord (no string argument and no `key=`) is skipped.
    pub fn parse(content: &str) -> Result<InputConfig, String> {
        let doc: kdl::KdlDocument = content.parse().map_err(|e| format!("{}", e))?;
        let mut domains: BTreeMap<String, Vec<BindingEntry>> = BTreeMap::new();
        let mut settings: BTreeMap<String, InputSettings> = BTreeMap::new();
        let mut global = InputSettings::default();
        let mut labels: BTreeMap<String, Vec<Option<String>>> = BTreeMap::new();
        for domain_node in doc.nodes() {
            // The top-level `input { }` block is global hardware defaults,
            // not a domain.
            if domain_node.name().value() == "input" {
                global = InputSettings::parse(domain_node);
                continue;
            }
            let Some(children) = domain_node.children() else { continue };
            let domain = domain_node.name().value().to_string();
            let entries = domains.entry(domain.clone()).or_default();
            let entry_labels = labels.entry(domain.clone()).or_default();
            for node in children.nodes() {
                // A domain's `input { }` child is its settings block.
                if node.name().value() == "input" {
                    settings.insert(domain.clone(), InputSettings::parse(node));
                    continue;
                }
                let mut chord: Option<String> = None;
                let mut command: Option<String> = None;
                let mut label: Option<String> = None;
                for entry in node.entries() {
                    let value = match entry.value() {
                        kdl::KdlValue::String(s) | kdl::KdlValue::RawString(s) => s.clone(),
                        _ => continue,
                    };
                    match entry.name().map(|n| n.value()) {
                        None | Some("key") => {
                            if chord.is_none() {
                                chord = Some(value);
                            }
                        }
                        Some("command") => command = Some(value),
                        Some("label") => label = Some(value),
                        Some(_) => {}
                    }
                }
                if let Some(chord) = chord {
                    entries.push(BindingEntry {
                        name: node.name().value().to_string(),
                        chord,
                        command,
                    });
                    entry_labels.push(label);
                }
            }
        }
        Ok(InputConfig { domains, settings, global, labels })
    }

    /// Load `input.kdl`. Missing file → empty config; a parse error is
    /// logged and also yields an empty config, so callers fall back to
    /// their defaults instead of losing all input.
    pub fn load() -> InputConfig {
        let path = get_input_path();
        let Ok(content) = std::fs::read_to_string(&path) else {
            return InputConfig::default();
        };
        match InputConfig::parse(&content) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("[cce-ui] failed to parse {}: {}", path.display(), e);
                InputConfig::default()
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.domains.values().all(|v| v.is_empty())
    }

    /// All entries of one domain, in file order.
    /// The `label="..."` of `domain`'s entry at `index` (its place in
    /// [`domain`](Self::domain)), if it has one.
    pub fn label(&self, domain: &str, index: usize) -> Option<&str> {
        self.labels.get(domain)?.get(index)?.as_deref()
    }

    pub fn domain(&self, domain: &str) -> &[BindingEntry] {
        self.domains.get(domain).map(Vec::as_slice).unwrap_or(&[])
    }

    /// First entry named `name` in `domain`, no fallback.
    pub fn get(&self, domain: &str, name: &str) -> Option<&BindingEntry> {
        self.domain(domain).iter().find(|e| e.name == name)
    }

    /// Domain resolution for apps: `<app>.<name>`, falling back to
    /// `cce-ui.<name>`.
    pub fn resolve(&self, app: &str, name: &str) -> Option<&BindingEntry> {
        self.get(app, name).or_else(|| self.get(UI_DOMAIN, name))
    }

    /// The top-level `input { }` block (global hardware defaults).
    pub fn global_input(&self) -> &InputSettings {
        &self.global
    }

    /// One domain's `input { }` behavior overrides.
    pub fn domain_input(&self, domain: &str) -> Option<&InputSettings> {
        self.settings.get(domain)
    }

    /// Setting resolution for apps, most specific first: the app domain's
    /// class value → its generic value → the cce-ui domain's class value →
    /// its generic value. The global `input` block is deliberately NOT in
    /// the chain — the compositor already applies it at the event source.
    pub fn resolve_setting(&self, app: &str, class: &str, key: &str) -> Option<&SettingValue> {
        self.domain_input(app)
            .and_then(|s| s.get_class(class, key))
            .or_else(|| self.domain_input(UI_DOMAIN).and_then(|s| s.get_class(class, key)))
    }

    /// Resolved chord string for widgets, with a compiled-in default as the
    /// last resort.
    pub fn resolve_chord(&self, app: &str, name: &str, default: &str) -> String {
        self.resolve(app, name).map(|e| e.chord.clone()).unwrap_or_else(|| default.to_string())
    }
}

/// Replace (or append) one domain block in `input.kdl` content, leaving all
/// other domains and their formatting untouched. `entries` becomes the whole
/// new block, in order; an empty slice removes the domain. Pure — the I/O
/// wrapper is `write_domain`.
pub fn upsert_domain(content: &str, domain: &str, entries: &[BindingEntry]) -> Result<String, String> {
    let mut doc: kdl::KdlDocument = if content.trim().is_empty() {
        kdl::KdlDocument::new()
    } else {
        content.parse().map_err(|e| format!("{}", e))?
    };

    let mut block = format!("{} {{\n", kdl_ident(domain));
    for entry in entries {
        block.push_str(&format!("    {} (keybind){:?}", kdl_ident(&entry.name), entry.chord));
        if let Some(ref command) = entry.command {
            block.push_str(&format!(" command={:?}", command));
        }
        block.push('\n');
    }
    block.push_str("}\n");

    doc.nodes_mut().retain(|n| n.name().value() != domain);
    if !entries.is_empty() {
        let node: kdl::KdlNode = block.parse().map_err(|e| format!("{}", e))?;
        doc.nodes_mut().push(node);
    }
    let mut out = doc.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// Quote a node name if it isn't a bare KDL identifier.
fn kdl_ident(name: &str) -> String {
    let bare = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit());
    if bare { name.to_string() } else { format!("{:?}", name) }
}

/// Rewrite one domain of the file at `path` (created if missing). This is
/// the editor API for settings UIs and migration tools.
pub fn write_domain(path: &std::path::Path, domain: &str, entries: &[BindingEntry]) -> Result<(), String> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let updated = upsert_domain(&content, domain, entries)?;
    let path_str = path.to_string_lossy();
    if crate::config::safe_write(&path_str, &updated) {
        Ok(())
    } else {
        Err(format!("failed to write {}", path_str))
    }
}

static CACHED: std::sync::OnceLock<InputConfig> = std::sync::OnceLock::new();

/// Process-wide cached `input.kdl`, loaded on first use. Widget-default
/// getters go through this so the file is read once per app.
pub fn cached() -> &'static InputConfig {
    CACHED.get_or_init(InputConfig::load)
}

/// Chord for one of this app's bindings: `<app>.<name>` → `cce-ui.<name>` →
/// the compiled-in default. The app domain is the binary name. This is the
/// standard way for a client to resolve its shortcuts at startup:
///
/// ```text
/// let open = cce_ui::input::app_chord("open_file", "enter");
/// ... cce_ui::widget::match_key_shortcut(event, &open) ...
/// ```
pub fn app_chord(name: &str, default: &str) -> String {
    let app = crate::config::get_app_name().unwrap_or_default();
    let chord = cached().resolve_chord(&app, name, default);
    published::record(name, &chord, toolkit_label(name));
    chord
}

/// [`app_chord`] with what the binding does, in the words the key-help
/// overlay shows ("New tab", "Archive message"). Prefer it for a new
/// shortcut: a binding resolved through plain `app_chord` is listed under
/// its name made readable, which is only as clear as the name.
pub fn app_shortcut(name: &str, default: &str, label: &str) -> String {
    let app = crate::config::get_app_name().unwrap_or_default();
    let chord = cached().resolve_chord(&app, name, default);
    published::record(name, &chord, Some(label));
    chord
}

/// What the toolkit's own bindings do — the `cce-ui` domain's names, which
/// the toolkit resolves itself through `app_chord` / `widget_chord` and no
/// app labels.
pub fn toolkit_label(name: &str) -> Option<&'static str> {
    Some(match name {
        "undo" => "Undo",
        "redo" => "Redo",
        "focus_next_group" => "Next pane",
        "focus_prev_group" => "Previous pane",
        "open_search" => "Search",
        "close_search" => "Close search",
        _ => return None,
    })
}

/// What this process's shortcuts resolved to, for the desktop's key-help
/// overlay (`cce-keys`, Super+/): every [`app_chord`] and [`widget_chord`]
/// answer is recorded here and written to
/// `$XDG_RUNTIME_DIR/cce/keys/<pid>`, so the overlay can list the FOCUSED
/// window's bindings — compiled-in defaults included, which input.kdl alone
/// cannot say — by the pid the compositor reports for it. Nothing to call
/// from an app: resolving a chord is publishing it.
///
/// The file is `start <starttime>` (the process's start time from
/// `/proc/<pid>/stat`, so a recycled pid never reads a dead app's file),
/// `app <name>`, then one `<name>\t<chord>[\t<label>]` line per binding in
/// first-resolved order; the label is [`app_shortcut`]'s, or the toolkit's
/// own ([`toolkit_label`]). It is rewritten (tmp + rename) only when a binding is new
/// or changed, which is a burst at startup and then nothing: `widget_chord`
/// runs per key event in a text box, and finds its answer already recorded.
pub mod published {
    use std::sync::Mutex;

    /// One published binding.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Binding {
        pub name: String,
        pub chord: String,
        /// What it does, when the app said ([`super::app_shortcut`]).
        pub label: Option<String>,
    }

    /// The bindings this process has resolved, in order.
    static RECORDED: Mutex<Vec<Binding>> = Mutex::new(Vec::new());

    /// The bindings a running process published: `(app, bindings)`.
    /// `None` when it published none, or the file outlived its process.
    pub fn read(pid: u32) -> Option<(String, Vec<Binding>)> {
        let text = std::fs::read_to_string(dir().join(pid.to_string())).ok()?;
        parse(&text, start_time(pid)?)
    }

    /// [`read`]'s parse, against the start time the pid has now.
    pub fn parse(text: &str, start: u64) -> Option<(String, Vec<Binding>)> {
        let mut lines = text.lines();
        let written: u64 = lines.next()?.strip_prefix("start ")?.trim().parse().ok()?;
        if written != start {
            return None;
        }
        let app = lines.next()?.strip_prefix("app ")?.trim().to_string();
        let bindings = lines
            .filter_map(|l| {
                let mut fields = l.split('\t');
                let name = fields.next()?.to_string();
                let chord = fields.next()?.to_string();
                let label = fields.next().filter(|s| !s.is_empty()).map(str::to_string);
                Some(Binding { name, chord, label })
            })
            .collect();
        Some((app, bindings))
    }

    /// The file's text for `bindings`.
    pub fn format(start: u64, app: &str, bindings: &[Binding]) -> String {
        // A tab or newline inside a field would end it; none occurs in a
        // binding name, chord or label, but a label is free text.
        let clean = |s: &str| s.replace(['\t', '\n'], " ");
        let mut out = format!("start {start}\napp {app}\n");
        for b in bindings {
            out.push_str(&format!("{}\t{}", clean(&b.name), clean(&b.chord)));
            if let Some(label) = &b.label {
                out.push_str(&format!("\t{}", clean(label)));
            }
            out.push('\n');
        }
        out
    }

    pub(super) fn record(name: &str, chord: &str, label: Option<&str>) {
        let Ok(mut recorded) = RECORDED.lock() else { return };
        let label = label.map(str::to_string);
        match recorded.iter_mut().find(|b| b.name == name) {
            // A later unlabeled resolution of a labeled name keeps the label.
            Some(b) if b.chord == chord && (label.is_none() || b.label == label) => return,
            Some(b) => {
                b.chord = chord.to_string();
                b.label = label.or(b.label.take());
            }
            None => recorded.push(Binding { name: name.to_string(), chord: chord.to_string(), label }),
        }
        // Not in a browser either: a page has no pid, and no runtime
        // directory for the file (std::process::id panics on wasm32).
        #[cfg(not(any(test, feature = "test-isolation", target_arch = "wasm32")))]
        write(&recorded);
    }

    #[cfg(not(any(test, feature = "test-isolation", target_arch = "wasm32")))]
    fn write(bindings: &[Binding]) {
        static PRUNED: std::sync::Once = std::sync::Once::new();
        let pid = std::process::id();
        let Some(start) = start_time(pid) else { return };
        let dir = dir();
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        // Files of processes that have exited: nothing else removes them.
        PRUNED.call_once(|| {
            let Ok(entries) = std::fs::read_dir(&dir) else { return };
            for e in entries.flatten() {
                let name = e.file_name();
                let Some(other) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else { continue };
                if !std::path::Path::new(&format!("/proc/{other}")).exists() {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        });
        let app = crate::config::get_app_name().unwrap_or_default();
        let path = dir.join(pid.to_string());
        let tmp = dir.join(format!(".{pid}.tmp"));
        if std::fs::write(&tmp, format(start, &app, bindings)).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }

    fn dir() -> std::path::PathBuf {
        crate::config::runtime_dir().join("cce").join("keys")
    }

    /// Field 22 of `/proc/<pid>/stat`, the start time in clock ticks. The
    /// comm field before it may hold spaces and parens, so count from the
    /// last `)`.
    fn start_time(pid: u32) -> Option<u64> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        stat.rsplit_once(')')?.1.split_whitespace().nth(19)?.parse().ok()
    }
}

/// Whether the trackpad scrolls NATURALLY — the content following the
/// fingers — per input.kdl's `trackpad { natural_scroll }`, as the
/// compositor applies it. Read once per process; a thread's override
/// (`force_natural_scroll`, for a test) wins over it, and under `cfg(test)`
/// with no override the answer is `false`, so the toolkit's own suite does
/// not read the machine.
///
/// What it is for: a VALUE control — a slider, a spinbox, a menu's slider
/// row — takes the wheel as "up is more", and a finger under natural
/// scrolling as the same thing, which is the opposite sign of the pixel
/// delta the runner hands it (the delta is what a LIST scrolls by, and a
/// list under natural scrolling moves its content the way the fingers
/// went). See `MouseScrollDelta::value_notches_y`.
pub fn natural_scroll() -> bool {
    if let Some(forced) = NATURAL_OVERRIDE.with(|f| f.get()) {
        return forced;
    }
    #[cfg(any(test, feature = "test-isolation"))]
    {
        false
    }
    #[cfg(not(any(test, feature = "test-isolation")))]
    {
        *NATURAL_SCROLL.get_or_init(|| {
            let input = cached();
            let app = crate::config::get_app_name().unwrap_or_default();
            input.resolve_setting(&app, "trackpad", "natural_scroll").and_then(SettingValue::as_bool).unwrap_or(false)
        })
    }
}

#[cfg(not(any(test, feature = "test-isolation")))]
static NATURAL_SCROLL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

thread_local! {
    static NATURAL_OVERRIDE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Force what [`natural_scroll`] answers on this thread, for a test that
/// drives a value control with a finger; `None` lifts it. Thread-local,
/// because a suite runs its tests in parallel and a process-wide override
/// set by one would race every other test's read. The macOS shell sets it
/// too, on the main thread its events arrive on, from each trackpad
/// event's `isDirectionInvertedFromDevice`: there the system's setting is
/// the one in force, not input.kdl's.
pub fn force_natural_scroll(natural: Option<bool>) {
    NATURAL_OVERRIDE.with(|f| f.set(natural));
}

/// Run `f` with [`natural_scroll`] answering `natural`, then put back
/// whatever override was in force. For an input whose direction is known
/// regardless of the trackpad setting: a touchscreen finger is always
/// natural (the runner's touch scroll, `backend/touch.rs`).
pub fn with_natural_scroll<R>(natural: bool, f: impl FnOnce() -> R) -> R {
    let prev = NATURAL_OVERRIDE.with(|o| o.replace(Some(natural)));
    let out = f();
    NATURAL_OVERRIDE.with(|o| o.set(prev));
    out
}

/// This app's effective wheel-delta multipliers, resolved once per process.
/// Pixel (smooth) deltas scale by `trackpad`, discrete clicks by `mouse`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollFactors {
    pub mouse: f64,
    pub trackpad: f64,
}

static SCROLL_FACTORS: std::sync::OnceLock<ScrollFactors> = std::sync::OnceLock::new();

pub fn scroll_factors() -> ScrollFactors {
    *SCROLL_FACTORS.get_or_init(|| {
        let input = cached();
        let app = crate::config::get_app_name().unwrap_or_default();
        let factor = |class: &str| {
            input
                .resolve_setting(&app, class, "scroll_factor")
                .and_then(SettingValue::as_f64)
                .filter(|f| f.is_finite() && *f > 0.0)
                .unwrap_or(1.0)
        };
        ScrollFactors { mouse: factor("mouse"), trackpad: factor("trackpad") }
    })
}

/// Chord for a widget binding, resolved by specificity: the app's own
/// `input.kdl` domain, then the caller-supplied legacy value (per-widget
/// `config.kdl` props), then the toolkit-wide `cce-ui` domain, then the
/// compiled-in default.
pub fn widget_chord(name: &str, legacy: &str, default: &str) -> String {
    let chord = resolve_widget_chord(name, legacy, default);
    published::record(name, &chord, toolkit_label(name));
    chord
}

fn resolve_widget_chord(name: &str, legacy: &str, default: &str) -> String {
    let input = cached();
    if let Some(app) = crate::config::get_app_name() {
        if let Some(e) = input.get(&app, name) {
            return e.chord.clone();
        }
    }
    if !legacy.is_empty() {
        return legacy.to_string();
    }
    if let Some(e) = input.get(UI_DOMAIN, name) {
        return e.chord.clone();
    }
    default.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
cce-window-manager {
    close_window "super+q"
    toggle_fullscreen (keybind)"super+f"
    spawn "super+d" command="cce-cloud --apps"
    spawn "super+t" command="foot"
}
cce-ui {
    open_search "ctrl+f"
    close_search "escape"
}
cce-files {
    open_file key="enter"
    open_search "/"
}
"#;

    #[test]
    fn parses_domains_and_entries() {
        let c = InputConfig::parse(SAMPLE).unwrap();
        assert_eq!(c.domain(WINDOW_MANAGER_DOMAIN).len(), 4);
        assert_eq!(c.get(WINDOW_MANAGER_DOMAIN, "close_window").unwrap().chord, "super+q");
        // Type annotations are transparent.
        assert_eq!(c.get(WINDOW_MANAGER_DOMAIN, "toggle_fullscreen").unwrap().chord, "super+f");
        // Repeated names keep every entry, in order, with their commands.
        let spawns: Vec<_> =
            c.domain(WINDOW_MANAGER_DOMAIN).iter().filter(|e| e.name == "spawn").collect();
        assert_eq!(spawns.len(), 2);
        assert_eq!(spawns[0].command.as_deref(), Some("cce-cloud --apps"));
        assert_eq!(spawns[1].chord, "super+t");
        // key= property form.
        assert_eq!(c.get("cce-files", "open_file").unwrap().chord, "enter");
    }

    #[test]
    fn published_bindings_round_trip_and_refuse_a_recycled_pid() {
        let b = |name: &str, chord: &str, label: Option<&str>| published::Binding {
            name: name.into(),
            chord: chord.into(),
            label: label.map(Into::into),
        };
        let bindings = vec![b("save", "ctrl+s", Some("Save the note")), b("open_search", "/", None)];
        let text = published::format(4242, "cce-notes", &bindings);
        assert_eq!(published::parse(&text, 4242), Some(("cce-notes".to_string(), bindings)));
        // Another process now holds the pid: its start time differs.
        assert_eq!(published::parse(&text, 4243), None);
        assert_eq!(published::parse("", 4242), None);
        // A file from before labels: two fields a line.
        let old = "start 1\napp x\nsave\tctrl+s\n";
        assert_eq!(published::parse(old, 1).unwrap().1, vec![b("save", "ctrl+s", None)]);
    }

    #[test]
    fn labels_ride_beside_their_entries() {
        let c = InputConfig::parse(
            "cce-window-manager {\n spawn \"super+d\" command=\"cce-cloud --apps\" label=\"App launcher\"\n close_window \"super+q\"\n}\n",
        )
        .unwrap();
        assert_eq!(c.label(WINDOW_MANAGER_DOMAIN, 0), Some("App launcher"));
        assert_eq!(c.label(WINDOW_MANAGER_DOMAIN, 1), None);
        assert_eq!(c.label(WINDOW_MANAGER_DOMAIN, 2), None);
        assert_eq!(c.domain(WINDOW_MANAGER_DOMAIN)[0].command.as_deref(), Some("cce-cloud --apps"));
    }

    #[test]
    fn resolution_prefers_app_over_ui_domain() {
        let c = InputConfig::parse(SAMPLE).unwrap();
        // Overridden in cce-files.
        assert_eq!(c.resolve("cce-files", "open_search").unwrap().chord, "/");
        // Not overridden: falls back to cce-ui.
        assert_eq!(c.resolve("cce-files", "close_search").unwrap().chord, "escape");
        // Unknown app: pure cce-ui fallback.
        assert_eq!(c.resolve("cce-mail", "open_search").unwrap().chord, "ctrl+f");
        // Nowhere: compiled-in default.
        assert_eq!(c.resolve_chord("cce-mail", "save", "ctrl+s"), "ctrl+s");
    }

    const SETTINGS_SAMPLE: &str = r#"
input {
    accel_profile "flat"
    accel_speed 1.0
    mouse {
        accel_speed 0.5
        scroll_factor 2.0
    }
    trackpad {
        tap_to_click true
        scroll_factor 1.5
    }
}
cce-ui {
    open_search "ctrl+f"
    input {
        scroll_factor 1.25
    }
}
cce-files {
    open_file "enter"
    input {
        scroll_factor 0.8
        trackpad {
            scroll_factor 0.6
        }
    }
}
"#;

    #[test]
    fn parses_settings_blocks() {
        let c = InputConfig::parse(SETTINGS_SAMPLE).unwrap();
        // The top-level input block is global, not a domain.
        assert!(c.domain("input").is_empty());
        let g = c.global_input();
        assert_eq!(g.get("accel_profile").and_then(SettingValue::as_str), Some("flat"));
        assert_eq!(g.get("accel_speed").and_then(SettingValue::as_f64), Some(1.0));
        // Class value wins over generic; missing class value falls back.
        assert_eq!(g.get_class("mouse", "accel_speed").and_then(SettingValue::as_f64), Some(0.5));
        assert_eq!(g.get_class("trackpad", "accel_speed").and_then(SettingValue::as_f64), Some(1.0));
        assert_eq!(g.get_class("trackpad", "tap_to_click").and_then(SettingValue::as_bool), Some(true));
        // Settings blocks don't pollute the binding lists.
        assert_eq!(c.domain("cce-files").len(), 1);
        assert_eq!(c.get("cce-files", "open_file").unwrap().chord, "enter");
    }

    #[test]
    fn setting_resolution_prefers_app_then_ui_domain() {
        let c = InputConfig::parse(SETTINGS_SAMPLE).unwrap();
        // App class value → app generic → cce-ui.
        let f = |app: &str, class: &str| {
            c.resolve_setting(app, class, "scroll_factor").and_then(SettingValue::as_f64)
        };
        assert_eq!(f("cce-files", "trackpad"), Some(0.6));
        assert_eq!(f("cce-files", "mouse"), Some(0.8)); // generic app value
        assert_eq!(f("cce-mail", "trackpad"), Some(1.25)); // cce-ui fallback
        // The global input block is not in the client chain.
        assert_eq!(c.resolve_setting("cce-mail", "mouse", "accel_speed"), None);
    }

    #[test]
    fn upsert_domain_round_trips() {
        let entries = vec![
            BindingEntry { name: "close_window".into(), chord: "super+q".into(), command: None },
            BindingEntry {
                name: "spawn".into(),
                chord: "super+d".into(),
                command: Some("cce-cloud --apps".into()),
            },
        ];
        // Insert into empty content, then read back.
        let out = upsert_domain("", WINDOW_MANAGER_DOMAIN, &entries).unwrap();
        let parsed = InputConfig::parse(&out).unwrap();
        assert_eq!(parsed.domain(WINDOW_MANAGER_DOMAIN).to_vec(), entries);

        // Replace the domain without touching other domains.
        let combined = format!("{}\n{}", SAMPLE, ""); // SAMPLE already has the domain
        let replaced = upsert_domain(
            &combined,
            WINDOW_MANAGER_DOMAIN,
            &entries[..1],
        )
        .unwrap();
        let parsed = InputConfig::parse(&replaced).unwrap();
        assert_eq!(parsed.domain(WINDOW_MANAGER_DOMAIN).len(), 1);
        assert_eq!(parsed.get("cce-files", "open_search").unwrap().chord, "/");

        // Empty entries removes the block entirely.
        let removed = upsert_domain(&replaced, WINDOW_MANAGER_DOMAIN, &[]).unwrap();
        let parsed = InputConfig::parse(&removed).unwrap();
        assert!(parsed.domain(WINDOW_MANAGER_DOMAIN).is_empty());
        assert_eq!(parsed.resolve("cce-files", "close_search").unwrap().chord, "escape");
    }

    #[test]
    fn empty_and_invalid_input() {
        assert!(InputConfig::parse("").unwrap().is_empty());
        // Chord-less entries are skipped, childless nodes ignored.
        let c = InputConfig::parse("cce-ui {\n    broken\n}\nstray-node\n").unwrap();
        assert!(c.is_empty());
        assert!(InputConfig::parse("cce-ui {").is_err());
    }
}
