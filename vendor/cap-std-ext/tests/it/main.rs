use anyhow::Result;

#[cfg(unix)]
use cap_std::fs::PermissionsExt;
use cap_std::fs::{Dir, File, Permissions};
use cap_std_ext::cap_std;
#[cfg(not(windows))]
use cap_std_ext::cmdext::{CapStdExtCommandExt, CmdFds, SystemdFdName};
use cap_std_ext::dirext::{CapStdExtDirExt, WalkConfiguration};
#[cfg(any(target_os = "android", target_os = "linux"))]
use cap_std_ext::RootDir;
#[cfg(unix)]
use rustix::path::DecInt;
use std::cmp::Ordering;
#[cfg(any(target_os = "android", target_os = "linux"))]
use std::ffi::OsStr;
use std::io::Write;
use std::ops::ControlFlow;
#[cfg(not(windows))]
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::{process::Command, sync::Arc};

/// Create a pipe, write `content` to it, and return the read end wrapped in `Arc`.
#[cfg(not(windows))]
fn make_pipe(content: &str) -> Result<Arc<rustix::fd::OwnedFd>> {
    let (r, w) = rustix::pipe::pipe()?;
    let r = Arc::new(r);
    let mut w: File = w.into();
    write!(w, "{content}")?;
    drop(w);
    Ok(r)
}

/// Run a bash script, return its stdout split into lines.
///
/// `setup` is called on the [`Command`] before spawning, so callers can
/// attach fds or configure the child.
#[cfg(not(windows))]
fn run_bash_piped(script: &str, setup: impl FnOnce(&mut Command)) -> Result<Vec<String>> {
    let mut c = Command::new("/bin/bash");
    c.arg("-c").arg(script);
    c.stdout(std::process::Stdio::piped());
    setup(&mut c);
    let out = c.output()?;
    assert!(
        out.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(stdout.lines().map(String::from).collect())
}

#[test]
#[cfg(not(windows))]
#[allow(deprecated)]
fn take_fd() -> Result<()> {
    let r = make_pipe("hello world")?;
    let lines = run_bash_piped("wc -c <&5", |c| {
        c.take_fd_n(r, 5);
    })?;
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].trim(), "11");
    Ok(())
}

#[test]
#[cfg(not(windows))]
fn fchdir() -> Result<()> {
    static CONTENTS: &[u8] = b"hello world";

    fn new_cmd() -> Command {
        #[cfg(any(target_os = "android", target_os = "linux"))]
        let mut c = Command::new("/usr/bin/cat");
        #[cfg(not(any(target_os = "android", target_os = "linux")))]
        let mut c = Command::new("/bin/cat");
        c.arg("somefile");
        c
    }

    fn test_cmd(mut c: Command) -> Result<()> {
        let st = c.output()?;
        if !st.status.success() {
            anyhow::bail!("Failed to exec cat");
        }
        assert_eq!(st.stdout.as_slice(), CONTENTS);
        Ok(())
    }

    let td = Arc::new(cap_tempfile::tempdir(cap_std::ambient_authority())?);

    td.write("somefile", CONTENTS)?;

    let mut c = new_cmd();
    let subdir = td.open_dir(".")?;
    c.cwd_dir(subdir.try_clone()?);
    test_cmd(c).unwrap();

    Ok(())
}

#[test]
fn optionals() -> Result<()> {
    let td = cap_tempfile::tempdir(cap_std::ambient_authority())?;

    // file
    assert!(td.open_optional("bar")?.is_none());
    assert!(td.metadata_optional("bar").unwrap().is_none());
    assert!(!(td.remove_file_optional("bar")?));
    td.write("bar", "testcontents")?;
    assert!(td.metadata_optional("bar").unwrap().is_some());
    assert!(td.symlink_metadata_optional("bar").unwrap().is_some());
    assert_eq!(td.read("bar")?.as_slice(), b"testcontents");
    assert!(td.remove_file_optional("bar")?);

    // directory
    assert!(td.open_dir_optional("somedir")?.is_none());
    td.create_dir("somedir")?;
    assert!(td.open_dir_optional("somedir")?.is_some());
    Ok(())
}

#[test]
fn read_optional() -> Result<()> {
    let td = cap_tempfile::tempdir(cap_std::ambient_authority())?;

    // Test non-existent file returns None
    assert!(td.read_optional("nonexistent")?.is_none());

    // Test existing file with string contents
    td.write("test.txt", "hello world")?;
    let contents = td.read_optional("test.txt")?;
    assert!(contents.is_some());
    assert_eq!(contents.unwrap(), b"hello world");

    // Test existing file with binary contents
    let binary_data = vec![0u8, 1, 2, 254, 255];
    td.write("test.bin", &binary_data)?;
    let contents = td.read_optional("test.bin")?;
    assert!(contents.is_some());
    assert_eq!(contents.unwrap(), binary_data);

    // Test empty file
    td.write("empty.txt", "")?;
    let contents = td.read_optional("empty.txt")?;
    assert!(contents.is_some());
    assert_eq!(contents.unwrap(), b"");

    // Test file in subdirectory
    td.create_dir_all("sub/dir")?;
    td.write("sub/dir/file.txt", "nested content")?;
    let contents = td.read_optional("sub/dir/file.txt")?;
    assert!(contents.is_some());
    assert_eq!(contents.unwrap(), b"nested content");

    // Test non-existent file in existing directory
    assert!(td.read_optional("sub/dir/missing.txt")?.is_none());

    // Test large file
    let large_data = vec![b'x'; 10000];
    td.write("large.txt", &large_data)?;
    let contents = td.read_optional("large.txt")?;
    assert!(contents.is_some());
    assert_eq!(contents.unwrap(), large_data);

    #[cfg(not(windows))]
    {
        // Test symlink to existing file
        td.symlink("test.txt", "link_to_test")?;
        let contents = td.read_optional("link_to_test")?;
        assert!(contents.is_some());
        assert_eq!(contents.unwrap(), b"hello world");

        // Test broken symlink returns None (NotFound is mapped to None)
        td.symlink("nonexistent_target", "broken_link")?;
        assert!(td.read_optional("broken_link")?.is_none());
    }

    Ok(())
}

#[test]
fn read_to_string_optional() -> Result<()> {
    let td = cap_tempfile::tempdir(cap_std::ambient_authority())?;

    // Test non-existent file returns None
    assert!(td.read_to_string_optional("nonexistent")?.is_none());

    // Test existing file with valid UTF-8 string
    td.write("test.txt", "hello world")?;
    let contents = td.read_to_string_optional("test.txt")?;
    assert!(contents.is_some());
    assert_eq!(contents.unwrap(), "hello world");

    // Test file with UTF-8 content including unicode
    let unicode_content = "Hello 世界 🌍";
    td.write("unicode.txt", unicode_content)?;
    let contents = td.read_to_string_optional("unicode.txt")?;
    assert!(contents.is_some());
    assert_eq!(contents.unwrap(), unicode_content);

    // Test empty file
    td.write("empty.txt", "")?;
    let contents = td.read_to_string_optional("empty.txt")?;
    assert!(contents.is_some());
    assert_eq!(contents.unwrap(), "");

    // Test file in subdirectory
    td.create_dir_all("sub/dir")?;
    td.write("sub/dir/file.txt", "nested content")?;
    let contents = td.read_to_string_optional("sub/dir/file.txt")?;
    assert!(contents.is_some());
    assert_eq!(contents.unwrap(), "nested content");

    // Test non-existent file in existing directory
    assert!(td.read_to_string_optional("sub/dir/missing.txt")?.is_none());

    // Test file with invalid UTF-8 should return an error
    let invalid_utf8 = vec![0xff, 0xfe, 0xfd];
    td.write("invalid.bin", invalid_utf8.as_slice())?;
    assert!(td.read_to_string_optional("invalid.bin").is_err());

    #[cfg(not(windows))]
    {
        // Test symlink to existing file
        td.symlink("test.txt", "link_to_test")?;
        let contents = td.read_to_string_optional("link_to_test")?;
        assert!(contents.is_some());
        assert_eq!(contents.unwrap(), "hello world");

        // Test broken symlink returns None (NotFound is mapped to None)
        td.symlink("nonexistent_target", "broken_link")?;
        assert!(td.read_to_string_optional("broken_link")?.is_none());
    }

    Ok(())
}

#[test]
fn ensuredir() -> Result<()> {
    let td = cap_tempfile::tempdir(cap_std::ambient_authority())?;

    let p = Path::new("somedir");
    let b = &cap_std::fs::DirBuilder::new();
    assert!(td.metadata_optional(p)?.is_none());
    assert!(td.symlink_metadata_optional(p)?.is_none());
    assert!(td.ensure_dir_with(p, b).unwrap());
    assert!(td.metadata_optional(p)?.is_some());
    assert!(td.symlink_metadata_optional(p)?.is_some());
    assert!(!td.ensure_dir_with(p, b).unwrap());

    let p = Path::new("somedir/without/existing-parent");
    // We should fail because the intermediate directory doesn't exist.
    assert!(td.ensure_dir_with(p, b).is_err());
    // Now create the parent
    assert!(td.ensure_dir_with(p.parent().unwrap(), b).unwrap());
    assert!(td.ensure_dir_with(p, b).unwrap());
    assert!(!td.ensure_dir_with(p, b).unwrap());

    // Verify we don't replace a file
    let p = Path::new("somefile");
    td.write(p, "some file contents")?;
    assert!(td.ensure_dir_with(p, b).is_err());

    #[cfg(not(windows))]
    {
        // Broken symlinks aren't followed and are errors
        let p = Path::new("linksrc");
        td.symlink("linkdest", p)?;
        assert!(td.metadata(p).is_err());
        assert!(td
            .symlink_metadata_optional(p)
            .unwrap()
            .unwrap()
            .is_symlink());
        // Non-broken symlinks are also an error
        assert!(td.ensure_dir_with(p, b).is_err());
        td.create_dir("linkdest")?;
        assert!(td.ensure_dir_with(p, b).is_err());
        assert!(td.metadata_optional(p).unwrap().unwrap().is_dir());
    }

    Ok(())
}

#[test]
fn test_remove_all_optional() -> Result<()> {
    let td = cap_tempfile::tempdir(cap_std::ambient_authority())?;

    let p = Path::new("somedir");
    assert!(!td.remove_all_optional(p).unwrap());
    td.create_dir(p)?;
    assert!(td.remove_all_optional(p).unwrap());
    let subpath = p.join("foo/bar");
    td.create_dir_all(subpath)?;
    assert!(td.remove_all_optional(p).unwrap());

    // regular file
    td.write(p, "test")?;
    assert!(td.remove_all_optional(p).unwrap());

    #[cfg(not(windows))]
    {
        // symlinks; broken and not
        let p = Path::new("linksrc");
        td.symlink("linkdest", p)?;
        assert!(td.remove_all_optional(p).unwrap());
        td.symlink("linkdest", p)?;
        assert!(td.remove_all_optional(p).unwrap());
    }

    Ok(())
}

/// Hack to determine the default mode for a file; we could
/// on Linux actually parse /proc/self/umask as is done in cap_tempfile,
/// but eh this is just to cross check with that code.
fn default_mode(d: &Dir) -> Result<Permissions> {
    let f = cap_tempfile::TempFile::new(d)?;
    Ok(f.as_file().metadata()?.permissions())
}

#[test]
#[cfg(not(windows))]
fn link_tempfile_with() -> Result<()> {
    let td = cap_tempfile::tempdir(cap_std::ambient_authority())?;
    let p = Path::new("foo");
    td.atomic_replace_with(p, |f| writeln!(f, "hello world"))
        .unwrap();
    assert_eq!(td.read_to_string(p).unwrap().as_str(), "hello world\n");
    let default_perms = default_mode(&td)?;
    assert_eq!(td.metadata(p)?.permissions(), default_perms);

    td.atomic_replace_with(p, |f| writeln!(f, "atomic replacement"))
        .unwrap();
    assert_eq!(
        td.read_to_string(p).unwrap().as_str(),
        "atomic replacement\n"
    );

    let e = td
        .atomic_replace_with(p, |f| {
            writeln!(f, "should not be written")?;
            Err::<(), _>(std::io::Error::other("oops"))
        })
        .err()
        .unwrap();
    assert!(e.to_string().contains("oops"));
    // We should not have written to the file!
    assert_eq!(
        td.read_to_string(p).unwrap().as_str(),
        "atomic replacement\n"
    );

    td.atomic_write(p, "atomic replacement write\n").unwrap();
    assert_eq!(
        td.read_to_string(p).unwrap().as_str(),
        "atomic replacement write\n"
    );
    assert_eq!(td.metadata(p)?.permissions(), default_perms);

    #[cfg(unix)]
    {
        td.atomic_write_with_perms(p, "atomic replacement 3\n", Permissions::from_mode(0o700))
            .unwrap();
        assert_eq!(
            td.read_to_string(p).unwrap().as_str(),
            "atomic replacement 3\n"
        );
        assert_eq!(td.metadata(p)?.permissions().mode() & 0o777, 0o700);

        // Ensure we preserve the executable bit on an existing file
        assert_eq!(td.metadata(p).unwrap().permissions().mode() & 0o700, 0o700);
        td.atomic_write(p, "atomic replacement 4\n").unwrap();
        assert_eq!(
            td.read_to_string(p).unwrap().as_str(),
            "atomic replacement 4\n"
        );
        assert_eq!(td.metadata(p)?.permissions().mode() & 0o777, 0o700);

        // But we should ignore permissions on a symlink (both existing and broken)
        td.remove_file(p)?;
        let p2 = Path::new("bar");
        td.atomic_write_with_perms(p2, "link target", Permissions::from_mode(0o755))
            .unwrap();
        td.symlink(p2, p)?;
        td.atomic_write(p, "atomic replacement symlink\n").unwrap();
        assert_eq!(td.metadata(p)?.permissions(), default_perms);
        // And break the link
        td.remove_file(p2)?;
        td.atomic_write(p, "atomic replacement symlink\n").unwrap();
        assert_eq!(td.metadata(p)?.permissions(), default_perms);

        // Also test with mode 0600
        td.atomic_write_with_perms(p, "self-only file", Permissions::from_mode(0o600))
            .unwrap();
        assert_eq!(td.metadata(p).unwrap().permissions().mode() & 0o777, 0o600);
        td.atomic_write(p, "self-only file v2").unwrap();
        assert_eq!(td.metadata(p).unwrap().permissions().mode() & 0o777, 0o600);
        // But we can override
        td.atomic_write_with_perms(p, "self-only file v3", Permissions::from_mode(0o640))
            .unwrap();
        assert_eq!(td.metadata(p).unwrap().permissions().mode() & 0o777, 0o640);
    }

    Ok(())
}

#[test]
#[cfg(unix)]
fn test_timestamps() -> Result<()> {
    let td = cap_tempfile::tempdir(cap_std::ambient_authority())?;
    let p = Path::new("foo");
    td.atomic_replace_with(p, |f| writeln!(f, "hello world"))
        .unwrap();
    let ts0 = td.metadata(p)?.modified()?;
    // This test assumes at least second granularity on filesystem timestamps, and
    // that the system clock is not rolled back during the test.
    std::thread::sleep(std::time::Duration::from_secs(1));
    let ts1 = td.metadata(p)?.modified()?;
    assert_eq!(ts0, ts1);
    td.update_timestamps(p).unwrap();
    let ts2 = td.metadata(p)?.modified()?;
    assert_ne!(ts1, ts2);
    assert!(ts2 > ts1);

    Ok(())
}

// For now just this test is copy-pasted to verify utf8
#[test]
#[cfg(feature = "fs_utf8")]
fn ensuredir_utf8() -> Result<()> {
    use cap_std::fs_utf8::camino::Utf8Path;
    use cap_std_ext::dirext::CapStdExtDirExtUtf8;
    let td = cap_tempfile::tempdir(cap_std::ambient_authority())?;
    let td = &cap_std::fs_utf8::Dir::from_cap_std((&*td).try_clone()?);

    let p = Utf8Path::new("somedir");
    let b = &cap_std::fs::DirBuilder::new();
    assert!(td.metadata_optional(p)?.is_none());
    assert!(td.symlink_metadata_optional(p)?.is_none());
    assert!(td.ensure_dir_with(p, b).unwrap());
    assert!(td.metadata_optional(p)?.is_some());
    assert!(td.symlink_metadata_optional(p)?.is_some());
    assert!(!td.ensure_dir_with(p, b).unwrap());

    let p = Utf8Path::new("somedir/without/existing-parent");
    // We should fail because the intermediate directory doesn't exist.
    assert!(td.ensure_dir_with(p, b).is_err());
    // Now create the parent
    assert!(td.ensure_dir_with(p.parent().unwrap(), b).unwrap());
    assert!(td.ensure_dir_with(p, b).unwrap());
    assert!(!td.ensure_dir_with(p, b).unwrap());

    // Verify we don't replace a file
    let p = Utf8Path::new("somefile");
    td.write(p, "some file contents")?;
    assert!(td.ensure_dir_with(p, b).is_err());

    #[cfg(not(windows))]
    {
        // Broken symlinks aren't followed and are errors
        let p = Utf8Path::new("linksrc");
        td.symlink("linkdest", p)?;
        assert!(td.metadata(p).is_err());
        assert!(td
            .symlink_metadata_optional(p)
            .unwrap()
            .unwrap()
            .is_symlink());
        // Non-broken symlinks are also an error
        assert!(td.ensure_dir_with(p, b).is_err());
        td.create_dir("linkdest")?;
        assert!(td.ensure_dir_with(p, b).is_err());
        assert!(td.metadata_optional(p).unwrap().unwrap().is_dir());
    }

    Ok(())
}

#[test]
#[cfg(feature = "fs_utf8")]
fn filenames_utf8() -> Result<()> {
    use std::collections::BTreeSet;

    use cap_std_ext::dirext::CapStdExtDirExtUtf8;
    let td = &cap_tempfile::utf8::TempDir::new(cap_std::ambient_authority())?;

    let mut expected = BTreeSet::new();
    const N: usize = 20;
    (0..N).try_for_each(|_| -> Result<()> {
        let fname = uuid::Uuid::new_v4().to_string();

        td.write(&fname, &fname)?;
        expected.insert(fname);
        Ok(())
    })?;
    let names = td.filenames_sorted().unwrap();
    for (a, b) in expected.iter().zip(names.iter()) {
        assert_eq!(a, b);
    }

    td.create_dir(".foo").unwrap();

    let names = td
        .filenames_filtered_sorted(|_ent, name| !name.starts_with('.'))
        .unwrap();
    assert_eq!(names.len(), N);
    for name in names.iter() {
        assert!(!name.starts_with('.'));
    }
    Ok(())
}

#[test]
#[cfg(any(target_os = "android", target_os = "linux"))]
fn test_rootdir_open() -> Result<()> {
    let td = &cap_tempfile::TempDir::new(cap_std::ambient_authority())?;
    let root = RootDir::new(td, ".").unwrap();

    assert!(root.open_optional("foo").unwrap().is_none());

    td.create_dir("etc")?;
    td.create_dir_all("usr/lib")?;

    let authjson = "usr/lib/auth.json";
    assert!(root.open(authjson).is_err());
    assert!(root.open_optional(authjson).unwrap().is_none());
    td.write(authjson, "auth contents")?;
    assert!(root.open_optional(authjson).unwrap().is_some());
    let contents = root.read_to_string(authjson).unwrap();
    assert_eq!(&contents, "auth contents");

    td.symlink_contents("/usr/lib/auth.json", "etc/auth.json")?;

    let contents = root.read_to_string("/etc/auth.json").unwrap();
    assert_eq!(&contents, "auth contents");

    // But this should fail due to an escape
    assert!(td.read_to_string("etc/auth.json").is_err());
    Ok(())
}

#[test]
#[cfg(any(target_os = "android", target_os = "linux"))]
fn test_rootdir_entries() -> Result<()> {
    let td = &cap_tempfile::TempDir::new(cap_std::ambient_authority())?;
    let root = RootDir::new(td, ".").unwrap();

    td.create_dir("etc")?;
    td.create_dir_all("usr/lib")?;

    let ents = root
        .entries()
        .unwrap()
        .collect::<std::io::Result<Vec<_>>>()?;
    assert_eq!(ents.len(), 2);
    Ok(())
}

#[test]
#[cfg(any(target_os = "android", target_os = "linux"))]
fn test_mountpoint() -> Result<()> {
    let root = &Dir::open_ambient_dir("/", cap_std::ambient_authority())?;
    assert_eq!(root.is_mountpoint(".").unwrap(), Some(true));
    let td = &cap_tempfile::TempDir::new(cap_std::ambient_authority())?;
    assert_eq!(td.is_mountpoint(".").unwrap(), Some(false));
    Ok(())
}

#[test]
#[cfg(unix)]
fn test_reopen_as_ownedfd() -> Result<()> {
    let td = &cap_tempfile::TempDir::new(cap_std::ambient_authority())?;
    let fd = td.reopen_as_ownedfd()?;
    rustix::fs::fsync(fd)?;
    Ok(())
}

#[test]
#[cfg(any(target_os = "android", target_os = "linux"))]
fn test_open_noxdev() -> Result<()> {
    let root = Dir::open_ambient_dir("/", cap_std::ambient_authority())?;
    // This hard requires the host setup to have /usr/bin on the same filesystem as /
    let usr = Dir::open_ambient_dir("/usr", cap_std::ambient_authority())?;
    assert!(usr.open_dir_noxdev("bin").unwrap().is_some());
    // Requires a mounted /proc, but that also seems sane.
    assert!(root.open_dir_noxdev("proc").unwrap().is_none());
    // Test an error case
    let td = cap_tempfile::TempDir::new(cap_std::ambient_authority())?;
    assert!(td.open_dir_noxdev("foo").is_err());
    Ok(())
}

#[test]
fn test_walk() -> std::io::Result<()> {
    let td = cap_tempfile::TempDir::new(cap_std::ambient_authority())?;

    td.create_dir_all("usr/bin")?;
    td.create_dir_all("usr/lib/foo")?;
    td.create_dir_all("usr/share")?;
    td.create_dir_all("etc")?;
    td.write("usr/bin/bash", b"bash")?;
    td.write("usr/bin/true", b"true")?;
    td.write("usr/lib/foo/libfoo.so", b"libfoo")?;
    td.write("usr/lib/libbar.so", b"libbar")?;
    #[cfg(not(windows))]
    {
        // Broken link
        td.symlink("usr/share/timezone", "usr/share/EST")?;
        // Symlink to self
        td.symlink(".", "usr/bin/selflink")?;
    }
    td.write("etc/foo.conf", b"fooconf")?;
    td.write("etc/blah.conf", b"blahconf")?;

    // A successful walk, but skipping /etc
    {
        let mut n_dirs = 0;
        let mut n_symlinks = 0;
        let mut n_regfiles = 0;
        td.walk(&WalkConfiguration::default(), |e| -> std::io::Result<_> {
            // Just a sanity check
            assert!(!e.path.is_absolute());
            if e.path.strip_prefix("usr/lib").is_ok() {
                return Ok(ControlFlow::Break(()));
            }
            if e.file_type.is_dir() {
                n_dirs += 1;
            } else if e.file_type.is_symlink() {
                n_symlinks += 1;
            } else if e.file_type.is_file() {
                n_regfiles += 1;
                // Verify we can open
                let _ = e.entry.open().unwrap();
            } else {
                unreachable!();
            }
            Ok(ControlFlow::Continue(()))
        })
        .unwrap();
        assert_eq!(n_dirs, 4);
        #[cfg(not(windows))]
        assert_eq!(n_symlinks, 2);
        assert_eq!(n_regfiles, 4);
    }

    // An error case
    {
        let r: std::io::Result<_> = td.walk(&WalkConfiguration::default(), |e| {
            if e.path.strip_prefix("usr/share").is_ok() {
                return Err(std::io::Error::other("oops"));
            }
            Ok(ControlFlow::Continue(()))
        });
        match r {
            Err(e) if e.kind() == std::io::ErrorKind::Other => {}
            _ => unreachable!(),
        }
    };

    Ok(())
}

#[test]
#[cfg(unix)]
fn test_walk_sorted() -> Result<()> {
    let td = cap_tempfile::TempDir::new(cap_std::ambient_authority())?;

    for _ in 0..5 {
        let d = rand::random_range(0..1000);
        let d = format!("dir{d}");
        if td.try_exists(&d)? {
            continue;
        }
        td.create_dir(&d)?;
        for _ in 0..25 {
            let j = rand::random_range(0..10);
            let path = format!("{d}/{j}");
            td.write(&path, DecInt::new(j).as_bytes())?;
        }
    }

    // A successful walk in sorted order
    {
        let mut depth = 0;
        let mut previous: Option<PathBuf> = None;
        // Also test a dummy path prefix, should not affect traversal
        let prefix = Path::new("foo/bar/baz");
        td.walk(
            &WalkConfiguration::default()
                .sort_by_file_name()
                .path_base(prefix),
            |e| -> std::io::Result<_> {
                assert!(e.path.strip_prefix(prefix).is_ok());
                let curdepth = e.path.components().count();
                if curdepth != depth {
                    depth = curdepth;
                    previous = None;
                } else if let Some(previous) = previous.as_deref() {
                    assert_eq!(previous.cmp(e.path), Ordering::Less);
                } else {
                    previous = Some(e.path.to_owned());
                }
                Ok(ControlFlow::Continue(()))
            },
        )
        .unwrap();
    }

    Ok(())
}

#[test]
#[cfg(any(target_os = "android", target_os = "linux"))]
fn test_walk_noxdev() -> Result<()> {
    let rootfs = Dir::open_ambient_dir("/", cap_std::ambient_authority())?;

    let n_root_entries = std::fs::read_dir("/")?.count();
    let mut i = 0;
    rootfs
        .walk(
            &WalkConfiguration::default().noxdev(),
            |e| -> std::io::Result<_> {
                // Don't traverse, unless it's proc...except we shouldn't traverse that either because
                // of noxdev.
                let proc = "proc";
                let first_component = e.path.components().next().unwrap();
                let is_proc = first_component.as_os_str() == OsStr::new(proc);
                let r = if e.file_type.is_dir() {
                    if is_proc {
                        ControlFlow::Continue(())
                    } else {
                        ControlFlow::Break(())
                    }
                } else {
                    ControlFlow::Continue(())
                };
                i += 1;
                Ok(r)
            },
        )
        .unwrap();
    assert_eq!(n_root_entries, i);

    Ok(())
}

#[test]
#[cfg(any(target_os = "android", target_os = "linux"))]
fn test_walk_skip_mountpoints() -> Result<()> {
    let rootfs = Dir::open_ambient_dir("/", cap_std::ambient_authority())?;

    // Count root entries that are mountpoints
    let n_root_entries = std::fs::read_dir("/")?.count();
    let mut n_mountpoints = 0;
    for entry in std::fs::read_dir("/")? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(true) = rootfs.is_mountpoint(entry.file_name())? {
                n_mountpoints += 1;
            }
        }
    }
    assert!(
        n_mountpoints > 0,
        "expected at least one mountpoint under /"
    );

    // Walk with skip_mountpoints; don't traverse into any directories.
    let mut visited = 0;
    rootfs
        .walk(
            &WalkConfiguration::default().skip_mountpoints(),
            |e| -> std::io::Result<_> {
                // Verify this entry is not a mountpoint
                if e.file_type.is_dir() {
                    let is_mp = e.dir.is_mountpoint(e.filename)?;
                    assert_ne!(
                        is_mp,
                        Some(true),
                        "mountpoint {:?} should have been skipped",
                        e.path
                    );
                }
                visited += 1;
                // Don't traverse into subdirectories, but continue iterating siblings
                if e.file_type.is_dir() {
                    Ok(ControlFlow::Break(()))
                } else {
                    Ok(ControlFlow::Continue(()))
                }
            },
        )
        .unwrap();
    // We should see all root entries minus the mountpoints
    assert_eq!(visited, n_root_entries - n_mountpoints);

    Ok(())
}

#[test]
#[cfg(any(target_os = "android", target_os = "linux"))]
fn test_xattrs() -> Result<()> {
    use std::os::unix::ffi::OsStrExt;

    let td = &cap_tempfile::TempDir::new(cap_std::ambient_authority())?;

    assert!(td.getxattr(".", "user.test").unwrap().is_none());
    td.setxattr(".", "user.test", "somevalue").unwrap();
    assert_eq!(
        td.getxattr(".", "user.test").unwrap().unwrap(),
        b"somevalue"
    );
    // Helper fn to list only user. xattrs - filter out especially
    // system xattrs like security.selinux which may be synthesized by the OS
    fn list_user_xattrs(d: &Dir, p: impl AsRef<Path>) -> Vec<std::ffi::OsString> {
        let attrs = d.listxattrs(p).unwrap();
        attrs
            .iter()
            .filter(|k| {
                let Some(k) = k.to_str() else {
                    return false;
                };
                k.split_once('.').unwrap().0 == "user"
            })
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    }
    match list_user_xattrs(td, ".").as_slice() {
        [] => unreachable!(),
        [v] => assert_eq!(v.as_bytes(), b"user.test"),
        _ => unreachable!(),
    };

    // Test multiple xattrs
    td.setxattr(".", "user.othertest", b"anothervalue").unwrap();
    assert_eq!(list_user_xattrs(td, ".").len(), 2);

    // Test operating on a subdirectory file
    td.create_dir_all("foo/bar/baz")?;
    let p = "foo/bar/baz/subfile";
    td.write(p, "afile")?;
    td.setxattr(p, "user.filetest", b"filexattr").unwrap();
    assert_eq!(
        td.getxattr(p, "user.filetest").unwrap().unwrap(),
        b"filexattr"
    );
    match list_user_xattrs(td, p).as_slice() {
        [v] => assert_eq!(v.as_bytes(), b"user.filetest"),
        _ => unreachable!(),
    };

    // Test listing xattrs on a broken symlink. We can't set user.
    // xattrs on a symlink, so we don't test that in unit tests.
    let p = "foo/bar/baz/somelink";
    td.symlink("enoent", p)?;
    assert!(list_user_xattrs(td, p).is_empty());

    Ok(())
}

#[test]
#[cfg(any(target_os = "android", target_os = "linux"))]
fn test_big_xattr() -> Result<()> {
    let td = &cap_tempfile::TempDir::new(cap_std::ambient_authority())?;

    let bigbuf = b"x".repeat(4000);
    td.setxattr(".", "user.test", &bigbuf).unwrap();
    let v = td.getxattr(".", "user.test").unwrap().unwrap();
    assert_eq!(bigbuf.len(), v.len());
    assert_eq!(bigbuf, v);

    Ok(())
}

#[test]
#[cfg(any(target_os = "linux", target_os = "android"))]
fn test_lifecycle_bind_to_parent_thread() -> Result<()> {
    let status = Command::new("true")
        .lifecycle_bind_to_parent_thread()
        .status()?;
    assert!(status.success());

    Ok(())
}

#[test]
#[cfg(not(windows))]
fn test_pass_systemd_fds() -> Result<()> {
    let r = make_pipe("sd-activate-test")?;

    // The child: verify LISTEN_PID matches $$, print the other vars, read fd 3.
    let script = r#"
test "$LISTEN_PID" = "$$" || { echo "LISTEN_PID=$LISTEN_PID but $$=$$" >&2; exit 1; }
printf '%s\n' "$LISTEN_FDS" "$LISTEN_FDNAMES"
cat <&3
"#;
    let fds = CmdFds::new_systemd_fds([(r, SystemdFdName::new("myproto"))]);
    let lines = run_bash_piped(script, |c| {
        c.take_fds(fds);
    })?;
    assert_eq!(lines.len(), 3, "unexpected output: {lines:?}");
    assert_eq!(lines[0], "1");
    assert_eq!(lines[1], "myproto");
    assert_eq!(lines[2], "sd-activate-test");

    Ok(())
}

#[test]
#[cfg(not(windows))]
fn test_systemd_fds_then_take_fd() -> Result<()> {
    // Systemd fds at 3 and 4, then auto-assigned take_fd gets 5.
    let r1 = make_pipe("first")?;
    let r2 = make_pipe("second")?;
    let r_extra = make_pipe("extra")?;

    let script = r#"
printf '%s\n' "$LISTEN_FDS" "$LISTEN_FDNAMES"
cat <&3
printf '\n'
cat <&4
printf '\n'
cat <&5
"#;
    let mut fds = CmdFds::new_systemd_fds([
        (r1, SystemdFdName::new("alpha")),
        (r2, SystemdFdName::new("beta")),
    ]);
    let extra_n = fds.take_fd(r_extra);
    assert_eq!(extra_n, 5);
    let lines = run_bash_piped(script, |c| {
        c.take_fds(fds);
    })?;
    assert_eq!(lines.len(), 5, "unexpected output: {lines:?}");
    assert_eq!(lines[0], "2");
    assert_eq!(lines[1], "alpha:beta");
    assert_eq!(lines[2], "first");
    assert_eq!(lines[3], "second");
    assert_eq!(lines[4], "extra");

    Ok(())
}

#[test]
#[cfg(not(windows))]
fn test_cmd_fds_take_fd_n_then_systemd() -> Result<()> {
    // Reserve fd 10 explicitly, then systemd fds at 3 — no conflict.
    let r_explicit = make_pipe("explicit")?;
    let r_sd = make_pipe("systemd")?;

    let script = r#"
cat <&10
printf '\n'
printf '%s\n' "$LISTEN_FDS" "$LISTEN_FDNAMES"
cat <&3
"#;
    let mut fds = CmdFds::new_systemd_fds([(r_sd, SystemdFdName::new("varlink"))]);
    fds.take_fd_n(r_explicit, 10);
    let lines = run_bash_piped(script, |c| {
        c.take_fds(fds);
    })?;
    assert_eq!(lines.len(), 4, "unexpected output: {lines:?}");
    assert_eq!(lines[0], "explicit");
    assert_eq!(lines[1], "1");
    assert_eq!(lines[2], "varlink");
    assert_eq!(lines[3], "systemd");

    Ok(())
}

/// Try to dup `fd` so its raw fd number equals `raw`.
/// Returns `Ok(Some(dup))` on success, `Ok(None)` if the slot is occupied.
/// Uses F_DUPFD starting at `raw`; if the kernel returns a higher number,
/// we close it and give up rather than fighting for the slot.
#[cfg(not(windows))]
fn try_fd_at_raw(fd: &rustix::fd::OwnedFd, raw: i32) -> Result<Option<rustix::fd::OwnedFd>> {
    let dup = rustix::io::fcntl_dupfd_cloexec(fd, raw)?;
    if dup.as_raw_fd() == raw {
        Ok(Some(dup))
    } else {
        // Slot was occupied; drop the dup (closes it) and signal failure.
        Ok(None)
    }
}

#[cfg(not(windows))]
mod proptest_fd {
    use super::*;
    use cap_std_ext::cmdext::{CapStdExtCommandExt, CmdFds};
    use proptest::prelude::*;

    /// Generate a Vec of distinct target fd numbers in [3, 15].
    fn fd_targets(max_count: usize) -> impl Strategy<Value = Vec<i32>> {
        proptest::collection::btree_set(3..=15i32, 1..=max_count)
            .prop_map(|s| s.into_iter().collect::<Vec<_>>())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(80))]

        /// For each generated set of target fds, create pipes with unique
        /// content, force some source fds into positions that collide with
        /// other targets (the worst case), then verify every target gets
        /// the right content through the child process.
        #[test]
        fn fd_shuffle_no_clobber(targets in fd_targets(5)) {
            // Build pipes with content "fdN" for each target N.
            let mut entries: Vec<(i32, Arc<rustix::fd::OwnedFd>)> = Vec::new();

            for (i, &tgt) in targets.iter().enumerate() {
                let pipe_r = make_pipe(&format!("fd{tgt}")).unwrap();

                // For odd-indexed entries, try to force the source fd's raw
                // number to equal the *next* entry's target (wrapping around).
                // This manufactures exactly the collision that broke CI.
                if i % 2 == 1 {
                    let collide_with = targets[(i + 1) % targets.len()];
                    if let Ok(Some(moved)) = try_fd_at_raw(&pipe_r, collide_with) {
                        entries.push((tgt, Arc::new(moved)));
                        continue;
                    }
                }
                entries.push((tgt, pipe_r));
            }

            // Build a bash script that reads from each target fd and prints
            // "targetN:content".
            let mut script = String::new();
            for &tgt in &targets {
                script.push_str(&format!(
                    "printf '{tgt}:'; cat <&{tgt}; printf '\\n'\n"
                ));
            }

            let mut fds = CmdFds::new();
            for (tgt, fd) in entries {
                fds.take_fd_n(fd, tgt);
            }
            let lines = run_bash_piped(&script, |c| { c.take_fds(fds); }).unwrap();

            // Verify each line is "N:fdN".
            let n_targets = targets.len();
            prop_assert_eq!(
                lines.len(), n_targets,
                "expected {} lines, got {}: {:?}", n_targets, lines.len(), lines
            );
            for (line, &tgt) in lines.iter().zip(targets.iter()) {
                let expected = format!("{tgt}:fd{tgt}");
                prop_assert_eq!(
                    line, &expected,
                    "wrong content for target fd {}", tgt
                );
            }
        }
    }
}

#[test]
#[cfg(not(windows))]
#[should_panic(expected = "fd 3 is already assigned")]
fn test_cmd_fds_overlap_panics() {
    let (r1, _w1) = rustix::pipe::pipe().unwrap();
    let (r2, _w2) = rustix::pipe::pipe().unwrap();
    let r1 = Arc::new(r1);
    let r2 = Arc::new(r2);

    let mut fds = CmdFds::new_systemd_fds([(r1, SystemdFdName::new("x"))]);
    // This should panic: fd 3 is already taken by systemd.
    fds.take_fd_n(r2, 3);
}
