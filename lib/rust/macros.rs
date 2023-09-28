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

pub(crate) use partial_ord_eq_ref;
