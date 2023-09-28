#[macro_export]
macro_rules! klabel {
    ($($key:tt=$val:literal),+$(,)?) => {
        Some(BTreeMap::from([$(($key.to_string(), $val.to_string())),+]))
    };
}

pub use std::collections::BTreeMap;

pub use klabel;
