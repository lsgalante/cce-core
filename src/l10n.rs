//! Translatable strings: a message catalogue per domain, in Project Fluent's format
//! (`docs/rfc-accessibility-locale.md` in cce-ui, phase 5).
//!
//! A domain is one program or library (`cce-ui`, an app's name). Its English messages are
//! built in ([`Catalog::new`], usually `include_str!` of its `en-US/<domain>.ftl`), so it
//! always has every message; a translation is a file a translator drops in, found on first
//! use for the user's locale ([`crate::locale::locale`]) — `ja-JP`, then `ja` — in, in
//! order:
//!
//! 1. `$CCE_LOCALE_DIR/<tag>/<domain>.ftl`
//! 2. `$XDG_DATA_HOME/cce/locale/<tag>/<domain>.ftl` (else `~/.local/share/…`)
//! 3. `<each of $XDG_DATA_DIRS>/cce/locale/<tag>/<domain>.ftl` (else `/usr/local/share`,
//!    `/usr/share`)
//!
//! The first file found for each tag is taken. A message is looked up most specific first
//! and falls back to English, then to its own id — so a missing one shows as its id, which
//! is a bug to see rather than a blank.
//!
//! **An id is the message's identity, never its English text.** Code that acts on a choice
//! keys the action by something else (cce-ui's menus carry a `ContextAction`); translating
//! a label must change what it says and nothing it does.
//!
//! Placeables are not wrapped in Unicode isolation marks (Fluent's default): the renderer
//! would draw them, and the toolkit's own messages put no user text inside right-to-left
//! sentences yet. A page has no files, so in the browser a catalogue is English unless
//! [`Catalog::add_translation`] gives it more. Under `cfg(test)` (or the `test-isolation`
//! feature) no directory is read: a suite never sees the machine's translations.

use std::sync::{OnceLock, RwLock};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use unic_langid::LanguageIdentifier;

/// One domain's messages: its English, and what was found for the user's locale.
pub struct Catalog {
    domain: &'static str,
    english: &'static str,
    /// Most specific first, English last. Built on first use.
    bundles: OnceLock<RwLock<Vec<FluentBundle<FluentResource>>>>,
}

impl Catalog {
    /// A domain's catalogue with its English messages (Fluent source). Nothing is read
    /// until the first lookup.
    pub const fn new(domain: &'static str, english: &'static str) -> Catalog {
        Catalog { domain, english, bundles: OnceLock::new() }
    }

    /// The domain's name, what its translation files are called.
    pub fn domain(&self) -> &'static str {
        self.domain
    }

    /// The message `id` in the user's language, with no arguments.
    pub fn get(&self, id: &str) -> String {
        self.format(id, &[])
    }

    /// The message `id` with its `{ $name }` placeables filled from `args`.
    pub fn format(&self, id: &str, args: &[(&str, &str)]) -> String {
        let bundles = self.bundles().read().unwrap_or_else(|e| e.into_inner());
        let fargs = (!args.is_empty()).then(|| {
            let mut a = FluentArgs::new();
            for (k, v) in args {
                a.set(*k, *v);
            }
            a
        });
        for bundle in bundles.iter() {
            let Some(pattern) = bundle.get_message(id).and_then(|m| m.value()) else { continue };
            let mut errors = Vec::new();
            let out = bundle.format_pattern(pattern, fargs.as_ref(), &mut errors);
            if !errors.is_empty() {
                log::warn!("l10n: {}: message {id}: {errors:?}", self.domain);
            }
            return out.into_owned();
        }
        log::warn!("l10n: {}: no message {id}", self.domain);
        id.to_string()
    }

    /// Add a translation for `tag` (a BCP 47 tag) ahead of everything found so far — what
    /// a page, which has no files, or a test does.
    pub fn add_translation(&self, tag: &str, source: &str) {
        let Some(bundle) = bundle_for(self.domain, tag, source.to_string()) else { return };
        let mut bundles = self.bundles().write().unwrap_or_else(|e| e.into_inner());
        bundles.insert(0, bundle);
    }

    fn bundles(&self) -> &RwLock<Vec<FluentBundle<FluentResource>>> {
        self.bundles.get_or_init(|| {
            let mut bundles = Vec::new();
            for tag in chain(crate::locale::locale()) {
                if let Some(source) = find(self.domain, &tag) {
                    if let Some(b) = bundle_for(self.domain, &tag, source) {
                        bundles.push(b);
                    }
                }
            }
            if let Some(b) = bundle_for(self.domain, "en-US", self.english.to_string()) {
                bundles.push(b);
            }
            RwLock::new(bundles)
        })
    }
}

/// The tags a locale is looked up under, most specific first: `ja-JP` then `ja`. English
/// is built in, so it is not looked for.
pub fn chain(locale: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut parts: Vec<&str> = locale.split('-').filter(|p| !p.is_empty()).collect();
    while !parts.is_empty() {
        let tag = parts.join("-");
        if tag != "en-US" && tag != "en" {
            out.push(tag);
        }
        parts.pop();
    }
    out
}

fn bundle_for(domain: &str, tag: &str, source: String) -> Option<FluentBundle<FluentResource>> {
    let lang: LanguageIdentifier = tag.parse().unwrap_or_else(|_| "en-US".parse().unwrap());
    let resource = match FluentResource::try_new(source) {
        Ok(r) => r,
        Err((r, errors)) => {
            log::warn!("l10n: {domain} ({tag}): {} syntax errors, the rest kept: {errors:?}", errors.len());
            r
        }
    };
    let mut bundle = FluentBundle::new_concurrent(vec![lang]);
    bundle.set_use_isolating(false);
    if let Err(errors) = bundle.add_resource(resource) {
        log::warn!("l10n: {domain} ({tag}): {errors:?}");
    }
    Some(bundle)
}

/// The directories a translation is looked for in, in order.
#[cfg(not(target_arch = "wasm32"))]
pub fn search_dirs() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty());
    let mut dirs = Vec::new();
    if let Some(d) = env("CCE_LOCALE_DIR") {
        dirs.push(PathBuf::from(d));
    }
    let data_home = env("XDG_DATA_HOME").map(PathBuf::from).or_else(|| env("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    if let Some(d) = data_home {
        dirs.push(d.join("cce/locale"));
    }
    let data_dirs = env("XDG_DATA_DIRS").map(|v| v.to_string_lossy().into_owned()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    for d in data_dirs.split(':').filter(|d| !d.is_empty()) {
        dirs.push(PathBuf::from(d).join("cce/locale"));
    }
    dirs
}

/// The first translation file for `domain` in `tag` along [`search_dirs`].
fn find(domain: &str, tag: &str) -> Option<String> {
    #[cfg(any(test, feature = "test-isolation", target_arch = "wasm32"))]
    {
        let _ = (domain, tag);
        None
    }
    #[cfg(not(any(test, feature = "test-isolation", target_arch = "wasm32")))]
    {
        search_dirs().into_iter().find_map(|d| std::fs::read_to_string(d.join(tag).join(format!("{domain}.ftl"))).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: &str = "greeting = Hello\nfile-label = File: { $file }\nonly-english = Only in English\n";

    #[test]
    fn a_message_is_found_formatted_and_falls_back() {
        let c = Catalog::new("test-domain", EN);
        assert_eq!(c.get("greeting"), "Hello");
        assert_eq!(c.format("file-label", &[("file", "a.kdl")]), "File: a.kdl", "no isolation marks");
        assert_eq!(c.get("no-such-message"), "no-such-message", "a missing message is its id");

        c.add_translation("de", "greeting = Hallo\nfile-label = Datei: { $file }\n");
        assert_eq!(c.get("greeting"), "Hallo");
        assert_eq!(c.format("file-label", &[("file", "a.kdl")]), "Datei: a.kdl");
        assert_eq!(c.get("only-english"), "Only in English", "what the translation lacks is English");
    }

    #[test]
    fn a_broken_translation_keeps_what_parses() {
        let c = Catalog::new("test-domain", EN);
        c.add_translation("fr", "greeting = Bonjour\nthis is not fluent\n");
        assert_eq!(c.get("greeting"), "Bonjour");
    }

    #[test]
    fn a_locale_is_looked_up_from_most_to_least_specific() {
        assert_eq!(chain("ja-JP"), ["ja-JP", "ja"]);
        assert_eq!(chain("zh-Hant-TW"), ["zh-Hant-TW", "zh-Hant", "zh"]);
        assert_eq!(chain("en-US"), Vec::<String>::new(), "English is built in");
        assert_eq!(chain("en-GB"), ["en-GB"]);
    }
}
