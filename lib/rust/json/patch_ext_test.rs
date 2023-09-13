use rstest::*;
use serde_json::{
    json,
    Value,
};

use super::patch_ext::*;

#[fixture]
fn data() -> Value {
    json!({
        "foo": [
            {"baz": {"buzz": 0}},
            {"baz": {"quzz": 1}},
            {"baz": {"fixx": 2}},
        ],
    })
}

#[rstest]
fn test_patch_ext_remove(mut data: Value) {
    let path = "/foo/*/baz";
    let res = patch_ext_remove(path, "quzz", &mut data);
    assert!(res.is_ok());
    assert_eq!(
        data,
        json!({
            "foo": [
                {"baz": {"buzz": 0}},
                {"baz": {}},
                {"baz": {"fixx": 2}},
            ],
        })
    );
}
