use {
    serde::{Deserialize, Serialize},
    std::{borrow::Cow, collections::HashMap},
};

pub type KvKey = String;
pub type KvValue = serde_json::Value;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DataStorage {
    pub source_kv: HashMap<KvKey, KvValue>,
}

pub fn value_as_str(v: &KvValue) -> Option<Cow<'_, str>> {
    match v {
        KvValue::String(s) => Some(Cow::Borrowed(s)),
        _ => None,
    }
}
pub fn value_to_string(v: KvValue) -> String {
    match v {
        KvValue::String(s) => s,
        v => v.to_string(),
    }
}
