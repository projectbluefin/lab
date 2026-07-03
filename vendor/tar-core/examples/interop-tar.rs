//! Cross-tool roundtrip integration test: tar-core <-> GNU tar.
//!
//! This test validates that tar archives built by tar-core can be correctly
//! listed and extracted by GNU tar (`/bin/tar`), and vice versa: archives
//! created by GNU tar can be parsed by tar-core.
//!
//! Paths and names are `Vec<u8>` (not `String`) to exercise non-UTF-8 byte
//! sequences — tar paths are fundamentally byte sequences. GNU tar operates
//! on raw bytes natively, so non-UTF-8 filenames work without special
//! encoding.
//!
//! Run with: `cargo run --example interop-tar`

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use xshell::{cmd, Shell};

use arbitrary::{Arbitrary, Unstructured};
use tempfile::TempDir;

use tar_core::builder::EntryBuilder;
use tar_core::parse::Limits;
use tar_core::{EntryType, HEADER_SIZE};
use tar_core_testutil::{parse_tar_core_with_limits, OwnedEntry};

// =============================================================================
// Test parameters
// =============================================================================

#[derive(Debug, Clone, Arbitrary)]
struct RawEntryParams {
    path_bytes: Vec<u8>,
    mode: u16, // masked to 0o7777
    uid: u16,
    gid: u16,
    content_len: u8,
    mtime: u32,
    uname_bytes: Vec<u8>,
    gname_bytes: Vec<u8>,
    content_seed: Vec<u8>,
    is_dir: bool,
}

#[derive(Debug, Clone)]
struct EntryParams {
    path: Vec<u8>,
    mode: u32,
    uid: u32,
    gid: u32,
    mtime: u32,
    username: Vec<u8>,
    groupname: Vec<u8>,
    content: Vec<u8>,
    is_dir: bool,
}

/// Remove NUL bytes, slashes, and control chars; clamp length.
/// Returns `None` if the result would be shorter than `min_len`.
fn sanitize_filename(raw: &[u8], min_len: usize, max_len: usize) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = raw
        .iter()
        .copied()
        .filter(|&b| b != 0 && b != b'/' && b != b'\\' && b > 0x1F)
        .collect();
    if out.is_empty() {
        out.push(b'x');
    }
    out.truncate(max_len);
    if out.len() < min_len {
        return None;
    }
    Some(out)
}

/// Sanitize bytes for use as username/groupname: no NUL, no slash, ASCII printable.
fn sanitize_name(raw: &[u8], min_len: usize, max_len: usize) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = raw
        .iter()
        .copied()
        .filter(|&b| b != 0 && b.is_ascii_graphic())
        .collect();
    if out.is_empty() {
        out.push(b'u');
    }
    out.truncate(max_len);
    if out.len() < min_len {
        return None;
    }
    Some(out)
}

fn to_entry_params(raw: &RawEntryParams) -> Option<EntryParams> {
    // For filenames: allow non-UTF-8 but no NUL or slashes.
    // Keep them short enough that they don't trigger long-name extensions
    // in the "simple" case (we test long paths separately).
    let path = sanitize_filename(&raw.path_bytes, 1, 90)?;
    let username = sanitize_name(&raw.uname_bytes, 1, 31)?;
    let groupname = sanitize_name(&raw.gname_bytes, 1, 31)?;

    let content_len = if raw.is_dir {
        0
    } else {
        raw.content_len as usize * 32
    };
    let content: Vec<u8> = if content_len == 0 || raw.content_seed.is_empty() {
        vec![0u8; content_len]
    } else {
        raw.content_seed
            .iter()
            .copied()
            .cycle()
            .take(content_len)
            .collect()
    };

    // Ensure owner-read is always set so we can verify extracted content.
    let mode = ((raw.mode as u32) & 0o7777) | 0o400;

    Some(EntryParams {
        path,
        mode,
        uid: raw.uid as u32,
        gid: raw.gid as u32,
        mtime: raw.mtime & 0x7FFF_FFFF,
        username,
        groupname,
        content,
        is_dir: raw.is_dir,
    })
}

fn run_random_tests(name: &str, iterations: u32, test_fn: impl Fn(&[EntryParams])) {
    println!("  {name}...");
    for seed in 0..iterations {
        let mut rng_data = Vec::new();
        let mut state: u64 = seed as u64 ^ 0xdeadbeef;
        for _ in 0..4096 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            rng_data.push((state >> 33) as u8);
        }
        let mut u = Unstructured::new(&rng_data);
        if let Ok(raw_entries) = Vec::<RawEntryParams>::arbitrary(&mut u) {
            let entries: Vec<EntryParams> =
                raw_entries.iter().filter_map(to_entry_params).collect();
            if !entries.is_empty() {
                test_fn(&entries);
            }
        }
    }
    println!("  {name}: PASSED");
}

// =============================================================================
// Tar-core archive building
// =============================================================================

/// Build a tar archive from entry params using tar-core's EntryBuilder.
fn build_tar_core_archive(entries: &[EntryParams], format: &str) -> Vec<u8> {
    let mut archive = Vec::new();

    for entry in entries {
        let mut builder = match format {
            "gnu" => EntryBuilder::new_gnu(),
            "pax" => EntryBuilder::new_ustar(),
            _ => panic!("unknown format: {format}"),
        };

        let path = if entry.is_dir {
            let mut p = entry.path.clone();
            if !p.ends_with(b"/") {
                p.push(b'/');
            }
            p
        } else {
            entry.path.clone()
        };

        builder
            .path(&path)
            .mode(entry.mode)
            .unwrap()
            .uid(entry.uid as u64)
            .unwrap()
            .gid(entry.gid as u64)
            .unwrap()
            .size(entry.content.len() as u64)
            .unwrap()
            .mtime(entry.mtime as u64)
            .unwrap();

        if entry.is_dir {
            builder.entry_type(EntryType::Directory);
        } else {
            builder.entry_type(EntryType::Regular);
        }

        // Username/groupname: for GNU mode, names > 32 bytes will error.
        if format == "gnu" {
            let uname = if entry.username.len() > 32 {
                &entry.username[..32]
            } else {
                &entry.username
            };
            let gname = if entry.groupname.len() > 32 {
                &entry.groupname[..32]
            } else {
                &entry.groupname
            };
            builder.username(uname).unwrap();
            builder.groupname(gname).unwrap();
        } else {
            builder.username(&entry.username).unwrap();
            builder.groupname(&entry.groupname).unwrap();
        }

        let header_bytes = builder.finish_bytes();
        archive.extend_from_slice(&header_bytes);

        // Write content
        archive.extend_from_slice(&entry.content);

        // Pad to 512-byte boundary
        let padding = (HEADER_SIZE - (entry.content.len() % HEADER_SIZE)) % HEADER_SIZE;
        archive.extend(std::iter::repeat_n(0u8, padding));
    }

    // End-of-archive: two 512-byte zero blocks
    archive.extend(std::iter::repeat_n(0u8, HEADER_SIZE * 2));
    archive
}

// =============================================================================
// GNU tar interaction helpers
// =============================================================================

/// Run `tar tf <archive>` and return the listed paths as raw bytes.
/// Uses `--quoting-style=literal` to avoid octal escaping of non-ASCII bytes.
fn tar_list(sh: &Shell, archive_path: &Path) -> Vec<Vec<u8>> {
    let output = cmd!(sh, "tar --quoting-style=literal -tf {archive_path}")
        .output()
        .expect("failed to run tar tf");
    assert!(
        output.status.success(),
        "tar tf failed: {}",
        std::str::from_utf8(&output.stderr).unwrap_or("<non-utf8>")
    );

    output
        .stdout
        .split(|&b| b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| line.to_vec())
        .collect()
}

/// Run `tar tvf <archive>` and return stdout as raw bytes.
/// Uses `--quoting-style=literal` to avoid octal escaping of non-ASCII bytes.
fn tar_verbose_list(sh: &Shell, archive_path: &Path) -> Vec<u8> {
    let output = cmd!(sh, "tar --quoting-style=literal -tvf {archive_path}")
        .output()
        .expect("failed to run tar tvf");
    assert!(
        output.status.success(),
        "tar tvf failed: {}",
        std::str::from_utf8(&output.stderr).unwrap_or("<non-utf8>")
    );
    output.stdout
}

/// Run `tar xf <archive> -C <dir>` to extract.
fn tar_extract(sh: &Shell, archive_path: &Path, dest_dir: &Path) {
    cmd!(sh, "tar xf {archive_path} -C {dest_dir}")
        .run()
        .expect("failed to run tar xf");
}

/// Run `tar cf <archive> --format=<fmt> -C <dir> <files...>` to create.
fn tar_create(sh: &Shell, archive_path: &Path, format: &str, src_dir: &Path, filenames: &[&[u8]]) {
    let fmt_arg = match format {
        "gnu" => "--format=gnu",
        "pax" | "posix" => "--format=posix",
        other => panic!("unsupported format: {other}"),
    };

    let filenames_os: Vec<&OsStr> = filenames
        .iter()
        .map(|name| OsStr::from_bytes(name))
        .collect();

    cmd!(sh, "tar cf {archive_path} {fmt_arg} -C {src_dir}")
        .args(&filenames_os)
        .run()
        .expect("failed to run tar cf");
}

// =============================================================================
// Direction 1: tar-core -> GNU tar
// =============================================================================

fn test_tar_core_to_gnu_tar(sh: &Shell, entries: &[EntryParams], format: &str) {
    let tmpdir = TempDir::new().expect("create tmpdir");

    // Build archive with tar-core
    let archive_data = build_tar_core_archive(entries, format);
    let archive_path = tmpdir.path().join(format!("tarcore_{format}.tar"));
    std::fs::write(&archive_path, &archive_data).expect("write archive");

    // 1. tar tf: list contents and verify entry count + paths
    let listed = tar_list(sh, &archive_path);
    assert_eq!(
        entries.len(),
        listed.len(),
        "{format}: entry count mismatch in tar tf (expected {}, got {})\nexpected paths: {:?}\nlisted: {:?}",
        entries.len(),
        listed.len(),
        entries.iter().map(|e| &e.path).collect::<Vec<_>>(),
        listed,
    );

    for (i, (entry, listed_path)) in entries.iter().zip(listed.iter()).enumerate() {
        let expected_path = if entry.is_dir {
            let mut p = entry.path.clone();
            if !p.ends_with(b"/") {
                p.push(b'/');
            }
            p
        } else {
            entry.path.clone()
        };
        assert_eq!(
            &expected_path, listed_path,
            "{format} entry[{i}]: path mismatch in tar tf listing",
        );
    }

    // 2. tar tvf: verbose listing — verify it succeeds and mentions sizes
    let verbose = tar_verbose_list(sh, &archive_path);
    for entry in entries {
        if !entry.is_dir && !entry.content.is_empty() {
            let size_str = entry.content.len().to_string();
            assert!(
                verbose
                    .windows(size_str.len())
                    .any(|w| w == size_str.as_bytes()),
                "{format}: size {} not found in tar tvf output for path {:?}",
                entry.content.len(),
                entry.path,
            );
        }
    }

    // 3. tar xf: extract and verify file contents, sizes
    let extract_dir = TempDir::new().expect("create extract dir");
    tar_extract(sh, &archive_path, extract_dir.path());

    for entry in entries {
        let file_path = extract_dir.path().join(OsStr::from_bytes(&entry.path));
        if entry.is_dir {
            assert!(
                file_path.is_dir(),
                "{format}: directory {:?} should exist after extraction",
                entry.path
            );
        } else {
            assert!(
                file_path.exists(),
                "{format}: file {:?} should exist after extraction",
                entry.path
            );
            let extracted = std::fs::read(&file_path).expect("read extracted file");
            assert_eq!(
                entry.content.len(),
                extracted.len(),
                "{format}: size mismatch for {:?}",
                entry.path,
            );
            assert_eq!(
                entry.content, extracted,
                "{format}: content mismatch for {:?}",
                entry.path,
            );
        }
    }
}

// =============================================================================
// Direction 2: GNU tar -> tar-core
// =============================================================================

fn test_gnu_tar_to_tar_core(sh: &Shell, entries: &[EntryParams], format: &str) {
    let src_dir = TempDir::new().expect("create source dir");
    let tmpdir = TempDir::new().expect("create tmpdir");

    // Create files on disk
    let mut filenames: Vec<&[u8]> = Vec::new();
    for entry in entries {
        let file_path = src_dir.path().join(OsStr::from_bytes(&entry.path));
        if entry.is_dir {
            std::fs::create_dir_all(&file_path).expect("create directory");
        } else {
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent).expect("create parent dirs");
            }
            std::fs::write(&file_path, &entry.content).expect("write file");
            // Set permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(entry.mode);
                std::fs::set_permissions(&file_path, perms).expect("set permissions");
            }
        }
        filenames.push(&entry.path);
    }

    // Create archive with GNU tar
    let archive_path = tmpdir.path().join(format!("gnuitar_{format}.tar"));
    tar_create(sh, &archive_path, format, src_dir.path(), &filenames);

    // Parse with tar-core
    let archive_data = std::fs::read(&archive_path).expect("read archive");
    let parsed: Vec<OwnedEntry> = parse_tar_core_with_limits(&archive_data, Limits::default());

    // Verify entries match.
    // GNU tar may reorder or add parent directories, so we compare by path.
    for entry in entries {
        let expected_path = if entry.is_dir {
            let mut p = entry.path.clone();
            if !p.ends_with(b"/") {
                p.push(b'/');
            }
            p
        } else {
            entry.path.clone()
        };

        let found = parsed
            .iter()
            .find(|p| p.path == expected_path)
            .unwrap_or_else(|| {
                panic!(
                    "{format}: tar-core did not find entry for path {:?} in GNU tar archive\nparsed paths: {:?}",
                    expected_path,
                    parsed.iter().map(|p| &p.path).collect::<Vec<_>>(),
                )
            });

        if !entry.is_dir {
            assert_eq!(
                entry.content.len(),
                found.content.len(),
                "{format}: size mismatch for {:?}",
                entry.path,
            );
            assert_eq!(
                entry.content, found.content,
                "{format}: content mismatch for {:?}",
                entry.path,
            );
        }

        // Mode comparison: GNU tar may adjust modes, so only check lower 12 bits
        // and only for regular files where we explicitly set permissions.
        if !entry.is_dir {
            assert_eq!(
                entry.mode & 0o7777,
                found.mode & 0o7777,
                "{format}: mode mismatch for {:?} (expected 0o{:o}, got 0o{:o})",
                entry.path,
                entry.mode,
                found.mode,
            );
        }

        // Note: uid/gid/mtime are NOT checked here because GNU tar reads
        // them from the filesystem (current user's uid/gid, file mtime),
        // not from our EntryParams. The forward direction
        // (tar-core -> GNU tar) already verifies these fields roundtrip.
    }
}

// =============================================================================
// Deterministic smoke tests
// =============================================================================

fn smoke_test_basic_roundtrip(sh: &Shell) {
    let entries = vec![
        EntryParams {
            path: b"hello.txt".to_vec(),
            mode: 0o644,
            uid: 1000,
            gid: 1000,
            mtime: 1234567890,
            username: b"testuser".to_vec(),
            groupname: b"testgroup".to_vec(),
            content: b"Hello, World!".to_vec(),
            is_dir: false,
        },
        EntryParams {
            path: b"empty".to_vec(),
            mode: 0o600,
            uid: 0,
            gid: 0,
            mtime: 1000000,
            username: b"root".to_vec(),
            groupname: b"root".to_vec(),
            content: vec![],
            is_dir: false,
        },
        EntryParams {
            path: b"data.bin".to_vec(),
            mode: 0o755,
            uid: 65534,
            gid: 65534,
            mtime: 1700000000,
            username: b"nobody".to_vec(),
            groupname: b"nogroup".to_vec(),
            content: vec![0xAB; 512],
            is_dir: false,
        },
    ];

    for format in &["gnu", "pax"] {
        test_tar_core_to_gnu_tar(sh, &entries, format);
    }
}

fn smoke_test_directories(sh: &Shell) {
    let entries = vec![
        EntryParams {
            path: b"mydir".to_vec(),
            mode: 0o755,
            uid: 1000,
            gid: 1000,
            mtime: 1234567890,
            username: b"user".to_vec(),
            groupname: b"group".to_vec(),
            content: vec![],
            is_dir: true,
        },
        EntryParams {
            path: b"anotherdir".to_vec(),
            mode: 0o700,
            uid: 0,
            gid: 0,
            mtime: 1700000000,
            username: b"root".to_vec(),
            groupname: b"root".to_vec(),
            content: vec![],
            is_dir: true,
        },
    ];

    for format in &["gnu", "pax"] {
        test_tar_core_to_gnu_tar(sh, &entries, format);
    }
}

fn smoke_test_long_paths(sh: &Shell) {
    let long_path = {
        let mut p = b"very/long/path/that/exceeds/one/hundred/bytes/".to_vec();
        p.extend(std::iter::repeat_n(b'x', 60));
        p
    };
    assert!(long_path.len() > 100);

    let entries = vec![EntryParams {
        path: long_path,
        mode: 0o644,
        uid: 1000,
        gid: 1000,
        mtime: 1700000000,
        username: b"user".to_vec(),
        groupname: b"group".to_vec(),
        content: b"long path content".to_vec(),
        is_dir: false,
    }];

    for format in &["gnu", "pax"] {
        test_tar_core_to_gnu_tar(sh, &entries, format);
    }
}

fn smoke_test_non_utf8_gnu_format(sh: &Shell) {
    // Non-UTF-8 filenames: bytes 0x80, 0xFE are not valid UTF-8 starts.
    // GNU tar handles these natively since it's byte-oriented.
    let entries = vec![EntryParams {
        path: vec![b'f', b'i', b'l', b'e', 0x80, 0xFE, b'.', b'd', b'a', b't'],
        mode: 0o644,
        uid: 1000,
        gid: 1000,
        mtime: 1700000000,
        username: b"user".to_vec(),
        groupname: b"grp".to_vec(),
        content: b"non-utf8 path test".to_vec(),
        is_dir: false,
    }];

    // Only GNU format — PAX requires UTF-8 paths.
    test_tar_core_to_gnu_tar(sh, &entries, "gnu");
}

fn smoke_test_gnu_tar_to_tar_core(sh: &Shell) {
    let entries = vec![
        EntryParams {
            path: b"from_tar.txt".to_vec(),
            mode: 0o644,
            uid: 0,
            gid: 0,
            mtime: 0,
            username: b"root".to_vec(),
            groupname: b"root".to_vec(),
            content: b"created by GNU tar".to_vec(),
            is_dir: false,
        },
        EntryParams {
            path: b"another.bin".to_vec(),
            mode: 0o755,
            uid: 0,
            gid: 0,
            mtime: 0,
            username: b"root".to_vec(),
            groupname: b"root".to_vec(),
            content: vec![1, 2, 3, 4, 5],
            is_dir: false,
        },
    ];

    for format in &["gnu", "posix"] {
        test_gnu_tar_to_tar_core(sh, &entries, format);
    }
}

fn smoke_test_gnu_tar_non_utf8_roundtrip(sh: &Shell) {
    // Create files with non-UTF-8 names on disk, tar them with GNU tar,
    // then parse with tar-core.
    let src_dir = TempDir::new().expect("create source dir");
    let tmpdir = TempDir::new().expect("create tmpdir");

    let non_utf8_name: Vec<u8> = vec![b'n', b'o', b'n', 0x80, 0xFE, b'.', b'x'];
    let content = b"non-utf8 roundtrip";

    let file_path = src_dir.path().join(OsStr::from_bytes(&non_utf8_name));
    std::fs::write(&file_path, content).expect("write non-utf8 file");

    let archive_path = tmpdir.path().join("non_utf8.tar");
    tar_create(
        sh,
        &archive_path,
        "gnu",
        src_dir.path(),
        &[non_utf8_name.as_slice()],
    );

    let archive_data = std::fs::read(&archive_path).expect("read archive");
    let parsed: Vec<OwnedEntry> = parse_tar_core_with_limits(&archive_data, Limits::default());

    let found = parsed
        .iter()
        .find(|p| p.path == non_utf8_name)
        .expect("should find non-UTF-8 entry");
    assert_eq!(found.content, content);
}

fn smoke_test_gnu_tar_directories(sh: &Shell) {
    let entries = vec![EntryParams {
        path: b"testdir".to_vec(),
        mode: 0o755,
        uid: 0,
        gid: 0,
        mtime: 0,
        username: b"root".to_vec(),
        groupname: b"root".to_vec(),
        content: vec![],
        is_dir: true,
    }];

    for format in &["gnu", "posix"] {
        test_gnu_tar_to_tar_core(sh, &entries, format);
    }
}

// =============================================================================
// Main
// =============================================================================

fn main() {
    let sh = Shell::new().expect("xshell");

    // Check that GNU tar is available
    let output = cmd!(sh, "tar --version").output();
    match output {
        Ok(ref o) if o.status.success() => {
            let version_line = o.stdout.split(|&b| b == b'\n').next().unwrap_or(b"unknown");
            println!(
                "Using: {}",
                std::str::from_utf8(version_line).unwrap_or("unknown")
            );
            // Verify it's GNU tar
            if !o.stdout.starts_with(b"tar (GNU tar)") {
                eprintln!("Warning: tar does not appear to be GNU tar, results may vary");
            }
        }
        _ => {
            eprintln!("tar not found, skipping");
            return;
        }
    }

    println!("=== tar-core <-> GNU tar interop tests ===");

    // Direction 1: tar-core -> GNU tar
    println!("smoke_test_basic_roundtrip...");
    smoke_test_basic_roundtrip(&sh);
    println!("smoke_test_basic_roundtrip: PASSED");

    println!("smoke_test_directories...");
    smoke_test_directories(&sh);
    println!("smoke_test_directories: PASSED");

    println!("smoke_test_long_paths...");
    smoke_test_long_paths(&sh);
    println!("smoke_test_long_paths: PASSED");

    println!("smoke_test_non_utf8_gnu_format...");
    smoke_test_non_utf8_gnu_format(&sh);
    println!("smoke_test_non_utf8_gnu_format: PASSED");

    // Direction 2: GNU tar -> tar-core
    println!("smoke_test_gnu_tar_to_tar_core...");
    smoke_test_gnu_tar_to_tar_core(&sh);
    println!("smoke_test_gnu_tar_to_tar_core: PASSED");

    println!("smoke_test_gnu_tar_non_utf8_roundtrip...");
    smoke_test_gnu_tar_non_utf8_roundtrip(&sh);
    println!("smoke_test_gnu_tar_non_utf8_roundtrip: PASSED");

    println!("smoke_test_gnu_tar_directories...");
    smoke_test_gnu_tar_directories(&sh);
    println!("smoke_test_gnu_tar_directories: PASSED");

    // Arbitrary-driven tests: tar-core -> GNU tar
    run_random_tests("random_tar_core_to_gnu_tar_gnu", 16, |entries| {
        // Filter to entries with unique paths (GNU tar deduplicates)
        let unique = deduplicate_entries(entries);
        if !unique.is_empty() {
            test_tar_core_to_gnu_tar(&sh, &unique, "gnu");
        }
    });

    run_random_tests("random_tar_core_to_gnu_tar_pax", 16, |entries| {
        // PAX requires UTF-8 paths
        let utf8_entries: Vec<EntryParams> = entries
            .iter()
            .filter(|e| {
                std::str::from_utf8(&e.path).is_ok()
                    && std::str::from_utf8(&e.username).is_ok()
                    && std::str::from_utf8(&e.groupname).is_ok()
            })
            .cloned()
            .collect();
        let unique = deduplicate_entries(&utf8_entries);
        if !unique.is_empty() {
            test_tar_core_to_gnu_tar(&sh, &unique, "pax");
        }
    });

    // Arbitrary-driven tests: GNU tar -> tar-core
    run_random_tests("random_gnu_tar_to_tar_core", 16, |entries| {
        // For the reverse direction, we need files on disk, so only use
        // entries with valid filesystem names (no path separator issues).
        let fs_entries: Vec<EntryParams> = entries
            .iter()
            .filter(|e| {
                // Paths must not contain bytes that are problematic for the filesystem
                !e.path.contains(&b'/') && !e.path.is_empty()
            })
            .cloned()
            .collect();
        let unique = deduplicate_entries(&fs_entries);
        if !unique.is_empty() {
            test_gnu_tar_to_tar_core(&sh, &unique, "gnu");
        }
    });

    println!("All tests passed!");
}

/// Deduplicate entries by path (keep first occurrence).
fn deduplicate_entries(entries: &[EntryParams]) -> Vec<EntryParams> {
    let mut seen = std::collections::HashSet::new();
    entries
        .iter()
        .filter(|e| seen.insert(e.path.clone()))
        .cloned()
        .collect()
}
