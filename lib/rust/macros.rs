pub use std::collections::BTreeMap;

// Generate labels for a k8s object, using klabel!("label1" = "value1", "label2" = "value2") syntax
#[macro_export]
macro_rules! klabel {
    ($($key:tt=$val:literal),+$(,)?) => {
        Some(BTreeMap::from([$(($key.to_string(), $val.to_string())),+]))
    };
}

// Implement PartialEq and PartialOrd for comparisons between an object and a reference to that
// object
macro_rules! partial_ord_eq_ref {
    ($type:ty) => {
        impl<'a> PartialEq<&'a $type> for $type {
            fn eq(&self, other: &&'a $type) -> bool {
                self == *other
            }
        }

        impl<'a> PartialEq<$type> for &'a $type {
            fn eq(&self, other: &$type) -> bool {
                *self == other
            }
        }

        impl<'a> PartialOrd<&'a $type> for $type {
            fn partial_cmp(&self, other: &&'a $type) -> Option<std::cmp::Ordering> {
                self.partial_cmp(*other)
            }
        }

        impl<'a> PartialOrd<$type> for &'a $type {
            fn partial_cmp(&self, other: &$type) -> Option<std::cmp::Ordering> {
                (*self).partial_cmp(other)
            }
        }
    };
}

pub use klabel;
pub(crate) use partial_ord_eq_ref;
