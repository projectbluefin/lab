use std::fs;
use std::io;
use std::io::Read;
use std::path::Path;

use cap_std::fs::Dir;
use cap_tempfile::cap_std;
use rustix::fd::AsFd;
use rustix::fd::BorrowedFd;
use rustix::fs::OFlags;
use rustix::fs::ResolveFlags;
use rustix::path::Arg;

pub(crate) fn open_beneath_rdonly(start: &BorrowedFd, path: &Path) -> io::Result<fs::File> {
    // We loop forever on EAGAIN right now. The cap-std version loops just 4 times,
    // which seems really arbitrary.
    let r = path.into_with_c_str(|path_c_str| 'start: loop {
        match rustix::fs::openat2(
            start,
            path_c_str,
            OFlags::CLOEXEC | OFlags::RDONLY,
            rustix::fs::Mode::empty(),
            ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS,
        ) {
            Ok(file) => {
                return Ok(file);
            }
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                continue 'start;
            }
            Err(e) => {
                return Err(e);
            }
        }
    })?;
    Ok(r.into())
}

/// Wrapper for a [`cap_std::fs::Dir`] that is defined to use `RESOLVE_IN_ROOT``
/// semantics when opening files and subdirectories. This currently only
/// offers a subset of the methods, primarily reading.
///
/// # When and how to use this
///
/// In general, if your use case possibly involves reading files that may be
/// absolute symlinks, or relative symlinks that may go outside the provided
/// directory, you will need to use this API instead of [`cap_std::fs::Dir`].
///
/// # Performing writes
///
/// If you want to simultaneously perform other operations (such as writing), at the moment
/// it requires explicitly maintaining a duplicate copy of a [`cap_std::fs::Dir`]
/// instance, or using direct [`rustix::fs`] APIs.
#[derive(Debug)]
pub struct RootDir(Dir);

impl RootDir {
    /// Create a new instance from an existing [`cap_std::fs::Dir`] instance.
    pub fn new(src: &Dir, path: impl AsRef<Path>) -> io::Result<Self> {
        src.open_dir(path).map(Self)
    }

    /// Create a new instance from an ambient path.
    pub fn open_ambient_root(
        path: impl AsRef<Path>,
        authority: cap_std::AmbientAuthority,
    ) -> io::Result<Self> {
        Dir::open_ambient_dir(path, authority).map(Self)
    }

    /// Open a file in this root, read-only.
    pub fn open(&self, path: impl AsRef<Path>) -> io::Result<fs::File> {
        let path = path.as_ref();
        open_beneath_rdonly(&self.0.as_fd(), path)
    }

    /// Open a file read-only, but return `Ok(None)` if it does not exist.
    pub fn open_optional(&self, path: impl AsRef<Path>) -> io::Result<Option<fs::File>> {
        crate::dirext::map_optional(self.open(path))
    }

    /// Read the contents of a file into a vector.
    pub fn read(&self, path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        let mut f = self.open(path.as_ref())?;
        let mut r = Vec::new();
        f.read_to_end(&mut r)?;
        Ok(r)
    }

    /// Read the contents of a file as a string.
    pub fn read_to_string(&self, path: impl AsRef<Path>) -> io::Result<String> {
        let mut f = self.open(path.as_ref())?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        Ok(s)
    }

    /// Return the directory entries.
    pub fn entries(&self) -> io::Result<cap_std::fs::ReadDir> {
        self.0.entries()
    }

    /// Return the directory entries of the target subdirectory.
    pub fn read_dir(&self, path: impl AsRef<Path>) -> io::Result<cap_std::fs::ReadDir> {
        self.0.read_dir(path.as_ref())
    }

    /// Create a [`cap_std::fs::Dir`] pointing to the same directory as `self`.
    /// This view will *not* use `RESOLVE_IN_ROOT`.
    pub fn reopen_cap_std(&self) -> io::Result<Dir> {
        Dir::reopen_dir(&self.0.as_fd())
    }
}

impl From<Dir> for RootDir {
    fn from(dir: Dir) -> Self {
        Self(dir)
    }
}
