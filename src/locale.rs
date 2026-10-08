//! The user's locale, as a BCP 47 tag (`ja-JP`).
//!
//! What the toolkit's font systems are built with: cosmic-text orders its font
//! fallback by it, so the Han characters Chinese and Japanese share are drawn from
//! the reader's own variant. It was the literal `"en-US"` in every constructor
//! until 2026-10-08 (`docs/rfc-accessibility-locale.md`, phase 0).
//!
//! Natively it comes from the environment, the way glibc's `setlocale` reads it
//! for text: `LC_ALL`, else `LC_CTYPE`, else `LANG`, the first one set and not
//! empty. A page has no environment, so the browser shell hands over
//! `navigator.language` through [`set_locale`] before anything shapes text.
//! `C`, `POSIX` and anything that is not a locale name read as `en-US`.

use std::sync::OnceLock;

/// What every lookup answers when nothing better is known.
pub const DEFAULT: &str = "en-US";

static LOCALE: OnceLock<String> = OnceLock::new();

/// The user's locale as a BCP 47 tag, decided once per process.
pub fn locale() -> &'static str {
    LOCALE.get_or_init(|| from_env(|k| std::env::var(k).ok()).unwrap_or_else(|| DEFAULT.to_string()))
}

/// Decide the locale for this process, ahead of the first [`locale`] call: the
/// browser shell's `navigator.language`. Taken as a BCP 47 tag or a POSIX name.
/// A later call, or one after [`locale`] has answered, changes nothing — every
/// font system in the process must agree — and says so by returning false.
pub fn set_locale(tag: &str) -> bool {
    let tag = from_posix(tag).unwrap_or_else(|| DEFAULT.to_string());
    LOCALE.set(tag).is_ok()
}

/// The locale the environment names, through `get`: `LC_ALL`, `LC_CTYPE`,
/// `LANG`, the first set and non-empty. `None` when none is, or it is `C` /
/// `POSIX` or unreadable.
pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Option<String> {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .filter_map(|k| get(k))
        .find(|v| !v.is_empty())
        .and_then(|v| from_posix(&v))
}

/// A POSIX locale name (`ja_JP.UTF-8`, `sr_RS@latin`, `de`) or a BCP 47 tag
/// (`pt-BR`) as a BCP 47 tag: the codeset and modifier dropped, `_` made `-`,
/// the language lower-case and a two-letter region upper-case. `None` for `C`,
/// `POSIX` and anything whose language is not two or three letters.
pub fn from_posix(name: &str) -> Option<String> {
    let base = name.trim().split(['.', '@']).next().unwrap_or("");
    let mut parts = base.split(['_', '-']).filter(|p| !p.is_empty());
    let lang = parts.next()?;
    if !(2..=3).contains(&lang.len()) || !lang.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut tag = lang.to_ascii_lowercase();
    for p in parts {
        if !p.chars().all(|c| c.is_ascii_alphanumeric()) {
            return None;
        }
        tag.push('-');
        if p.len() == 2 && p.chars().all(|c| c.is_ascii_alphabetic()) {
            tag.push_str(&p.to_ascii_uppercase());
        } else {
            tag.push_str(p);
        }
    }
    Some(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_names_become_bcp47_tags() {
        assert_eq!(from_posix("ja_JP.UTF-8").as_deref(), Some("ja-JP"));
        assert_eq!(from_posix("en_US.UTF-8").as_deref(), Some("en-US"));
        assert_eq!(from_posix("de_DE@euro").as_deref(), Some("de-DE"));
        assert_eq!(from_posix("sr_RS@latin").as_deref(), Some("sr-RS"));
        assert_eq!(from_posix("fr").as_deref(), Some("fr"));
        assert_eq!(from_posix("pt-BR").as_deref(), Some("pt-BR"), "already a tag");
        assert_eq!(from_posix("zh-Hant-TW").as_deref(), Some("zh-Hant-TW"), "a script subtag kept");
        assert_eq!(from_posix("ast_ES.UTF-8").as_deref(), Some("ast-ES"), "three-letter language");
    }

    #[test]
    fn c_posix_and_junk_are_not_locales() {
        assert_eq!(from_posix("C"), None);
        assert_eq!(from_posix("C.UTF-8"), None);
        assert_eq!(from_posix("POSIX"), None);
        assert_eq!(from_posix(""), None);
        assert_eq!(from_posix("en_U$"), None);
    }

    #[test]
    fn the_environment_is_read_as_setlocale_reads_it() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
        };
        assert_eq!(from_env(env(&[("LANG", "de_DE.UTF-8")])).as_deref(), Some("de-DE"));
        assert_eq!(
            from_env(env(&[("LANG", "de_DE.UTF-8"), ("LC_CTYPE", "ja_JP.UTF-8")])).as_deref(),
            Some("ja-JP"),
            "LC_CTYPE before LANG"
        );
        assert_eq!(
            from_env(env(&[("LC_ALL", "fr_FR.UTF-8"), ("LC_CTYPE", "ja_JP.UTF-8")])).as_deref(),
            Some("fr-FR"),
            "LC_ALL before everything"
        );
        assert_eq!(
            from_env(env(&[("LC_ALL", ""), ("LANG", "it_IT.UTF-8")])).as_deref(),
            Some("it-IT"),
            "an empty variable is unset"
        );
        assert_eq!(from_env(env(&[("LANG", "C.UTF-8")])), None, "C is no locale");
        assert_eq!(from_env(env(&[])), None);
    }
}
