use anyhow::anyhow;
use serde_json::Value;

pub fn patch_ext_remove(path: &str, key: &str, value: &mut Value) -> anyhow::Result<()> {
    let parts: Vec<&str> = path.split('*').collect();
    for v in patch_ext_helper(&parts, value).ok_or(anyhow!("invalid JSON pointer {}", path))? {
        v.as_object_mut()
            .ok_or(anyhow!("JSON field not an object at {}", path))?
            .remove(key);
    }

    Ok(())
}

pub(super) fn patch_ext_helper<'a>(parts: &[&str], value: &'a mut Value) -> Option<Vec<&'a mut Value>> {
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
