use std::fmt;

use kube::api::{
    DynamicObject,
    GroupVersionKind,
};
use serde::{
    de,
    Deserialize,
    Deserializer,
    Serialize,
    Serializer,
};

use crate::errors::*;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct GVKKey {
    pub gvk: GroupVersionKind,
}

impl GVKKey {
    pub fn from_dynamic_obj(obj: &DynamicObject) -> anyhow::Result<Self> {
        match &obj.types {
            Some(t) => Ok(GVKKey { gvk: t.try_into()? }),
            None => bail!("no type data present"),
        }
    }
}

impl Serialize for GVKKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let skey = format!("{}/{}.{}", self.gvk.group, self.gvk.version, self.gvk.kind);
        serializer.serialize_str(&skey)
    }
}

struct ObjectKeyVisitor;

impl<'de> de::Visitor<'de> for ObjectKeyVisitor {
    type Value = GVKKey;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a GroupVersionKind in the format group/version.kind")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let parts: Vec<_> = value.split(|c| c == '/' || c == '.').collect();
        if parts.len() != 3 {
            return Err(E::custom(format!("invalid format for gvk: {}", value)));
        }
        Ok(GVKKey {
            gvk: GroupVersionKind {
                group: parts[0].into(),
                version: parts[1].into(),
                kind: parts[2].into(),
            },
        })
    }
}

impl<'de> Deserialize<'de> for GVKKey {
    fn deserialize<D>(deserializer: D) -> Result<GVKKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ObjectKeyVisitor)
    }
}
