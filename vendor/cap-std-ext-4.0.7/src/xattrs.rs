use std::io::Result;
use std::{
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use cap_tempfile::cap_std::fs::Dir;
use rustix::buffer::spare_capacity;

use crate::dirext::validate_relpath_no_uplinks;

/// Convert the directory and path pair into a /proc/self/fd path
/// which is useful for xattr functions in particular to be able
/// to operate on symlinks.
///
/// Absolute paths as well as paths with uplinks (`..`) are an error.
fn proc_self_path(d: &Dir, path: &Path) -> Result<PathBuf> {
    use rustix::path::DecInt;
    use std::os::fd::{AsFd, AsRawFd};

    // Require relative paths here.
    let path = validate_relpath_no_uplinks(path)?;

    let mut pathbuf = PathBuf::from("/proc/self/fd");
    pathbuf.push(DecInt::new(d.as_fd().as_raw_fd()));
    pathbuf.push(path);
    Ok(pathbuf)
}

pub(crate) fn impl_getxattr(d: &Dir, path: &Path, key: &OsStr) -> Result<Option<Vec<u8>>> {
    let path = &proc_self_path(d, path)?;

    // In my experience few extended attributes exceed this
    let mut buf = Vec::with_capacity(256);

    loop {
        match rustix::fs::lgetxattr(path, key, spare_capacity(&mut buf)) {
            Ok(_) => {
                return Ok(Some(buf));
            }
            Err(rustix::io::Errno::NODATA) => {
                return Ok(None);
            }
            Err(rustix::io::Errno::RANGE) => {
                buf.reserve(buf.capacity().saturating_mul(2));
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }
}

/// A list of extended attribute value names
#[derive(Debug)]
#[cfg(any(target_os = "android", target_os = "linux"))]
pub struct XattrList {
    /// Contents of the return value from the llistxattr system call;
    /// effectively Vec<OsStr> with an empty value as terminator.
    /// Not public - we expect callers to invoke the `iter()` method.
    /// When Rust has lending iterators then we could implement IntoIterator
    /// in a way that borrows from this value.
    buf: Vec<u8>,
}

#[cfg(any(target_os = "android", target_os = "linux"))]
impl XattrList {
    /// Return an iterator over the elements of this extended attribute list.
    pub fn iter(&self) -> impl Iterator<Item = &'_ std::ffi::OsStr> {
        self.buf.split(|&v| v == 0).filter_map(|v| {
            // Note this case should only happen once at the end
            if v.is_empty() {
                None
            } else {
                Some(OsStr::from_bytes(v))
            }
        })
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub(crate) fn impl_listxattrs(d: &Dir, path: &Path) -> Result<XattrList> {
    let path = &proc_self_path(d, path)?;

    let mut buf = Vec::with_capacity(512);

    loop {
        match rustix::fs::llistxattr(path, spare_capacity(&mut buf)) {
            Ok(_) => {
                return Ok(XattrList { buf });
            }
            Err(rustix::io::Errno::RANGE) => {
                buf.reserve(buf.capacity().saturating_mul(2));
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub(crate) fn impl_setxattr(d: &Dir, path: &Path, key: &OsStr, value: &[u8]) -> Result<()> {
    let path = &proc_self_path(d, path)?;
    rustix::fs::lsetxattr(path, key, value, rustix::fs::XattrFlags::empty())?;
    Ok(())
}
