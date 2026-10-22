//! Turning the schema of one tagged enum into one schema per tool.
//!
//! `EditCommand` is internally tagged, so its schema is a `oneOf` whose branches each
//! pin `type` to a constant. A tool's arguments are one of those branches with the tag
//! taken out: the caller picked the tool by name, so repeating the name inside the
//! arguments is a second place to get it wrong.
//!
//! Doing it this way rather than writing nineteen schemas by hand is the point. The
//! descriptions are the doc comments above each variant, and a variant added later
//! arrives with its schema already written.

use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The tag `serde` writes, and which this reads back.
const TAG: &str = "type";

/// One input schema per variant, keyed by the tag value, which is the variant's name.
///
/// Every branch gets its own copy of the definitions, because a branch on its own still
/// refers to them and a reference with nothing to resolve against is worse than no
/// schema at all.
pub fn per_variant(schema: &Value) -> BTreeMap<String, Value> {
    let defs = schema.get("$defs").cloned();
    let Some(branches) = schema.get("oneOf").and_then(Value::as_array) else {
        return BTreeMap::new();
    };
    branches
        .iter()
        .filter_map(|branch| {
            let name = branch
                .pointer(&format!("/properties/{TAG}/const"))?
                .as_str()?
                .to_string();
            Some((name, without_tag(branch, defs.clone())))
        })
        .collect()
}

/// A branch with the tag removed and the definitions attached.
fn without_tag(branch: &Value, defs: Option<Value>) -> Value {
    let mut out = branch.clone();
    let Some(object) = out.as_object_mut() else {
        return out;
    };

    if let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) {
        properties.remove(TAG);
    }
    if let Some(required) = object.get_mut("required").and_then(Value::as_array_mut) {
        required.retain(|name| name.as_str() != Some(TAG));
        if required.is_empty() {
            object.remove("required");
        }
    }
    // A command whose only field was the tag takes no arguments, and an empty
    // `properties` says that more plainly than a missing one.
    if !object.contains_key("properties") {
        object.insert("properties".into(), Value::Object(Map::new()));
    }
    if let Some(defs) = defs {
        object.insert("$defs".into(), defs);
    }
    out
}

/// A schema for a type that takes no arguments at all.
pub fn nothing() -> Value {
    serde_json::json!({ "type": "object", "properties": {} })
}
