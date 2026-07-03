//! Extensions for [`cap_std::fs::Dir`].  Key features here include:
//!
//! - "optional" variants that return `Result<Option<T>>` for nonexistent paths, when
//!   it is a normal case that paths may not exist.
//! - A helper to update timestamps
//! - "atomic write" APIs that create a new file, then rename over the existing one
//!   to avoid half-written updates to files.
//!
//! [`cap_std::fs::Dir`]: https://docs.rs/cap-std/latest/cap_std/fs/struct.Dir.html

use cap_primitives::fs::FileType;
use cap_std::fs::{Dir, File, Metadata};
use cap_tempfile::cap_std;
use cap_tempfile::cap_std::fs::DirEntry;
#[cfg(any(target_os = "android", target_os = "linux"))]
use rustix::path::Arg;
use std::cmp::Ordering;
use std::ffi::OsStr;
use std::io::{self, Write};
use std::io::{Read, Result};
use std::ops::Deref;
#[cfg(unix)]
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

#[cfg(feature = "fs_utf8")]
use cap_std::fs_utf8;
#[cfg(feature = "fs_utf8")]
use fs_utf8::camino::Utf8Path;

/// A directory entry encountered when using the `walk` function.
#[non_exhaustive]
#[derive(Debug)]
pub struct WalkComponent<'p, 'd> {
    /// The relative path to the entry. This will
    /// include the filename of [`entry`]. Note
    /// that this is purely informative; the filesystem
    /// traversal provides this path, but does not itself
    /// use it.
    ///
    /// The [`WalkConfiguration::path_base`] function configures
    /// the base for this path.
    pub path: &'p Path,
    /// The parent directory.
    pub dir: &'d Dir,
    /// The filename of the directory entry.
    /// Note that this will also be present in [`path`].
    pub filename: &'p OsStr,
    /// The file type.
    pub file_type: FileType,
    /// The directory entry.
    pub entry: &'p DirEntry,
}

/// Options controlling recursive traversal with `walk`.
#[non_exhaustive]
#[derive(Default)]
pub struct WalkConfiguration<'p> {
    /// Do not cross devices.
    noxdev: bool,

    path_base: Option<&'p Path>,

    // It's not *that* complex of a type, come on clippy...
    #[allow(clippy::type_complexity)]
    sorter: Option<Box<dyn Fn(&DirEntry, &DirEntry) -> Ordering + 'static>>,
}

/// The return value of a [`walk`] callback.
pub type WalkResult<E> = std::result::Result<std::ops::ControlFlow<()>, E>;

impl std::fmt::Debug for WalkConfiguration<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalkConfiguration")
            .field("noxdev", &self.noxdev)
            .field("sorter", &self.sorter.as_ref().map(|_| true))
            .finish()
    }
}

impl<'p> WalkConfiguration<'p> {
    /// Enable configuration to not traverse mount points
    pub fn noxdev(mut self) -> Self {
        self.noxdev = true;
        self
    }

    /// Set a function for sorting directory entries.
    pub fn sort_by<F>(mut self, cmp: F) -> Self
    where
        F: Fn(&DirEntry, &DirEntry) -> Ordering + 'static,
    {
        self.sorter = Some(Box::new(cmp));
        self
    }

    /// Sort directory entries by file name.
    pub fn sort_by_file_name(self) -> Self {
        self.sort_by(|a, b| a.file_name().cmp(&b.file_name()))
    }

    /// Change the inital state for the path. By default the
    /// computed path is relative. This has no effect
    /// on the filesystem traversal - it solely affects
    /// the value of [`WalkComponent::path`].
    pub fn path_base(mut self, base: &'p Path) -> Self {
        self.path_base = Some(base);
        self
    }
}

/// Extension trait for [`cap_std::fs::Dir`].
///
/// [`cap_std::fs::Dir`]: https://docs.rs/cap-std/latest/cap_std/fs/struct.Dir.html
pub trait CapStdExtDirExt {
    /// Open a file read-only, but return `Ok(None)` if it does not exist.
    fn open_optional(&self, path: impl AsRef<Path>) -> Result<Option<File>>;

    /// Open a directory, but return `Ok(None)` if it does not exist.
    fn open_dir_optional(&self, path: impl AsRef<Path>) -> Result<Option<Dir>>;

    /// Create a special variant of [`cap_std::fs::Dir`] which uses `RESOLVE_IN_ROOT`
    /// to support absolute symlinks.
    #[cfg(any(target_os = "android", target_os = "linux"))]
    fn open_dir_rooted_ext(&self, path: impl AsRef<Path>) -> Result<crate::RootDir>;

    /// Open the target directory, but return Ok(None) if this would cross a mount point.
    #[cfg(any(target_os = "android", target_os = "linux"))]
    fn open_dir_noxdev(&self, path: impl AsRef<Path>) -> Result<Option<Dir>>;

    /// Create the target directory, but do nothing if a directory already exists at that path.
    /// The return value will be `true` if the directory was created.  An error will be
    /// returned if the path is a non-directory.  Symbolic links will be followed.
    fn ensure_dir_with(
        &self,
        p: impl AsRef<Path>,
        builder: &cap_std::fs::DirBuilder,
    ) -> Result<bool>;

    /// Gather metadata, but return `Ok(None)` if it does not exist.
    fn metadata_optional(&self, path: impl AsRef<Path>) -> Result<Option<Metadata>>;

    /// Gather metadata (but do not follow symlinks), but return `Ok(None)` if it does not exist.
    fn symlink_metadata_optional(&self, path: impl AsRef<Path>) -> Result<Option<Metadata>>;

    /// Remove (delete) a file, but return `Ok(false)` if the file does not exist.
    fn remove_file_optional(&self, path: impl AsRef<Path>) -> Result<bool>;

    /// Remove a file or directory but return `Ok(false)` if the file does not exist.
    /// Symbolic links are not followed.
    fn remove_all_optional(&self, path: impl AsRef<Path>) -> Result<bool>;

    /// Read the complete contents of a file, but return `Ok(None)` if the file does not exist.
    fn read_optional(&self, path: impl AsRef<Path>) -> Result<Option<Vec<u8>>>;

    /// Read the complete contents of a file as a string, but return `Ok(None)` if the file does not exist.
    fn read_to_string_optional(&self, path: impl AsRef<Path>) -> Result<Option<String>>;

    /// Set the access and modification times to the current time.  Symbolic links are not followed.
    #[cfg(unix)]
    fn update_timestamps(&self, path: impl AsRef<Path>) -> Result<()>;

    /// Atomically write a file by calling the provided closure.
    ///
    /// This uses [`cap_tempfile::TempFile`], which is wrapped in a [`std::io::BufWriter`]
    /// and passed to the closure.
    ///
    /// # Atomicity
    ///
    /// The function takes care of:
    /// - flushing the BufWriter
    /// - calling sync_all() on the TempFile
    /// - calling sync_all() on the parent directory (after the rename)
    ///
    /// # Existing files and metadata
    ///
    /// If the target path already exists and is a regular file (not a symbolic link or directory),
    /// then its access permissions (Unix mode) will be preserved.  However, other metadata
    /// such as extended attributes will *not* be preserved automatically. To do this will
    /// require a higher level wrapper which queries the existing file and gathers such metadata
    /// before replacement.
    ///
    /// # Example, including setting permissions
    ///
    /// The closure may also perform other file operations beyond writing, such as changing
    /// file permissions:
    ///
    /// ```rust
    /// # use std::io;
    /// # use std::io::Write;
    /// # use cap_tempfile::cap_std;
    /// # fn main() -> io::Result<()> {
    /// # let somedir = cap_tempfile::tempdir(cap_std::ambient_authority())?;
    /// use cap_std_ext::prelude::*;
    /// let contents = b"hello world\n";
    /// somedir.atomic_replace_with("somefilename", |f| -> io::Result<_> {
    ///     f.write_all(contents)?;
    ///     f.flush()?;
    ///     use cap_std::fs::PermissionsExt;
    ///     let perms = cap_std::fs::Permissions::from_mode(0o600);
    ///     f.get_mut().as_file_mut().set_permissions(perms)?;
    ///     Ok(())
    /// })
    /// # }
    /// ```
    ///
    /// Any existing file will be replaced.
    #[cfg(not(windows))]
    fn atomic_replace_with<F, T, E>(
        &self,
        destname: impl AsRef<Path>,
        f: F,
    ) -> std::result::Result<T, E>
    where
        F: FnOnce(&mut std::io::BufWriter<cap_tempfile::TempFile>) -> std::result::Result<T, E>,
        E: From<std::io::Error>;

    /// Atomically write the provided contents to a file.
    #[cfg(not(windows))]
    fn atomic_write(&self, destname: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()>;

    /// Atomically write the provided contents to a file, using specified permissions.
    #[cfg(not(windows))]
    fn atomic_write_with_perms(
        &self,
        destname: impl AsRef<Path>,
        contents: impl AsRef<[u8]>,
        perms: cap_std::fs::Permissions,
    ) -> Result<()>;

    /// By default, cap-std `Dir` instances are opened using `O_PATH`.
    /// There are some operations such as `fsync` and `fsetxattr` that
    /// cannot be performed on `O_PATH` file descriptors. Use this
    /// function to create a non-`O_PATH` copy of the directory
    /// file descriptor.
    #[cfg(unix)]
    fn reopen_as_ownedfd(&self) -> Result<OwnedFd>;

    #[cfg(any(target_os = "android", target_os = "linux"))]
    /// Returns `Some(true)` if the target is known to be a mountpoint, or
    /// `Some(false)` if the target is definitively known not to be a mountpoint.
    ///
    /// In some scenarios (such as an older kernel) this currently may not be possible
    /// to determine, and `None` will be returned in those cases.
    fn is_mountpoint(&self, path: impl AsRef<Path>) -> Result<Option<bool>>;

    #[cfg(not(windows))]
    /// Get the value of an extended attribute. If the attribute is not present,
    /// this function will return `Ok(None)`.
    fn getxattr(&self, path: impl AsRef<Path>, key: impl AsRef<OsStr>) -> Result<Option<Vec<u8>>>;

    #[cfg(not(windows))]
    /// List all extended attribute keys for this path.
    fn listxattrs(&self, path: impl AsRef<Path>) -> Result<crate::XattrList>;

    #[cfg(not(windows))]
    /// Set the value of an extended attribute.
    fn setxattr(
        &self,
        path: impl AsRef<Path>,
        key: impl AsRef<OsStr>,
        value: impl AsRef<[u8]>,
    ) -> Result<()>;

    /// Recursively walk a directory. If the function returns [`std::ops::ControlFlow::Break`]
    /// while inspecting a directory, traversal of that directory is skipped. If
    /// [`std::ops::ControlFlow::Break`] is returned when inspecting a non-directory,
    /// then all further entries in the directory are skipped.
    fn walk<C, E>(&self, config: &WalkConfiguration, callback: C) -> std::result::Result<(), E>
    where
        C: FnMut(&WalkComponent) -> WalkResult<E>,
        E: From<std::io::Error>;
}

#[cfg(feature = "fs_utf8")]
/// Extension trait for [`cap_std::fs_utf8::Dir`].
///
/// [`cap_std::fs_utf8::Dir`]: https://docs.rs/cap-std/latest/cap_std/fs_utf8/struct.Dir.html
pub trait CapStdExtDirExtUtf8 {
    /// Open a file read-only, but return `Ok(None)` if it does not exist.
    fn open_optional(&self, path: impl AsRef<Utf8Path>) -> Result<Option<fs_utf8::File>>;

    /// Open a directory, but return `Ok(None)` if it does not exist.
    fn open_dir_optional(&self, path: impl AsRef<Utf8Path>) -> Result<Option<fs_utf8::Dir>>;

    /// Create the target directory, but do nothing if a directory already exists at that path.
    /// The return value will be `true` if the directory was created.  An error will be
    /// returned if the path is a non-directory.  Symbolic links will be followed.
    fn ensure_dir_with(
        &self,
        p: impl AsRef<Utf8Path>,
        builder: &cap_std::fs::DirBuilder,
    ) -> Result<bool>;

    /// Gather metadata, but return `Ok(None)` if it does not exist.
    fn metadata_optional(&self, path: impl AsRef<Utf8Path>) -> Result<Option<Metadata>>;

    /// Gather metadata (but do not follow symlinks), but return `Ok(None)` if it does not exist.
    fn symlink_metadata_optional(&self, path: impl AsRef<Utf8Path>) -> Result<Option<Metadata>>;

    /// Remove (delete) a file, but return `Ok(false)` if the file does not exist.
    fn remove_file_optional(&self, path: impl AsRef<Utf8Path>) -> Result<bool>;

    /// Remove a file or directory but return `Ok(false)` if the file does not exist.
    /// Symbolic links are not followed.
    fn remove_all_optional(&self, path: impl AsRef<Utf8Path>) -> Result<bool>;

    /// Set the access and modification times to the current time.  Symbolic links are not followed.
    #[cfg(unix)]
    fn update_timestamps(&self, path: impl AsRef<Utf8Path>) -> Result<()>;

    /// Atomically write a file by calling the provided closure.
    ///
    /// This uses [`cap_tempfile::TempFile`], which is wrapped in a [`std::io::BufWriter`]
    /// and passed to the closure.
    ///
    /// # Atomicity
    ///
    /// The function takes care of:
    /// - flushing the BufWriter
    /// - calling sync_all() on the TempFile
    /// - calling sync_all() on the parent directory (after the rename)
    ///
    /// # Existing files and metadata
    ///
    /// If the target path already exists and is a regular file (not a symbolic link or directory),
    /// then its access permissions (Unix mode) will be preserved.  However, other metadata
    /// such as extended attributes will *not* be preserved automatically. To do this will
    /// require a higher level wrapper which queries the existing file and gathers such metadata
    /// before replacement.
    ///
    /// # Example, including setting permissions
    ///
    /// The closure may also perform other file operations beyond writing, such as changing
    /// file permissions:
    ///
    /// ```rust
    /// # use std::io;
    /// # use std::io::Write;
    /// # use cap_tempfile::cap_std;
    /// # fn main() -> io::Result<()> {
    /// # let somedir = cap_tempfile::tempdir(cap_std::ambient_authority())?;
    /// # let somedir = cap_std::fs_utf8::Dir::from_cap_std((&*somedir).try_clone()?);
    /// use cap_std_ext::prelude::*;
    /// let contents = b"hello world\n";
    /// somedir.atomic_replace_with("somefilename", |f| -> io::Result<_> {
    ///     f.write_all(contents)?;
    ///     f.flush()?;
    ///     use cap_std::fs::PermissionsExt;
    ///     let perms = cap_std::fs::Permissions::from_mode(0o600);
    ///     f.get_mut().as_file_mut().set_permissions(perms)?;
    ///     Ok(())
    /// })
    /// # }
    /// ```
    ///
    /// Any existing file will be replaced.
    fn atomic_replace_with<F, T, E>(
        &self,
        destname: impl AsRef<Utf8Path>,
        f: F,
    ) -> std::result::Result<T, E>
    where
        F: FnOnce(&mut std::io::BufWriter<cap_tempfile::TempFile>) -> std::result::Result<T, E>,
        E: From<std::io::Error>;

    /// Atomically write the provided contents to a file.
    fn atomic_write(
        &self,
        destname: impl AsRef<Utf8Path>,
        contents: impl AsRef<[u8]>,
    ) -> Result<()>;

    /// Atomically write the provided contents to a file, using specified permissions.
    fn atomic_write_with_perms(
        &self,
        destname: impl AsRef<Utf8Path>,
        contents: impl AsRef<[u8]>,
        perms: cap_std::fs::Permissions,
    ) -> Result<()>;

    /// Read all filenames in this directory, sorted
    fn filenames_sorted(&self) -> Result<Vec<String>> {
        self.filenames_sorted_by(|a, b| a.cmp(b))
    }

    /// Read all filenames in this directory, sorted by the provided comparison function.
    fn filenames_sorted_by<C>(&self, compare: C) -> Result<Vec<String>>
    where
        C: FnMut(&str, &str) -> std::cmp::Ordering,
    {
        self.filenames_filtered_sorted_by(|_, _| true, compare)
    }

    /// Read all filenames in this directory, applying a filter and sorting the result.
    fn filenames_filtered_sorted<F>(&self, f: F) -> Result<Vec<String>>
    where
        F: FnMut(&fs_utf8::DirEntry, &str) -> bool,
    {
        self.filenames_filtered_sorted_by(f, |a, b| a.cmp(b))
    }

    /// Read all filenames in this directory, applying a filter and sorting the result with a custom comparison function.
    fn filenames_filtered_sorted_by<F, C>(&self, f: F, compare: C) -> Result<Vec<String>>
    where
        F: FnMut(&fs_utf8::DirEntry, &str) -> bool,
        C: FnMut(&str, &str) -> std::cmp::Ordering;
}

pub(crate) fn map_optional<R>(r: Result<R>) -> Result<Option<R>> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}

enum DirOwnedOrBorrowed<'d> {
    Owned(Dir),
    Borrowed(&'d Dir),
}

impl<'d> Deref for DirOwnedOrBorrowed<'d> {
    type Target = Dir;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(d) => d,
            Self::Borrowed(d) => d,
        }
    }
}

/// Given a directory reference and a path, if the path includes a subdirectory (e.g. on Unix has a `/`)
/// then open up the target directory, and return the file name.
///
/// Otherwise, reborrow the directory and return the file name.
///
/// It is an error if the target path does not name a file.
fn subdir_of<'d, 'p>(d: &'d Dir, p: &'p Path) -> io::Result<(DirOwnedOrBorrowed<'d>, &'p OsStr)> {
    let name = p
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Not a file name"))?;
    let r = if let Some(subdir) = p
        .parent()
        .filter(|v| !v.as_os_str().is_empty())
        .map(|p| d.open_dir(p))
    {
        DirOwnedOrBorrowed::Owned(subdir?)
    } else {
        DirOwnedOrBorrowed::Borrowed(d)
    };
    Ok((r, name))
}

#[cfg(any(target_os = "android", target_os = "linux"))]
/// A thin wrapper for [`openat2`] but that retries on interruption.
fn openat2_with_retry(
    dirfd: impl std::os::fd::AsFd,
    path: impl AsRef<Path>,
    oflags: rustix::fs::OFlags,
    mode: rustix::fs::Mode,
    resolve: rustix::fs::ResolveFlags,
) -> rustix::io::Result<std::os::fd::OwnedFd> {
    let dirfd = dirfd.as_fd();
    let path = path.as_ref();
    // We loop forever on EAGAIN right now. The cap-std version loops just 4 times,
    // which seems really arbitrary.
    path.into_with_c_str(|path_c_str| 'start: loop {
        match rustix::fs::openat2(dirfd, path_c_str, oflags, mode, resolve) {
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
    })
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn is_mountpoint_impl_statx(root: &Dir, path: &Path) -> Result<Option<bool>> {
    // https://github.com/systemd/systemd/blob/8fbf0a214e2fe474655b17a4b663122943b55db0/src/basic/mountpoint-util.c#L176
    use rustix::fs::StatxAttributes;
    use rustix::fs::{AtFlags, StatxFlags};
    use std::os::fd::AsFd;

    // SAFETY(unwrap): We can infallibly convert an i32 into a u64.
    match rustix::fs::statx(
        root.as_fd(),
        path,
        AtFlags::NO_AUTOMOUNT | AtFlags::SYMLINK_NOFOLLOW,
        StatxFlags::empty(),
    ) {
        Ok(r) => {
            let present = r.stx_attributes_mask.contains(StatxAttributes::MOUNT_ROOT);
            Ok(present.then_some(r.stx_attributes.contains(StatxAttributes::MOUNT_ROOT)))
        }
        Err(e) if e == rustix::io::Errno::NOSYS => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Open the target directory, but return Ok(None) if this would cross a mount point.
#[cfg(any(target_os = "android", target_os = "linux"))]
fn impl_open_dir_noxdev(
    d: &Dir,
    path: impl AsRef<std::path::Path>,
) -> std::io::Result<Option<Dir>> {
    use rustix::fs::{Mode, OFlags, ResolveFlags};
    match openat2_with_retry(
        d,
        path,
        OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::NO_XDEV | ResolveFlags::BENEATH,
    ) {
        Ok(r) => Ok(Some(Dir::reopen_dir(&r)?)),
        Err(e) if e == rustix::io::Errno::XDEV => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Implementation of a recursive directory walk
fn walk_inner<E>(
    d: &Dir,
    path: &mut PathBuf,
    callback: &mut dyn FnMut(&WalkComponent) -> WalkResult<E>,
    config: &WalkConfiguration,
) -> std::result::Result<(), E>
where
    E: From<std::io::Error>,
{
    let entries = d.entries()?;
    // If sorting is enabled, then read all entries now and sort them.
    let entries: Box<dyn Iterator<Item = Result<DirEntry>>> =
        if let Some(sorter) = config.sorter.as_ref() {
            let mut entries = entries.collect::<Result<Vec<_>>>()?;
            entries.sort_by(|a, b| sorter(a, b));
            Box::new(entries.into_iter().map(Ok))
        } else {
            Box::new(entries.into_iter())
        };
    // Operate on each entry
    for entry in entries {
        let entry = &entry?;
        // Gather basic data
        let ty = entry.file_type()?;
        let is_dir = ty.is_dir();
        let name = entry.file_name();
        // The path provided to the user includes the current filename
        path.push(&name);
        let filename = &name;
        let component = WalkComponent {
            path,
            dir: d,
            filename,
            file_type: ty,
            entry,
        };
        // Invoke the user path:callback
        let flow = callback(&component)?;
        // Did the callback tell us to stop iteration?
        let is_break = matches!(flow, std::ops::ControlFlow::Break(()));
        // Handle the non-directory case first.
        if !is_dir {
            path.pop();
            // If we got a break, then we're completely done.
            if is_break {
                return Ok(());
            } else {
                // Otherwise, process the next entry.
                continue;
            }
        } else if is_break {
            // For break on a directory, we continue processing the next entry.
            path.pop();
            continue;
        }
        // We're operating on a directory, and the callback must have told
        // us to continue.
        debug_assert!(matches!(flow, std::ops::ControlFlow::Continue(())));
        // Open the child directory, using the noxdev API if
        // we're configured not to cross devices,
        #[cfg(any(target_os = "android", target_os = "linux"))]
        let d = {
            if !config.noxdev {
                entry.open_dir()?
            } else if let Some(d) = impl_open_dir_noxdev(d, filename)? {
                d
            } else {
                path.pop();
                continue;
            }
        };

        #[cfg(not(any(target_os = "android", target_os = "linux")))]
        let d = entry.open_dir()?;

        // Recurse into the target directory
        walk_inner(&d, path, callback, config)?;
        path.pop();
    }
    Ok(())
}

// Ensure that the target path isn't absolute, and doesn't
// have any parent references.
pub(crate) fn validate_relpath_no_uplinks(path: &Path) -> Result<&Path> {
    let is_absolute = path.is_absolute();
    let contains_uplinks = path
        .components()
        .any(|e| e == std::path::Component::ParentDir);
    if is_absolute || contains_uplinks {
        Err(crate::escape_attempt())
    } else {
        Ok(path)
    }
}

impl CapStdExtDirExt for Dir {
    fn open_optional(&self, path: impl AsRef<Path>) -> Result<Option<File>> {
        map_optional(self.open(path.as_ref()))
    }

    fn open_dir_optional(&self, path: impl AsRef<Path>) -> Result<Option<Dir>> {
        map_optional(self.open_dir(path.as_ref()))
    }

    #[cfg(any(target_os = "android", target_os = "linux"))]
    fn open_dir_rooted_ext(&self, path: impl AsRef<Path>) -> Result<crate::RootDir> {
        crate::RootDir::new(self, path)
    }

    /// Open the target directory, but return Ok(None) if this would cross a mount point.
    #[cfg(any(target_os = "android", target_os = "linux"))]
    fn open_dir_noxdev(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<Option<Dir>> {
        impl_open_dir_noxdev(self, path)
    }

    fn ensure_dir_with(
        &self,
        p: impl AsRef<Path>,
        builder: &cap_std::fs::DirBuilder,
    ) -> Result<bool> {
        let p = p.as_ref();
        match self.create_dir_with(p, builder) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if !self.symlink_metadata(p)?.is_dir() {
                    // TODO use https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.NotADirectory
                    // once it's stable.
                    return Err(io::Error::new(io::ErrorKind::Other, "Found non-directory"));
                }
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }

    fn metadata_optional(&self, path: impl AsRef<Path>) -> Result<Option<Metadata>> {
        map_optional(self.metadata(path.as_ref()))
    }

    fn symlink_metadata_optional(&self, path: impl AsRef<Path>) -> Result<Option<Metadata>> {
        map_optional(self.symlink_metadata(path.as_ref()))
    }

    fn remove_file_optional(&self, path: impl AsRef<Path>) -> Result<bool> {
        match self.remove_file(path.as_ref()) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }

    fn remove_all_optional(&self, path: impl AsRef<Path>) -> Result<bool> {
        let path = path.as_ref();
        // This is obviously racy, but correctly matching on the errors
        // runs into the fact that e.g. https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.NotADirectory
        // is unstable right now.
        let meta = match self.symlink_metadata_optional(path)? {
            Some(m) => m,
            None => return Ok(false),
        };
        if meta.is_dir() {
            self.remove_dir_all(path)?;
        } else {
            self.remove_file(path)?;
        }
        Ok(true)
    }

    fn read_optional(&self, path: impl AsRef<Path>) -> Result<Option<Vec<u8>>> {
        let mut r = Vec::new();
        let Some(mut f) = self.open_optional(path.as_ref())? else {
            return Ok(None);
        };
        f.read_to_end(&mut r)?;
        Ok(Some(r))
    }

    fn read_to_string_optional(&self, path: impl AsRef<Path>) -> Result<Option<String>> {
        let mut r = String::new();
        let Some(mut f) = self.open_optional(path.as_ref())? else {
            return Ok(None);
        };
        f.read_to_string(&mut r)?;
        Ok(Some(r))
    }

    #[cfg(unix)]
    fn update_timestamps(&self, path: impl AsRef<Path>) -> Result<()> {
        use rustix::fd::AsFd;
        use rustix::fs::UTIME_NOW;

        let path = path.as_ref();
        let now = rustix::fs::Timespec {
            tv_sec: 0,
            tv_nsec: UTIME_NOW,
        };
        // https://github.com/bytecodealliance/rustix/commit/69af396b79e296717bece8148b1f6165b810885c
        // means that Timespec only implements Copy on 64 bit right now.
        #[allow(clippy::clone_on_copy)]
        let times = rustix::fs::Timestamps {
            last_access: now.clone(),
            last_modification: now.clone(),
        };
        rustix::fs::utimensat(
            self.as_fd(),
            path,
            &times,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )?;
        Ok(())
    }

    #[cfg(not(windows))]
    fn atomic_replace_with<F, T, E>(
        &self,
        destname: impl AsRef<Path>,
        f: F,
    ) -> std::result::Result<T, E>
    where
        F: FnOnce(&mut std::io::BufWriter<cap_tempfile::TempFile>) -> std::result::Result<T, E>,
        E: From<std::io::Error>,
    {
        let destname = destname.as_ref();
        let (d, name) = subdir_of(self, destname)?;
        let existing_metadata = d.symlink_metadata_optional(destname)?;
        // If the target is already a file, then acquire its mode, which we will preserve by default.
        // We don't follow symlinks here for replacement, and so we definitely don't want to pick up its mode.
        let existing_perms = existing_metadata
            .filter(|m| m.is_file())
            .map(|m| m.permissions());
        let mut t = cap_tempfile::TempFile::new(&d)?;
        // Apply the permissions, if we have them
        if let Some(existing_perms) = existing_perms {
            t.as_file_mut().set_permissions(existing_perms)?;
        }
        // We always operate in terms of buffered writes
        let mut bufw = std::io::BufWriter::new(t);
        // Call the provided closure to generate the file content
        let r = f(&mut bufw)?;
        // Flush the buffer, get the TempFile
        t = bufw.into_inner().map_err(From::from)?;
        // fsync the TempFile
        t.as_file().sync_all()?;
        // rename the TempFile
        t.replace(name)?;
        // fsync the directory
        d.open(".")?.sync_all()?;
        Ok(r)
    }

    #[cfg(not(windows))]
    fn atomic_write(&self, destname: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()> {
        self.atomic_replace_with(destname, |f| f.write_all(contents.as_ref()))
    }

    #[cfg(not(windows))]
    fn atomic_write_with_perms(
        &self,
        destname: impl AsRef<Path>,
        contents: impl AsRef<[u8]>,
        perms: cap_std::fs::Permissions,
    ) -> Result<()> {
        self.atomic_replace_with(destname, |f| -> io::Result<_> {
            // If the user is overriding the permissions, let's make the default be
            // writable by us but not readable by anyone else, in case it has
            // secret data.
            #[cfg(unix)]
            {
                use cap_std::fs::PermissionsExt;
                let perms = cap_std::fs::Permissions::from_mode(0o600);
                f.get_mut().as_file_mut().set_permissions(perms)?;
            }
            f.write_all(contents.as_ref())?;
            f.flush()?;
            f.get_mut().as_file_mut().set_permissions(perms)?;
            Ok(())
        })
    }

    #[cfg(unix)]
    fn reopen_as_ownedfd(&self) -> Result<OwnedFd> {
        use rustix::fs::{Mode, OFlags};
        use std::os::fd::AsFd;
        rustix::fs::openat(
            self.as_fd(),
            ".",
            OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::RDONLY,
            Mode::empty(),
        )
        .map_err(Into::into)
    }

    #[cfg(any(target_os = "android", target_os = "linux"))]
    fn is_mountpoint(&self, path: impl AsRef<Path>) -> Result<Option<bool>> {
        is_mountpoint_impl_statx(self, path.as_ref()).map_err(Into::into)
    }

    #[cfg(not(windows))]
    fn getxattr(&self, path: impl AsRef<Path>, key: impl AsRef<OsStr>) -> Result<Option<Vec<u8>>> {
        crate::xattrs::impl_getxattr(self, path.as_ref(), key.as_ref())
    }

    #[cfg(not(windows))]
    fn listxattrs(&self, path: impl AsRef<Path>) -> Result<crate::XattrList> {
        crate::xattrs::impl_listxattrs(self, path.as_ref())
    }

    #[cfg(not(windows))]
    fn setxattr(
        &self,
        path: impl AsRef<Path>,
        key: impl AsRef<OsStr>,
        value: impl AsRef<[u8]>,
    ) -> Result<()> {
        crate::xattrs::impl_setxattr(self, path.as_ref(), key.as_ref(), value.as_ref())
    }

    fn walk<C, E>(&self, config: &WalkConfiguration, mut callback: C) -> std::result::Result<(), E>
    where
        C: FnMut(&WalkComponent) -> WalkResult<E>,
        E: From<std::io::Error>,
    {
        let mut pb = config
            .path_base
            .as_ref()
            .map(|v| v.to_path_buf())
            .unwrap_or_default();
        walk_inner(self, &mut pb, &mut callback, config)
    }
}

// Implementation for the Utf8 variant of Dir. You shouldn't need to add
// any real logic here, just delegate to the non-UTF8 version via `as_cap_std()`
// in general.
#[cfg(feature = "fs_utf8")]
impl CapStdExtDirExtUtf8 for cap_std::fs_utf8::Dir {
    fn open_optional(&self, path: impl AsRef<Utf8Path>) -> Result<Option<fs_utf8::File>> {
        map_optional(self.open(path.as_ref()))
    }

    fn open_dir_optional(&self, path: impl AsRef<Utf8Path>) -> Result<Option<fs_utf8::Dir>> {
        map_optional(self.open_dir(path.as_ref()))
    }

    fn ensure_dir_with(
        &self,
        p: impl AsRef<Utf8Path>,
        builder: &cap_std::fs::DirBuilder,
    ) -> Result<bool> {
        self.as_cap_std()
            .ensure_dir_with(p.as_ref().as_std_path(), builder)
    }

    fn metadata_optional(&self, path: impl AsRef<Utf8Path>) -> Result<Option<Metadata>> {
        self.as_cap_std()
            .metadata_optional(path.as_ref().as_std_path())
    }

    fn symlink_metadata_optional(&self, path: impl AsRef<Utf8Path>) -> Result<Option<Metadata>> {
        self.as_cap_std()
            .symlink_metadata_optional(path.as_ref().as_std_path())
    }

    fn remove_file_optional(&self, path: impl AsRef<Utf8Path>) -> Result<bool> {
        self.as_cap_std()
            .remove_file_optional(path.as_ref().as_std_path())
    }

    fn remove_all_optional(&self, path: impl AsRef<Utf8Path>) -> Result<bool> {
        self.as_cap_std()
            .remove_all_optional(path.as_ref().as_std_path())
    }

    #[cfg(unix)]
    fn update_timestamps(&self, path: impl AsRef<Utf8Path>) -> Result<()> {
        self.as_cap_std()
            .update_timestamps(path.as_ref().as_std_path())
    }

    fn atomic_replace_with<F, T, E>(
        &self,
        destname: impl AsRef<Utf8Path>,
        f: F,
    ) -> std::result::Result<T, E>
    where
        F: FnOnce(&mut std::io::BufWriter<cap_tempfile::TempFile>) -> std::result::Result<T, E>,
        E: From<std::io::Error>,
    {
        self.as_cap_std()
            .atomic_replace_with(destname.as_ref().as_std_path(), f)
    }

    fn atomic_write(
        &self,
        destname: impl AsRef<Utf8Path>,
        contents: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.as_cap_std()
            .atomic_write(destname.as_ref().as_std_path(), contents)
    }

    fn atomic_write_with_perms(
        &self,
        destname: impl AsRef<Utf8Path>,
        contents: impl AsRef<[u8]>,
        perms: cap_std::fs::Permissions,
    ) -> Result<()> {
        self.as_cap_std()
            .atomic_write_with_perms(destname.as_ref().as_std_path(), contents, perms)
    }

    fn filenames_filtered_sorted_by<F, C>(&self, mut f: F, mut compare: C) -> Result<Vec<String>>
    where
        F: FnMut(&fs_utf8::DirEntry, &str) -> bool,
        C: FnMut(&str, &str) -> std::cmp::Ordering,
    {
        let mut r =
            self.entries()?
                .try_fold(Vec::new(), |mut acc, ent| -> Result<Vec<String>> {
                    let ent = ent?;
                    let name = ent.file_name()?;
                    if f(&ent, name.as_str()) {
                        acc.push(name);
                    }
                    Ok(acc)
                })?;
        r.sort_by(|a, b| compare(a.as_str(), b.as_str()));
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_validate_relpath_no_uplinks() {
        let ok_cases = ["foo", "foo/bar", "foo/bar/"];
        #[cfg(unix)]
        let err_cases = ["/foo", "/", "../foo", "foo/../bar"];
        #[cfg(windows)]
        let err_cases = ["C:\\foo", "../foo", "foo/../bar"];

        for case in ok_cases {
            assert!(validate_relpath_no_uplinks(Path::new(case)).is_ok());
        }
        for case in err_cases {
            assert!(validate_relpath_no_uplinks(Path::new(case)).is_err());
        }
    }
}
