use std::ffi::OsStr;
use std::ops::ControlFlow;
use std::os::fd::AsRawFd;
use std::path::PathBuf;

use anyhow::{Context, Result};
use cap_std_ext::cap_std::fs::Dir;
use cap_std_ext::dirext::{CapStdExtDirExt, WalkConfiguration};
use rustix::path::DecInt;

fn verify_selinux_label_exists(d: &Dir, filename: &OsStr) -> Result<bool> {
    let mut buf = [0u8; 1024];
    let mut fdpath = PathBuf::from("/proc/self/fd");
    fdpath.push(DecInt::new(d.as_raw_fd()));
    fdpath.push(filename);
    match rustix::fs::lgetxattr(fdpath, "security.selinux", &mut buf) {
        // Ignore EOPNOTSUPPORTED
        Ok(_) | Err(rustix::io::Errno::OPNOTSUPP) => Ok(true),
        Err(rustix::io::Errno::NODATA) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn verify_selinux_recurse(root: &Dir, warn: bool) -> Result<()> {
    root.walk(&WalkConfiguration::default().noxdev(), |e| {
        let exists = verify_selinux_label_exists(e.dir, e.filename)
            .with_context(|| format!("Failed to look up context for {:?}", e.path))?;
        if !exists {
            if warn {
                eprintln!("No SELinux label found for: {:?}", e.path);
            } else {
                anyhow::bail!("No SELinux label found for: {:?}", e.path);
            }
        }
        anyhow::Ok(ControlFlow::Continue(()))
    })?;
    Ok(())
}
