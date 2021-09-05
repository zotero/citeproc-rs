use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct JsonSchema {
    pub description: Option<String>,
    #[serde(rename = "$id")]
    pub id: Option<String>,
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    #[serde(default)]
    pub definitions: HashMap<String, SchemaNode>,

    #[serde(flatten)]
    pub inner: SchemaNode,
}

macro_rules! tag {
    ($vis:vis $tname:ident, $tagval:literal) => {
        #[derive(Debug, Copy, Clone, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        $vis enum $tname {
            #[serde(rename = $tagval)]
            Value,
        }
        impl Default for $tname {
            fn default() -> Self { Self::Value }
        }
    };
}

tag!(pub ArrayTag, "array");
tag!(pub ObjectTag, "object");
tag!(pub StringTag, "string");

#[derive(Debug, Copy, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Primitive {
    String,
    Integer,
    Float,
    Number,
    Boolean,
    Null,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SchemaNode {
    String {
        #[serde(rename = "type")]
        typ: StringTag,
        #[serde(rename = "enum")]
        enm: Option<Vec<String>>,
        #[serde(flatten)]
        common: Common,
    },
    Primitive {
        #[serde(rename = "type")]
        typ: Primitive,
        #[serde(flatten)]
        common: Common,
    },
    Ref {
        #[serde(rename = "$ref")]
        // #[serde(deserialize_with = "pointer_from_str")]
        pointer: String,
    },
    Multi {
        #[serde(rename = "type")]
        types: Vec<Primitive>,
        #[serde(flatten)]
        common: Common,
    },
    AnyOf {
        #[serde(rename = "anyOf")]
        any_of: Vec<SchemaNode>,
        #[serde(flatten)]
        common: Common,
    },
    AllOf {
        #[serde(rename = "allOf")]
        all_of: Vec<SchemaNode>,
        #[serde(flatten)]
        common: Common,
    },
    OneOf {
        #[serde(rename = "oneOf")]
        one_of: Vec<SchemaNode>,
        #[serde(flatten)]
        common: Common,
    },
    Not {
        not: Box<SchemaNode>,
        #[serde(flatten)]
        common: Common,
    },
    Array {
        #[serde(default, rename = "type")]
        tag: ArrayTag,
        items: Box<SchemaNode>,
        #[serde(rename = "minItems")]
        min_items: Option<u32>,
        #[serde(rename = "maxItems")]
        max_items: Option<u32>,
        #[serde(flatten)]
        common: Common,
    },
    Object {
        #[serde(default, rename = "type")]
        tag: ObjectTag,
        #[serde(default)]
        properties: HashMap<String, SchemaNode>,
        #[serde(default)]
        required: Vec<String>,
        #[serde(default = "bool_true", rename = "additionalProperties")]
        additional_properties: bool,
        #[serde(flatten)]
        common: Common,
    },
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Common {
    description: Option<String>,
    title: Option<String>,
    examples: Option<Vec<Value>>,
}

fn bool_true() -> bool {
    true
}

// not necessary
// use json_pointer::JsonPointer;
// fn pointer_from_str<'de, D>(d: D) -> Result<JsonPointer<String, Vec<String>>, D::Error> where D: serde::de::Deserializer<'de> {
//     let s = String::deserialize(d)?;
//     s.parse().map_err(|_e| serde::de::Error::invalid_value(serde::de::Unexpected::Str(&s), &"json pointer"))
// }
