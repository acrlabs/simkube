use serde_json::Value;

use crate::errors::*;

// This module provides some unofficial "extensions" to the [jsonpatch](https://jsonpatch.com)
// format for describing changes to a JSON document.  In particular, it adds the `*` operator as a
// valid token for arrays in a JSON document.  It means: apply this change to all elements of this
// array.  For example, consider the following document:
//
// ```json
// {
//   "foo": {
//     "bar": [
//       {"baz": 1},
//       {"baz": 2},
//       {"baz": 3},
//     ]
//   }
// }
// ```
//
// The pathspec `/foo/bar/*/baz` would reference the `baz` field of all three array entries in the
// `bar` array.  It is an error to use `*` to reference a field that is not an array.  Currently
// the only supported operations are `add` and `remove`.

err_impl! {JsonPatchError,
    #[error("invalid JSON pointer: {0}")]
    InvalidPointer(String),

    #[error("index out of bounds at {0}")]
    OutOfBounds(String),

    #[error("unexpected type at {0}")]
    UnexpectedType(String),
}

pub fn escape(path: &str) -> String {
    let path = path.replace('~', "~0");
    path.replace('/', "~1")
}

pub fn add(path: &str, key: &str, value: &Value, obj: &mut Value, overwrite: bool) -> EmptyResult {
    let parts: Vec<_> = path.split('*').collect();
    for v in patch_ext_helper(&parts, obj).ok_or(JsonPatchError::invalid_pointer(path))? {
        match v {
            Value::Object(map) => {
                if overwrite || !map.contains_key(key) {
                    map.insert(key.into(), value.clone());
                }
            },
            Value::Array(vec) => {
                if key == "-" {
                    vec.push(value.clone());
                } else if let Ok(idx) = key.parse::<usize>() {
                    ensure!(idx <= vec.len(), JsonPatchError::out_of_bounds(&format!("{path}/{key}")));
                    vec.insert(idx, value.clone());
                } else {
                    bail!(JsonPatchError::out_of_bounds(path));
                }
            },
            _ => bail!(JsonPatchError::unexpected_type(path)),
        }
    }

    Ok(())
}

pub fn remove(path: &str, key: &str, obj: &mut Value) -> EmptyResult {
    let parts: Vec<_> = path.split('*').collect();
    for v in patch_ext_helper(&parts, obj).ok_or(JsonPatchError::invalid_pointer(path))? {
        v.as_object_mut().ok_or(JsonPatchError::unexpected_type(path))?.remove(key);
    }

    Ok(())
}

// Given a list of "path parts", i.e., paths split by `*`, recursively walk through all the
// possible "end" values that the path references; return a mutable reference so we can make
// modifications at those points.  We assume that this function is never called with an empty
// `parts` array, which is valid in normal use since "some_string".split('*') will return
// ["some_string"].
fn patch_ext_helper<'a>(parts: &[&str], value: &'a mut Value) -> Option<Vec<&'a mut Value>> {
    if parts.len() == 1 {
        return Some(vec![value.pointer_mut(parts[0])?]);
    }

    let mut res = vec![];

    // If there was an array value, e.g., /foo/bar/*/baz, our path parts will look like
    // /foo/bar/ and /baz; so we need to strip off the trailing '/' in our first part
    let len = parts[0].len();
    let next_array_val = value.pointer_mut(&parts[0][..len - 1])?.as_array_mut()?;
    for v in next_array_val {
        let cons = patch_ext_helper(&parts[1..], v)?;
        res.extend(cons);
    }
    Some(res)
}
