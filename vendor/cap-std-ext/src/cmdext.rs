//! Extensions for [`std::process::Command`] that operate on concepts from cap-std.
//!
//! The key APIs here are:
//!
//! - File descriptor passing
//! - Changing to a file-descriptor relative directory
//! - Systemd socket activation fd passing

use cap_std::fs::Dir;
use cap_std::io_lifetimes;
use cap_tempfile::cap_std;
use io_lifetimes::OwnedFd;
use rustix::fd::{AsFd, FromRawFd, IntoRawFd};
use rustix::io::FdFlags;
use std::collections::BTreeSet;
use std::ffi::CString;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::sync::Arc;

/// The file descriptor number at which systemd passes the first socket.
/// See `sd_listen_fds(3)`.
const SD_LISTEN_FDS_START: i32 = 3;

/// A validated name for a systemd socket-activation file descriptor.
///
/// Names appear in the `LISTEN_FDNAMES` environment variable as
/// colon-separated values.  The constructor validates that the name
/// conforms to systemd's `fdname_is_valid()` rules: at most 255
/// printable ASCII characters, excluding `:`.
///
/// ```
/// use cap_std_ext::cmdext::SystemdFdName;
/// let name = SystemdFdName::new("varlink");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct SystemdFdName<'a>(&'a str);

impl<'a> SystemdFdName<'a> {
    /// Create a new `SystemdFdName`, panicking if `name` is invalid.
    ///
    /// # Panics
    ///
    /// Panics if `name` is longer than 255 bytes or contains any
    /// character that is not printable ASCII (i.e. control characters,
    /// DEL, non-ASCII bytes, or `:`).
    pub const fn new(name: &'a str) -> Self {
        assert!(
            name.len() <= 255,
            "systemd fd name must be at most 255 characters"
        );
        let bytes = name.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            assert!(
                b >= b' ' && b < 127 && b != b':',
                "systemd fd name must only contain printable ASCII characters except ':'"
            );
            i += 1;
        }
        Self(name)
    }

    /// Return the name as a string slice.
    pub fn as_str(&self) -> &'a str {
        self.0
    }
}

/// File descriptor allocator for child processes.
///
/// Collects fd assignments and optional systemd socket-activation
/// configuration, then applies them all at once via
/// [`CapStdExtCommandExt::take_fds`].
///
/// - [`new_systemd_fds`](Self::new_systemd_fds) creates an allocator
///   with systemd socket-activation fds at 3, 4, … (`SD_LISTEN_FDS_START`).
/// - [`take_fd`](Self::take_fd) auto-assigns the next fd above all
///   previously assigned ones (minimum 3).
/// - [`take_fd_n`](Self::take_fd_n) places an fd at an explicit number,
///   panicking on overlap.
///
/// ```no_run
/// # use std::sync::Arc;
/// # use cap_std_ext::cmdext::{CmdFds, CapStdExtCommandExt, SystemdFdName};
/// # let varlink_fd: Arc<rustix::fd::OwnedFd> = todo!();
/// # let extra_fd: Arc<rustix::fd::OwnedFd> = todo!();
/// let mut cmd = std::process::Command::new("myservice");
/// let mut fds = CmdFds::new_systemd_fds([(varlink_fd, SystemdFdName::new("varlink"))]);
/// let extra_n = fds.take_fd(extra_fd);
/// cmd.take_fds(fds);
/// ```
#[derive(Debug)]
pub struct CmdFds {
    taken: BTreeSet<i32>,
    fds: Vec<(i32, Arc<OwnedFd>)>,
    /// Pre-built CStrings for the systemd env vars, set by new_systemd_fds.
    systemd_env: Option<(CString, CString)>,
}

impl Default for CmdFds {
    fn default() -> Self {
        Self::new()
    }
}

impl CmdFds {
    /// Create a new fd allocator.
    pub fn new() -> Self {
        Self {
            taken: BTreeSet::new(),
            fds: Vec::new(),
            systemd_env: None,
        }
    }

    /// Create a new fd allocator with systemd socket-activation fds.
    ///
    /// Each `(fd, name)` pair is assigned a consecutive fd number starting
    /// at `SD_LISTEN_FDS_START` (3). The `LISTEN_PID`, `LISTEN_FDS`, and
    /// `LISTEN_FDNAMES` environment variables will be set in the child
    /// when [`CapStdExtCommandExt::take_fds`] is called.
    ///
    /// Additional (non-systemd) fds can be registered afterwards via
    /// [`take_fd`](Self::take_fd) or [`take_fd_n`](Self::take_fd_n).
    ///
    /// [sd_listen_fds]: https://www.freedesktop.org/software/systemd/man/latest/sd_listen_fds.html
    pub fn new_systemd_fds<'a>(
        fds: impl IntoIterator<Item = (Arc<OwnedFd>, SystemdFdName<'a>)>,
    ) -> Self {
        let mut this = Self::new();
        this.register_systemd_fds(fds);
        this
    }

    /// Compute the next fd number above everything already taken
    /// (minimum `SD_LISTEN_FDS_START`).
    fn next_fd(&self) -> i32 {
        self.taken
            .last()
            .map(|n| n.checked_add(1).expect("fd number overflow"))
            .unwrap_or(SD_LISTEN_FDS_START)
    }

    fn insert_fd(&mut self, n: i32) {
        let inserted = self.taken.insert(n);
        assert!(inserted, "fd {n} is already assigned");
    }

    /// Register a file descriptor at the next available fd number.
    ///
    /// Returns the fd number that will be assigned in the child.
    /// Call [`CapStdExtCommandExt::take_fds`] to apply.
    pub fn take_fd(&mut self, fd: Arc<OwnedFd>) -> i32 {
        let n = self.next_fd();
        self.insert_fd(n);
        self.fds.push((n, fd));
        n
    }

    /// Register a file descriptor at a specific fd number.
    ///
    /// Call [`CapStdExtCommandExt::take_fds`] to apply.
    ///
    /// # Panics
    ///
    /// Panics if `target` has already been assigned.
    pub fn take_fd_n(&mut self, fd: Arc<OwnedFd>, target: i32) -> &mut Self {
        self.insert_fd(target);
        self.fds.push((target, fd));
        self
    }

    fn register_systemd_fds<'a>(
        &mut self,
        fds: impl IntoIterator<Item = (Arc<OwnedFd>, SystemdFdName<'a>)>,
    ) {
        let mut n_fds: i32 = 0;
        let mut names = Vec::new();
        for (fd, name) in fds {
            let target = SD_LISTEN_FDS_START
                .checked_add(n_fds)
                .expect("too many fds");
            self.insert_fd(target);
            self.fds.push((target, fd));
            names.push(name.as_str());
            n_fds = n_fds.checked_add(1).expect("too many fds");
        }

        let fd_count = CString::new(n_fds.to_string()).unwrap();
        // SAFETY: SystemdFdName guarantees no NUL bytes.
        let fd_names = CString::new(names.join(":")).unwrap();
        self.systemd_env = Some((fd_count, fd_names));
    }
}

/// Extension trait for [`std::process::Command`].
///
/// [`cap_std::fs::Dir`]: https://docs.rs/cap-std/latest/cap_std/fs/struct.Dir.html
pub trait CapStdExtCommandExt {
    /// Pass a file descriptor into the target process at a specific fd number.
    ///
    /// # Deprecated
    ///
    /// Use [`CmdFds`] with [`take_fds`](Self::take_fds) instead. This method
    /// registers an independent `pre_exec` hook per call, which means
    /// multiple `take_fd_n` calls (or mixing with `take_fds`) can clobber
    /// each other when a source fd's raw number equals another mapping's
    /// target. `take_fds` handles this correctly with atomic fd shuffling.
    #[deprecated = "Use CmdFds with take_fds() instead"]
    fn take_fd_n(&mut self, fd: Arc<OwnedFd>, target: i32) -> &mut Self;

    /// Apply a [`CmdFds`] to this command, passing all registered file
    /// descriptors and (if configured) setting up the systemd
    /// socket-activation environment.
    ///
    /// # Important: Do not use `Command::env()` with systemd fds
    ///
    /// When systemd socket-activation environment variables are configured
    /// (via [`CmdFds::new_systemd_fds`]), they are set using `setenv(3)` in
    /// a `pre_exec` hook. If `Command::env()` is also called, Rust will
    /// build an `envp` array that replaces the process environment, causing
    /// the `LISTEN_*` variables set by the hook to be lost. `Command::envs()`
    /// is equally problematic. If you need to set additional environment
    /// variables alongside systemd fds, set them via `pre_exec` + `setenv`
    /// as well.
    fn take_fds(&mut self, fds: CmdFds) -> &mut Self;

    /// Use the given directory as the current working directory for the process.
    fn cwd_dir(&mut self, dir: Dir) -> &mut Self;

    /// On Linux, arrange for [`SIGTERM`] to be delivered to the child if the
    /// parent *thread* exits. This helps avoid leaking child processes if
    /// the parent crashes for example.
    ///
    /// # IMPORTANT
    ///
    /// Due to the semantics of <https://man7.org/linux/man-pages/man2/prctl.2.html> this
    /// will cause the child to exit when the parent *thread* (not process) exits. In
    /// particular this can become problematic when used with e.g. a threadpool such
    /// as Tokio's <https://kobzol.github.io/rust/2025/02/23/tokio-plus-prctl-equals-nasty-bug.html>.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn lifecycle_bind_to_parent_thread(&mut self) -> &mut Self;
}

/// Wrapper around `libc::setenv` that checks the return value.
///
/// # Safety
///
/// Must only be called in a single-threaded context (e.g. after `fork()`
/// and before `exec()`).
#[allow(unsafe_code)]
unsafe fn check_setenv(
    key: *const std::ffi::c_char,
    val: *const std::ffi::c_char,
) -> std::io::Result<()> {
    // SAFETY: Caller guarantees we are in a single-threaded context
    // with valid nul-terminated C strings.
    if unsafe { libc::setenv(key, val, 1) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[allow(unsafe_code)]
#[allow(deprecated)]
impl CapStdExtCommandExt for std::process::Command {
    fn take_fd_n(&mut self, fd: Arc<OwnedFd>, target: i32) -> &mut Self {
        unsafe {
            self.pre_exec(move || {
                let mut target = OwnedFd::from_raw_fd(target);
                // If the fd is already what we want, then just ensure that
                // O_CLOEXEC is stripped off.
                if target.as_raw_fd() == fd.as_raw_fd() {
                    let fl = rustix::io::fcntl_getfd(&target)?;
                    rustix::io::fcntl_setfd(&mut target, fl.difference(FdFlags::CLOEXEC))?;
                } else {
                    // Otherwise create a dup, which will also default to not setting O_CLOEXEC.
                    rustix::io::dup2(&*fd, &mut target)?;
                }
                // Intentionally leak into the child.
                let _ = target.into_raw_fd();
                Ok(())
            });
        }
        self
    }

    fn take_fds(&mut self, fds: CmdFds) -> &mut Self {
        // Use a single pre_exec hook that handles all fd shuffling atomically.
        // This avoids the problem where separate hooks clobber each other when
        // a source fd number equals a target fd number from a different mapping.
        unsafe {
            self.pre_exec(move || {
                // Dup each source fd to a temporary location above all
                // targets, so that no dup2() in step 2 can clobber a source.
                let safe_min = fds
                    .fds
                    .iter()
                    .map(|(t, _)| *t)
                    .max()
                    .unwrap_or(0)
                    .checked_add(1)
                    .expect("fd number overflow");
                let mut safe_copies: Vec<(i32, OwnedFd)> = Vec::new();
                for (target, fd) in &fds.fds {
                    let copy = rustix::io::fcntl_dupfd_cloexec(fd, safe_min)?;
                    safe_copies.push((*target, copy));
                }

                // Place each fd at its target via dup2.
                // We use raw dup2 to avoid fabricating an OwnedFd for a
                // target number we don't yet own (which would be unsound
                // if dup2 failed — the OwnedFd drop would close a wrong fd).
                for (target, copy) in safe_copies {
                    // SAFETY: target is a non-negative fd number that dup2
                    // will atomically (re)open; we don't own it beforehand.
                    let r = libc::dup2(copy.as_raw_fd(), target);
                    if r < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    // `copy` drops here, closing the temporary fd.
                }

                // Handle systemd env vars, if configured
                if let Some((ref fd_count, ref fd_names)) = fds.systemd_env {
                    let pid = rustix::process::getpid();
                    let pid_dec = rustix::path::DecInt::new(pid.as_raw_nonzero().get());
                    // SAFETY: After fork() and before exec(), the child is
                    // single-threaded, so setenv (which is not thread-safe)
                    // is safe to call here.
                    check_setenv(c"LISTEN_PID".as_ptr(), pid_dec.as_c_str().as_ptr())?;
                    check_setenv(c"LISTEN_FDS".as_ptr(), fd_count.as_ptr())?;
                    check_setenv(c"LISTEN_FDNAMES".as_ptr(), fd_names.as_ptr())?;
                }
                Ok(())
            });
        }
        self
    }

    fn cwd_dir(&mut self, dir: Dir) -> &mut Self {
        unsafe {
            self.pre_exec(move || {
                rustix::process::fchdir(dir.as_fd())?;
                Ok(())
            });
        }
        self
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn lifecycle_bind_to_parent_thread(&mut self) -> &mut Self {
        // SAFETY: This API is safe to call in a forked child.
        unsafe {
            self.pre_exec(|| {
                rustix::process::set_parent_process_death_signal(Some(
                    rustix::process::Signal::TERM,
                ))
                .map_err(Into::into)
            });
        }
        self
    }
}

#[cfg(all(test, any(target_os = "android", target_os = "linux")))]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[allow(deprecated)]
    #[test]
    fn test_take_fdn() -> anyhow::Result<()> {
        // Pass srcfd == destfd and srcfd != destfd
        for i in 0..=1 {
            let tempd = cap_tempfile::TempDir::new(cap_std::ambient_authority())?;
            let tempd_fd = Arc::new(tempd.as_fd().try_clone_to_owned()?);
            let n = tempd_fd.as_raw_fd() + i;
            #[cfg(any(target_os = "android", target_os = "linux"))]
            let path = format!("/proc/self/fd/{n}");
            #[cfg(not(any(target_os = "android", target_os = "linux")))]
            let path = format!("/dev/fd/{n}");
            let st = std::process::Command::new("/usr/bin/env")
                .arg("readlink")
                .arg(path)
                .take_fd_n(tempd_fd, n)
                .status()?;
            assert!(st.success());
        }
        Ok(())
    }
}
