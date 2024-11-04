mod hash;
pub mod patch_ext;

pub use hash::{
    hash,
    hash_option,
    order_json,
    ordered_eq,
    ordered_hash,
};
pub use patch_ext::escape;

#[cfg(test)]
mod tests;
