//! The leaf rules of Obsidian's Markdown, with no dependencies: what a `#tag` is, how a link's
//! target splits, what counts as a URL, what size an embed asks for, which paths are images,
//! where the frontmatter is, and which line a byte is on. A link destination is decoded with
//! [`crate::fmt::percent_decode`] (`My%20Note.md`).

use std::ops::Range;

/// The name of a `#tag` whose `#` comes just before `rest`: letters, digits, `_`, `-` and `/`,
/// without a trailing `/`. `None` when that is empty or only digits and slashes — `#1` and
/// `#2024/10` are not tags. Whether the `#` starts a word is the caller's to decide.
pub fn tag_name(rest: &str) -> Option<&str> {
    let len: usize = rest
        .chars()
        .take_while(|&c| c.is_alphanumeric() || c == '_' || c == '-' || c == '/')
        .map(char::len_utf8)
        .sum();
    let name = rest[..len].trim_end_matches('/');
    if name.is_empty() || name.chars().all(|c| c.is_ascii_digit() || c == '/') {
        None
    } else {
        Some(name)
    }
}

/// A link's `target#subpath` as (target, subpath), both trimmed; an empty subpath is none.
pub fn split_target(dest: &str) -> (&str, Option<&str>) {
    match dest.split_once('#') {
        Some((t, s)) => (t.trim(), Some(s.trim()).filter(|s| !s.is_empty())),
        None => (dest.trim(), None),
    }
}

/// Whether a link destination leaves the vault: any `scheme:` (http, mailto, obsidian://,
/// file:, tel:) — but not a Windows drive letter, which nobody writes in a vault link anyway.
/// A note's name cannot hold a `:` (Obsidian refuses one), so a vault path never reads as a
/// scheme.
pub fn is_url(dest: &str) -> bool {
    match dest.find(':') {
        Some(c) if c > 1 => dest[..c].chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '+' || ch == '-' || ch == '.'),
        _ => false,
    }
}

/// An embed's requested display size, in px: a width, and a height only
/// when both were given (`300x200`); otherwise the height follows the
/// image's aspect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbedSize {
    pub width: u32,
    pub height: Option<u32>,
}

/// The size an embed's alias text asks for: the part after its last `|`
/// (or all of it), read as `W` or `WxH`, each a positive number. Anything
/// else asks for none.
pub fn embed_size(alias: &str) -> Option<EmbedSize> {
    let spec = alias.rsplit('|').next().unwrap_or(alias).trim();
    let (w, h) = match spec.split_once('x') {
        Some((w, h)) => (w, Some(h)),
        None => (spec, None),
    };
    let width = w.trim().parse::<u32>().ok().filter(|w| *w > 0)?;
    let height = match h {
        Some(h) => Some(h.trim().parse::<u32>().ok().filter(|h| *h > 0)?),
        None => None,
    };
    Some(EmbedSize { width, height })
}

/// Whether a vault path names an image Obsidian shows inline when embedded.
pub fn is_image(path: &str) -> bool {
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    matches!(ext.as_deref(), Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "avif"))
}

/// `---\n...\n---` at the very top of the file: (whole block, inner YAML).
pub fn frontmatter_range(src: &str) -> Option<(Range<usize>, Range<usize>)> {
    let first_end = src.find('\n')?;
    if src[..first_end].trim_end_matches('\r') != "---" {
        return None;
    }
    let inner_start = first_end + 1;
    let mut pos = inner_start;
    while pos <= src.len() {
        let end = src[pos..].find('\n').map(|i| pos + i).unwrap_or(src.len());
        if src[pos..end].trim_end_matches('\r') == "---" {
            let whole_end = if end < src.len() { end + 1 } else { end };
            return Some((0..whole_end, inner_start..pos));
        }
        if end == src.len() {
            break;
        }
        pos = end + 1;
    }
    None
}

/// Byte offset → 0-based line.
pub struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(src: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(src.match_indices('\n').map(|(i, _)| i + 1));
        LineIndex { starts }
    }
    pub fn line_of(&self, pos: usize) -> usize {
        self.starts.partition_point(|&s| s <= pos) - 1
    }
    pub fn start_of(&self, line: usize) -> usize {
        self.starts.get(line).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags() {
        assert_eq!(tag_name("tag/x and"), Some("tag/x"));
        assert_eq!(tag_name("a/ b"), Some("a"), "a trailing slash is not part of the name");
        assert_eq!(tag_name("été!"), Some("été"));
        assert_eq!(tag_name("2024/10"), None);
        assert_eq!(tag_name(" x"), None);
        assert_eq!(tag_name("#x"), None);
    }

    #[test]
    fn targets_and_urls() {
        assert_eq!(split_target(" Note # Sec "), ("Note", Some("Sec")));
        assert_eq!(split_target("Note#"), ("Note", None));
        assert_eq!(split_target("Note"), ("Note", None));
        assert!(is_url("https://a.b") && is_url("mailto:x@y") && is_url("obsidian://open?x") && is_url("tel:123"));
        assert!(!is_url("Other.md") && !is_url("C:/x.png") && !is_url("dir/a:b.md") && !is_url(":x"));
    }

    #[test]
    fn embed_sizes() {
        assert_eq!(embed_size("300"), Some(EmbedSize { width: 300, height: None }));
        assert_eq!(embed_size("cap|300x200"), Some(EmbedSize { width: 300, height: Some(200) }));
        assert_eq!(embed_size("0"), None);
        assert_eq!(embed_size("300x"), None);
        assert_eq!(embed_size("caption"), None);
        assert!(is_image("Pics/A.PNG") && !is_image("Note.md") && !is_image("README"));
    }

    #[test]
    fn frontmatter_and_lines() {
        let src = "---\na: 1\n---\nbody\n";
        assert_eq!(frontmatter_range(src), Some((0..13, 4..9)));
        assert_eq!(frontmatter_range("---\nunclosed\n"), None);
        assert_eq!(frontmatter_range("text\n---\n"), None);
        let lines = LineIndex::new(src);
        assert_eq!(lines.line_of(0), 0);
        assert_eq!(lines.line_of(12), 2);
        assert_eq!(lines.line_of(13), 3);
        assert_eq!(lines.start_of(3), 13);
    }
}
