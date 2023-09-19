use serde_json::Value;

use crate::errors::*;

err_impl! {JsonPatchError,
    #[error("invalid JSON pointer: {0}")]
    InvalidPointer(String),

    #[error("index out of bounds at {0}")]
    OutOfBounds(String),

    #[error("unexpected type at {0}")]
    UnexpectedType(String),
}

pub fn add(path: &str, key: &str, value: &Value, obj: &mut Value, overwrite: bool) -> anyhow::Result<()> {
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
                    ensure!(idx <= vec.len(), JsonPatchError::out_of_bounds(&format!("{}/{}", path, key)));
                    vec.insert(idx, value.clone());
                }
            },
            _ => bail!(JsonPatchError::unexpected_type(path)),
        }
    }

    Ok(())
}

pub fn remove(path: &str, key: &str, obj: &mut Value) -> anyhow::Result<()> {
    let parts: Vec<_> = path.split('*').collect();
    for v in patch_ext_helper(&parts, obj).ok_or(JsonPatchError::invalid_pointer(path))? {
        v.as_object_mut().ok_or(JsonPatchError::unexpected_type(path))?.remove(key);
    }

    Ok(())
}

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
