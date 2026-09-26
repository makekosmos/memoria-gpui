//! ARK wire records (`ArkObjectRecord`, `ArkObjectSummaryRecord`,
//! `ArkObjectTypeRecord`, `ArkObjectLinkRecord` from kepler-entry-mappers.ts).
//! Field order mirrors the Engine wire shape so decode→encode round-trips the
//! fixture byte-for-byte; unknown keys are kept in `extra` and written back.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// `typeVersion` arrives as a semver string; accept numbers too.
fn de_opt_version<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::String(s)) => Some(s),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    })
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArkObjectRecord {
    pub id: String,
    #[serde(rename = "typeId")]
    pub type_id: String,
    #[serde(
        rename = "typeVersion",
        default,
        deserialize_with = "de_opt_version",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_version: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(rename = "contentJson", default)]
    pub content_json: Value,
    #[serde(rename = "propsJson", default)]
    pub props_json: Value,
    #[serde(rename = "createdAt", default)]
    pub created_at: Value,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Value,
    #[serde(rename = "deletedAt", default)]
    pub deleted_at: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArkObjectSummary {
    pub id: String,
    #[serde(rename = "typeId")]
    pub type_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(rename = "propsJson", default)]
    pub props_json: Value,
    #[serde(rename = "createdAt", default)]
    pub created_at: Value,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Value,
    #[serde(rename = "deletedAt", default)]
    pub deleted_at: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArkObjectType {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "schemaJson", default)]
    pub schema_json: String,
    #[serde(rename = "uiSchemaJson", default)]
    pub ui_schema_json: String,
    #[serde(rename = "createdAt", default)]
    pub created_at: Value,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Value,
    #[serde(rename = "systemLocked", default)]
    pub system_locked: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArkObjectLink {
    pub id: String,
    #[serde(rename = "sourceObjectId")]
    pub source_object_id: String,
    #[serde(rename = "targetObjectId")]
    pub target_object_id: String,
    #[serde(rename = "linkType")]
    pub link_type: String,
    #[serde(rename = "createdAt", default)]
    pub created_at: Value,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `ensureList<T>` from the Vue API shims — unwraps bare arrays and the
/// `{items|objects|links|types}` envelope variants.
pub fn ensure_list(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items.clone(),
        Value::Object(map) => {
            for key in ["items", "objects", "links", "types"] {
                if let Some(Value::Array(items)) = map.get(key) {
                    return items.clone();
                }
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}
