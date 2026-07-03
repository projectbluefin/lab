use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;

/// Environment variable holding a reference to our original binary
pub const ORIG: &str = "_BOOTC_ORIG_EXE";

/// Return the path to our own executable. In some cases (SELinux) we may have
/// performed a re-exec with a temporary copy of the binary and
/// this environment variable will hold the path to the original binary.
pub fn executable_path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os(ORIG) {
        Ok(p.into())
    } else {
        std::env::current_exe().map_err(Into::into)
    }
}

/// Re-execute the current process if the provided environment variable is not set.
pub fn reexec_with_guardenv(k: &str, prefix_args: &[&str]) -> Result<()> {
    if std::env::var_os(k).is_some() {
        tracing::trace!("Skipping re-exec due to env var {k}");
        return Ok(());
    }
    let self_exe = executable_path()?;
    let mut prefix_args = prefix_args.iter();
    let mut cmd = if let Some(p) = prefix_args.next() {
        let mut c = Command::new(p);
        c.args(prefix_args);
        c.arg(self_exe);
        c
    } else {
        Command::new(self_exe)
    };
    cmd.env(k, "1");
    cmd.args(std::env::args_os().skip(1));
    cmd.arg0(crate::NAME);
    tracing::debug!("Re-executing current process for {k}");
    Err(cmd.exec().into())
}
