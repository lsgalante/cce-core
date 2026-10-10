//! Desktop entries (`.desktop` files): where they live, and their
//! `[Desktop Entry]` group.
//!
//! Five crates read them — the launcher (cce-cloud), the file manager's
//! "open with", the settings app's default-apps page, the desktop portal's
//! notification names and the display manager's session list — and each had
//! its own XDG walk and line parser. Several read past the `[Desktop Entry]`
//! group (an action's `Name=` could become the app's) or mangled a literal
//! `%%` in `Exec=`. The walk, the group parse and the field-code strip are
//! here; what an app keeps from an entry stays the app's.

use std::path::{Path, PathBuf};

/// The XDG data directories in precedence order: `$XDG_DATA_HOME` (else
/// `~/.local/share`), then each of `$XDG_DATA_DIRS` (else `/usr/local/share`
/// and `/usr/share`). Relative entries are skipped, as the base-directory
/// spec says.
pub fn data_dirs() -> Vec<PathBuf> {
    let var = |key: &str| std::env::var_os(key).filter(|v| !v.is_empty());
    let home = var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| Some(PathBuf::from(var("HOME")?).join(".local/share")));
    let dirs: Vec<PathBuf> = var("XDG_DATA_DIRS")
        .map(|v| std::env::split_paths(&v).filter(|p| p.is_absolute()).collect())
        .filter(|d: &Vec<PathBuf>| !d.is_empty())
        .unwrap_or_else(|| vec!["/usr/local/share".into(), "/usr/share".into()]);
    home.into_iter().chain(dirs).collect()
}

/// Every `applications/` directory, in [`data_dirs`] order: where desktop
/// entries are looked up, and where an earlier one shadows a later one with
/// the same id.
pub fn applications_dirs() -> Vec<PathBuf> {
    data_dirs().into_iter().map(|d| d.join("applications")).collect()
}

/// The file for desktop-file id `id` (`firefox.desktop`; the `.desktop` is
/// added when missing) in the first applications directory that has it.
/// Ids with a `-` that came from a subdirectory (`kde-foo.desktop` for
/// `kde/foo.desktop`) are found under their plain name only.
pub fn find(id: &str) -> Option<PathBuf> {
    let file = if id.ends_with(".desktop") { id.to_string() } else { format!("{id}.desktop") };
    applications_dirs().into_iter().map(|d| d.join(&file)).find(|p| p.is_file())
}

/// The `[Desktop Entry]` group of one desktop file: its keys and values in
/// file order. Other groups (`[Desktop Action …]`) are not read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DesktopEntry {
    fields: Vec<(String, String)>,
}

impl DesktopEntry {
    pub fn parse(text: &str) -> DesktopEntry {
        let mut fields = Vec::new();
        let mut in_entry = false;
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                in_entry = line == "[Desktop Entry]";
            } else if in_entry && !line.starts_with('#') {
                if let Some((k, v)) = line.split_once('=') {
                    fields.push((k.trim().to_string(), v.trim().to_string()));
                }
            }
        }
        DesktopEntry { fields }
    }

    /// The file at `path`, or `None` when it cannot be read.
    pub fn read(path: &Path) -> Option<DesktopEntry> {
        std::fs::read_to_string(path).ok().map(|t| DesktopEntry::parse(&t))
    }

    /// The value of `key` (exact, so `Name[de]` is its own key); the first
    /// one when a broken file repeats it.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// A boolean key: `true` (any case) is true, anything else or absent is false.
    pub fn flag(&self, key: &str) -> bool {
        self.get(key).is_some_and(|v| v.eq_ignore_ascii_case("true"))
    }

    /// A `;`-separated list key (`MimeType`, `Categories`), empty items dropped.
    pub fn list(&self, key: &str) -> Vec<&str> {
        self.get(key).map(|v| v.split(';').map(str::trim).filter(|s| !s.is_empty()).collect()).unwrap_or_default()
    }

    /// `Type=Application`.
    pub fn is_application(&self) -> bool {
        self.get("Type") == Some("Application")
    }

    /// `Hidden=true`: the spec's "treat as deleted".
    pub fn hidden(&self) -> bool {
        self.flag("Hidden")
    }

    /// `NoDisplay=true`: a real entry that a launcher should not list.
    pub fn no_display(&self) -> bool {
        self.flag("NoDisplay")
    }
}

/// `Exec=` with its field codes removed (`%f`, `%U`, …, and the deprecated
/// ones), `%%` turned into `%`, and the words rejoined with single spaces:
/// the command to run with no file or URL argument. A word that was only a
/// field code goes entirely.
pub fn strip_field_codes(exec: &str) -> String {
    let mut words = Vec::new();
    for word in exec.split_whitespace() {
        let mut out = String::with_capacity(word.len());
        let mut chars = word.chars();
        while let Some(c) = chars.next() {
            if c != '%' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('%') => out.push('%'),
                // A field code (current or deprecated): dropped.
                Some(_) | None => {}
            }
        }
        if !out.is_empty() {
            words.push(out);
        }
    }
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "\
# a comment
[Desktop Entry]
Type=Application
Name=Files
Name[de]=Dateien
Exec=cce-files %U
Icon = cce-files
MimeType=inode/directory;application/x-gnome-saved-search;
NoDisplay=TRUE

[Desktop Action new-window]
Name=New Window
Exec=cce-files --new
";

    #[test]
    fn only_the_desktop_entry_group_is_read() {
        let e = DesktopEntry::parse(FILE);
        assert_eq!(e.get("Name"), Some("Files"));
        assert_eq!(e.get("Name[de]"), Some("Dateien"));
        assert_eq!(e.get("Exec"), Some("cce-files %U"));
        assert_eq!(e.get("Icon"), Some("cce-files"));
        assert_eq!(e.list("MimeType"), vec!["inode/directory", "application/x-gnome-saved-search"]);
        assert!(e.is_application() && e.no_display() && !e.hidden());
        assert_eq!(e.get("Comment"), None);
    }

    #[test]
    fn field_codes_go_and_a_literal_percent_stays() {
        assert_eq!(strip_field_codes("cce-files %U"), "cce-files");
        assert_eq!(strip_field_codes("app --name=%c --file=%f x"), "app --name= --file= x");
        assert_eq!(strip_field_codes("printf 100%% %i"), "printf 100%");
        assert_eq!(strip_field_codes("echo %%f"), "echo %f");
    }

    #[test]
    fn data_dirs_skip_relative_entries() {
        // Only the shape is checked: the environment is the test runner's.
        assert!(data_dirs().iter().all(|d| d.is_absolute()));
        assert!(applications_dirs().iter().all(|d| d.ends_with("applications")));
    }
}
