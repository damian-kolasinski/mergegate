//! JSON that keeps its objects' keys in the order the file had them, for
//! editing an agent's settings without reordering the operator's. The
//! workspace's `serde_json` sorts keys, and turning on its `preserve_order`
//! would change that for every crate in `mg`.

use std::fmt;

use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub(crate) fn parse(text: &str) -> serde_json::Result<Self> {
        serde_json::from_str(text)
    }

    /// Pretty-printed with two spaces, as agents write their own settings,
    /// and a final newline.
    pub(crate) fn print(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_default();
        text.push('\n');
        text
    }

    pub(crate) fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The value at `key`, added last as `Json::Object` when missing. `None`
    /// when `self` isn't an object.
    pub(crate) fn entry(&mut self, key: &str) -> Option<&mut Json> {
        let Json::Object(fields) = self else {
            return None;
        };
        let at = match fields.iter().position(|(k, _)| k == key) {
            Some(at) => at,
            None => {
                fields.push((key.to_owned(), Json::Object(Vec::new())));
                fields.len() - 1
            }
        };
        Some(&mut fields[at].1)
    }

    pub(crate) fn remove(&mut self, key: &str) {
        if let Json::Object(fields) = self {
            fields.retain(|(k, _)| k != key);
        }
    }

    pub(crate) fn is_empty_container(&self) -> bool {
        match self {
            Json::Array(items) => items.is_empty(),
            Json::Object(fields) => fields.is_empty(),
            _ => false,
        }
    }
}

impl Serialize for Json {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Json::Null => serializer.serialize_unit(),
            Json::Bool(b) => serializer.serialize_bool(*b),
            Json::Number(n) => n.serialize(serializer),
            Json::String(s) => serializer.serialize_str(s),
            Json::Array(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Json::Object(fields) => {
                let mut map = serializer.serialize_map(Some(fields.len()))?;
                for (key, value) in fields {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(JsonVisitor)
    }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
    type Value = Json;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_unit<E>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_bool<E>(self, b: bool) -> Result<Json, E> {
        Ok(Json::Bool(b))
    }

    fn visit_i64<E>(self, n: i64) -> Result<Json, E> {
        Ok(Json::Number(n.into()))
    }

    fn visit_u64<E>(self, n: u64) -> Result<Json, E> {
        Ok(Json::Number(n.into()))
    }

    fn visit_f64<E: de::Error>(self, n: f64) -> Result<Json, E> {
        serde_json::Number::from_f64(n)
            .map(Json::Number)
            .ok_or_else(|| E::custom("a number JSON can't hold"))
    }

    fn visit_str<E>(self, s: &str) -> Result<Json, E> {
        Ok(Json::String(s.to_owned()))
    }

    fn visit_string<E>(self, s: String) -> Result<Json, E> {
        Ok(Json::String(s))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Json::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        let mut fields: Vec<(String, Json)> = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, Json>()? {
            // A repeated key means the last one, as serde_json reads it.
            fields.retain(|(k, _)| *k != key);
            fields.push((key, value));
        }
        Ok(Json::Object(fields))
    }
}

#[cfg(test)]
#[path = "json_tests.rs"]
mod tests;
