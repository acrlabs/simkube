use rstest::*;
use serde_json::{
    json,
    Value,
};

use super::*;

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
#[case::overwrite(true)]
#[case::no_overwrite(false)]
fn test_patch_ext_add(mut data: Value, #[case] overwrite: bool) {
    let path = "/foo/*/baz";
    let res = patch_ext::add(path, "buzz", &json!(42), &mut data, overwrite);
    assert!(res.is_ok());
    assert_eq!(
        data,
        json!({
            "foo": [
                {"baz": {"buzz": if overwrite { 42 } else { 0 } }},
                {"baz": {"quzz": 1, "buzz": 42}},
                {"baz": {"fixx": 2, "buzz": 42}},
            ],
        })
    );
}

#[rstest]
fn test_patch_ext_remove(mut data: Value) {
    let path = "/foo/*/baz";
    let res = patch_ext::remove(path, "quzz", &mut data);
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
