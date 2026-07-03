//! Extensions for [`std::process::Command`] that operate on concepts from cap-std.
//!
//! The key APIs here are:
//!
//! - File descriptor passing
//! - Changing to a file-descriptor relative directory

use cap_std::fs::Dir;
use cap_std::io_lifetimes;
use cap_tempfile::cap_std;
use io_lifetimes::OwnedFd;
use rustix::fd::{AsFd, FromRawFd, IntoRawFd};
use rustix::io::FdFlags;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::sync::Arc;

/// Extension trait for [`std::process::Command`].
///
/// [`cap_std::fs::Dir`]: https://docs.rs/cap-std/latest/cap_std/fs/struct.Dir.html
pub trait CapStdExtCommandExt {
    /// Pass a file descriptor into the target process.
    fn take_fd_n(&mut self, fd: Arc<OwnedFd>, target: i32) -> &mut Self;

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

#[allow(unsafe_code)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_take_fdn() -> anyhow::Result<()> {
        // Pass srcfd == destfd and srcfd != destfd
        for i in 0..=1 {
            let tempd = cap_tempfile::TempDir::new(cap_std::ambient_authority())?;
            let tempd_fd = Arc::new(tempd.as_fd().try_clone_to_owned()?);
            let n = tempd_fd.as_raw_fd() + i;
            let st = std::process::Command::new("ls")
                .arg(format!("/proc/self/fd/{n}"))
                .take_fd_n(tempd_fd, n)
                .status()?;
            assert!(st.success());
        }
        Ok(())
    }
}
