// See https://doc.rust-lang.org/rustc/lints/listing/allowed-by-default.html
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]
#![deny(missing_debug_implementations)]
#![forbid(unused_must_use)]
#![deny(unsafe_code)]

use std::io;

// Re-export our dependencies
pub use cap_primitives;
#[cfg(feature = "fs_utf8")]
pub use cap_std::fs_utf8::camino;
pub use cap_tempfile;
pub use cap_tempfile::cap_std;

#[cfg(not(windows))]
pub mod cmdext;
pub mod dirext;

#[cfg(any(target_os = "android", target_os = "linux"))]
mod rootdir;
#[cfg(any(target_os = "android", target_os = "linux"))]
pub use rootdir::*;
#[cfg(not(windows))]
mod xattrs;
#[cfg(not(windows))]
pub use xattrs::XattrList;

#[cold]
pub(crate) fn escape_attempt() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "a path led outside of the filesystem",
    )
}

/// Prelude, intended for glob import.
pub mod prelude {
    #[cfg(not(windows))]
    pub use super::cmdext::CapStdExtCommandExt;
    pub use super::dirext::CapStdExtDirExt;
    #[cfg(feature = "fs_utf8")]
    pub use super::dirext::CapStdExtDirExtUtf8;
}
