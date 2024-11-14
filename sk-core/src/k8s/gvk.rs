use std::borrow::Cow;
use std::fmt;
use std::ops::Deref;

use kube::api::{
    DynamicObject,
    GroupVersionKind,
    TypeMeta,
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
// (unless the group is "core", and then we serialize to "version.kind", but can deserialize from
// either "version.kind" or "/version.kind" for backwards compatibility)
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

    pub fn into_type_meta(&self) -> TypeMeta {
        TypeMeta {
            api_version: self.0.api_version(),
            kind: self.0.kind.clone(),
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

impl fmt::Display for GVK {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut group = Cow::from(&self.0.group);
        if !group.is_empty() {
            group.to_mut().push('/');
        }

        write!(f, "{group}{}.{}", self.0.version, self.0.kind)
    }
}

impl Serialize for GVK {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // reuse the display impl for serializing
        serializer.serialize_str(&format!("{self}"))
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
        let (group, rest) = match p1.len() {
            2 => (p1[0], p1[1]),
            1 => ("", p1[0]),
            _ => return Err(E::custom(format!("invalid format for gvk: {value}"))),
        };
        let p2: Vec<_> = rest.split('.').collect();
        let (version, kind) = match p2.len() {
            2 => (p2[0], p2[1]),
            _ => return Err(E::custom(format!("invalid format for gvk: {value}"))),
        };

        Ok(GVK(GroupVersionKind::gvk(group, version, kind)))
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

#[cfg(test)]
mod test {
    use assertables::*;
    use rstest::*;
    use serde::de::value::{
        Error as SerdeError,
        StrDeserializer,
    };
    use serde::de::IntoDeserializer;

    use super::*;

    #[rstest]
    fn test_serialize() {
        // I had to think about this for a minute, but strings in JSON have to include quotes,
        // which is why they're escaped out here.
        assert_eq!(serde_json::to_string(&GVK::new("foo", "v1", "bar")).unwrap(), "\"foo/v1.bar\"");
        assert_eq!(serde_json::to_string(&GVK::new("", "v1", "bar")).unwrap(), "\"v1.bar\"");
    }

    #[rstest]
    fn test_deserialize() {
        let d1: StrDeserializer<SerdeError> = "foo/v1.bar".into_deserializer();
        assert_eq!(GVK::deserialize(d1).unwrap(), GVK::new("foo", "v1", "bar"));

        let d2: StrDeserializer<SerdeError> = "/v1.bar".into_deserializer();
        assert_eq!(GVK::deserialize(d2).unwrap(), GVK::new("", "v1", "bar"));

        let d3: StrDeserializer<SerdeError> = "v1.bar".into_deserializer();
        assert_eq!(GVK::deserialize(d3).unwrap(), GVK::new("", "v1", "bar"));

        let d4: StrDeserializer<SerdeError> = "asdf".into_deserializer();
        assert_err!(GVK::deserialize(d4));

        let d5: StrDeserializer<SerdeError> = "foo/asdf/asdf".into_deserializer();
        assert_err!(GVK::deserialize(d5));
    }
}
