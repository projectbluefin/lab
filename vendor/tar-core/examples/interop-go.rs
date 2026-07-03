//! Cross-language roundtrip integration test: tar-core <-> Go archive/tar.
//!
//! This test validates that tar archives built by tar-core can be correctly
//! parsed by Go's archive/tar package, and vice versa. It uses the `arbitrary`
//! crate to generate random entry parameters and tests both GNU and PAX
//! extension modes.
//!
//! Paths and names are `Vec<u8>` (not `String`) to exercise non-UTF-8 byte
//! sequences — tar paths are fundamentally byte sequences.  The JSON protocol
//! base64-encodes path, uname, and gname fields.
//!
//! Requires Go 1.23+. Run with:
//!   GOPATH=$HOME/gopath PATH=$HOME/go/bin:$PATH cargo run --example interop-go

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use xshell::{cmd, Shell};

use arbitrary::{Arbitrary, Unstructured};
use base64::Engine;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

use tar_core::builder::EntryBuilder;
use tar_core::parse::{Limits, ParseEvent, Parser};
use tar_core::{EntryType, SparseEntry as TarSparseEntry, HEADER_SIZE};

// ============================================================================
// Go helper program (embedded source)
// ============================================================================

const GO_HELPER_SRC: &str = r#"
package main

import (
	"archive/tar"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"time"
)

type Command struct {
	Mode    string  `json:"mode"`
	TarPath string  `json:"tar_path"`
	Format  string  `json:"format,omitempty"`
	Entries []Entry `json:"entries,omitempty"`
}

type Entry struct {
	Path    string `json:"path"`    // base64-encoded raw bytes
	Mode    int64  `json:"mode"`
	Uid     int    `json:"uid"`
	Gid     int    `json:"gid"`
	Size    int64  `json:"size"`
	Mtime   int64  `json:"mtime"`
	Uname   string `json:"uname"`  // base64-encoded raw bytes
	Gname   string `json:"gname"`  // base64-encoded raw bytes
	Content string `json:"content"` // base64-encoded
}

func parseTar(path string) {
	f, err := os.Open(path)
	if err != nil {
		fmt.Fprintf(os.Stderr, "open: %v\n", err)
		os.Exit(1)
	}
	defer f.Close()

	tr := tar.NewReader(f)
	var entries []Entry
	for {
		hdr, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			fmt.Fprintf(os.Stderr, "tar next: %v\n", err)
			os.Exit(1)
		}

		var content []byte
		if hdr.Size > 0 {
			content, err = io.ReadAll(tr)
			if err != nil {
				fmt.Fprintf(os.Stderr, "read content: %v\n", err)
				os.Exit(1)
			}
		}

		// Go strings are byte slices, so we can base64-encode them directly
		entries = append(entries, Entry{
			Path:    base64.StdEncoding.EncodeToString([]byte(hdr.Name)),
			Mode:    int64(hdr.Mode),
			Uid:     hdr.Uid,
			Gid:     hdr.Gid,
			Size:    hdr.Size,
			Mtime:   hdr.ModTime.Unix(),
			Uname:   base64.StdEncoding.EncodeToString([]byte(hdr.Uname)),
			Gname:   base64.StdEncoding.EncodeToString([]byte(hdr.Gname)),
			Content: base64.StdEncoding.EncodeToString(content),
		})
	}

	json.NewEncoder(os.Stdout).Encode(entries)
}

func generateTar(cmd Command) {
	f, err := os.Create(cmd.TarPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "create: %v\n", err)
		os.Exit(1)
	}
	defer f.Close()

	tw := tar.NewWriter(f)
	defer tw.Close()

	for _, e := range cmd.Entries {
		content, err := base64.StdEncoding.DecodeString(e.Content)
		if err != nil {
			fmt.Fprintf(os.Stderr, "base64 decode content: %v\n", err)
			os.Exit(1)
		}

		pathBytes, err := base64.StdEncoding.DecodeString(e.Path)
		if err != nil {
			fmt.Fprintf(os.Stderr, "base64 decode path: %v\n", err)
			os.Exit(1)
		}
		unameBytes, err := base64.StdEncoding.DecodeString(e.Uname)
		if err != nil {
			fmt.Fprintf(os.Stderr, "base64 decode uname: %v\n", err)
			os.Exit(1)
		}
		gnameBytes, err := base64.StdEncoding.DecodeString(e.Gname)
		if err != nil {
			fmt.Fprintf(os.Stderr, "base64 decode gname: %v\n", err)
			os.Exit(1)
		}

		var format tar.Format
		switch cmd.Format {
		case "gnu":
			format = tar.FormatGNU
		case "pax":
			format = tar.FormatPAX
		default:
			format = tar.FormatGNU
		}

		hdr := &tar.Header{
			Typeflag: tar.TypeReg,
			Name:     string(pathBytes),
			Mode:     e.Mode,
			Uid:      e.Uid,
			Gid:      e.Gid,
			Size:     int64(len(content)),
			ModTime:  timeFromUnix(e.Mtime),
			Uname:    string(unameBytes),
			Gname:    string(gnameBytes),
			Format:   format,
		}

		if err := tw.WriteHeader(hdr); err != nil {
			fmt.Fprintf(os.Stderr, "write header: %v\n", err)
			os.Exit(1)
		}
		if _, err := tw.Write(content); err != nil {
			fmt.Fprintf(os.Stderr, "write content: %v\n", err)
			os.Exit(1)
		}
	}
}

func timeFromUnix(sec int64) time.Time {
	return time.Unix(sec, 0)
}

func main() {
	var cmd Command
	if err := json.NewDecoder(os.Stdin).Decode(&cmd); err != nil {
		fmt.Fprintf(os.Stderr, "json decode: %v\n", err)
		os.Exit(1)
	}

	switch cmd.Mode {
	case "parse":
		parseTar(cmd.TarPath)
	case "generate":
		generateTar(cmd)
	default:
		fmt.Fprintf(os.Stderr, "unknown mode: %s\n", cmd.Mode)
		os.Exit(1)
	}
}
"#;

// ============================================================================
// JSON types matching the Go program
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct GoCommand {
    mode: String,
    tar_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entries: Option<Vec<GoEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GoEntry {
    path: String, // base64-encoded raw bytes
    mode: i64,
    uid: i32,
    gid: i32,
    size: i64,
    mtime: i64,
    uname: String,   // base64-encoded raw bytes
    gname: String,   // base64-encoded raw bytes
    content: String, // base64-encoded
}

// ============================================================================
// Go binary compilation (cached)
// ============================================================================

/// Find the `go` binary: try PATH first, then common install locations.
fn find_go(sh: &Shell) -> Option<String> {
    if cmd!(sh, "go version").quiet().ignore_status().run().is_ok() {
        return Some("go".into());
    }
    if let Ok(home) = std::env::var("HOME") {
        let candidate = format!("{home}/go/bin/go");
        if cmd!(sh, "{candidate} version")
            .quiet()
            .ignore_status()
            .run()
            .is_ok()
        {
            return Some(candidate);
        }
    }
    None
}

fn compile_go_helper(sh: &Shell, dir: &Path) -> PathBuf {
    let src_path = dir.join("helper.go");
    let bin_path = dir.join("helper");

    std::fs::write(&src_path, GO_HELPER_SRC).expect("write Go source");

    let go_bin = find_go(sh).expect("Go not found on PATH or at $HOME/go/bin/go");
    cmd!(sh, "{go_bin} build -o {bin_path} {src_path}")
        .run()
        .expect("go build failed");

    bin_path
}

/// Returns the path to the compiled Go helper binary.
/// Compiles it once on first call (uses a static TempDir to keep the binary alive).
fn go_helper_bin(sh: &Shell) -> &'static Path {
    static HELPER: OnceLock<(TempDir, PathBuf)> = OnceLock::new();
    let (_, bin) = HELPER.get_or_init(|| {
        let dir = TempDir::new().expect("create tempdir for Go helper");
        let bin = compile_go_helper(sh, dir.path());
        (dir, bin)
    });
    bin.as_path()
}

fn run_go_parse(sh: &Shell, go_bin: &Path, tar_path: &Path) -> Vec<GoEntry> {
    let go_cmd = GoCommand {
        mode: "parse".into(),
        tar_path: tar_path.to_str().unwrap().into(),
        format: None,
        entries: None,
    };
    let input = serde_json::to_string(&go_cmd).unwrap();

    let output_str = cmd!(sh, "{go_bin}")
        .stdin(&input)
        .read()
        .expect("failed to run Go helper (parse)");

    serde_json::from_str(&output_str).expect("parse Go JSON output")
}

fn run_go_generate(
    sh: &Shell,
    go_bin: &Path,
    tar_path: &Path,
    format: &str,
    entries: Vec<GoEntry>,
) {
    let go_cmd = GoCommand {
        mode: "generate".into(),
        tar_path: tar_path.to_str().unwrap().into(),
        format: Some(format.into()),
        entries: Some(entries),
    };
    let input = serde_json::to_string(&go_cmd).unwrap();

    cmd!(sh, "{go_bin}")
        .stdin(&input)
        .read()
        .expect("failed to run Go helper (generate)");
}

// ============================================================================
// tar-core archive building
// ============================================================================

/// Parameters for a single tar entry.
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
    /// Sparse data map (empty for non-sparse entries).
    sparse_map: Vec<TarSparseEntry>,
    /// Logical file size for sparse entries (0 for non-sparse).
    real_size: u64,
}

/// Build a tar archive from entry params using tar-core's EntryBuilder.
fn build_tar_core_archive(entries: &[EntryParams], format: &str) -> Vec<u8> {
    let mut archive = Vec::new();

    for entry in entries {
        let mut builder = match format {
            "gnu" => EntryBuilder::new_gnu(),
            "pax" => EntryBuilder::new_ustar(),
            _ => panic!("unknown format: {format}"),
        };

        builder
            .path(&entry.path)
            .mode(entry.mode)
            .unwrap()
            .uid(entry.uid as u64)
            .unwrap()
            .gid(entry.gid as u64)
            .unwrap()
            .size(entry.content.len() as u64)
            .unwrap()
            .mtime(entry.mtime as u64)
            .unwrap()
            .entry_type(EntryType::Regular);

        // username/groupname: in PAX mode, long names go into PAX extensions
        // In GNU mode, names > 32 bytes will error, so we truncate for GNU.
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

/// Build a sparse tar archive from entry params using tar-core's EntryBuilder.
///
/// Entries with non-empty `sparse_map` are written as sparse files. The
/// `content` field holds only the data region bytes (concatenated), and
/// `real_size` is the logical file size.
fn build_sparse_tar_core_archive(entries: &[EntryParams], format: &str) -> Vec<u8> {
    let mut archive = Vec::new();

    for entry in entries {
        let mut builder = match format {
            "gnu" => EntryBuilder::new_gnu(),
            "pax" => EntryBuilder::new_ustar(),
            _ => panic!("unknown format: {format}"),
        };

        builder
            .path(&entry.path)
            .mode(entry.mode)
            .unwrap()
            .uid(entry.uid as u64)
            .unwrap()
            .gid(entry.gid as u64)
            .unwrap()
            .mtime(entry.mtime as u64)
            .unwrap()
            .entry_type(EntryType::Regular);

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

        if !entry.sparse_map.is_empty() {
            // Sparse entry: size is the on-disk content length
            builder
                .size(entry.content.len() as u64)
                .unwrap()
                .sparse(&entry.sparse_map, entry.real_size);
        } else {
            builder.size(entry.content.len() as u64).unwrap();
        }

        let header_bytes = builder.finish_bytes();
        archive.extend_from_slice(&header_bytes);

        // Write content (for sparse entries, this is the concatenated data regions)
        archive.extend_from_slice(&entry.content);

        // Pad to 512-byte boundary
        let padding = (HEADER_SIZE - (entry.content.len() % HEADER_SIZE)) % HEADER_SIZE;
        archive.extend(std::iter::repeat_n(0u8, padding));
    }

    // End-of-archive: two 512-byte zero blocks
    archive.extend(std::iter::repeat_n(0u8, HEADER_SIZE * 2));

    archive
}

/// Parse a tar archive using tar-core's sans-IO parser.
fn parse_tar_core_archive(data: &[u8]) -> Vec<EntryParams> {
    let mut parser = Parser::new(Limits::default());
    let mut results = Vec::new();
    let mut offset = 0;

    loop {
        let input = &data[offset..];
        match parser.parse(input).expect("parse should succeed") {
            ParseEvent::NeedData { .. } => {
                panic!("unexpected NeedData — archive should be complete in memory");
            }
            ParseEvent::Entry { consumed, entry } => {
                offset += consumed;

                let path = entry.path.to_vec();
                let mode = entry.mode;
                let uid = entry.uid as u32;
                let gid = entry.gid as u32;
                let size = entry.size as usize;
                let mtime = entry.mtime as u32;
                let uname = entry.uname.as_ref().map(|u| u.to_vec()).unwrap_or_default();
                let gname = entry.gname.as_ref().map(|g| g.to_vec()).unwrap_or_default();

                // Read content
                let content = data[offset..offset + size].to_vec();
                let padded_size = size.next_multiple_of(HEADER_SIZE);
                offset += padded_size;

                results.push(EntryParams {
                    path,
                    mode,
                    uid,
                    gid,
                    mtime,
                    username: uname,
                    groupname: gname,
                    content,
                    sparse_map: Vec::new(),
                    real_size: 0,
                });
            }
            ParseEvent::SparseEntry {
                consumed,
                entry,
                sparse_map,
                real_size,
            } => {
                offset += consumed;

                let path = entry.path.to_vec();
                let mode = entry.mode;
                let uid = entry.uid as u32;
                let gid = entry.gid as u32;
                let size = entry.size as usize;
                let mtime = entry.mtime as u32;
                let uname = entry.uname.as_ref().map(|u| u.to_vec()).unwrap_or_default();
                let gname = entry.gname.as_ref().map(|g| g.to_vec()).unwrap_or_default();

                // Read the on-disk content (sparse data regions only)
                let content = data[offset..offset + size].to_vec();
                let padded_size = size.next_multiple_of(HEADER_SIZE);
                offset += padded_size;

                results.push(EntryParams {
                    path,
                    mode,
                    uid,
                    gid,
                    mtime,
                    username: uname,
                    groupname: gname,
                    content,
                    sparse_map,
                    real_size,
                });
            }
            ParseEvent::GlobalExtensions { consumed, .. } => {
                offset += consumed;
            }
            ParseEvent::End { .. } => break,
        }
    }

    results
}

// ============================================================================
// Comparison helpers
// ============================================================================

fn assert_entries_match_go(label: &str, expected: &[EntryParams], actual: &[GoEntry]) {
    let b64 = base64::engine::general_purpose::STANDARD;

    assert_eq!(
        expected.len(),
        actual.len(),
        "{label}: entry count mismatch: expected {}, got {}",
        expected.len(),
        actual.len()
    );

    for (i, (exp, act)) in expected.iter().zip(actual.iter()).enumerate() {
        let act_path = b64
            .decode(&act.path)
            .unwrap_or_else(|e| panic!("{label} entry[{i}]: bad base64 path: {e}"));
        let act_uname = b64
            .decode(&act.uname)
            .unwrap_or_else(|e| panic!("{label} entry[{i}]: bad base64 uname: {e}"));
        let act_gname = b64
            .decode(&act.gname)
            .unwrap_or_else(|e| panic!("{label} entry[{i}]: bad base64 gname: {e}"));

        assert_eq!(exp.path, act_path, "{label} entry[{i}]: path mismatch");
        assert_eq!(
            exp.mode as i64, act.mode,
            "{label} entry[{i}]: mode mismatch (expected 0o{:o}, got 0o{:o})",
            exp.mode, act.mode
        );
        assert_eq!(exp.uid as i32, act.uid, "{label} entry[{i}]: uid mismatch");
        assert_eq!(exp.gid as i32, act.gid, "{label} entry[{i}]: gid mismatch");
        assert_eq!(
            exp.content.len() as i64,
            act.size,
            "{label} entry[{i}]: size mismatch"
        );
        assert_eq!(
            exp.mtime as i64, act.mtime,
            "{label} entry[{i}]: mtime mismatch"
        );

        assert_eq!(
            exp.username, act_uname,
            "{label} entry[{i}]: uname mismatch"
        );
        assert_eq!(
            exp.groupname, act_gname,
            "{label} entry[{i}]: gname mismatch"
        );

        // Content comparison
        let actual_content = b64.decode(&act.content).unwrap_or_default();
        assert_eq!(
            exp.content,
            actual_content,
            "{label} entry[{i}]: content mismatch (expected {} bytes, got {} bytes)",
            exp.content.len(),
            actual_content.len()
        );
    }
}

fn assert_parsed_entries_match(label: &str, expected: &[EntryParams], actual: &[EntryParams]) {
    assert_eq!(
        expected.len(),
        actual.len(),
        "{label}: entry count mismatch: expected {}, got {}",
        expected.len(),
        actual.len()
    );

    for (i, (exp, act)) in expected.iter().zip(actual.iter()).enumerate() {
        assert_eq!(exp.path, act.path, "{label} entry[{i}]: path mismatch");
        assert_eq!(
            exp.mode, act.mode,
            "{label} entry[{i}]: mode mismatch (expected 0o{:o}, got 0o{:o})",
            exp.mode, act.mode
        );
        assert_eq!(exp.uid, act.uid, "{label} entry[{i}]: uid mismatch");
        assert_eq!(exp.gid, act.gid, "{label} entry[{i}]: gid mismatch");
        assert_eq!(
            exp.content.len(),
            act.content.len(),
            "{label} entry[{i}]: size mismatch"
        );
        assert_eq!(exp.mtime, act.mtime, "{label} entry[{i}]: mtime mismatch");
        assert_eq!(
            exp.username, act.username,
            "{label} entry[{i}]: uname mismatch"
        );
        assert_eq!(
            exp.groupname, act.groupname,
            "{label} entry[{i}]: gname mismatch"
        );
        assert_eq!(
            exp.content, act.content,
            "{label} entry[{i}]: content mismatch"
        );
    }
}

fn entries_to_go(entries: &[EntryParams]) -> Vec<GoEntry> {
    let b64 = base64::engine::general_purpose::STANDARD;
    entries
        .iter()
        .map(|e| GoEntry {
            path: b64.encode(&e.path),
            mode: e.mode as i64,
            uid: e.uid as i32,
            gid: e.gid as i32,
            size: e.content.len() as i64,
            mtime: e.mtime as i64,
            uname: b64.encode(&e.username),
            gname: b64.encode(&e.groupname),
            content: b64.encode(&e.content),
        })
        .collect()
}

// ============================================================================
// Arbitrary-based random test generation
// ============================================================================

#[derive(Debug, Clone, Arbitrary)]
struct RawEntryParams {
    path_bytes: Vec<u8>,
    mode: u16,
    uid: u16,
    gid: u16,
    content_len: u8,
    mtime: u32,
    uname_bytes: Vec<u8>,
    gname_bytes: Vec<u8>,
    content_seed: Vec<u8>,
}

/// Remove NUL bytes and clamp length.
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

fn to_entry_params(raw: &RawEntryParams) -> Option<EntryParams> {
    let path = sanitize_bytes(&raw.path_bytes, 1, 200)?;
    let username = sanitize_bytes(&raw.uname_bytes, 1, 64)?;
    let groupname = sanitize_bytes(&raw.gname_bytes, 1, 64)?;

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
        uid: raw.uid as u32,
        gid: raw.gid as u32,
        mtime: raw.mtime & 0x7FFFFFFF,
        username,
        groupname,
        content,
        sparse_map: Vec::new(),
        real_size: 0,
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

// ============================================================================
// Core roundtrip logic
// ============================================================================

/// Roundtrip: tar-core builds -> Go parses -> verify
/// Then: Go builds -> tar-core parses -> verify
fn roundtrip_test(sh: &Shell, entries: &[EntryParams], format: &str) {
    let go_bin = go_helper_bin(sh);
    let tmp = TempDir::new().expect("create tempdir");

    // -- Direction 1: tar-core -> Go --
    let tar_path = tmp.path().join("rust_built.tar");
    let archive_data = build_tar_core_archive(entries, format);
    std::fs::write(&tar_path, &archive_data).expect("write tar file");

    let go_parsed = run_go_parse(sh, go_bin, &tar_path);

    // For GNU mode, usernames > 32 bytes get truncated. Build the expected
    // comparison entries accordingly.
    let expected_for_go: Vec<EntryParams> = entries
        .iter()
        .map(|e| {
            let mut e = e.clone();
            if format == "gnu" {
                if e.username.len() > 32 {
                    e.username.truncate(32);
                }
                if e.groupname.len() > 32 {
                    e.groupname.truncate(32);
                }
            }
            e
        })
        .collect();

    assert_entries_match_go(
        &format!("tar-core->{format}->Go"),
        &expected_for_go,
        &go_parsed,
    );

    // -- Direction 2: Go -> tar-core --
    let go_tar_path = tmp.path().join("go_built.tar");
    let go_entries = entries_to_go(&expected_for_go);
    run_go_generate(sh, go_bin, &go_tar_path, format, go_entries);

    let go_archive_data = std::fs::read(&go_tar_path).expect("read Go-built tar");
    let rust_parsed = parse_tar_core_archive(&go_archive_data);

    assert_parsed_entries_match(
        &format!("Go->{format}->tar-core"),
        &expected_for_go,
        &rust_parsed,
    );
}

// ============================================================================
// Deterministic smoke tests
// ============================================================================

fn smoke_test_roundtrip(sh: &Shell) {
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
            sparse_map: Vec::new(),
            real_size: 0,
        },
        EntryParams {
            path: b"empty.txt".to_vec(),
            mode: 0o600,
            uid: 0,
            gid: 0,
            mtime: 0,
            username: b"root".to_vec(),
            groupname: b"root".to_vec(),
            content: vec![],
            sparse_map: Vec::new(),
            real_size: 0,
        },
        EntryParams {
            // Long path > 100 bytes
            path: {
                let mut p = b"very/long/path/".to_vec();
                p.extend(std::iter::repeat_n(b'x', 100));
                p
            },
            mode: 0o755,
            uid: 65534,
            gid: 65534,
            mtime: 1700000000,
            username: b"nobody".to_vec(),
            groupname: b"nogroup".to_vec(),
            content: vec![0xAB; 512],
            sparse_map: Vec::new(),
            real_size: 0,
        },
    ];

    for format in &["gnu", "pax"] {
        println!("  smoke_test_roundtrip ({format})...");
        roundtrip_test(sh, &entries, format);
        println!("  smoke_test_roundtrip ({format}): PASSED");
    }
}

fn smoke_test_pax_long_uname(sh: &Shell) {
    let entries = vec![EntryParams {
        path: b"file_with_long_uname.dat".to_vec(),
        mode: 0o644,
        uid: 1000,
        gid: 1000,
        mtime: 1234567890,
        username: b"a_very_long_username_that_exceeds_32_bytes_easily".to_vec(),
        groupname: b"shortgrp".to_vec(),
        content: vec![42; 64],
        sparse_map: Vec::new(),
        real_size: 0,
    }];

    roundtrip_test(sh, &entries, "pax");
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
        sparse_map: Vec::new(),
        real_size: 0,
    }];

    // GNU handles arbitrary bytes; PAX requires UTF-8 for paths, so only test GNU
    roundtrip_test(sh, &entries, "gnu");
}

// ============================================================================
// Sparse roundtrip helpers
// ============================================================================

/// Expand sparse data regions into a full logical buffer (holes are zeroed).
fn expand_sparse_content(sparse_map: &[TarSparseEntry], data: &[u8], real_size: u64) -> Vec<u8> {
    let mut buf = vec![0u8; real_size as usize];
    let mut data_offset = 0usize;
    for region in sparse_map {
        let len = region.length as usize;
        let dst_start = region.offset as usize;
        buf[dst_start..dst_start + len].copy_from_slice(&data[data_offset..data_offset + len]);
        data_offset += len;
    }
    buf
}

/// Test: tar-core builds a sparse archive → Go parses it correctly.
///
/// We verify that Go sees the correct logical size and that reading the
/// content through Go's sparse-aware reader produces the expected bytes
/// (data regions filled, holes zeroed).
fn test_sparse_tar_core_to_go(sh: &Shell, format: &str) {
    let go_bin = go_helper_bin(sh);
    let tmp = TempDir::new().expect("create tempdir");

    // Create a sparse entry: 1024-byte logical file with two data regions.
    //   [0..10):   "AAAAAAAAAA"
    //   [512..522): "BBBBBBBBBB"
    // Everything else is zero (holes).
    let sparse_map = vec![
        TarSparseEntry {
            offset: 0,
            length: 10,
        },
        TarSparseEntry {
            offset: 512,
            length: 10,
        },
    ];
    let real_size: u64 = 1024;
    // On-disk content is the concatenation of data regions.
    let mut on_disk_content = Vec::new();
    on_disk_content.extend_from_slice(&[b'A'; 10]);
    on_disk_content.extend_from_slice(&[b'B'; 10]);

    let entry = EntryParams {
        path: b"sparse.dat".to_vec(),
        mode: 0o644,
        uid: 1000,
        gid: 1000,
        mtime: 1700000000,
        username: b"user".to_vec(),
        groupname: b"group".to_vec(),
        content: on_disk_content.clone(),
        sparse_map: sparse_map.clone(),
        real_size,
    };

    let archive_data = build_sparse_tar_core_archive(&[entry], format);
    let tar_path = tmp.path().join("sparse_rust.tar");
    std::fs::write(&tar_path, &archive_data).expect("write sparse tar");

    let go_parsed = run_go_parse(sh, go_bin, &tar_path);
    assert_eq!(go_parsed.len(), 1, "expected 1 entry from Go");

    let b64 = base64::engine::general_purpose::STANDARD;
    let go_entry = &go_parsed[0];

    let go_path = b64.decode(&go_entry.path).unwrap();
    assert_eq!(go_path, b"sparse.dat", "path mismatch");

    // Go should report the logical size
    assert_eq!(
        go_entry.size, real_size as i64,
        "Go should see logical size {real_size}, got {}",
        go_entry.size
    );

    // Go's content should be the fully expanded file
    let go_content = b64.decode(&go_entry.content).unwrap();
    let expected_content = expand_sparse_content(&sparse_map, &on_disk_content, real_size);
    assert_eq!(
        go_content, expected_content,
        "expanded sparse content mismatch (format={format})"
    );
}

/// Test: tar-core sparse roundtrip (build → parse).
///
/// Verifies that tar-core can re-parse its own sparse archives, recovering
/// the sparse map, real size, and on-disk data correctly.
fn test_sparse_tar_core_roundtrip(format: &str) {
    let sparse_map = vec![
        TarSparseEntry {
            offset: 0,
            length: 10,
        },
        TarSparseEntry {
            offset: 512,
            length: 10,
        },
    ];
    let real_size: u64 = 1024;
    let mut on_disk_content = Vec::new();
    on_disk_content.extend_from_slice(&[b'A'; 10]);
    on_disk_content.extend_from_slice(&[b'B'; 10]);

    let entry = EntryParams {
        path: b"sparse_roundtrip.dat".to_vec(),
        mode: 0o644,
        uid: 1000,
        gid: 1000,
        mtime: 1700000000,
        username: b"user".to_vec(),
        groupname: b"group".to_vec(),
        content: on_disk_content.clone(),
        sparse_map: sparse_map.clone(),
        real_size,
    };

    let archive_data = build_sparse_tar_core_archive(&[entry], format);
    let parsed = parse_tar_core_archive(&archive_data);
    assert_eq!(parsed.len(), 1, "expected 1 entry");

    let result = &parsed[0];
    assert_eq!(result.path, b"sparse_roundtrip.dat");
    assert_eq!(
        result.real_size, real_size,
        "real_size mismatch (format={format})"
    );
    assert_eq!(
        result.sparse_map.len(),
        2,
        "expected 2 sparse data regions (format={format})"
    );
    assert_eq!(result.sparse_map[0].offset, 0);
    assert_eq!(result.sparse_map[0].length, 10);
    assert_eq!(result.sparse_map[1].offset, 512);
    assert_eq!(result.sparse_map[1].length, 10);
    assert_eq!(
        result.content, on_disk_content,
        "on-disk content mismatch (format={format})"
    );
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    let sh = Shell::new().expect("xshell");

    // Check that Go is available
    if find_go(&sh).is_none() {
        eprintln!("Go not found on PATH or at $HOME/go/bin/go, skipping");
        return;
    }

    println!("=== tar-core <-> Go interop tests ===");

    // Deterministic smoke tests
    println!("smoke_test_roundtrip...");
    smoke_test_roundtrip(&sh);
    println!("smoke_test_roundtrip: PASSED");

    println!("smoke_test_pax_long_uname...");
    smoke_test_pax_long_uname(&sh);
    println!("smoke_test_pax_long_uname: PASSED");

    println!("smoke_test_non_utf8_path...");
    smoke_test_non_utf8_path(&sh);
    println!("smoke_test_non_utf8_path: PASSED");

    // Sparse roundtrip tests
    for format in &["gnu", "pax"] {
        println!("test_sparse_tar_core_to_go ({format})...");
        test_sparse_tar_core_to_go(&sh, format);
        println!("test_sparse_tar_core_to_go ({format}): PASSED");
    }

    for format in &["gnu", "pax"] {
        println!("test_sparse_tar_core_roundtrip ({format})...");
        test_sparse_tar_core_roundtrip(format);
        println!("test_sparse_tar_core_roundtrip ({format}): PASSED");
    }

    // Arbitrary-driven tests
    run_random_tests("roundtrip_gnu", 32, |entries| {
        roundtrip_test(&sh, entries, "gnu");
    });

    run_random_tests("roundtrip_pax", 32, |entries| {
        // PAX requires UTF-8 paths, so filter to entries with valid UTF-8
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
            roundtrip_test(&sh, &utf8_entries, "pax");
        }
    });

    println!("All tests passed!");
}
