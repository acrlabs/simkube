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
use crate::prelude::*;

// GVK is a "newtype" wrapper around the metav1::GroupVersionKind object that lets me provide
// custom serialization methods.  We also add some handy helper/conversion functions.
//
// Specifically for serialization/deserialization, we convert to the format "group/version.kind".
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct GVK(GroupVersionKind);

impl GVK {
    pub fn new(group: &str, version: &str, kind: &str) -> GVK {
        GVK(GroupVersionKind::gvk(group, version, kind))
    }

    pub fn from_dynamic_obj(obj: &DynamicObject) -> anyhow::Result<GVK> {
        match &obj.types {
            Some(t) => Ok(GVK(t.try_into()?)),
            None => bail!("no type data present"),
        }
    }

    pub fn from_owner_ref(rf: &metav1::OwnerReference) -> anyhow::Result<GVK> {
        let parts: Vec<_> = rf.api_version.split('/').collect();

        if parts.len() == 1 {
            Ok(GVK(GroupVersionKind::gvk("", parts[0], &rf.kind)))
        } else if parts.len() == 2 {
            Ok(GVK(GroupVersionKind::gvk(parts[0], parts[1], &rf.kind)))
        } else {
            bail!("invalid format for api_version: {}", rf.api_version);
        }
    }
}

// Impl Deref lets a GVK act like a GroupVersionKind anywhere one of those is expected
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
        let p1: Vec<_> = value.split('/').collect();
        if p1.len() != 2 {
            return Err(E::custom(format!("invalid format for gvk: {value}")));
        }
        let p2: Vec<_> = p1[1].split('.').collect();
        if p2.len() != 2 {
            return Err(E::custom(format!("invalid format for gvk: {value}")));
        }

        let parts = [p1[0], p2[0], p2[1]];
        Ok(GVK(GroupVersionKind::gvk(parts[0], parts[1], parts[2])))
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
