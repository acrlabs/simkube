mod hash;
pub mod patch_ext;

pub use hash::{
    hash,
    hash_option,
};
pub use patch_ext::escape;

#[cfg(test)]
mod tests;
