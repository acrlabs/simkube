use std::fmt;
use std::ops::Deref;

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
pub struct GVK(GroupVersionKind);

impl GVK {
    pub fn from_dynamic_obj(obj: &DynamicObject) -> anyhow::Result<Self> {
        match &obj.types {
            Some(t) => Ok(GVK(t.try_into()?)),
            None => bail!("no type data present"),
        }
    }
}

impl Deref for GVK {
    type Target = GroupVersionKind;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Serialize for GVK {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let skey = format!("{}/{}.{}", self.0.group, self.0.version, self.0.kind);
        serializer.serialize_str(&skey)
    }
}

struct GVKVisitor;

impl<'de> de::Visitor<'de> for GVKVisitor {
    type Value = GVK;

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
        Ok(GVK(GroupVersionKind {
            group: parts[0].into(),
            version: parts[1].into(),
            kind: parts[2].into(),
        }))
    }
}

impl<'de> Deserialize<'de> for GVK {
    fn deserialize<D>(deserializer: D) -> Result<GVK, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(GVKVisitor)
    }
}
