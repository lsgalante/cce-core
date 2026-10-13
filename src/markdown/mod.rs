//! Obsidian-flavoured Markdown, shared by the notes domain (cce-vault) and the toolkit (cce-ui's
//! `MarkdownView` and `DocEditor`), so the vault's index, the reading view and the editor's live
//! preview read a note by the same rules.
//!
//! | module | holds | needs |
//! |---|---|---|
//! | `syntax` | the leaf rules: tag names, link targets, URLs, embed sizes, image paths; the frontmatter fences; byte offset → line | nothing |
//! | `properties` | the frontmatter's YAML as JSON values, in file order | the `markdown` feature |
//! | `reading` | a note as a reading view needs it: blocks of styled spans ([`blocks`]) | the `markdown` feature |
//!
//! The leaf rules are always built: the DocEditor, which takes no dependencies, uses them, and
//! so does cce-vault's index, which scans raw bytes for exact spans rather than parsing.

mod syntax;
pub use syntax::*;

#[cfg(feature = "markdown")]
mod properties;
#[cfg(feature = "markdown")]
pub use properties::*;

#[cfg(feature = "markdown")]
mod reading;
#[cfg(feature = "markdown")]
pub use reading::*;
