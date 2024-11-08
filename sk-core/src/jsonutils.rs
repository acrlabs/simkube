use std::collections::hash_map::DefaultHasher;
use std::hash::{
    Hash,
    Hasher,
};

use serde_json as json;

struct HashableJsonValue<'a>(&'a json::Value);

impl<'a> Hash for HashableJsonValue<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self.0 {
            json::Value::Null => None::<()>.hash(state),
            json::Value::Bool(b) => b.hash(state),
            json::Value::Number(n) => n.hash(state),
            json::Value::String(s) => s.hash(state),
            json::Value::Array(a) => {
                for v in a {
                    HashableJsonValue(v).hash(state);
                }
            },
            json::Value::Object(o) => {
                for (k, v) in o {
                    k.hash(state);
                    HashableJsonValue(v).hash(state);
                }
            },
        }
    }
}

pub fn hash_option(maybe_v: Option<&json::Value>) -> u64 {
    let mut s = DefaultHasher::new();
    match maybe_v {
        None => HashableJsonValue(&json::Value::Null).hash(&mut s),
        Some(v) => HashableJsonValue(v).hash(&mut s),
    }
    s.finish()
}

pub fn hash(v: &json::Value) -> u64 {
    let mut s = DefaultHasher::new();
    HashableJsonValue(v).hash(&mut s);
    s.finish()
}
