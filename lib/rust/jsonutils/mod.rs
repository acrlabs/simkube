mod hash;
pub mod patch_ext;

pub use hash::{
    hash,
    hash_option,
};

#[cfg(test)]
mod tests;
