//! Cross-language roundtrip integration test: tar-core <-> Python tarfile.
//!
//! This test validates that tar archives built by tar-core can be correctly
//! parsed by Python's `tarfile` module, and vice versa. It uses the `arbitrary`
//! crate to generate random entry parameters and tests both GNU and PAX
//! extension modes.
//!
//! Paths and names are `Vec<u8>` (not `String`) to exercise non-UTF-8 byte
//! sequences — tar paths are fundamentally byte sequences.  The JSON protocol
//! base64-encodes path, uname, and gname fields.
//!
//! Run with: `cargo run --example interop-python`

use arbitrary::{Arbitrary, Unstructured};
use base64::Engine;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use xshell::{cmd, Shell};

use tar_core::builder::EntryBuilder;
use tar_core::parse::{Limits, ParseEvent, Parser};
use tar_core::{EntryType, HEADER_SIZE};

// =============================================================================
// Python helper script (embedded)
// =============================================================================

const PYTHON_HELPER: &str = r#"
import json, sys, tarfile, base64, io, os

def _encoding_for_format(fmt):
    """GNU format stores raw bytes; use latin-1 (1:1 byte mapping).
    PAX format encodes metadata as UTF-8 per spec; use utf-8."""
    return "utf-8" if fmt == "pax" else "latin-1"

def _detect_format(tar_path):
    """Peek at the archive to decide gnu vs pax encoding."""
    with tarfile.open(tar_path, "r:") as tf:
        for m in tf.getmembers():
            if m.pax_headers:
                return "pax"
        return "gnu"

def parse_tar(tar_path):
    fmt = _detect_format(tar_path)
    enc = _encoding_for_format(fmt)
    entries = []
    with tarfile.open(tar_path, "r:", encoding=enc) as tf:
        for member in tf.getmembers():
            path_bytes = member.name.encode(enc)
            uname_bytes = member.uname.encode(enc)
            gname_bytes = member.gname.encode(enc)
            entry = {
                "path": base64.b64encode(path_bytes).decode("ascii"),
                "mode": member.mode,
                "uid": member.uid,
                "gid": member.gid,
                "size": member.size,
                "mtime": member.mtime,
                "uname": base64.b64encode(uname_bytes).decode("ascii"),
                "gname": base64.b64encode(gname_bytes).decode("ascii"),
                "content": "",
            }
            if member.isreg() and member.size > 0:
                f = tf.extractfile(member)
                if f is not None:
                    entry["content"] = base64.b64encode(f.read()).decode("ascii")
            entries.append(entry)
    return entries

def generate_tar(tar_path, fmt, entries):
    fmt_map = {"gnu": tarfile.GNU_FORMAT, "pax": tarfile.PAX_FORMAT}
    tar_fmt = fmt_map[fmt]
    enc = _encoding_for_format(fmt)
    with tarfile.open(tar_path, "w:", format=tar_fmt, encoding=enc) as tf:
        for e in entries:
            path_bytes = base64.b64decode(e["path"])
            uname_bytes = base64.b64decode(e["uname"])
            gname_bytes = base64.b64decode(e["gname"])
            info = tarfile.TarInfo(name=path_bytes.decode(enc))
            info.mode = e["mode"]
            info.uid = e["uid"]
            info.gid = e["gid"]
            info.size = e["size"]
            info.mtime = e["mtime"]
            info.uname = uname_bytes.decode(enc)
            info.gname = gname_bytes.decode(enc)
            info.type = tarfile.REGTYPE
            content = base64.b64decode(e["content"])
            assert len(content) == e["size"], f"content length {len(content)} != size {e['size']}"
            tf.addfile(info, io.BytesIO(content))

cmd = json.loads(sys.stdin.read())
if cmd["mode"] == "parse":
    result = parse_tar(cmd["tar_path"])
    print(json.dumps(result))
elif cmd["mode"] == "generate":
    generate_tar(cmd["tar_path"], cmd["format"], cmd["entries"])
    print(json.dumps({"ok": True}))
else:
    print(json.dumps({"error": "unknown mode"}), file=sys.stderr)
    sys.exit(1)
"#;

// =============================================================================
// JSON types for communication with Python
// =============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TarEntryJson {
    path: String, // base64-encoded bytes
    mode: u32,
    uid: u64,
    gid: u64,
    size: u64,
    mtime: u64,
    uname: String,   // base64-encoded bytes
    gname: String,   // base64-encoded bytes
    content: String, // base64-encoded bytes
}

#[derive(Debug, Serialize)]
struct ParseCommand {
    mode: &'static str,
    tar_path: String,
}

#[derive(Debug, Serialize)]
struct GenerateCommand {
    mode: &'static str,
    tar_path: String,
    format: String,
    entries: Vec<TarEntryJson>,
}

// =============================================================================
// Test parameters
// =============================================================================

#[derive(Debug, Clone, Arbitrary)]
struct RawEntryParams {
    path_bytes: Vec<u8>,
    mode: u16, // will mask to 0o7777
    uid: u16,
    gid: u16,
    content_len: u8, // 0..255, scaled to reasonable size
    mtime: u32,
    uname_bytes: Vec<u8>,
    gname_bytes: Vec<u8>,
    content_seed: Vec<u8>,
}

#[derive(Debug, Clone)]
struct EntryParams {
    path: Vec<u8>,
    mode: u32,
    uid: u64,
    gid: u64,
    mtime: u64,
    username: Vec<u8>,
    groupname: Vec<u8>,
    content: Vec<u8>,
}

/// Remove NUL bytes and clamp length.  Returns `None` if the result would be
/// shorter than `min_len`.
fn sanitize_bytes(raw: &[u8], min_len: usize, max_len: usize) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = raw.iter().copied().filter(|&b| b != 0).collect();
    if out.is_empty() {
        out.push(b'x');
    }
    out.truncate(max_len);
    if out.len() < min_len {
        return None;
    }
    Some(out)
}

/// Convert arbitrary raw params into valid `EntryParams`, or `None` if the
/// random data can't produce valid inputs.
fn to_entry_params(raw: &RawEntryParams) -> Option<EntryParams> {
    let path = sanitize_bytes(&raw.path_bytes, 1, 200)?;
    let username = sanitize_bytes(&raw.uname_bytes, 1, 64)?;
    let groupname = sanitize_bytes(&raw.gname_bytes, 1, 64)?;

    // Build content — use content_seed repeated/truncated to content_len
    let content_len = raw.content_len as usize * 32; // 0..8160
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

    Some(EntryParams {
        path,
        mode: (raw.mode as u32) & 0o7777,
        uid: raw.uid as u64,
        gid: raw.gid as u64,
        mtime: raw.mtime as u64,
        username,
        groupname,
        content,
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
// Tar building helpers
// =============================================================================

/// Build a tar archive from entry params using tar-core's EntryBuilder.
fn build_tar_core_archive(entries: &[EntryParams], use_pax: bool) -> Vec<u8> {
    let mut archive = Vec::new();

    for entry in entries {
        let mut builder = if use_pax {
            EntryBuilder::new_ustar()
        } else {
            EntryBuilder::new_gnu()
        };

        builder
            .path(&entry.path)
            .mode(entry.mode)
            .expect("mode fits")
            .uid(entry.uid)
            .expect("uid fits")
            .gid(entry.gid)
            .expect("gid fits")
            .size(entry.content.len() as u64)
            .expect("size fits")
            .mtime(entry.mtime)
            .expect("mtime fits")
            .entry_type(EntryType::Regular);

        // Username/groupname: for PAX mode, long names get stored as PAX
        // extensions automatically. For GNU mode, names must fit in 32 bytes.
        if use_pax {
            builder
                .username(&entry.username)
                .expect("pax handles overflow");
            builder
                .groupname(&entry.groupname)
                .expect("pax handles overflow");
        } else if entry.username.len() <= 32 && entry.groupname.len() <= 32 {
            builder.username(&entry.username).expect("fits in gnu");
            builder.groupname(&entry.groupname).expect("fits in gnu");
        }
        // else: skip username/groupname for GNU mode with long names

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

/// Parse a tar archive using tar-core's sans-IO Parser, returning metadata
/// and content for each entry.
fn parse_with_tar_core(data: &[u8]) -> Vec<EntryParams> {
    let mut parser = Parser::new(Limits::default());
    let mut results = Vec::new();
    let mut offset = 0;

    loop {
        let input = &data[offset..];
        match parser.parse(input).expect("parse should succeed") {
            ParseEvent::NeedData { .. } => {
                panic!("unexpected NeedData: archive should be complete in memory");
            }
            ParseEvent::Entry { consumed, entry } => {
                offset += consumed;

                let size = entry.size as usize;
                let path = entry.path.to_vec();
                let uname = entry.uname.as_ref().map(|u| u.to_vec()).unwrap_or_default();
                let gname = entry.gname.as_ref().map(|g| g.to_vec()).unwrap_or_default();

                // Read content
                let content = data[offset..offset + size].to_vec();
                let padded = size.next_multiple_of(HEADER_SIZE);
                offset += padded;

                results.push(EntryParams {
                    path,
                    mode: entry.mode,
                    uid: entry.uid,
                    gid: entry.gid,
                    mtime: entry.mtime,
                    username: uname,
                    groupname: gname,
                    content,
                });
            }
            ParseEvent::SparseEntry { .. } => {
                panic!("unexpected SparseEntry in interop test");
            }
            ParseEvent::GlobalExtensions { consumed, .. } => {
                offset += consumed;
            }
            ParseEvent::End { .. } => break,
        }
    }

    results
}

// =============================================================================
// Python interaction helpers
// =============================================================================

fn run_python(sh: &Shell, input_json: &str) -> String {
    cmd!(sh, "python3 -c {PYTHON_HELPER}")
        .stdin(input_json)
        .read()
        .expect("python3 failed")
}

fn python_parse(sh: &Shell, tar_path: &str) -> Vec<TarEntryJson> {
    let cmd = ParseCommand {
        mode: "parse",
        tar_path: tar_path.to_string(),
    };
    let input = serde_json::to_string(&cmd).unwrap();
    let output = run_python(sh, &input);
    serde_json::from_str(&output).unwrap_or_else(|e| {
        panic!("failed to parse python output: {e}\noutput: {output}");
    })
}

fn python_generate(sh: &Shell, tar_path: &str, format: &str, entries: &[TarEntryJson]) {
    let cmd = GenerateCommand {
        mode: "generate",
        tar_path: tar_path.to_string(),
        format: format.to_string(),
        entries: entries.to_vec(),
    };
    let input = serde_json::to_string(&cmd).unwrap();
    let output = run_python(sh, &input);
    let result: serde_json::Value = serde_json::from_str(&output).unwrap_or_else(|e| {
        panic!("failed to parse python generate output: {e}\noutput: {output}");
    });
    assert!(
        result.get("ok").is_some(),
        "python generate failed: {result}"
    );
}

fn entry_params_to_json(params: &EntryParams) -> TarEntryJson {
    let b64 = base64::engine::general_purpose::STANDARD;
    TarEntryJson {
        path: b64.encode(&params.path),
        mode: params.mode,
        uid: params.uid,
        gid: params.gid,
        size: params.content.len() as u64,
        mtime: params.mtime,
        uname: b64.encode(&params.username),
        gname: b64.encode(&params.groupname),
        content: b64.encode(&params.content),
    }
}

// =============================================================================
// Assertion helpers
// =============================================================================

fn assert_entry_matches(label: &str, expected: &EntryParams, actual: &EntryParams) {
    assert_eq!(expected.path, actual.path, "{label}: path mismatch");
    assert_eq!(
        expected.mode, actual.mode,
        "{label}: mode mismatch (expected {:#o}, got {:#o})",
        expected.mode, actual.mode
    );
    assert_eq!(expected.uid, actual.uid, "{label}: uid mismatch");
    assert_eq!(expected.gid, actual.gid, "{label}: gid mismatch");
    assert_eq!(
        expected.content.len(),
        actual.content.len(),
        "{label}: size mismatch"
    );
    assert_eq!(expected.mtime, actual.mtime, "{label}: mtime mismatch");
    // Username/groupname: in GNU mode with long names (>32 bytes), we skip
    // setting them entirely, so the expected side may be empty. Only assert
    // when the expected side has a value.
    if !expected.username.is_empty() {
        assert_eq!(
            expected.username, actual.username,
            "{label}: uname mismatch"
        );
    }
    if !expected.groupname.is_empty() {
        assert_eq!(
            expected.groupname, actual.groupname,
            "{label}: gname mismatch"
        );
    }
    assert_eq!(
        expected.content,
        actual.content,
        "{label}: content mismatch (lengths: expected={}, got={})",
        expected.content.len(),
        actual.content.len()
    );
}

/// Decode a base64-encoded `TarEntryJson` (from Python) into an `EntryParams`.
fn json_entry_to_params(j: &TarEntryJson) -> EntryParams {
    let b64 = base64::engine::general_purpose::STANDARD;
    EntryParams {
        path: b64
            .decode(&j.path)
            .unwrap_or_else(|e| panic!("bad base64 path: {e}")),
        mode: j.mode,
        uid: j.uid,
        gid: j.gid,
        mtime: j.mtime,
        username: b64
            .decode(&j.uname)
            .unwrap_or_else(|e| panic!("bad base64 uname: {e}")),
        groupname: b64
            .decode(&j.gname)
            .unwrap_or_else(|e| panic!("bad base64 gname: {e}")),
        content: b64.decode(&j.content).unwrap_or_default(),
    }
}

// =============================================================================
// The actual roundtrip test
// =============================================================================

fn roundtrip_test(sh: &Shell, entries: &[EntryParams], use_pax: bool) {
    let tmpdir = TempDir::new().expect("failed to create tmpdir");
    let format_name = if use_pax { "pax" } else { "gnu" };

    // --- Direction 1: tar-core -> Python ---

    let tar_core_path = tmpdir.path().join(format!("tarcore_{format_name}.tar"));
    let tar_data = build_tar_core_archive(entries, use_pax);
    std::fs::write(&tar_core_path, &tar_data).expect("failed to write tar");

    let parsed_by_python = python_parse(sh, tar_core_path.to_str().unwrap());

    assert_eq!(
        entries.len(),
        parsed_by_python.len(),
        "{format_name}: entry count mismatch (tar-core -> python)"
    );

    for (i, (expected, py_json)) in entries.iter().zip(parsed_by_python.iter()).enumerate() {
        let actual = json_entry_to_params(py_json);
        assert_entry_matches(
            &format!("{format_name} tar-core->python entry[{i}]"),
            expected,
            &actual,
        );
    }

    // --- Direction 2: Python -> tar-core ---

    let python_tar_path = tmpdir.path().join(format!("python_{format_name}.tar"));
    let json_entries: Vec<TarEntryJson> = entries.iter().map(entry_params_to_json).collect();
    python_generate(
        sh,
        python_tar_path.to_str().unwrap(),
        format_name,
        &json_entries,
    );

    let python_tar_data = std::fs::read(&python_tar_path).expect("failed to read python tar");
    let parsed_by_tarcore = parse_with_tar_core(&python_tar_data);

    assert_eq!(
        entries.len(),
        parsed_by_tarcore.len(),
        "{format_name}: entry count mismatch (python -> tar-core)"
    );

    for (i, (expected, parsed)) in entries.iter().zip(parsed_by_tarcore.iter()).enumerate() {
        assert_entry_matches(
            &format!("{format_name} python->tar-core entry[{i}]"),
            expected,
            parsed,
        );
    }
}

// =============================================================================
// Deterministic smoke tests
// =============================================================================

fn smoke_test_roundtrip(sh: &Shell) {
    let entries = vec![
        // Short path, small content
        EntryParams {
            path: b"hello.txt".to_vec(),
            mode: 0o644,
            uid: 1000,
            gid: 1000,
            mtime: 1234567890,
            username: b"testuser".to_vec(),
            groupname: b"testgroup".to_vec(),
            content: b"Hello, World!".to_vec(),
        },
        // Empty file
        EntryParams {
            path: b"empty".to_vec(),
            mode: 0o600,
            uid: 0,
            gid: 0,
            mtime: 0,
            username: b"root".to_vec(),
            groupname: b"root".to_vec(),
            content: vec![],
        },
        // Long path (>100 bytes, triggers extensions)
        EntryParams {
            path: {
                let mut p = b"very/long/path/that/exceeds/one/hundred/bytes/".to_vec();
                p.extend(std::iter::repeat_n(b'x', 60));
                p
            },
            mode: 0o755,
            uid: 65535,
            gid: 65535,
            mtime: 0xFFFFFFF0,
            username: b"nobody".to_vec(),
            groupname: b"nogroup".to_vec(),
            content: vec![0xAB; 512],
        },
    ];

    roundtrip_test(sh, &entries, false); // GNU
    roundtrip_test(sh, &entries, true); // PAX
}

fn smoke_test_long_username_pax(sh: &Shell) {
    // Username > 32 bytes, only works with PAX
    let long_uname = b"a_very_long_username_that_exceeds_thirtytwo_bytes";
    assert!(long_uname.len() > 32);

    let entries = vec![EntryParams {
        path: b"file_with_long_uname.txt".to_vec(),
        mode: 0o644,
        uid: 1000,
        gid: 1000,
        mtime: 1700000000,
        username: long_uname.to_vec(),
        groupname: b"staff".to_vec(),
        content: b"data".to_vec(),
    }];

    roundtrip_test(sh, &entries, true);
}

fn smoke_test_non_utf8_path(sh: &Shell) {
    // Path containing bytes > 127 that aren't valid UTF-8
    let entries = vec![EntryParams {
        path: vec![b'f', b'i', b'l', b'e', 0x80, 0xFE, b'.', b'd', b'a', b't'],
        mode: 0o644,
        uid: 1000,
        gid: 1000,
        mtime: 1700000000,
        username: vec![b'u', 0xC0, b's', b'r'],
        groupname: b"grp".to_vec(),
        content: b"non-utf8 path test".to_vec(),
    }];

    // GNU handles arbitrary bytes; PAX requires UTF-8 for paths, so only test GNU
    roundtrip_test(sh, &entries, false);
}

// =============================================================================
// Main
// =============================================================================

fn main() {
    let sh = Shell::new().expect("xshell");

    // Check that python3 is available
    if cmd!(sh, "python3 --version")
        .quiet()
        .ignore_status()
        .run()
        .is_err()
    {
        eprintln!("python3 not found, skipping");
        return;
    }

    println!("=== tar-core <-> Python interop tests ===");

    // Deterministic smoke tests
    println!("smoke_test_roundtrip...");
    smoke_test_roundtrip(&sh);
    println!("smoke_test_roundtrip: PASSED");

    println!("smoke_test_long_username_pax...");
    smoke_test_long_username_pax(&sh);
    println!("smoke_test_long_username_pax: PASSED");

    println!("smoke_test_non_utf8_path...");
    smoke_test_non_utf8_path(&sh);
    println!("smoke_test_non_utf8_path: PASSED");

    // Arbitrary-driven tests
    run_random_tests("roundtrip_gnu", 32, |entries| {
        roundtrip_test(&sh, entries, false);
    });

    run_random_tests("roundtrip_pax", 32, |entries| {
        // PAX requires UTF-8 paths, so filter to entries with valid UTF-8 paths
        let utf8_entries: Vec<EntryParams> = entries
            .iter()
            .filter(|e| {
                std::str::from_utf8(&e.path).is_ok()
                    && std::str::from_utf8(&e.username).is_ok()
                    && std::str::from_utf8(&e.groupname).is_ok()
            })
            .cloned()
            .collect();
        if !utf8_entries.is_empty() {
            roundtrip_test(&sh, &utf8_entries, true);
        }
    });

    println!("All tests passed!");
}
