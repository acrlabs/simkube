pub(crate) use anyhow::{
    anyhow,
    bail,
    ensure,
};
pub(crate) use paste::paste;
pub(crate) use thiserror::Error;

macro_rules! err_impl_helper {
    ($errtype:ident, $item:ident, String) => {
        paste! {
            pub(crate) fn [<$item:snake>](in_: &str) -> anyhow::Error {
                anyhow!{$errtype::$item(in_.into())}
            }
        }
    };

    ($errtype:ident, $item:ident, $($dtype:tt)::+) => {
        paste! {
            pub(crate) fn [<$item:snake>](in_: &$($dtype)::+) -> anyhow::Error {
                anyhow!{$errtype::$item(in_.clone())}
            }
        }
    };
}

macro_rules! err_impl {
    ($errtype:ident,
        $(#[$errinfo:meta] $item:ident($($dtype:tt)::+),)+
    ) => (
        #[derive(Debug, Error)]
        pub(crate) enum $errtype {
            $(#[$errinfo] $item($($dtype)::+)),+
        }

        impl $errtype {
            $(err_impl_helper! {$errtype, $item, $($dtype)::+})+
        }
    )
}

pub(crate) use {
    err_impl,
    err_impl_helper,
};
