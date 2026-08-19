use std::collections::hash_map::DefaultHasher;
use std::hash::{
    Hash,
    Hasher,
};

use kube::api::DynamicObject;
use serde_json::Value;

fn hash<H: Hasher>(val: &Value, state: &mut H) {
    match val {
        Value::Null => None::<()>.hash(state),
        Value::Bool(b) => b.hash(state),
        Value::Number(n) => n.hash(state),
        Value::String(s) => s.hash(state),
        Value::Array(a) => {
            for v in a {
                hash(v, state);
            }
        },
        Value::Object(o) => {
            for (k, v) in o {
                hash(v, state);
                k.hash(state);
            }
        },
    }
}

pub(crate) fn hash_dynamic_object(obj: &DynamicObject) -> u64 {
    let mut state = DefaultHasher::new();
    match obj.data.get("spec") {
        None => hash(&Value::Null, &mut state),
        Some(val) => hash(val, &mut state),
    };
    state.finish()
}

#[cfg(test)]
mod tests {
    use sk_testutils::*;

    use super::*;

    #[rstest]
    fn test_hash_dynamic_object_no_spec(test_deployment_no_spec: DynamicObject) {
        assert_eq!(hash_dynamic_object(&test_deployment_no_spec), TEST_DEPL_NO_SPEC_HASH);
    }

    #[rstest]
    fn test_hash_dynamic_object(test_deployment: DynamicObject) {
        assert_eq!(hash_dynamic_object(&test_deployment), TEST_DEPL_HASH);
    }
}
