mod merge;
mod package;
mod rewrite;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use merge::{merge_files, MergeError};

#[cfg(target_arch = "wasm32")]
pub use merge::{merge_files_wasm, MergeOptions};
