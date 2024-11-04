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

struct OrderedHashableJsonValue<'a>(&'a json::Value);

impl<'a> Hash for OrderedHashableJsonValue<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self.0 {
            json::Value::Null => None::<()>.hash(state),
            json::Value::Bool(b) => b.hash(state),
            json::Value::Number(n) => n.hash(state),
            json::Value::String(s) => s.hash(state),
            json::Value::Array(a) => {
                let mut hashes = a
                    .into_iter()
                    .map(|v| {
                        let hasher = &mut DefaultHasher::new();
                        OrderedHashableJsonValue(v).hash(hasher);

                        hasher.finish()
                    })
                    .collect::<Vec<_>>();
                hashes.sort();

                for hash in hashes {
                    hash.hash(state);
                }
            },
            json::Value::Object(o) => {
                let mut hashes = o
                    .into_iter()
                    .map(|(k, v)| {
                        let hasher = &mut DefaultHasher::new();
                        k.hash(hasher);
                        OrderedHashableJsonValue(v).hash(hasher);

                        hasher.finish()
                    })
                    .collect::<Vec<_>>();
                hashes.sort();

                for hash in hashes {
                    hash.hash(state);
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

pub fn ordered_hash(v: &json::Value) -> u64 {
    let mut s = DefaultHasher::new();
    OrderedHashableJsonValue(v).hash(&mut s);
    s.finish()
}

pub fn ordered_eq(v1: &json::Value, v2: &json::Value) -> bool {
    order_json(v1) == order_json(v2)
}

pub fn order_json(value: &json::Value) -> json::Value {
    match value {
        json::Value::Array(a) => {
            let mut pairs: Vec<(u64, &json::Value)> = a.iter().map(|v| (ordered_hash(v), v)).collect();
            pairs.sort_by_key(|(h, _)| *h);

            json::Value::Array(pairs.into_iter().map(|(_, v)| order_json(v)).collect())
        },
        json::Value::Object(o) => {
            let mut pairs: Vec<_> = o.iter().map(|(k, v)| (k.clone(), order_json(v))).collect();

            pairs.sort_by(|(k1, _), (k2, _)| std::cmp::Ord::cmp(k1, k2));

            json::Value::Object(pairs.into_iter().collect())
        },
        _ => value.clone(),
    }
}
