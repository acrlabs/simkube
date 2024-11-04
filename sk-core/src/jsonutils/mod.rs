mod hash;
pub mod patch_ext;

pub use hash::{
    hash,
    hash_option,
    ordered_hash,
    order_json,
    ordered_eq,
};
pub use patch_ext::escape;

#[cfg(test)]
mod tests;
