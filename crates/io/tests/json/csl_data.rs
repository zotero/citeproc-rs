use serde::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;

use super::schema::{JsonSchema, Primitive, SchemaNode};

#[derive(Deserialize, Debug)]
#[serde(transparent)]
pub struct CslDataSchema(JsonSchema);

impl SchemaNode {
    fn match_ref(&self, ref_name: &str) -> bool {
        if let SchemaNode::Ref { pointer } = self {
            return pointer == ref_name;
        }
        false
    }
    fn unwrap_object(&self) -> &HashMap<String, SchemaNode> {
        match self {
            SchemaNode::Object { properties, .. } => properties,
            _ => panic!("could not unwrap object from SchemaNode: {:?}", self),
        }
    }
}

impl CslDataSchema {
    pub const IGNORED_VARIABLES: &'static [&'static str] = &["id", "type", "categories", "custom"];
    pub const DATE_VARIABLE: &'static str = "#/definitions/date-variable";
    pub const NAME_VARIABLE: &'static str = "#/definitions/name-variable";
    pub const EDTF_DATATYPE: &'static str = "#/definitions/edtf-datatype";

    pub fn variables(&self) -> &HashMap<String, SchemaNode> {
        match &self.0.inner {
            SchemaNode::Array { items, .. } => match &**items {
                SchemaNode::Object { properties, .. } => return properties,
                _ => panic!("csl-data.json was an array of something other than an object"),
            },
            _ => panic!("csl-data.json schema didn't have an array as the top level element"),
        }
    }
    pub fn date_variables(&self) -> Vec<&str> {
        let mut ret = Vec::new();
        let vars = self.variables();
        for (var, sch) in vars.iter() {
            if sch.match_ref(Self::DATE_VARIABLE) {
                ret.push(var.as_str());
            }
        }
        ret.retain(Self::not_ignored);
        ret
    }

    pub fn named_schema(&self, pointer: &str) -> &SchemaNode {
        let name = pointer.trim_start_matches("#/definitions/");
        self.0
            .definitions
            .get(name)
            .unwrap_or_else(|| panic!("{} not defined in definitions section", name))
    }

    /// Comes with an additional "year" property that's not in the spec.
    pub fn date_schemas_properties(&self) -> HashMap<String, SchemaNode> {
        let date_schema = self.named_schema(Self::DATE_VARIABLE);
        if let SchemaNode::AnyOf { any_of, .. } = date_schema {
            let obj = any_of.get(1).expect("date-schema only had one anyOf?");
            let mut obj = obj.unwrap_object().clone();
            obj.insert(
                "year".to_owned(),
                SchemaNode::Multi {
                    types: vec![Primitive::String, Primitive::Number],
                    common: Default::default(),
                },
            );
            obj
        } else {
            panic!("date-variable schema is not anyOf")
        }
    }

    // not needed, really. nothing to test about it.
    // pub fn name_schema(&self) -> &SchemaNode {
    //     self.0.definitions.get(Self::NAME_VARIABLE).expect("name-variable not defined in definitions section")
    // }

    pub fn name_variables(&self) -> Vec<&str> {
        let mut ret = Vec::new();
        let vars = self.variables();
        for (var, sch) in vars.iter() {
            if let SchemaNode::Array { items, .. } = sch {
                if items.match_ref(Self::NAME_VARIABLE) {
                    ret.push(var.as_str());
                }
            }
        }
        ret.retain(Self::not_ignored);
        ret
    }

    pub fn string_variables(&self) -> Vec<&str> {
        let mut ret = Vec::new();
        let vars = self.variables();
        for (var, sch) in vars.iter() {
            if matches!(sch, SchemaNode::String { enm: None, .. }) {
                ret.push(var.as_str());
            }
        }
        ret.retain(Self::not_ignored);
        ret
    }

    pub fn not_ignored(var: &&str) -> bool {
        !Self::IGNORED_VARIABLES.contains(&&var)
    }

    pub fn number_variables(&self) -> Vec<&str> {
        let mut ret = Vec::new();
        let vars = self.variables();
        let prims = vec![Primitive::String, Primitive::Number];
        for (var, sch) in vars.iter() {
            if matches!(&sch, SchemaNode::Multi { types, .. } if types == &prims) {
                ret.push(var.as_str());
            }
        }
        ret.retain(Self::not_ignored);
        ret
    }

    pub fn all_variables(&self) -> HashSet<&str> {
        let vars = self.variables();
        vars.iter().map(|x| x.0.as_str()).collect()
    }
}
