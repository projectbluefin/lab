//! Handling of system restarts/reboot

use std::{io::Write, process::Command};

use bootc_utils::CommandRunExt;
use fn_error_context::context;

/// Initiate a system reboot.
/// This function will only return in case of error.
#[context("Initiating reboot")]
pub(crate) fn reboot() -> anyhow::Result<()> {
    // Flush output streams
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    Command::new("systemd-run")
        .args([
            "--quiet",
            "--",
            "systemctl",
            "reboot",
            "--message=Initiated by bootc",
        ])
        .run_capture_stderr()?;
    // We expect to be terminated via SIGTERM here. We sleep
    // instead of exiting an exit would necessarily appear
    // racy to calling processes in that sometimes we'd
    // win the race to exit, other times might get killed
    // via SIGTERM.
    tracing::debug!("Initiated reboot, sleeping");
    loop {
        std::thread::park();
    }
}
