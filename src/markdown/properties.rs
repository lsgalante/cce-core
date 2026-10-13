//! The frontmatter's YAML as JSON values, in file order (with the `markdown` feature).

use serde_json::{Map, Value};

/// Frontmatter properties, in file order. An `IndexMap` rather than
/// `serde_json`'s `preserve_order` feature, which would unify across the
/// workspace and reorder every other crate's JSON (see cce-vault's `canvas`).
/// Maps nested inside a value are ordinary sorted `serde_json` maps.
pub type Properties = indexmap::IndexMap<String, Value>;

/// YAML → JSON values. A block that does not parse, or is not a mapping,
/// yields no properties rather than an error: Obsidian shows such a note
/// with its frontmatter as plain text, and the index should still hold it.
/// The block is the inner range [`super::frontmatter_range`] finds.
pub fn parse_properties(yaml: &str) -> Properties {
    let docs = match yaml_rust2::YamlLoader::load_from_str(yaml) {
        Ok(docs) => docs,
        Err(_) => return Properties::new(),
    };
    match docs.into_iter().next() {
        Some(yaml_rust2::Yaml::Hash(h)) => h
            .into_iter()
            .filter_map(|(k, v)| Some((yaml_key(k)?, yaml_to_json(v))))
            .collect(),
        _ => Properties::new(),
    }
}

fn yaml_key(k: yaml_rust2::Yaml) -> Option<String> {
    use yaml_rust2::Yaml;
    match k {
        Yaml::String(s) | Yaml::Real(s) => Some(s),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

fn yaml_to_json(y: yaml_rust2::Yaml) -> Value {
    use yaml_rust2::Yaml;
    match y {
        Yaml::Real(s) => s
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or(Value::String(s)),
        Yaml::Integer(i) => Value::from(i),
        Yaml::String(s) => Value::String(s),
        Yaml::Boolean(b) => Value::Bool(b),
        Yaml::Array(a) => Value::Array(a.into_iter().map(yaml_to_json).collect()),
        Yaml::Hash(h) => Value::Object(
            h.into_iter().filter_map(|(k, v)| Some((yaml_key(k)?, yaml_to_json(v)))).collect::<Map<_, _>>(),
        ),
        Yaml::Null | Yaml::Alias(_) | Yaml::BadValue => Value::Null,
    }
}
