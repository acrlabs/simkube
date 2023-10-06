pub(crate) use anyhow::{
    anyhow,
    bail,
    ensure,
};
pub(crate) use paste::paste;
pub(crate) use thiserror::Error;

pub type EmptyResult = anyhow::Result<()>;

// This macro creates an enum which derives from thiserror::Error, and also
// creates constructor functions in snake case for each of the enum variants
macro_rules! err_impl {
    (@hidden $errtype:ident, $item:ident, String) => {
        paste! {
            pub(crate) fn [<$item:snake>](in_: &str) -> anyhow::Error {
                anyhow!{$errtype::$item(in_.into())}
            }
        }
    };

    (@hidden $errtype:ident, $item:ident, $($dtype:tt)::+) => {
        paste! {
            pub(crate) fn [<$item:snake>](in_: &$($dtype)::+) -> anyhow::Error {
                anyhow!{$errtype::$item(in_.clone())}
            }
        }
    };

    ($errtype:ident,
        $(#[$errinfo:meta] $item:ident($($dtype:tt)::+),)+
    ) => {
        #[derive(Debug, Error)]
        pub(crate) enum $errtype {
            $(#[$errinfo] $item($($dtype)::+)),+
        }

        impl $errtype {
            $(err_impl! {@hidden $errtype, $item, $($dtype)::+})+
        }
    };
}

pub(crate) use err_impl;
