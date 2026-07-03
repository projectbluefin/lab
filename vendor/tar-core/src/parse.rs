//! Sans-IO tar archive parser.
//!
//! This module provides a sans-IO state machine parser for tar archives.
//! It operates on `&[u8]` slices directly (no `Read` trait bound), making
//! it suitable for:
//!
//! - Async I/O (tokio, async-std)
//! - Custom buffering strategies
//! - Zero-copy parsing in memory-mapped archives
//! - Embedding in other parsers
//!
//! In addition to the parser itself, this module contains the configuration
//! and error types it uses: [`Limits`] for security limits and [`ParseError`]
//! for error reporting.
//!
//! # Design
//!
//! The parser is a state machine that processes bytes and emits [`ParseEvent`]s.
//! The caller is responsible for:
//!
//! 1. Providing input data via [`Parser::parse`]
//! 2. Handling events (headers, content markers, end-of-archive)
//! 3. Managing the buffer and reading more data when needed
//!
//! # Example
//!
//! ```
//! use tar_core::parse::{Parser, ParseEvent, Limits};
//!
//! let mut parser = Parser::new(Limits::default());
//!
//! // Simulated tar data (in practice, read from file/network)
//! let data = [0u8; 1024]; // Two zero blocks = end of archive
//!
//! match parser.parse(&data) {
//!     Ok(ParseEvent::End { consumed }) => {
//!         println!("End of archive after {} bytes", consumed);
//!     }
//!     Ok(event) => {
//!         println!("Got event {:?}", event);
//!     }
//!     Err(e) => {
//!         eprintln!("Parse error: {}", e);
//!     }
//! }
//! ```

use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::str::Utf8Error;

use thiserror::Error;
use zerocopy::FromBytes;

use crate::{
    EntryType, GnuExtSparseHeader, Header, HeaderError, PaxError, PaxExtensions, SparseEntry,
    HEADER_SIZE, PAX_GID, PAX_GNAME, PAX_GNU_SPARSE_MAJOR, PAX_GNU_SPARSE_MAP,
    PAX_GNU_SPARSE_MINOR, PAX_GNU_SPARSE_NAME, PAX_GNU_SPARSE_NUMBYTES, PAX_GNU_SPARSE_OFFSET,
    PAX_GNU_SPARSE_REALSIZE, PAX_GNU_SPARSE_SIZE, PAX_LINKPATH, PAX_MTIME, PAX_PATH,
    PAX_SCHILY_XATTR, PAX_SIZE, PAX_UID, PAX_UNAME,
};

// ============================================================================
// Limits
// ============================================================================

/// Configurable security limits for tar archive parsing.
///
/// These limits protect against malicious or malformed archives that could
/// exhaust memory or create excessively long paths.
///
/// # Example
///
/// ```
/// use tar_core::parse::Limits;
///
/// // Use defaults
/// let limits = Limits::default();
///
/// // Customize limits
/// let limits = Limits {
///     max_metadata_size: 64 * 1024,
///     // Set to libc::PATH_MAX when extracting to disk
///     max_path_len: Some(4096),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Maximum total size of all extension metadata for a single entry, in bytes.
    ///
    /// This is an aggregate budget: the combined size of PAX extended headers,
    /// GNU long name, and GNU long link data for one file entry must not exceed
    /// this limit. Exceeding it will cause a [`ParseError::MetadataTooLarge`]
    /// error.
    ///
    /// Default: 1 MiB (1,048,576 bytes).
    pub max_metadata_size: u32,

    /// Optional maximum path length in bytes.
    ///
    /// When set, paths and link targets exceeding this limit will cause a
    /// [`ParseError::PathTooLong`] error. When `None`, no path length check
    /// is performed (the default).
    ///
    /// Callers extracting to a real filesystem should set this to
    /// `libc::PATH_MAX` (4096 on Linux, 1024 on macOS) or the appropriate
    /// platform constant.
    ///
    /// Default: `None`.
    pub max_path_len: Option<u32>,

    /// Maximum number of consecutive metadata entries before an actual entry.
    ///
    /// Prevents infinite loops from malformed archives that contain only
    /// metadata entries (GNU long name, PAX headers) without actual file entries.
    /// Exceeding this limit will cause a [`ParseError::TooManyPendingEntries`] error.
    ///
    /// Default: 16 entries.
    pub max_pending_entries: usize,

    /// Maximum number of sparse data entries (chunks) in a sparse file.
    ///
    /// Prevents unbounded memory allocation from a malicious archive that
    /// claims an enormous number of sparse regions (see CVE-2025-58183 for
    /// a similar issue in Go's `archive/tar`).
    ///
    /// For old GNU sparse format, each 512-byte extension block holds 21
    /// descriptors, so 1000 entries requires ~48 extension blocks (~24 KiB).
    ///
    /// Default: 10000.
    pub max_sparse_entries: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_metadata_size: 1024 * 1024, // 1 MiB
            max_path_len: None,
            max_pending_entries: 16,
            max_sparse_entries: 10_000,
        }
    }
}

impl Limits {
    /// Create a new `Limits` with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create permissive limits suitable for trusted archives.
    ///
    /// This sets very high limits that effectively disable most checks.
    /// Only use this for archives from trusted sources.
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            max_metadata_size: u32::MAX,
            max_path_len: None,
            max_pending_entries: usize::MAX,
            max_sparse_entries: 1_000_000,
        }
    }

    /// Check a path length against the configured limit.
    ///
    /// Returns `Ok(())` if the path is within the limit (or no limit is set),
    /// or `Err(ParseError::PathTooLong)` if it exceeds it.
    pub fn check_path_len(&self, len: usize) -> Result<()> {
        if let Some(limit) = self.max_path_len {
            if len > limit as usize {
                return Err(ParseError::PathTooLong { len, limit });
            }
        }
        Ok(())
    }
}

// ============================================================================
// Errors
// ============================================================================

/// Errors that can occur during tar archive parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    /// I/O error from the underlying reader.
    #[cfg(feature = "std")]
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Header parsing error (checksum, invalid octal, etc.).
    #[error("header error: {0}")]
    Header(#[from] HeaderError),

    /// PAX extension parsing error.
    #[error("PAX error: {0}")]
    Pax(#[from] PaxError),

    /// Invalid UTF-8 in PAX key.
    #[error("invalid UTF-8 in PAX key: {0}")]
    InvalidUtf8(#[from] Utf8Error),

    /// Path exceeds configured maximum length.
    #[error("path exceeds limit: {len} bytes > {limit} bytes")]
    PathTooLong {
        /// Actual path length.
        len: usize,
        /// Configured limit.
        limit: u32,
    },

    /// Extension metadata exceeds configured maximum size.
    ///
    /// The aggregate size of all extension data (PAX headers, GNU long
    /// name/link) for a single entry exceeded [`Limits::max_metadata_size`].
    #[error("metadata exceeds limit: {size} bytes > {limit} bytes")]
    MetadataTooLarge {
        /// Total metadata size that would result.
        size: u64,
        /// Configured limit.
        limit: u32,
    },

    /// Duplicate GNU long name entry without an intervening actual entry.
    #[error("duplicate GNU long name entry")]
    DuplicateGnuLongName,

    /// Duplicate GNU long link entry without an intervening actual entry.
    #[error("duplicate GNU long link entry")]
    DuplicateGnuLongLink,

    /// Duplicate PAX extended header without an intervening actual entry.
    #[error("duplicate PAX extended header")]
    DuplicatePaxHeader,

    /// Metadata entries (GNU long name, PAX, etc.) found but no actual entry followed.
    #[error("metadata entries without a following actual entry")]
    OrphanedMetadata,

    /// Too many consecutive metadata entries (possible infinite loop or malicious archive).
    #[error("too many pending metadata entries: {count} > {limit}")]
    TooManyPendingEntries {
        /// Number of pending metadata entries.
        count: usize,
        /// Configured limit.
        limit: usize,
    },

    /// Too many sparse entries (possible denial-of-service attack).
    #[error("too many sparse entries: {count} > {limit}")]
    TooManySparseEntries {
        /// Number of sparse entries found.
        count: usize,
        /// Configured limit.
        limit: usize,
    },

    /// Sparse entry type present but header is not GNU format.
    #[error("sparse entry type but header is not GNU format")]
    SparseNotGnu,

    /// A PAX sparse map field is malformed.
    #[error("invalid PAX sparse map: {0}")]
    InvalidPaxSparseMap(Cow<'static, str>),

    /// A PAX extension value failed to parse.
    #[error("invalid PAX {key} value: {value:?}")]
    InvalidPaxValue {
        /// The PAX key (e.g. "uid", "size").
        key: &'static str,
        /// The raw value string.
        value: Cow<'static, str>,
    },

    /// Entry path is empty after applying all overrides (GNU long name, PAX path, etc.).
    #[error("entry has empty path")]
    EmptyPath,

    /// Entry size in header is invalid (e.g., overflow when computing padded size).
    #[error("invalid entry size: {0}")]
    InvalidSize(u64),

    /// Unexpected EOF while reading entry content or padding.
    #[error("unexpected EOF at position {pos}")]
    UnexpectedEof {
        /// Position in the stream where EOF occurred.
        pos: u64,
    },
}

/// Result type for parsing operations.
pub type Result<T> = core::result::Result<T, ParseError>;

// ============================================================================
// Parser
// ============================================================================

/// Events emitted by the sans-IO parser.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ParseEvent<'a> {
    /// Need more data to continue parsing.
    ///
    /// No bytes are consumed from the input when this event is returned.
    /// The caller should ensure at least `min_bytes` bytes are available
    /// before calling `parse` again with the same (or larger) buffer.
    NeedData {
        /// Minimum number of bytes needed to make progress.
        min_bytes: usize,
    },

    /// A complete entry header has been parsed.
    ///
    /// The entry contains resolved metadata (path, link target, etc.) with
    /// GNU long name/link and PAX extensions applied.
    ///
    /// After this event, the caller must read or skip `entry.size` bytes
    /// of content plus padding to the next 512-byte boundary before
    /// calling `parse()` again with the next header bytes.
    Entry {
        /// Number of bytes consumed from the input for this entry's header(s).
        consumed: usize,
        /// The parsed entry with resolved metadata.
        entry: ParsedEntry<'a>,
    },

    /// A GNU sparse file entry has been parsed.
    ///
    /// This is emitted instead of [`Entry`](ParseEvent::Entry) when the entry
    /// type is `GnuSparse` (type 'S'). The sparse map describes which regions
    /// of the logical file contain real data; gaps are implicitly zero-filled.
    ///
    /// After this event, the caller must read or skip `entry.size` bytes
    /// of content (the on-disk data for the sparse regions) plus padding to
    /// the next 512-byte boundary before calling `parse()` again. The
    /// `consumed` count already includes any GNU sparse extension blocks
    /// that followed the header.
    SparseEntry {
        /// Number of bytes consumed from the input for this entry's header(s),
        /// including any GNU sparse extension blocks.
        consumed: usize,
        /// The parsed entry with resolved metadata.
        /// `entry.size` is the on-disk content size (sum of sparse chunk
        /// lengths). The logical file size is `real_size`.
        entry: ParsedEntry<'a>,
        /// The sparse data map: regions of real data within the logical file.
        sparse_map: Vec<SparseEntry>,
        /// The logical (uncompressed) size of the file, from the GNU header's
        /// `realsize` field.
        real_size: u64,
    },

    /// A PAX global extended header (type 'g') has been parsed.
    ///
    /// Per POSIX, global headers apply default attributes to all subsequent
    /// entries in the archive. However, this parser does **not** apply them
    /// automatically — it surfaces the raw data so the caller can decide
    /// how to handle it (e.g., merge into a defaults table, ignore, etc.).
    ///
    /// The `pax_data` can be parsed with
    /// [`PaxExtensions::new`](crate::PaxExtensions::new).
    GlobalExtensions {
        /// Number of bytes consumed from the input (header + padded content).
        consumed: usize,
        /// The raw PAX key-value data from the global header.
        pax_data: &'a [u8],
    },

    /// Archive end marker reached (two consecutive zero blocks, or clean EOF).
    End {
        /// Number of bytes consumed from the input.
        consumed: usize,
    },
}

impl<'a> ParseEvent<'a> {
    /// Adjust byte offsets in this event to account for `n` bytes that were
    /// already consumed from the front of the original input before the
    /// sub-slice was handed to a recursive `parse_header` call.
    ///
    /// For `Entry`, `SparseEntry`, and `End`, `n` is added to `consumed`.
    ///
    /// For `NeedData`, `n` is added to `min_bytes` so the requirement is
    /// expressed relative to the *original* input buffer, not the sub-slice.
    fn add_consumed(self, n: usize) -> Self {
        match self {
            ParseEvent::NeedData { min_bytes } => ParseEvent::NeedData {
                min_bytes: min_bytes.saturating_add(n),
            },
            ParseEvent::Entry { consumed, entry } => ParseEvent::Entry {
                consumed: consumed.saturating_add(n),
                entry,
            },
            ParseEvent::SparseEntry {
                consumed,
                entry,
                sparse_map,
                real_size,
            } => ParseEvent::SparseEntry {
                consumed: consumed.saturating_add(n),
                entry,
                sparse_map,
                real_size,
            },
            ParseEvent::GlobalExtensions { consumed, pax_data } => ParseEvent::GlobalExtensions {
                consumed: consumed.saturating_add(n),
                pax_data,
            },
            ParseEvent::End { consumed } => ParseEvent::End {
                consumed: consumed.saturating_add(n),
            },
        }
    }
}

/// A fully-resolved tar entry with all extensions applied.
///
/// Borrowed data comes from the input slice, so the entry is valid only
/// as long as the input buffer is live.
#[derive(Debug)]
pub struct ParsedEntry<'a> {
    /// The raw 512-byte header.
    pub header: &'a Header,

    /// The entry type (Regular, Directory, Symlink, etc.).
    pub entry_type: EntryType,

    /// The resolved file path.
    ///
    /// Priority: PAX `path` > GNU long name > header `name` (+ UStar `prefix`).
    pub path: Cow<'a, [u8]>,

    /// The resolved link target (for symlinks and hardlinks).
    ///
    /// Priority: PAX `linkpath` > GNU long link > header `linkname`.
    pub link_target: Option<Cow<'a, [u8]>>,

    /// File mode/permissions.
    pub mode: u32,

    /// Owner UID.
    pub uid: u64,

    /// Owner GID.
    pub gid: u64,

    /// Modification time as Unix timestamp.
    pub mtime: u64,

    /// Content size in bytes.
    pub size: u64,

    /// User name.
    pub uname: Option<Cow<'a, [u8]>>,

    /// Group name.
    pub gname: Option<Cow<'a, [u8]>>,

    /// Device major number (for block/char devices).
    pub dev_major: Option<u32>,

    /// Device minor number (for block/char devices).
    pub dev_minor: Option<u32>,

    /// Extended attributes from PAX `SCHILY.xattr.*` entries.
    #[allow(clippy::type_complexity)]
    pub xattrs: Vec<(Cow<'a, [u8]>, Cow<'a, [u8]>)>,

    /// Raw PAX extended header data, if a PAX `'x'` entry preceded this entry.
    ///
    /// This is the unprocessed content of the PAX extension entry, preserved
    /// so that callers can iterate all PAX key-value pairs (not just the ones
    /// tar-core resolves into struct fields). Parse it with
    /// [`PaxExtensions::new`](crate::PaxExtensions::new).
    pub pax: Option<&'a [u8]>,
}

impl<'a> ParsedEntry<'a> {
    /// Get the path as a lossy UTF-8 string.
    #[must_use]
    pub fn path_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.path)
    }

    /// Get the link target as a lossy UTF-8 string, if present.
    #[must_use]
    pub fn link_target_lossy(&self) -> Option<Cow<'_, str>> {
        self.link_target
            .as_ref()
            .map(|t| String::from_utf8_lossy(t))
    }

    /// Check if this is a regular file.
    #[must_use]
    pub fn is_file(&self) -> bool {
        self.entry_type.is_file()
    }

    /// Check if this is a directory.
    #[must_use]
    pub fn is_dir(&self) -> bool {
        self.entry_type.is_dir()
    }

    /// Check if this is a symbolic link.
    #[must_use]
    pub fn is_symlink(&self) -> bool {
        self.entry_type.is_symlink()
    }

    /// Check if this is a hard link.
    #[must_use]
    pub fn is_hard_link(&self) -> bool {
        self.entry_type.is_hard_link()
    }

    /// Get the padded size (rounded up to block boundary).
    #[must_use]
    pub fn padded_size(&self) -> u64 {
        self.size.next_multiple_of(HEADER_SIZE as u64)
    }
}

/// Internal parser state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Waiting to read a header.
    ReadHeader,
    /// Archive is complete.
    Done,
}

/// The kind of extension header being processed.
#[derive(Debug, Clone, Copy)]
enum ExtensionKind {
    GnuLongName,
    GnuLongLink,
    Pax,
}

/// Borrowed extension data accumulated during recursive extension processing.
///
/// This is threaded through `parse_header` → `handle_extension` → `parse_header`
/// calls within a single `parse()` invocation. Since extension chains are always
/// fully resolved within one call (or discarded on `NeedData`), we can borrow
/// directly from the input slice — no allocation needed.
#[derive(Debug, Default, Clone, Copy)]
struct PendingMetadata<'a> {
    gnu_long_name: Option<&'a [u8]>,
    gnu_long_link: Option<&'a [u8]>,
    pax_extensions: Option<&'a [u8]>,
    count: usize,
    /// Running total of all extension data bytes accumulated so far.
    metadata_size: u64,
}

/// Context for GNU sparse entries, passed from `handle_gnu_sparse` to
/// `emit_entry` to produce a `ParseEvent::SparseEntry`.
struct SparseContext {
    sparse_map: Vec<SparseEntry>,
    real_size: u64,
    /// Number of bytes consumed by extension blocks (not counting the
    /// main header itself).
    ext_consumed: usize,
}

impl PendingMetadata<'_> {
    fn is_empty(&self) -> bool {
        self.gnu_long_name.is_none()
            && self.gnu_long_link.is_none()
            && self.pax_extensions.is_none()
    }
}

/// Check PAX extensions for GNU sparse version.
///
/// Returns `Some((major, minor))` if `GNU.sparse.major` and
/// `GNU.sparse.minor` are both present and parseable, `None` if
/// the keys are absent. When `ignore_errors` is true, malformed values
/// are silently skipped instead of producing errors.
fn pax_sparse_version(pax: &[u8], ignore_errors: bool) -> Result<Option<(u64, u64)>> {
    let mut major = None;
    let mut minor = None;
    for ext in PaxExtensions::new(pax) {
        let ext = ext?;
        let key = match ext.key() {
            Ok(k) => k,
            Err(_) if ignore_errors => continue,
            Err(e) => return Err(ParseError::from(e)),
        };
        match key {
            PAX_GNU_SPARSE_MAJOR => {
                let s = match ext.value() {
                    Ok(s) => s,
                    Err(_) if ignore_errors => continue,
                    Err(_) => {
                        return Err(ParseError::InvalidPaxValue {
                            key: PAX_GNU_SPARSE_MAJOR,
                            value: Cow::Borrowed("<non-UTF-8>"),
                        })
                    }
                };
                match s.parse::<u64>() {
                    Ok(v) => major = Some(v),
                    Err(_) if ignore_errors => {}
                    Err(_) => {
                        return Err(ParseError::InvalidPaxValue {
                            key: PAX_GNU_SPARSE_MAJOR,
                            value: s.to_owned().into(),
                        })
                    }
                }
            }
            PAX_GNU_SPARSE_MINOR => {
                let s = match ext.value() {
                    Ok(s) => s,
                    Err(_) if ignore_errors => continue,
                    Err(_) => {
                        return Err(ParseError::InvalidPaxValue {
                            key: PAX_GNU_SPARSE_MINOR,
                            value: Cow::Borrowed("<non-UTF-8>"),
                        })
                    }
                };
                match s.parse::<u64>() {
                    Ok(v) => minor = Some(v),
                    Err(_) if ignore_errors => {}
                    Err(_) => {
                        return Err(ParseError::InvalidPaxValue {
                            key: PAX_GNU_SPARSE_MINOR,
                            value: s.to_owned().into(),
                        })
                    }
                }
            }
            _ => {}
        }
        if major.is_some() && minor.is_some() {
            break;
        }
    }
    match (major, minor) {
        (Some(maj), Some(min)) => Ok(Some((maj, min))),
        _ => Ok(None),
    }
}

/// Sans-IO tar archive parser.
///
/// This parser operates as a state machine on `&[u8]` input slices.
/// It does not perform any I/O itself - the caller is responsible for
/// providing data and handling the parsed events.
///
/// # Usage
///
/// The caller feeds header bytes to `parse()`. On `Entry`, the caller
/// reads/skips `entry.size` bytes of content (plus padding to the next
/// 512-byte boundary) from its own I/O source, then calls `parse()`
/// again with the next header bytes. The parser does not see or track
/// content bytes.
///
/// ```ignore
/// let mut parser = Parser::new(Limits::default());
/// let mut buf = vec![0u8; 65536];
/// let mut filled = 0;
///
/// loop {
///     match parser.parse(&buf[..filled]) {
///         Ok(ParseEvent::NeedData { min_bytes }) => {
///             let n = read_more(&mut buf[filled..])?;
///             filled += n;
///             if n == 0 && filled < min_bytes {
///                 return Err("unexpected EOF");
///             }
///         }
///         Ok(ParseEvent::Entry { consumed, entry }) => {
///             process_entry(&entry);
///             // Read/skip entry.size bytes + padding, then clear buf
///             skip_content(entry.padded_size())?;
///             filled = 0;
///         }
///         Ok(ParseEvent::End { .. }) => break,
///         Err(e) => return Err(e),
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Parser {
    limits: Limits,
    state: State,
    /// When true, entries with empty paths are allowed through instead of
    /// returning [`ParseError::EmptyPath`].
    allow_empty_path: bool,
    /// When false, header checksum verification is skipped. This is useful
    /// for fuzzing, where random input almost never has valid checksums,
    /// preventing the fuzzer from exercising deeper parser logic.
    ///
    /// Default: `true`.
    verify_checksums: bool,
    /// When true, malformed PAX extension values (invalid UTF-8, unparseable
    /// integers for uid/gid/size/mtime) are silently skipped instead of
    /// producing errors. This matches the behavior of many real-world tar
    /// implementations.
    ///
    /// Default: `false`.
    ignore_pax_errors: bool,
}

impl Parser {
    /// Create a new parser with the given limits.
    #[must_use]
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            state: State::ReadHeader,
            allow_empty_path: false,
            verify_checksums: true,
            ignore_pax_errors: false,
        }
    }

    /// Allow entries with empty paths instead of rejecting them with
    /// [`ParseError::EmptyPath`].
    pub fn set_allow_empty_path(&mut self, allow: bool) {
        self.allow_empty_path = allow;
    }

    /// Control whether header checksums are verified during parsing.
    ///
    /// When set to `false`, the parser skips [`Header::verify_checksum`]
    /// calls, accepting headers regardless of their checksum field. This
    /// is primarily useful for fuzz testing, where random input almost
    /// never produces valid checksums, preventing the fuzzer from reaching
    /// deeper parser code paths.
    ///
    /// Default: `true`.
    pub fn set_verify_checksums(&mut self, verify: bool) {
        self.verify_checksums = verify;
    }

    /// Control whether malformed PAX extension values are silently ignored.
    ///
    /// When set to `true`, PAX values that fail to parse (invalid UTF-8,
    /// unparseable integers for `uid`, `gid`, `size`, `mtime`) are skipped
    /// instead of producing [`ParseError::InvalidPaxValue`] errors. This
    /// matches the lenient behavior of many real-world tar implementations.
    ///
    /// Default: `false` (malformed values produce errors).
    pub fn set_ignore_pax_errors(&mut self, ignore: bool) {
        self.ignore_pax_errors = ignore;
    }

    /// Create a new parser with default limits.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(Limits::default())
    }

    /// Get the current limits.
    #[must_use]
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Check if the parser is done (archive complete).
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.state == State::Done
    }

    /// Parse the next event from the input buffer.
    ///
    /// Returns a [`ParseEvent`] on success. `Entry` and `End` events include
    /// a `consumed` field indicating how many bytes were consumed from the
    /// input; the caller should advance past that many bytes in their buffer.
    ///
    /// # Events
    ///
    /// - `NeedData { min_bytes }`: Need at least `min_bytes` more data (nothing consumed)
    /// - `Entry { consumed, entry }`: A complete entry header; caller must handle content
    /// - `End { consumed }`: Archive is complete
    ///
    /// After receiving an `Entry` event, the caller is responsible for
    /// reading or skipping `entry.size` bytes of content (plus padding to
    /// the next 512-byte boundary) before calling `parse()` again.
    pub fn parse<'a>(&mut self, input: &'a [u8]) -> Result<ParseEvent<'a>> {
        match self.state {
            State::Done => Ok(ParseEvent::End { consumed: 0 }),
            State::ReadHeader => self.parse_header(input, PendingMetadata::default()),
        }
    }

    /// Parse a header from the input.
    fn parse_header<'a>(
        &mut self,
        input: &'a [u8],
        slices: PendingMetadata<'a>,
    ) -> Result<ParseEvent<'a>> {
        // Need at least one header block
        if input.len() < HEADER_SIZE {
            return Ok(ParseEvent::NeedData {
                min_bytes: HEADER_SIZE,
            });
        }

        // Check for zero block (end of archive marker).
        //
        // NB: No state mutation happens before a potential NeedData return,
        // so the caller can safely retry with more data.
        let header_bytes: &[u8; HEADER_SIZE] = input[..HEADER_SIZE]
            .try_into()
            .expect("already checked input.len() >= HEADER_SIZE");
        if header_bytes.iter().all(|&b| b == 0) {
            // Need a second block to decide whether this is end-of-archive
            // or a stray zero block.
            if input.len() < 2 * HEADER_SIZE {
                return Ok(ParseEvent::NeedData {
                    min_bytes: 2 * HEADER_SIZE,
                });
            }
            // Check second block
            let second_block = &input[HEADER_SIZE..2 * HEADER_SIZE];
            if second_block.iter().all(|&b| b == 0) {
                self.state = State::Done;
                if !slices.is_empty() {
                    return Err(ParseError::OrphanedMetadata);
                }
                return Ok(ParseEvent::End {
                    consumed: 2 * HEADER_SIZE,
                });
            }
            // Not end of archive — single stray zero block; skip it and
            // continue with the next block as a header.
            return self
                .parse_header(&input[HEADER_SIZE..], slices)
                .map(|e| e.add_consumed(HEADER_SIZE));
        }

        // Check pending entry limit
        if slices.count > self.limits.max_pending_entries {
            return Err(ParseError::TooManyPendingEntries {
                count: slices.count,
                limit: self.limits.max_pending_entries,
            });
        }

        // Parse header
        let header = Header::from_bytes(header_bytes);
        if self.verify_checksums {
            header.verify_checksum()?;
        }

        let entry_type = header.entry_type();
        let size = header.entry_size()?;
        let padded_size = size
            .checked_next_multiple_of(HEADER_SIZE as u64)
            .ok_or(ParseError::InvalidSize(size))?;

        // Metadata entry types (GNU long name/link, PAX headers, GNU sparse)
        // only make sense in archives that actually use those formats. A V7-
        // style header whose type flag byte happens to be 'L' or 'x' should
        // be treated as a regular entry, not as a metadata extension. This
        // matches tar-rs's `is_recognized_header` guard.
        let is_extension_format = header.is_gnu() || header.is_ustar();
        match entry_type {
            EntryType::GnuLongName if is_extension_format => {
                self.handle_extension(input, size, padded_size, ExtensionKind::GnuLongName, slices)
            }
            EntryType::GnuLongLink if is_extension_format => {
                self.handle_extension(input, size, padded_size, ExtensionKind::GnuLongLink, slices)
            }
            EntryType::XHeader if is_extension_format => {
                self.handle_extension(input, size, padded_size, ExtensionKind::Pax, slices)
            }
            // Global PAX headers (type 'g') are defined by POSIX
            // independently of the UStar/GNU magic, so we always handle
            // them here. Routing through emit_entry would fail because
            // global headers have arbitrary metadata fields.
            EntryType::XGlobalHeader => {
                // Check size limit
                if size > self.limits.max_metadata_size as u64 {
                    return Err(ParseError::MetadataTooLarge {
                        size,
                        limit: self.limits.max_metadata_size,
                    });
                }

                let total_size = (HEADER_SIZE as u64)
                    .checked_add(padded_size)
                    .ok_or(ParseError::InvalidSize(size))?;
                if (input.len() as u64) < total_size {
                    return Ok(ParseEvent::NeedData {
                        min_bytes: total_size as usize,
                    });
                }

                let content_start = HEADER_SIZE;
                let content_end = content_start + size as usize;
                let pax_data = &input[content_start..content_end];

                Ok(ParseEvent::GlobalExtensions {
                    consumed: total_size as usize,
                    pax_data,
                })
            }
            EntryType::GnuSparse if is_extension_format => {
                self.handle_gnu_sparse(input, header, size, slices)
            }
            _ => {
                // Check for PAX v1.0 sparse before emitting — it requires
                // reading the sparse map from the data stream.
                let sparse_version = if let Some(pax) = slices.pax_extensions {
                    pax_sparse_version(pax, self.ignore_pax_errors)?
                } else {
                    None
                };
                if sparse_version == Some((1, 0)) {
                    self.handle_pax_sparse_v1(input, header, size, slices)
                } else {
                    // Actual entry — emit_entry handles v0.0/v0.1 PAX sparse
                    // inline during PAX extension processing.
                    self.emit_entry(header, size, None, slices)
                }
            }
        }
    }

    /// Process a GNU long name/link or PAX extension entry.
    ///
    /// Extracts the extension data as a borrowed slice (zero-copy), adds it
    /// to `slices`, and recurses to parse the next header. No state is stored
    /// in `self`, so on `NeedData` the recursion simply unwinds — the caller
    /// retries from scratch, re-parsing the extension chain.
    fn handle_extension<'a>(
        &mut self,
        input: &'a [u8],
        size: u64,
        padded_size: u64,
        kind: ExtensionKind,
        slices: PendingMetadata<'a>,
    ) -> Result<ParseEvent<'a>> {
        // Check for duplicate
        let has_dup = match kind {
            ExtensionKind::GnuLongName => slices.gnu_long_name.is_some(),
            ExtensionKind::GnuLongLink => slices.gnu_long_link.is_some(),
            ExtensionKind::Pax => slices.pax_extensions.is_some(),
        };
        if has_dup {
            return Err(match kind {
                ExtensionKind::GnuLongName => ParseError::DuplicateGnuLongName,
                ExtensionKind::GnuLongLink => ParseError::DuplicateGnuLongLink,
                ExtensionKind::Pax => ParseError::DuplicatePaxHeader,
            });
        }

        // Check aggregate metadata size limit
        let new_metadata_size = slices.metadata_size + size;
        if new_metadata_size > self.limits.max_metadata_size as u64 {
            return Err(ParseError::MetadataTooLarge {
                size: new_metadata_size,
                limit: self.limits.max_metadata_size,
            });
        }

        let total_size = (HEADER_SIZE as u64)
            .checked_add(padded_size)
            .ok_or(ParseError::InvalidSize(size))?;
        if (input.len() as u64) < total_size {
            return Ok(ParseEvent::NeedData {
                min_bytes: total_size as usize,
            });
        }

        // Extract content as a borrowed slice (zero-copy)
        let content_start = HEADER_SIZE;
        let content_end = content_start + size as usize;
        let mut data: &'a [u8] = &input[content_start..content_end];

        // Strip trailing null for GNU long name/link
        if matches!(
            kind,
            ExtensionKind::GnuLongName | ExtensionKind::GnuLongLink
        ) {
            if let Some(trimmed) = data.strip_suffix(&[0]) {
                data = trimmed;
            }
            self.limits.check_path_len(data.len())?;
        }

        // Build new pending metadata with the added extension data
        let mut new_slices = PendingMetadata {
            count: slices.count + 1,
            metadata_size: new_metadata_size,
            ..slices
        };
        match kind {
            ExtensionKind::GnuLongName => new_slices.gnu_long_name = Some(data),
            ExtensionKind::GnuLongLink => new_slices.gnu_long_link = Some(data),
            ExtensionKind::Pax => new_slices.pax_extensions = Some(data),
        }

        self.parse_header(&input[total_size as usize..], new_slices)
            .map(|e| e.add_consumed(total_size as usize))
    }

    /// Handle a PAX v1.0 sparse entry.
    ///
    /// The sparse map is encoded as newline-delimited decimal values at
    /// the start of the file's data block:
    ///
    /// ```text
    /// <num_entries>\n
    /// <offset_0>\n
    /// <length_0>\n
    /// ...
    /// ```
    ///
    /// followed by padding to the next 512-byte boundary. This prefix is
    /// consumed by the parser and not included in the entry's content.
    fn handle_pax_sparse_v1<'a>(
        &mut self,
        input: &'a [u8],
        header: &'a Header,
        size: u64,
        slices: PendingMetadata<'a>,
    ) -> Result<ParseEvent<'a>> {
        // Extract sparse metadata from PAX extensions.
        let pax = slices
            .pax_extensions
            .ok_or(ParseError::InvalidPaxSparseMap(Cow::Borrowed(
                "missing PAX extensions",
            )))?;

        let ignore_errors = self.ignore_pax_errors;
        let mut real_size = None;
        let mut sparse_name = None;
        for ext in PaxExtensions::new(pax) {
            let ext = ext?;
            let key = match ext.key() {
                Ok(k) => k,
                Err(_) if ignore_errors => continue,
                Err(e) => return Err(ParseError::from(e)),
            };
            match key {
                PAX_GNU_SPARSE_REALSIZE | PAX_GNU_SPARSE_SIZE => {
                    let s = match ext.value() {
                        Ok(s) => s,
                        Err(_) if ignore_errors => continue,
                        Err(_) => {
                            return Err(ParseError::InvalidPaxValue {
                                key: PAX_GNU_SPARSE_REALSIZE,
                                value: Cow::Borrowed("<non-UTF-8>"),
                            })
                        }
                    };
                    match s.parse::<u64>() {
                        Ok(v) => real_size = Some(v),
                        Err(_) if ignore_errors => {}
                        Err(_) => {
                            return Err(ParseError::InvalidPaxValue {
                                key: PAX_GNU_SPARSE_REALSIZE,
                                value: s.to_owned().into(),
                            })
                        }
                    }
                }
                PAX_GNU_SPARSE_NAME => {
                    sparse_name = Some(ext.value_bytes());
                }
                _ => {}
            }
        }

        let real_size = real_size.ok_or(ParseError::InvalidPaxSparseMap(Cow::Borrowed(
            "missing GNU.sparse.realsize",
        )))?;

        // The sparse map data starts right after the header (at offset
        // HEADER_SIZE within the input). We need to parse it without
        // knowing its exact size upfront — we read line by line.
        //
        // To remain sans-IO, we scan the available input. If we don't
        // have enough, return NeedData.
        let data_start = HEADER_SIZE;
        let data = &input[data_start..];

        // Parse newline-delimited sparse map.
        let mut pos = 0usize;

        // Helper: read next decimal line from data[pos..]
        let read_line = |data: &[u8], pos: &mut usize| -> Option<Result<u64>> {
            let remaining = &data[*pos..];
            let nl = remaining.iter().position(|&b| b == b'\n')?;
            let line = &remaining[..nl];
            *pos += nl + 1;
            let s = match core::str::from_utf8(line) {
                Ok(s) => s,
                Err(_) => {
                    return Some(Err(ParseError::InvalidPaxSparseMap(Cow::Borrowed(
                        "non-UTF8 in sparse map",
                    ))))
                }
            };
            match s.parse::<u64>() {
                Ok(v) => Some(Ok(v)),
                Err(_) => Some(Err(ParseError::InvalidPaxSparseMap(
                    format!("invalid decimal: {s:?}").into(),
                ))),
            }
        };

        // Read the entry count.
        let num_entries = match read_line(data, &mut pos) {
            Some(r) => r?,
            None => {
                // Need more data — we need at least enough to see the
                // first newline. Request a generous amount.
                return Ok(ParseEvent::NeedData {
                    min_bytes: data_start + pos + HEADER_SIZE,
                });
            }
        };

        if num_entries as usize > self.limits.max_sparse_entries {
            return Err(ParseError::TooManySparseEntries {
                count: num_entries as usize,
                limit: self.limits.max_sparse_entries,
            });
        }

        // Cap pre-allocation to avoid trusting the claimed count for memory.
        // The actual loop below will still process exactly num_entries items.
        let mut sparse_map = Vec::with_capacity((num_entries as usize).min(1024));
        for _ in 0..num_entries {
            let offset = match read_line(data, &mut pos) {
                Some(r) => r?,
                None => {
                    return Ok(ParseEvent::NeedData {
                        min_bytes: data_start + pos + HEADER_SIZE,
                    });
                }
            };
            let length = match read_line(data, &mut pos) {
                Some(r) => r?,
                None => {
                    return Ok(ParseEvent::NeedData {
                        min_bytes: data_start + pos + HEADER_SIZE,
                    });
                }
            };
            sparse_map.push(SparseEntry { offset, length });
        }

        // The sparse map data is padded to a 512-byte boundary.
        let map_size = pos.next_multiple_of(HEADER_SIZE);

        // Verify we have enough input for the padded map.
        if data.len() < map_size {
            return Ok(ParseEvent::NeedData {
                min_bytes: data_start + map_size,
            });
        }

        // The remaining content size is the original size minus the
        // sparse map prefix (including padding).
        let content_size =
            size.checked_sub(map_size as u64)
                .ok_or(ParseError::InvalidPaxSparseMap(Cow::Borrowed(
                    "sparse map prefix larger than entry size",
                )))?;

        let sparse_ctx = SparseContext {
            sparse_map,
            real_size,
            // Extension consumed = the sparse map data prefix.
            ext_consumed: map_size,
        };

        // Override the path with GNU.sparse.name if present by
        // stashing it in the slices so emit_entry picks it up.
        let slices = if let Some(name) = sparse_name {
            PendingMetadata {
                gnu_long_name: Some(name),
                ..slices
            }
        } else {
            slices
        };

        self.emit_entry(header, content_size, Some(sparse_ctx), slices)
    }

    /// Handle a GNU sparse entry (type 'S').
    ///
    /// Reads the inline sparse descriptors from the GNU header and any
    /// extension blocks that follow. Returns NeedData if the extension
    /// blocks aren't fully available yet (side-effect-free: no state is
    /// mutated before we know we have enough data).
    fn handle_gnu_sparse<'a>(
        &mut self,
        input: &'a [u8],
        header: &'a Header,
        size: u64,
        slices: PendingMetadata<'a>,
    ) -> Result<ParseEvent<'a>> {
        let gnu = header.try_as_gnu().ok_or(ParseError::SparseNotGnu)?;
        let real_size = gnu.real_size()?;

        // Collect sparse entries from the 4 inline descriptors.
        let mut sparse_map = Vec::new();
        for desc in &gnu.sparse {
            if desc.is_empty() {
                break;
            }
            let entry = desc.to_sparse_entry()?;
            sparse_map.push(entry);
        }

        // If there are extension blocks, we need to read them all.
        // Each extension block is 512 bytes and may chain to the next.
        // We must not mutate any state before we know we have enough input,
        // so we scan forward to find all extension blocks first.
        let mut ext_consumed = 0usize;
        if gnu.is_extended() {
            let mut offset = HEADER_SIZE; // start past the main header
            loop {
                if input.len() < offset + HEADER_SIZE {
                    return Ok(ParseEvent::NeedData {
                        min_bytes: offset + HEADER_SIZE,
                    });
                }

                let ext_bytes: &[u8; HEADER_SIZE] = input[offset..offset + HEADER_SIZE]
                    .try_into()
                    .expect("checked length");
                let ext = GnuExtSparseHeader::ref_from_bytes(ext_bytes)
                    .expect("GnuExtSparseHeader is 512 bytes");

                for desc in &ext.sparse {
                    if desc.is_empty() {
                        break;
                    }
                    if sparse_map.len() >= self.limits.max_sparse_entries {
                        return Err(ParseError::TooManySparseEntries {
                            count: sparse_map.len() + 1,
                            limit: self.limits.max_sparse_entries,
                        });
                    }
                    let entry = desc.to_sparse_entry()?;
                    sparse_map.push(entry);
                }

                offset += HEADER_SIZE;

                if !ext.is_extended() {
                    break;
                }
            }
            ext_consumed = offset - HEADER_SIZE; // bytes consumed by extension blocks
        }

        // Also check the inline descriptors against the limit.
        if sparse_map.len() > self.limits.max_sparse_entries {
            return Err(ParseError::TooManySparseEntries {
                count: sparse_map.len(),
                limit: self.limits.max_sparse_entries,
            });
        }

        let sparse_ctx = SparseContext {
            sparse_map,
            real_size,
            ext_consumed,
        };

        self.emit_entry(header, size, Some(sparse_ctx), slices)
    }

    fn emit_entry<'a>(
        &mut self,
        header: &'a Header,
        size: u64,
        sparse: Option<SparseContext>,
        slices: PendingMetadata<'a>,
    ) -> Result<ParseEvent<'a>> {
        // Start with header values
        let mut path: Cow<'a, [u8]> = Cow::Borrowed(header.path_bytes());
        let mut link_target: Option<Cow<'a, [u8]>> = None;
        let mut uid = header.uid()?;
        let mut gid = header.gid()?;
        let mut mtime = header.mtime()?;
        let mut entry_size = size;
        let mut xattrs = Vec::new();
        let mut uname: Option<Cow<'a, [u8]>> = header
            .username()
            .filter(|b| !b.is_empty())
            .map(Cow::Borrowed);
        let mut gname: Option<Cow<'a, [u8]>> = header
            .groupname()
            .filter(|b| !b.is_empty())
            .map(Cow::Borrowed);

        // Handle UStar prefix for path
        if let Some(prefix) = header.prefix() {
            if !prefix.is_empty() {
                let mut full_path = prefix.to_vec();
                full_path.push(b'/');
                full_path.extend_from_slice(header.path_bytes());
                path = Cow::Owned(full_path);
            }
        }

        // Apply GNU long name (overrides header + prefix)
        if let Some(long_name) = slices.gnu_long_name {
            path = Cow::Borrowed(long_name);
        }

        // Apply GNU long link
        if let Some(long_link) = slices.gnu_long_link {
            link_target = Some(Cow::Borrowed(long_link));
        } else {
            let header_link = header.link_name_bytes();
            if !header_link.is_empty() {
                link_target = Some(Cow::Borrowed(header_link));
            }
        }

        // Apply PAX extensions (highest priority)
        let raw_pax = slices.pax_extensions;

        // PAX sparse v0.0/v0.1 tracking. v0.0 uses repeated offset/numbytes
        // pairs; v0.1 uses a single comma-separated map string.
        let mut pax_sparse_map: Option<Vec<SparseEntry>> = None;
        let mut pax_sparse_real_size: Option<u64> = None;
        let mut pax_sparse_name: Option<&'a [u8]> = None;
        // v0.0: current offset waiting for its numbytes pair
        let mut pax_sparse_pending_offset: Option<u64> = None;

        if let Some(pax) = raw_pax {
            let ignore_errors = self.ignore_pax_errors;
            let extensions = PaxExtensions::new(pax);

            // Helper: parse a PAX numeric value, returning Ok(None) when
            // ignore_pax_errors is set and the value is unparseable.
            let parse_pax_u64 =
                |ext: &crate::PaxExtension<'_>, key: &'static str| -> Result<Option<u64>> {
                    let s = match ext.value() {
                        Ok(s) => s,
                        Err(_) if ignore_errors => return Ok(None),
                        Err(_) => {
                            return Err(ParseError::InvalidPaxValue {
                                key,
                                value: Cow::Borrowed("<non-UTF-8>"),
                            })
                        }
                    };
                    match s.parse::<u64>() {
                        Ok(v) => Ok(Some(v)),
                        Err(_) if ignore_errors => Ok(None),
                        Err(_) => Err(ParseError::InvalidPaxValue {
                            key,
                            value: s.to_owned().into(),
                        }),
                    }
                };

            for ext in extensions {
                let ext = ext?;
                let key = ext.key().map_err(ParseError::from)?;
                let value = ext.value_bytes();

                match key {
                    PAX_PATH => {
                        self.limits.check_path_len(value.len())?;
                        path = Cow::Borrowed(value);
                    }
                    PAX_LINKPATH => {
                        self.limits.check_path_len(value.len())?;
                        link_target = Some(Cow::Borrowed(value));
                    }
                    PAX_SIZE => {
                        if let Some(v) = parse_pax_u64(&ext, PAX_SIZE)? {
                            entry_size = v;
                        }
                    }
                    PAX_UID => {
                        if let Some(v) = parse_pax_u64(&ext, PAX_UID)? {
                            uid = v;
                        }
                    }
                    PAX_GID => {
                        if let Some(v) = parse_pax_u64(&ext, PAX_GID)? {
                            gid = v;
                        }
                    }
                    PAX_MTIME => {
                        // mtime may have fractional seconds (e.g. "1234567890.5");
                        // parse only the integer part.
                        let s = match ext.value() {
                            Ok(s) => s,
                            Err(_) if ignore_errors => continue,
                            Err(_) => {
                                return Err(ParseError::InvalidPaxValue {
                                    key: PAX_MTIME,
                                    value: Cow::Borrowed("<non-UTF-8>"),
                                })
                            }
                        };
                        let int_part = s.split('.').next().unwrap_or(s);
                        match int_part.parse::<u64>() {
                            Ok(v) => mtime = v,
                            Err(_) if ignore_errors => {}
                            Err(_) => {
                                return Err(ParseError::InvalidPaxValue {
                                    key: PAX_MTIME,
                                    value: s.to_owned().into(),
                                })
                            }
                        }
                    }
                    PAX_UNAME => {
                        uname = Some(Cow::Borrowed(value));
                    }
                    PAX_GNAME => {
                        gname = Some(Cow::Borrowed(value));
                    }

                    // PAX sparse v0.0: repeated offset/numbytes pairs
                    PAX_GNU_SPARSE_OFFSET => {
                        let v = parse_pax_u64(&ext, PAX_GNU_SPARSE_OFFSET)?;
                        pax_sparse_pending_offset = v;
                    }
                    PAX_GNU_SPARSE_NUMBYTES => {
                        if let (Some(offset), Some(length)) = (
                            pax_sparse_pending_offset.take(),
                            parse_pax_u64(&ext, PAX_GNU_SPARSE_NUMBYTES)?,
                        ) {
                            let map = pax_sparse_map.get_or_insert_with(Vec::new);
                            if map.len() >= self.limits.max_sparse_entries {
                                return Err(ParseError::TooManySparseEntries {
                                    count: map.len() + 1,
                                    limit: self.limits.max_sparse_entries,
                                });
                            }
                            map.push(SparseEntry { offset, length });
                        }
                    }

                    // PAX sparse v0.1: comma-separated map
                    PAX_GNU_SPARSE_MAP => {
                        let s = match ext.value() {
                            Ok(s) => s,
                            Err(_) if ignore_errors => continue,
                            Err(_) => {
                                return Err(ParseError::InvalidPaxSparseMap(Cow::Borrowed(
                                    "non-UTF8 sparse map",
                                )))
                            }
                        };
                        let mut map = Vec::new();
                        let parts: Vec<&str> = s.split(',').filter(|p| !p.is_empty()).collect();
                        if parts.len() % 2 != 0 {
                            return Err(ParseError::InvalidPaxSparseMap(Cow::Borrowed(
                                "odd number of values in GNU.sparse.map",
                            )));
                        }
                        for pair in parts.chunks(2) {
                            if map.len() >= self.limits.max_sparse_entries {
                                return Err(ParseError::TooManySparseEntries {
                                    count: map.len() + 1,
                                    limit: self.limits.max_sparse_entries,
                                });
                            }
                            let offset = pair[0].parse::<u64>().map_err(|_| {
                                ParseError::InvalidPaxSparseMap(
                                    format!("invalid offset: {:?}", pair[0]).into(),
                                )
                            })?;
                            let length = pair[1].parse::<u64>().map_err(|_| {
                                ParseError::InvalidPaxSparseMap(
                                    format!("invalid length: {:?}", pair[1]).into(),
                                )
                            })?;
                            map.push(SparseEntry { offset, length });
                        }
                        pax_sparse_map = Some(map);
                    }

                    // PAX sparse: real size and name (shared across versions)
                    PAX_GNU_SPARSE_REALSIZE | PAX_GNU_SPARSE_SIZE => {
                        if let Some(v) = parse_pax_u64(&ext, PAX_GNU_SPARSE_REALSIZE)? {
                            pax_sparse_real_size = Some(v);
                        }
                    }
                    PAX_GNU_SPARSE_NAME => {
                        self.limits.check_path_len(value.len())?;
                        pax_sparse_name = Some(value);
                    }

                    // Skip version fields — already handled in
                    // pending_pax_sparse_version() for v1.0 routing.
                    PAX_GNU_SPARSE_MAJOR | PAX_GNU_SPARSE_MINOR => {}

                    _ => {
                        if let Some(attr_name) = key.strip_prefix(PAX_SCHILY_XATTR) {
                            xattrs
                                .push((Cow::Borrowed(attr_name.as_bytes()), Cow::Borrowed(value)));
                        }
                    }
                }
            }
        }

        // Apply PAX sparse name override (highest priority for path).
        if let Some(name) = pax_sparse_name {
            path = Cow::Borrowed(name);
        }

        // Normalize: empty optional byte fields are semantically equivalent to
        // absent.  PAX overrides and GNU long link can set empty values that
        // would otherwise surface as `Some([])` instead of `None`.
        if link_target.as_ref().is_some_and(|v| v.is_empty()) {
            link_target = None;
        }
        if uname.as_ref().is_some_and(|v| v.is_empty()) {
            uname = None;
        }
        if gname.as_ref().is_some_and(|v| v.is_empty()) {
            gname = None;
        }

        // Reject entries with empty paths
        if path.is_empty() && !self.allow_empty_path {
            return Err(ParseError::EmptyPath);
        }

        // Validate final path length
        self.limits.check_path_len(path.len())?;

        let entry = ParsedEntry {
            header,
            entry_type: header.entry_type(),
            path,
            link_target,
            mode: header.mode()?,
            uid,
            gid,
            mtime,
            size: entry_size,
            uname,
            gname,
            dev_major: header.device_major()?,
            dev_minor: header.device_minor()?,
            xattrs,
            pax: raw_pax,
        };

        // Determine the sparse context. Priority:
        // 1. Explicit sparse context (from GNU sparse type 'S' or PAX v1.0)
        // 2. PAX sparse v0.0/v0.1 data collected during PAX processing
        let sparse = sparse.or_else(|| {
            pax_sparse_map.map(|map| SparseContext {
                sparse_map: map,
                real_size: pax_sparse_real_size.unwrap_or(entry_size),
                ext_consumed: 0, // PAX v0.0/v0.1 has no extra blocks
            })
        });

        if let Some(ctx) = sparse {
            // Consume the main header plus any extension blocks.
            Ok(ParseEvent::SparseEntry {
                consumed: HEADER_SIZE + ctx.ext_consumed,
                entry,
                sparse_map: ctx.sparse_map,
                real_size: ctx.real_size,
            })
        } else {
            // Only consume the header - content is left for caller
            Ok(ParseEvent::Entry {
                consumed: HEADER_SIZE,
                entry,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GNU_MAGIC, GNU_VERSION, USTAR_MAGIC, USTAR_VERSION};

    #[test]
    fn test_default_limits() {
        let limits = Limits::default();
        assert_eq!(limits.max_metadata_size, 1024 * 1024);
        assert_eq!(limits.max_path_len, None);
        assert_eq!(limits.max_pending_entries, 16);
    }

    #[test]
    fn test_permissive_limits() {
        let limits = Limits::permissive();
        assert_eq!(limits.max_metadata_size, u32::MAX);
        assert_eq!(limits.max_path_len, None);
    }

    #[test]
    fn test_permissive_limits_relaxed() {
        let limits = Limits::permissive();
        assert!(limits.max_metadata_size > Limits::default().max_metadata_size);
        assert!(limits.max_pending_entries > Limits::default().max_pending_entries);
    }

    #[test]
    fn test_parser_empty_archive() {
        let mut parser = Parser::new(Limits::default());

        // Two zero blocks = end of archive
        let data = [0u8; 1024];

        let event = parser.parse(&data).unwrap();
        assert!(matches!(event, ParseEvent::End { consumed: 1024 }));
        assert!(parser.is_done());
    }

    #[test]
    fn test_parser_need_data() {
        let mut parser = Parser::new(Limits::default());

        // Not enough data for a header
        let data = [0u8; 256];

        let event = parser.parse(&data).unwrap();
        assert!(matches!(event, ParseEvent::NeedData { min_bytes: 512 }));
    }

    #[test]
    fn test_parser_need_more_for_end() {
        let mut parser = Parser::new(Limits::default());

        // One zero block - need second to confirm end
        let data = [0u8; 512];

        let event = parser.parse(&data).unwrap();
        assert!(matches!(event, ParseEvent::NeedData { min_bytes: 1024 }));
    }

    #[test]
    fn test_parser_with_real_header() {
        let mut parser = Parser::new(Limits::default());

        // Create a minimal valid tar header
        let mut data = vec![0u8; 2048];

        // Set up header at offset 0
        // name: "test.txt"
        data[0..8].copy_from_slice(b"test.txt");
        // mode: 0000644
        data[100..107].copy_from_slice(b"0000644");
        // uid: 0
        data[108..115].copy_from_slice(b"0000000");
        // gid: 0
        data[116..123].copy_from_slice(b"0000000");
        // size: 0 (empty file)
        data[124..135].copy_from_slice(b"00000000000");
        // mtime: 0
        data[136..147].copy_from_slice(b"00000000000");
        // typeflag: '0' (regular file)
        data[156] = b'0';
        // magic: "ustar\0"
        data[257..263].copy_from_slice(USTAR_MAGIC);
        // version: "00"
        data[263..265].copy_from_slice(USTAR_VERSION);

        // Compute and set checksum
        let header = Header::from_bytes((&data[..512]).try_into().unwrap());
        let checksum = header.compute_checksum();
        let checksum_str = format!("{checksum:06o}\0 ");
        data[148..156].copy_from_slice(checksum_str.as_bytes());

        // Two zero blocks at the end
        // data[512..1536] is already zeros

        let event = parser.parse(&data).unwrap();
        match event {
            ParseEvent::Entry { consumed, entry } => {
                assert_eq!(consumed, 512);
                assert_eq!(entry.path_lossy(), "test.txt");
                assert_eq!(entry.size, 0);
                assert!(entry.is_file());
            }
            other => panic!("Expected Entry, got {:?}", other),
        }

        // Now parse end
        let event = parser.parse(&data[512..]).unwrap();
        assert!(matches!(event, ParseEvent::End { consumed: 1024 }));
    }

    #[test]
    fn test_parser_entry_with_content() {
        let mut parser = Parser::new(Limits::default());

        // Create a tar with a file containing "hello"
        let mut data = vec![0u8; 2560]; // header + content block + 2 zero blocks

        // Header
        data[0..8].copy_from_slice(b"test.txt");
        data[100..107].copy_from_slice(b"0000644");
        data[108..115].copy_from_slice(b"0000000");
        data[116..123].copy_from_slice(b"0000000");
        data[124..135].copy_from_slice(b"00000000005"); // size = 5
        data[136..147].copy_from_slice(b"00000000000");
        data[156] = b'0';
        data[257..263].copy_from_slice(USTAR_MAGIC);
        data[263..265].copy_from_slice(USTAR_VERSION);

        // Checksum
        let header = Header::from_bytes((&data[..512]).try_into().unwrap());
        let checksum = header.compute_checksum();
        let checksum_str = format!("{checksum:06o}\0 ");
        data[148..156].copy_from_slice(checksum_str.as_bytes());

        // Content at 512..517
        data[512..517].copy_from_slice(b"hello");

        let event = parser.parse(&data).unwrap();
        match event {
            ParseEvent::Entry { consumed, entry } => {
                assert_eq!(consumed, 512);
                assert_eq!(entry.path_lossy(), "test.txt");
                assert_eq!(entry.size, 5);
                assert_eq!(entry.padded_size(), 512);
            }
            other => panic!("Expected Entry, got {:?}", other),
        }

        // Content at data[512..517], padded to 512.
        // Caller skips past content + padding, then parses the next header.

        // Parse end (zero blocks at 1024..2048)
        let event = parser.parse(&data[1024..]).unwrap();
        assert!(matches!(event, ParseEvent::End { consumed: 1024 }));
    }

    // =========================================================================
    // Helper functions for building test tar archives
    // =========================================================================

    /// Create a valid tar header with computed checksum.
    ///
    /// # Arguments
    /// * `name` - File name (max 100 bytes)
    /// * `size` - Content size in bytes
    /// * `typeflag` - Entry type (b'0' for regular, b'L' for GNU long name, etc.)
    fn make_header(name: &[u8], size: u64, typeflag: u8) -> [u8; HEADER_SIZE] {
        let mut header = [0u8; HEADER_SIZE];

        // name (0..100)
        let name_len = name.len().min(100);
        header[0..name_len].copy_from_slice(&name[..name_len]);

        // mode (100..108): 0000644
        header[100..107].copy_from_slice(b"0000644");

        // uid (108..116): 0001750 (1000 in octal)
        header[108..115].copy_from_slice(b"0001750");

        // gid (116..124): 0001750 (1000 in octal)
        header[116..123].copy_from_slice(b"0001750");

        // size (124..136): 11-digit octal
        let size_str = format!("{size:011o}");
        header[124..135].copy_from_slice(size_str.as_bytes());

        // mtime (136..148): arbitrary timestamp
        header[136..147].copy_from_slice(b"14712345670");

        // typeflag (156)
        header[156] = typeflag;

        // magic (257..263): "ustar\0"
        header[257..263].copy_from_slice(USTAR_MAGIC);

        // version (263..265): "00"
        header[263..265].copy_from_slice(USTAR_VERSION);

        // Compute and set checksum
        let hdr = Header::from_bytes(&header);
        let checksum = hdr.compute_checksum();
        let checksum_str = format!("{checksum:06o}\0 ");
        header[148..156].copy_from_slice(checksum_str.as_bytes());

        header
    }

    /// Create a tar header with a link target (for symlinks/hardlinks).
    fn make_link_header(name: &[u8], link_target: &[u8], typeflag: u8) -> [u8; HEADER_SIZE] {
        let mut header = make_header(name, 0, typeflag);

        // linkname (157..257)
        let link_len = link_target.len().min(100);
        header[157..157 + link_len].copy_from_slice(&link_target[..link_len]);

        // Recompute checksum
        let hdr = Header::from_bytes(&header);
        let checksum = hdr.compute_checksum();
        let checksum_str = format!("{checksum:06o}\0 ");
        header[148..156].copy_from_slice(checksum_str.as_bytes());

        header
    }

    /// Create a GNU long name entry (type 'L') with the given long name.
    ///
    /// Returns the complete entry: header + padded content.
    fn make_gnu_long_name(name: &[u8]) -> Vec<u8> {
        // GNU long name: content is the name with a trailing null
        let content_size = name.len() + 1; // +1 for null terminator
        let padded = content_size.next_multiple_of(HEADER_SIZE);
        let header = make_header(b"././@LongLink", content_size as u64, b'L');

        let mut result = Vec::with_capacity(HEADER_SIZE + padded);
        result.extend_from_slice(&header);
        result.extend_from_slice(name);
        result.push(0); // null terminator
        result.extend(zeroes(padded - content_size));

        result
    }

    /// Create a GNU long link entry (type 'K') with the given long link target.
    ///
    /// Returns the complete entry: header + padded content.
    fn make_gnu_long_link(link: &[u8]) -> Vec<u8> {
        let content_size = link.len() + 1; // +1 for null terminator
        let padded = content_size.next_multiple_of(HEADER_SIZE);
        let header = make_header(b"././@LongLink", content_size as u64, b'K');

        let mut result = Vec::with_capacity(HEADER_SIZE + padded);
        result.extend_from_slice(&header);
        result.extend_from_slice(link);
        result.push(0); // null terminator
        result.extend(zeroes(padded - content_size));

        result
    }

    /// Build a PAX-style header (local 'x' or global 'g') with the given key-value pairs.
    fn make_pax_entry(name: &[u8], type_flag: u8, entries: &[(&str, &[u8])]) -> Vec<u8> {
        use crate::builder::DecU64;

        // Build PAX content: each record is "<length> <key>=<value>\n"
        let mut content = Vec::new();
        for (key, value) in entries {
            // rest_len covers: " " + key + "=" + value + "\n"
            let rest_len = 3 + key.len() + value.len();
            let mut len_len = 1;
            let mut max_len = 10;
            while rest_len + len_len >= max_len {
                len_len += 1;
                max_len *= 10;
            }
            let total_len = rest_len + len_len;
            let len_dec = DecU64::new(total_len as u64);
            content.extend_from_slice(len_dec.as_bytes());
            content.push(b' ');
            content.extend_from_slice(key.as_bytes());
            content.push(b'=');
            content.extend_from_slice(value);
            content.push(b'\n');
        }

        let content_size = content.len();
        let header = make_header(name, content_size as u64, type_flag);

        let padded = content_size.next_multiple_of(HEADER_SIZE);
        let mut result = Vec::with_capacity(HEADER_SIZE + padded);
        result.extend_from_slice(&header);
        result.extend_from_slice(&content);
        result.extend(zeroes(padded - content_size));

        result
    }

    fn make_pax_header(entries: &[(&str, &[u8])]) -> Vec<u8> {
        make_pax_entry(b"PaxHeader/file", b'x', entries)
    }

    fn make_pax_global_header(entries: &[(&str, &[u8])]) -> Vec<u8> {
        make_pax_entry(b"pax_global_header", b'g', entries)
    }

    /// Return `n` zero bytes (for end-of-archive markers, padding, etc.).
    fn zeroes(n: usize) -> impl Iterator<Item = u8> {
        std::iter::repeat_n(0u8, n)
    }

    // =========================================================================
    // GNU long name tests
    // =========================================================================

    #[test]
    fn test_parser_gnu_long_name() {
        // Create archive with GNU long name entry followed by actual file
        let long_name =
            "very/long/path/that/exceeds/one/hundred/bytes/".to_string() + &"x".repeat(60);
        assert!(long_name.len() > 100);

        let mut archive = Vec::new();
        archive.extend(make_gnu_long_name(long_name.as_bytes()));
        archive.extend_from_slice(&make_header(b"placeholder", 5, b'0'));
        // Content: "hello"
        let mut content_block = [0u8; 512];
        content_block[0..5].copy_from_slice(b"hello");
        archive.extend_from_slice(&content_block);
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        // Should consume GNU long name header + content + actual header
        let consumed = match &event {
            ParseEvent::Entry { consumed, entry } => {
                assert!(*consumed > 512);
                assert_eq!(entry.path.as_ref(), long_name.as_bytes());
                assert_eq!(entry.size, 5);
                assert!(entry.is_file());
                *consumed
            }
            other => panic!("Expected Entry, got {:?}", other),
        };

        // Parse end (skip past content + padding)
        let remaining = &archive[consumed + 512..];
        let event = parser.parse(remaining).unwrap();
        assert!(matches!(event, ParseEvent::End { .. }));
    }

    // =========================================================================
    // GNU long link tests
    // =========================================================================

    #[test]
    fn test_parser_gnu_long_link() {
        // Create archive with GNU long link entry followed by symlink
        let long_target = "/some/very/long/symlink/target/path/".to_string() + &"t".repeat(80);
        assert!(long_target.len() > 100);

        let mut archive = Vec::new();
        archive.extend(make_gnu_long_link(long_target.as_bytes()));
        // Symlink header with placeholder linkname
        archive.extend_from_slice(&make_link_header(b"mylink", b"placeholder", b'2'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        let consumed = match &event {
            ParseEvent::Entry { consumed, entry } => {
                assert_eq!(entry.path.as_ref(), b"mylink");
                assert!(entry.is_symlink());
                assert_eq!(
                    entry.link_target.as_ref().unwrap().as_ref(),
                    long_target.as_bytes()
                );
                *consumed
            }
            other => panic!("Expected Entry, got {:?}", other),
        };

        let remaining = &archive[consumed..];
        let event = parser.parse(remaining).unwrap();
        assert!(matches!(event, ParseEvent::End { .. }));
    }

    // =========================================================================
    // PAX extension tests
    // =========================================================================

    #[test]
    fn test_parser_pax_path_override() {
        // PAX header should override the path in the actual header
        let pax_path = "pax/overridden/path/to/file.txt";

        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[("path", pax_path.as_bytes())]));
        archive.extend_from_slice(&make_header(b"original.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::Entry { entry, .. } => {
                assert_eq!(entry.path.as_ref(), pax_path.as_bytes());
            }
            other => panic!("Expected Entry, got {:?}", other),
        }
    }

    #[test]
    fn test_parser_pax_size_override() {
        // PAX header should override the size in the actual header
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[("size", b"999")]));
        // Header says size=5, but PAX says 999
        archive.extend_from_slice(&make_header(b"file.txt", 5, b'0'));
        // We still need content padded to the PAX size for proper parsing
        archive.extend(zeroes(1024)); // More than enough

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::Entry { entry, .. } => {
                assert_eq!(entry.size, 999);
            }
            other => panic!("Expected Entry, got {:?}", other),
        }
    }

    #[test]
    fn test_parser_pax_metadata() {
        // PAX header overriding uid, gid, and mtime
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[
            ("uid", b"65534"),
            ("gid", b"65535"),
            ("mtime", b"1700000000.123456789"),
        ]));
        archive.extend_from_slice(&make_header(b"file.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::Entry { entry, .. } => {
                assert_eq!(entry.uid, 65534);
                assert_eq!(entry.gid, 65535);
                // mtime should be the integer part only
                assert_eq!(entry.mtime, 1700000000);
            }
            other => panic!("Expected Entry, got {:?}", other),
        }
    }

    #[test]
    fn test_parser_pax_xattr() {
        // PAX SCHILY.xattr.* entries for extended attributes
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[
            ("SCHILY.xattr.user.test", b"test_value"),
            (
                "SCHILY.xattr.security.selinux",
                b"system_u:object_r:unlabeled_t:s0",
            ),
        ]));
        archive.extend_from_slice(&make_header(b"file.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::Entry { entry, .. } => {
                assert_eq!(entry.xattrs.len(), 2);

                // Check xattrs (order should be preserved)
                assert_eq!(entry.xattrs[0].0.as_ref(), b"user.test");
                assert_eq!(entry.xattrs[0].1.as_ref(), b"test_value");

                assert_eq!(entry.xattrs[1].0.as_ref(), b"security.selinux");
                assert_eq!(
                    entry.xattrs[1].1.as_ref(),
                    b"system_u:object_r:unlabeled_t:s0"
                );
            }
            other => panic!("Expected Entry, got {:?}", other),
        }
    }

    #[test]
    fn test_parser_pax_raw_bytes_preserved() {
        // The raw PAX data should be available in ParsedEntry::pax
        // so callers can iterate all key-value pairs, not just the ones
        // tar-core resolves into struct fields.
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[
            ("path", b"custom/path.txt"),
            ("SCHILY.xattr.user.key", b"val"),
            ("myfancykey", b"myfancyvalue"),
        ]));
        archive.extend_from_slice(&make_header(b"orig.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::Entry { entry, .. } => {
                // Resolved fields work as before
                assert_eq!(entry.path.as_ref(), b"custom/path.txt");
                assert_eq!(entry.xattrs.len(), 1);

                // Raw PAX data is preserved
                let raw = entry.pax.expect("pax should be Some");
                let exts = PaxExtensions::new(raw);
                let keys: Vec<&str> = exts
                    .filter_map(|e| e.ok())
                    .filter_map(|e| e.key().ok())
                    .collect();
                assert_eq!(keys, &["path", "SCHILY.xattr.user.key", "myfancykey"]);
            }
            other => panic!("Expected Entry, got {:?}", other),
        }
    }

    #[test]
    fn test_parser_no_pax_means_none() {
        // Entries without PAX extensions should have pax == None
        let mut archive = Vec::new();
        archive.extend_from_slice(&make_header(b"plain.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::Entry { entry, .. } => {
                assert!(entry.pax.is_none());
            }
            other => panic!("Expected Entry, got {:?}", other),
        }
    }

    #[test]
    fn test_parser_gnu_long_name_no_pax() {
        // GNU long name without PAX should still have pax == None
        let long_name = "long/path/".to_string() + &"x".repeat(100);
        let mut archive = Vec::new();
        archive.extend(make_gnu_long_name(long_name.as_bytes()));
        archive.extend_from_slice(&make_header(b"short", 0, b'0'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::Entry { entry, .. } => {
                assert_eq!(entry.path.as_ref(), long_name.as_bytes());
                assert!(entry.pax.is_none());
            }
            other => panic!("Expected Entry, got {:?}", other),
        }
    }

    #[test]
    fn test_parser_pax_linkpath() {
        // PAX linkpath for symlink targets
        let pax_linkpath = "/a/very/long/symlink/target/from/pax";

        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[("linkpath", pax_linkpath.as_bytes())]));
        archive.extend_from_slice(&make_link_header(b"mylink", b"short", b'2'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::Entry { entry, .. } => {
                assert!(entry.is_symlink());
                assert_eq!(
                    entry.link_target.as_ref().unwrap().as_ref(),
                    pax_linkpath.as_bytes()
                );
            }
            other => panic!("Expected Entry, got {:?}", other),
        }
    }

    // =========================================================================
    // PAX global header tests
    // =========================================================================

    #[test]
    fn test_parser_global_pax_header() {
        // A global PAX header should be surfaced as a GlobalExtensions event,
        // not silently skipped.
        let mut archive = Vec::new();
        archive.extend(make_pax_global_header(&[
            ("mtime", b"1700000000"),
            (
                "SCHILY.xattr.security.selinux",
                b"system_u:object_r:default_t:s0",
            ),
        ]));
        // Followed by a regular file entry
        archive.extend_from_slice(&make_header(b"file.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());

        // First event: GlobalExtensions
        let event = parser.parse(&archive).unwrap();
        let consumed = match &event {
            ParseEvent::GlobalExtensions { consumed, pax_data } => {
                // Verify the raw PAX data can be parsed
                let exts = PaxExtensions::new(pax_data);
                let keys: Vec<&str> = exts
                    .filter_map(|e| e.ok())
                    .filter_map(|e| e.key().ok())
                    .collect();
                assert_eq!(keys, &["mtime", "SCHILY.xattr.security.selinux"]);
                *consumed
            }
            other => panic!("Expected GlobalExtensions, got {:?}", other),
        };

        // Second event: the actual file entry (global headers don't affect it)
        let event = parser.parse(&archive[consumed..]).unwrap();
        match event {
            ParseEvent::Entry { entry, .. } => {
                assert_eq!(entry.path_lossy(), "file.txt");
                // Global header should NOT have modified this entry's metadata
                assert!(entry.pax.is_none());
            }
            other => panic!("Expected Entry, got {:?}", other),
        }
    }

    #[test]
    fn test_parser_global_pax_header_need_data() {
        // Global PAX header present but content not yet available
        let header = make_header(b"pax_global_header", 100, b'g');

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&header).unwrap();

        match event {
            ParseEvent::NeedData { min_bytes } => {
                assert_eq!(min_bytes, 1024); // header (512) + padded content (512)
            }
            other => panic!("Expected NeedData, got {:?}", other),
        }
    }

    #[test]
    fn test_parser_global_pax_header_too_large() {
        // Global PAX header exceeding max_metadata_size should error
        let large_value = "x".repeat(1000);

        let mut archive = Vec::new();
        archive.extend(make_pax_global_header(&[(
            "comment",
            large_value.as_bytes(),
        )]));
        archive.extend_from_slice(&make_header(b"file.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let limits = Limits {
            max_metadata_size: 100,
            ..Default::default()
        };
        let mut parser = Parser::new(limits);
        let result = parser.parse(&archive);

        assert!(matches!(result, Err(ParseError::MetadataTooLarge { .. })));
    }

    #[test]
    fn test_parser_multiple_global_pax_headers() {
        // Multiple global PAX headers in a row should each produce a
        // separate GlobalExtensions event (they don't use the pending
        // metadata mechanism).
        let mut archive = Vec::new();
        archive.extend(make_pax_global_header(&[("comment", b"first")]));
        archive.extend(make_pax_global_header(&[("comment", b"second")]));
        archive.extend_from_slice(&make_header(b"file.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());

        // First global header
        let event = parser.parse(&archive).unwrap();
        let consumed1 = match &event {
            ParseEvent::GlobalExtensions { consumed, pax_data } => {
                let exts: Vec<_> = PaxExtensions::new(pax_data)
                    .filter_map(|e| e.ok())
                    .collect();
                assert_eq!(exts[0].value_bytes(), b"first");
                *consumed
            }
            other => panic!("Expected GlobalExtensions, got {:?}", other),
        };

        // Second global header
        let event = parser.parse(&archive[consumed1..]).unwrap();
        let consumed2 = match &event {
            ParseEvent::GlobalExtensions { consumed, pax_data } => {
                let exts: Vec<_> = PaxExtensions::new(pax_data)
                    .filter_map(|e| e.ok())
                    .collect();
                assert_eq!(exts[0].value_bytes(), b"second");
                *consumed
            }
            other => panic!("Expected GlobalExtensions, got {:?}", other),
        };

        // Then the actual file entry
        let event = parser.parse(&archive[consumed1 + consumed2..]).unwrap();
        assert!(matches!(event, ParseEvent::Entry { .. }));
    }

    #[test]
    fn test_parser_global_pax_does_not_interfere_with_local_pax() {
        // A global PAX header followed by a local PAX header should produce
        // both events independently.
        let mut archive = Vec::new();
        archive.extend(make_pax_global_header(&[("mtime", b"1000000000")]));
        archive.extend(make_pax_header(&[("path", b"overridden.txt")]));
        archive.extend_from_slice(&make_header(b"original.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());

        // First: global extensions event
        let event = parser.parse(&archive).unwrap();
        let consumed = match &event {
            ParseEvent::GlobalExtensions { consumed, .. } => *consumed,
            other => panic!("Expected GlobalExtensions, got {:?}", other),
        };

        // Second: entry with local PAX applied
        let event = parser.parse(&archive[consumed..]).unwrap();
        match event {
            ParseEvent::Entry { entry, .. } => {
                // Local PAX should have been applied
                assert_eq!(entry.path.as_ref(), b"overridden.txt");
                assert!(entry.pax.is_some());
            }
            other => panic!("Expected Entry, got {:?}", other),
        }
    }

    // =========================================================================
    // Error case tests
    // =========================================================================

    #[test]
    fn test_parser_orphaned_metadata() {
        // GNU long name entry followed by end of archive (no actual entry)
        let mut archive = Vec::new();
        archive.extend(make_gnu_long_name(b"some/long/name/here"));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let result = parser.parse(&archive);

        assert!(matches!(result, Err(ParseError::OrphanedMetadata)));
    }

    #[test]
    fn test_parser_orphaned_pax_metadata() {
        // PAX header followed by end of archive
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[("path", b"test")]));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let result = parser.parse(&archive);

        assert!(matches!(result, Err(ParseError::OrphanedMetadata)));
    }

    #[test]
    fn test_parser_duplicate_gnu_long_name() {
        // Two GNU long name entries in a row should error
        let mut archive = Vec::new();
        archive.extend(make_gnu_long_name(b"first/long/name"));
        archive.extend(make_gnu_long_name(b"second/long/name"));
        archive.extend_from_slice(&make_header(b"file.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let result = parser.parse(&archive);

        assert!(matches!(result, Err(ParseError::DuplicateGnuLongName)));
    }

    #[test]
    fn test_parser_duplicate_gnu_long_link() {
        // Two GNU long link entries in a row should error
        let mut archive = Vec::new();
        archive.extend(make_gnu_long_link(b"first/long/target"));
        archive.extend(make_gnu_long_link(b"second/long/target"));
        archive.extend_from_slice(&make_link_header(b"link", b"x", b'2'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let result = parser.parse(&archive);

        assert!(matches!(result, Err(ParseError::DuplicateGnuLongLink)));
    }

    #[test]
    fn test_parser_duplicate_pax_header() {
        // Two PAX headers in a row should error
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[("path", b"first")]));
        archive.extend(make_pax_header(&[("path", b"second")]));
        archive.extend_from_slice(&make_header(b"file.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let result = parser.parse(&archive);

        assert!(matches!(result, Err(ParseError::DuplicatePaxHeader)));
    }

    // =========================================================================
    // Combined GNU and PAX tests
    // =========================================================================

    #[test]
    fn test_parser_combined_gnu_pax() {
        // Both GNU long name and PAX path - PAX should win
        let gnu_name = "gnu/long/name/".to_string() + &"g".repeat(100);
        let pax_path = "pax/should/win/file.txt";

        let mut archive = Vec::new();
        archive.extend(make_gnu_long_name(gnu_name.as_bytes()));
        archive.extend(make_pax_header(&[("path", pax_path.as_bytes())]));
        archive.extend_from_slice(&make_header(b"header.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::Entry { entry, .. } => {
                // PAX path should override GNU long name
                assert_eq!(entry.path.as_ref(), pax_path.as_bytes());
            }
            other => panic!("Expected Entry, got {:?}", other),
        }
    }

    #[test]
    fn test_parser_gnu_long_name_and_link_combined() {
        // Both GNU long name and long link for the same entry
        let long_name = "long/symlink/name/".to_string() + &"n".repeat(100);
        let long_target = "long/target/path/".to_string() + &"t".repeat(100);

        let mut archive = Vec::new();
        archive.extend(make_gnu_long_name(long_name.as_bytes()));
        archive.extend(make_gnu_long_link(long_target.as_bytes()));
        archive.extend_from_slice(&make_link_header(b"short", b"short", b'2'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::Entry { entry, .. } => {
                assert_eq!(entry.path.as_ref(), long_name.as_bytes());
                assert_eq!(
                    entry.link_target.as_ref().unwrap().as_ref(),
                    long_target.as_bytes()
                );
                assert!(entry.is_symlink());
            }
            other => panic!("Expected Entry, got {:?}", other),
        }
    }

    #[test]
    fn test_parser_pax_multiple_entries() {
        // Multiple PAX entries for different files
        let mut archive = Vec::new();

        // First file with PAX
        archive.extend(make_pax_header(&[("path", b"first/file.txt")]));
        archive.extend_from_slice(&make_header(b"f1", 5, b'0'));
        let mut content1 = [0u8; 512];
        content1[0..5].copy_from_slice(b"hello");
        archive.extend_from_slice(&content1);

        // Second file with PAX
        archive.extend(make_pax_header(&[("path", b"second/file.txt")]));
        archive.extend_from_slice(&make_header(b"f2", 5, b'0'));
        let mut content2 = [0u8; 512];
        content2[0..5].copy_from_slice(b"world");
        archive.extend_from_slice(&content2);

        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());

        // Parse first entry
        let event1 = parser.parse(&archive).unwrap();
        let consumed1 = match &event1 {
            ParseEvent::Entry { consumed, entry } => {
                assert_eq!(entry.path.as_ref(), b"first/file.txt");
                assert_eq!(entry.size, 5);
                *consumed
            }
            other => panic!("Expected Entry, got {:?}", other),
        };

        // Parse second entry (skip past first entry's content + padding)
        let offset = consumed1 + 512;
        let event2 = parser.parse(&archive[offset..]).unwrap();
        let consumed2 = match &event2 {
            ParseEvent::Entry { consumed, entry } => {
                assert_eq!(entry.path.as_ref(), b"second/file.txt");
                assert_eq!(entry.size, 5);
                *consumed
            }
            other => panic!("Expected Entry, got {:?}", other),
        };

        // Parse end (skip past second entry's content + padding)
        let final_offset = offset + consumed2 + 512;
        let event3 = parser.parse(&archive[final_offset..]).unwrap();
        assert!(matches!(event3, ParseEvent::End { .. }));
    }

    #[test]
    fn test_parser_pax_uname_gname() {
        // PAX uname and gname override
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[
            ("uname", b"testuser"),
            ("gname", b"testgroup"),
        ]));
        archive.extend_from_slice(&make_header(b"file.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::Entry { entry, .. } => {
                assert_eq!(entry.uname.as_ref().unwrap().as_ref(), b"testuser");
                assert_eq!(entry.gname.as_ref().unwrap().as_ref(), b"testgroup");
            }
            other => panic!("Expected Entry, got {:?}", other),
        }
    }

    // =========================================================================
    // Size limit tests
    // =========================================================================

    #[test]
    fn test_parser_gnu_long_too_large() {
        let long_name = "x".repeat(200);

        let mut archive = Vec::new();
        archive.extend(make_gnu_long_name(long_name.as_bytes()));
        archive.extend_from_slice(&make_header(b"file.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let limits = Limits {
            max_metadata_size: 100,
            ..Default::default()
        };
        let mut parser = Parser::new(limits);
        let result = parser.parse(&archive);

        assert!(matches!(result, Err(ParseError::MetadataTooLarge { .. })));
    }

    #[test]
    fn test_parser_pax_path_too_long() {
        let long_path = "x".repeat(200);

        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[("path", long_path.as_bytes())]));
        archive.extend_from_slice(&make_header(b"file.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let limits = Limits {
            max_path_len: Some(100),
            ..Default::default()
        };
        let mut parser = Parser::new(limits);
        let result = parser.parse(&archive);

        assert!(matches!(
            result,
            Err(ParseError::PathTooLong {
                len: 200,
                limit: 100
            })
        ));
    }

    #[test]
    fn test_parser_pax_too_large() {
        // Create a PAX header that exceeds the metadata size limit
        let large_value = "x".repeat(1000);

        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[("path", large_value.as_bytes())]));
        archive.extend_from_slice(&make_header(b"file.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let limits = Limits {
            max_metadata_size: 100,
            ..Default::default()
        };
        let mut parser = Parser::new(limits);
        let result = parser.parse(&archive);

        assert!(matches!(result, Err(ParseError::MetadataTooLarge { .. })));
    }

    // =========================================================================
    // Need data tests for extension entries
    // =========================================================================

    #[test]
    fn test_parser_need_data_for_gnu_long_content() {
        // Create a GNU long name header, but don't provide the content
        let header = make_header(b"././@LongLink", 200, b'L');

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&header).unwrap();

        // Need header (512) + padded content (512) = 1024
        match event {
            ParseEvent::NeedData { min_bytes } => {
                assert_eq!(min_bytes, 1024);
            }
            other => panic!("Expected NeedData, got {:?}", other),
        }
    }

    #[test]
    fn test_parser_need_data_for_pax_content() {
        // Create a PAX header, but don't provide the content
        let header = make_header(b"PaxHeader/file", 100, b'x');

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&header).unwrap();

        // Need header (512) + padded content (512) = 1024
        match event {
            ParseEvent::NeedData { min_bytes } => {
                assert_eq!(min_bytes, 1024);
            }
            other => panic!("Expected NeedData, got {:?}", other),
        }
    }

    #[test]
    fn test_need_data_adjusted_through_extension_headers() {
        // Regression test: NeedData.min_bytes must be relative to the
        // original buffer, not the recursive sub-slice.
        //
        // Provide a complete GNU long name entry (header + content = 1024 bytes)
        // but no following header. The recursive parse_header call on the
        // sub-slice needs 512 more bytes for the next header. After
        // add_consumed(1024), min_bytes must be 1024 + 512 = 1536.
        let long_name = "long/path/name/".to_string() + &"x".repeat(90);
        let gnu_entry = make_gnu_long_name(long_name.as_bytes());
        // gnu_entry is header(512) + padded_content(512) = 1024 bytes
        assert_eq!(gnu_entry.len(), 1024);

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&gnu_entry).unwrap();

        match event {
            ParseEvent::NeedData { min_bytes } => {
                // The recursive call needs 512 bytes (one header) from its
                // sub-slice. add_consumed(1024) must adjust this to 1536.
                assert_eq!(
                    min_bytes, 1536,
                    "NeedData.min_bytes must account for bytes consumed by \
                     extension headers (1024 + 512 = 1536)"
                );
            }
            other => panic!("Expected NeedData, got {:?}", other),
        }
    }

    /// Test for CVE-2025-62518 (TARmageddon): PAX size must override header size
    ///
    /// The vulnerability occurs when:
    /// 1. PAX header specifies size=X (e.g., 1024)
    /// 2. ustar header specifies size=0
    /// 3. Vulnerable parser uses header size (0) instead of PAX size (1024)
    /// 4. Parser advances 0 bytes, treating nested tar content as outer entries
    ///
    /// tar-core MUST use PAX size for content advancement to be secure.
    #[test]
    fn test_cve_2025_62518_pax_size_overrides_header() {
        // PAX header with size=1024
        let pax_entries: &[(&str, &[u8])] = &[("size", b"1024")];
        let pax_data = make_pax_header(pax_entries);

        // Actual file header with size=0 in ustar (the attack vector!)
        // A vulnerable parser would skip 0 bytes and see the "content" as headers
        let file_header = make_header(b"nested.tar", 0, b'0'); // size=0 in header!

        // The "content" - in an attack this would be a nested tar archive
        // with malicious files that get extracted
        let mut content = vec![0u8; 1024];
        // Put something that looks like a tar header to detect if parser is confused
        content[0..9].copy_from_slice(b"MALICIOUS");
        content[156] = b'0'; // Would be parsed as regular file if vulnerable

        // Build full archive: PAX header + actual header + content + padding + end markers
        let mut archive = Vec::new();
        archive.extend_from_slice(&pax_data);
        archive.extend_from_slice(&file_header);
        archive.extend_from_slice(&content);
        // Pad to 512 boundary (1024 is already aligned)
        archive.extend(zeroes(1024));

        // Parse
        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        let consumed = match &event {
            ParseEvent::Entry { consumed, entry } => {
                // CRITICAL: entry.size MUST be 1024 (from PAX), not 0 (from header)
                assert_eq!(
                    entry.size, 1024,
                    "CVE-2025-62518: Parser MUST use PAX size (1024), not header size (0)"
                );

                // padded_size should also be 1024
                assert_eq!(entry.padded_size(), 1024, "Padded size must match PAX size");

                // Path should be from header
                assert_eq!(entry.path_lossy(), "nested.tar");

                *consumed
            }
            other => panic!("Expected Entry, got {:?}", other),
        };

        // Continue parsing - should get End, NOT another entry
        let remaining = &archive[consumed + 1024..]; // consumed headers + 1024 bytes content
        let event = parser.parse(remaining).unwrap();

        match event {
            ParseEvent::End { .. } => {
                // Correct! Parser properly skipped the 1024-byte content
            }
            ParseEvent::Entry { entry, .. } => {
                panic!(
                    "CVE-2025-62518 VULNERABLE: Parser found unexpected entry '{}' \
                     because it used header size (0) instead of PAX size (1024)",
                    entry.path_lossy()
                );
            }
            other => panic!("Expected End, got {:?}", other),
        }
    }

    /// Additional test: ensure parser state tracks PAX-overridden size
    #[test]
    fn test_pax_size_affects_parser_state() {
        // PAX specifies 512 bytes, header says 0
        let pax_entries: &[(&str, &[u8])] = &[("size", b"512")];
        let pax_data = make_pax_header(pax_entries);
        let file_header = make_header(b"test.bin", 0, b'0');

        let content = vec![0u8; 512];
        let mut archive = Vec::new();
        archive.extend_from_slice(&pax_data);
        archive.extend_from_slice(&file_header);
        archive.extend_from_slice(&content);
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());

        // Parse entry
        let event = parser.parse(&archive).unwrap();
        let size = match event {
            ParseEvent::Entry { entry, .. } => entry.size,
            other => panic!("Expected Entry, got {:?}", other),
        };

        assert_eq!(size, 512, "Entry size must reflect PAX override");
    }

    // =========================================================================
    // Strict mode tests
    // =========================================================================

    /// Build a minimal archive with a PAX header overriding `key` to `value`,
    /// followed by a regular file entry and end-of-archive.
    fn make_archive_with_pax(key: &str, value: &[u8]) -> Vec<u8> {
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[(key, value)]));
        archive.extend_from_slice(&make_header(b"file.txt", 0, b'0'));
        archive.extend(zeroes(1024));
        archive
    }

    #[test]
    fn test_strict_rejects_invalid_pax_uid() {
        let archive = make_archive_with_pax("uid", b"notanumber");
        let mut parser = Parser::new(Limits::default());
        let err = parser.parse(&archive).unwrap_err();
        assert!(
            matches!(err, ParseError::InvalidPaxValue { key: "uid", .. }),
            "expected InvalidPaxValue for uid, got {err:?}"
        );
    }

    #[test]
    fn test_strict_rejects_invalid_pax_size() {
        let archive = make_archive_with_pax("size", b"xyz");
        let mut parser = Parser::new(Limits::default());
        let err = parser.parse(&archive).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidPaxValue { key: "size", .. }
        ));
    }

    #[test]
    fn test_strict_rejects_invalid_pax_gid() {
        let archive = make_archive_with_pax("gid", b"bad");
        let mut parser = Parser::new(Limits::default());
        let err = parser.parse(&archive).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidPaxValue { key: "gid", .. }
        ));
    }

    #[test]
    fn test_strict_rejects_invalid_pax_mtime() {
        let archive = make_archive_with_pax("mtime", b"nottime");
        let mut parser = Parser::new(Limits::default());
        let err = parser.parse(&archive).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidPaxValue { key: PAX_MTIME, .. }
        ));
    }

    #[test]
    fn test_lenient_ignores_invalid_pax_uid() {
        let archive = make_archive_with_pax("uid", b"notanumber");
        let mut parser = Parser::new(Limits::default());
        parser.set_ignore_pax_errors(true);
        let event = parser.parse(&archive).unwrap();
        match event {
            ParseEvent::Entry { entry, .. } => {
                // Should fall back to header uid (1000 from make_header)
                assert_eq!(entry.uid, 1000);
            }
            other => panic!("Expected Entry, got {other:?}"),
        }
    }

    #[test]
    fn test_lenient_ignores_invalid_pax_size() {
        let archive = make_archive_with_pax("size", b"xyz");
        let mut parser = Parser::new(Limits::default());
        parser.set_ignore_pax_errors(true);
        let event = parser.parse(&archive).unwrap();
        match event {
            ParseEvent::Entry { entry, .. } => {
                // Should fall back to header size (0 from make_header)
                assert_eq!(entry.size, 0);
            }
            other => panic!("Expected Entry, got {other:?}"),
        }
    }

    #[test]
    fn test_strict_accepts_valid_pax_values() {
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[
            ("uid", b"2000"),
            ("gid", b"3000"),
            ("size", b"42"),
            ("mtime", b"1700000000"),
        ]));
        archive.extend_from_slice(&make_header(b"file.txt", 0, b'0'));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();
        match event {
            ParseEvent::Entry { entry, .. } => {
                assert_eq!(entry.uid, 2000);
                assert_eq!(entry.gid, 3000);
                assert_eq!(entry.size, 42);
                assert_eq!(entry.mtime, 1700000000);
            }
            other => panic!("Expected Entry, got {other:?}"),
        }
    }

    #[test]
    fn test_strict_accepts_fractional_mtime() {
        let archive = make_archive_with_pax("mtime", b"1234567890.123456");
        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();
        match event {
            ParseEvent::Entry { entry, .. } => {
                assert_eq!(entry.mtime, 1234567890);
            }
            other => panic!("Expected Entry, got {other:?}"),
        }
    }

    // =========================================================================
    // Sparse entry helpers
    // =========================================================================

    /// Encode a u64 as an 12-byte octal field (for sparse descriptor fields).
    fn encode_octal_12(value: u64) -> [u8; 12] {
        let s = format!("{value:011o}\0");
        let mut field = [0u8; 12];
        field.copy_from_slice(s.as_bytes());
        field
    }

    /// Create a GNU sparse header (type 'S') with inline sparse descriptors.
    ///
    /// `entries` are (offset, length) pairs for the sparse map (max 4).
    /// `on_disk_size` is the header's size field (total data bytes on disk).
    /// `real_size` is the logical file size.
    /// If `is_extended` is true, the isextended flag is set.
    fn make_gnu_sparse_header(
        name: &[u8],
        entries: &[(u64, u64)],
        on_disk_size: u64,
        real_size: u64,
        is_extended: bool,
    ) -> [u8; HEADER_SIZE] {
        assert!(entries.len() <= 4, "max 4 inline sparse descriptors");

        let mut header = [0u8; HEADER_SIZE];

        // name (0..100)
        let name_len = name.len().min(100);
        header[0..name_len].copy_from_slice(&name[..name_len]);

        // mode (100..108)
        header[100..107].copy_from_slice(b"0000644");
        // uid (108..116)
        header[108..115].copy_from_slice(b"0001750");
        // gid (116..124)
        header[116..123].copy_from_slice(b"0001750");

        // size (124..136): on-disk data size
        let size_str = format!("{on_disk_size:011o}");
        header[124..135].copy_from_slice(size_str.as_bytes());

        // mtime (136..148)
        header[136..147].copy_from_slice(b"14712345670");

        // typeflag (156): 'S' for sparse
        header[156] = b'S';

        // magic (257..263): GNU
        header[257..263].copy_from_slice(GNU_MAGIC);
        // version (263..265): GNU
        header[263..265].copy_from_slice(GNU_VERSION);

        // sparse descriptors at offset 386, each 24 bytes
        for (i, &(offset, length)) in entries.iter().enumerate() {
            let base = 386 + i * 24;
            header[base..base + 12].copy_from_slice(&encode_octal_12(offset));
            header[base + 12..base + 24].copy_from_slice(&encode_octal_12(length));
        }

        // isextended at offset 482
        header[482] = if is_extended { 1 } else { 0 };

        // realsize at offset 483
        let real_str = format!("{real_size:011o}");
        header[483..494].copy_from_slice(real_str.as_bytes());

        // Compute and set checksum
        let hdr = Header::from_bytes(&header);
        let checksum = hdr.compute_checksum();
        let checksum_str = format!("{checksum:06o}\0 ");
        header[148..156].copy_from_slice(checksum_str.as_bytes());

        header
    }

    /// Create a GNU extended sparse block (512 bytes) with up to 21
    /// descriptors. Returns a 512-byte block.
    fn make_gnu_ext_sparse(entries: &[(u64, u64)], is_extended: bool) -> [u8; HEADER_SIZE] {
        assert!(entries.len() <= 21, "max 21 descriptors per ext block");

        let mut block = [0u8; HEADER_SIZE];

        for (i, &(offset, length)) in entries.iter().enumerate() {
            let base = i * 24;
            block[base..base + 12].copy_from_slice(&encode_octal_12(offset));
            block[base + 12..base + 24].copy_from_slice(&encode_octal_12(length));
        }

        // isextended at offset 504 (byte after 21 * 24 = 504)
        block[504] = if is_extended { 1 } else { 0 };

        block
    }

    // =========================================================================
    // Sparse entry tests
    // =========================================================================

    #[test]
    fn test_sparse_basic() {
        // Sparse file with 2 data regions: [0x1000..0x1005) and [0x3000..0x3005)
        // On-disk size: 10 bytes (5 + 5), real size: 0x3005
        let header = make_gnu_sparse_header(
            b"sparse.txt",
            &[(0x1000, 5), (0x3000, 5)],
            10,     // on-disk
            0x3005, // real size
            false,
        );

        let mut archive = Vec::new();
        archive.extend_from_slice(&header);
        // Content (10 bytes, padded to 512)
        let mut content = [0u8; HEADER_SIZE];
        content[0..5].copy_from_slice(b"hello");
        content[5..10].copy_from_slice(b"world");
        archive.extend_from_slice(&content);
        archive.extend(zeroes(1024)); // end of archive

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::SparseEntry {
                consumed,
                entry,
                sparse_map,
                real_size,
            } => {
                assert_eq!(consumed, HEADER_SIZE);
                assert_eq!(entry.path_lossy(), "sparse.txt");
                assert_eq!(entry.size, 10);
                assert_eq!(real_size, 0x3005);
                assert_eq!(sparse_map.len(), 2);
                assert_eq!(
                    sparse_map[0],
                    SparseEntry {
                        offset: 0x1000,
                        length: 5
                    }
                );
                assert_eq!(
                    sparse_map[1],
                    SparseEntry {
                        offset: 0x3000,
                        length: 5
                    }
                );
            }
            other => panic!("Expected SparseEntry, got {other:?}"),
        }
    }

    #[test]
    fn test_sparse_no_entries() {
        // Sparse file with no data regions (all zeros), real size 4096
        let header = make_gnu_sparse_header(b"empty_sparse.txt", &[], 0, 4096, false);

        let mut archive = Vec::new();
        archive.extend_from_slice(&header);
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::SparseEntry {
                sparse_map,
                real_size,
                entry,
                ..
            } => {
                assert!(sparse_map.is_empty());
                assert_eq!(real_size, 4096);
                assert_eq!(entry.size, 0);
            }
            other => panic!("Expected SparseEntry, got {other:?}"),
        }
    }

    #[test]
    fn test_sparse_four_inline_entries() {
        // Max inline: 4 sparse descriptors
        let entries = [(0u64, 512), (1024, 512), (2048, 512), (3072, 512)];
        let on_disk: u64 = entries.iter().map(|(_, l)| l).sum();
        let real_size = 3072 + 512;
        let header = make_gnu_sparse_header(b"four.txt", &entries, on_disk, real_size, false);

        let mut archive = Vec::new();
        archive.extend_from_slice(&header);
        archive.extend(zeroes(on_disk.next_multiple_of(512) as usize));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::SparseEntry {
                sparse_map,
                real_size: rs,
                ..
            } => {
                assert_eq!(sparse_map.len(), 4);
                assert_eq!(rs, real_size);
                for (i, &(off, len)) in entries.iter().enumerate() {
                    assert_eq!(sparse_map[i].offset, off);
                    assert_eq!(sparse_map[i].length, len);
                }
            }
            other => panic!("Expected SparseEntry, got {other:?}"),
        }
    }

    #[test]
    fn test_sparse_with_extension_block() {
        // 4 inline + 2 in extension block = 6 total
        let inline_entries = [(0u64, 100), (512, 100), (1024, 100), (1536, 100)];
        let ext_entries = [(2048u64, 100), (2560, 100)];
        let on_disk: u64 = 600; // 6 * 100
        let real_size = 2660; // 2560 + 100

        let header =
            make_gnu_sparse_header(b"extended.txt", &inline_entries, on_disk, real_size, true);
        let ext = make_gnu_ext_sparse(&ext_entries, false);

        let mut archive = Vec::new();
        archive.extend_from_slice(&header);
        archive.extend_from_slice(&ext);
        archive.extend(zeroes(on_disk.next_multiple_of(512) as usize));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::SparseEntry {
                consumed,
                sparse_map,
                real_size: rs,
                ..
            } => {
                // consumed = main header + 1 extension block
                assert_eq!(consumed, 2 * HEADER_SIZE);
                assert_eq!(rs, real_size);
                assert_eq!(sparse_map.len(), 6);
                assert_eq!(sparse_map[4].offset, 2048);
                assert_eq!(sparse_map[5].offset, 2560);
            }
            other => panic!("Expected SparseEntry, got {other:?}"),
        }
    }

    #[test]
    fn test_sparse_multiple_extension_blocks() {
        // 4 inline + 21 in ext1 + 3 in ext2 = 28 total
        let inline = [(0u64, 10), (100, 10), (200, 10), (300, 10)];
        let mut ext1_entries = Vec::new();
        for i in 0..21 {
            ext1_entries.push((400 + i * 100, 10u64));
        }
        let ext2_entries = [(2500u64, 10), (2600, 10), (2700, 10)];
        let on_disk = 28 * 10u64;
        let real_size = 2710;

        let header = make_gnu_sparse_header(b"multi_ext.txt", &inline, on_disk, real_size, true);
        let ext1 = make_gnu_ext_sparse(&ext1_entries, true);
        let ext2 = make_gnu_ext_sparse(&ext2_entries, false);

        let mut archive = Vec::new();
        archive.extend_from_slice(&header);
        archive.extend_from_slice(&ext1);
        archive.extend_from_slice(&ext2);
        archive.extend(zeroes(on_disk.next_multiple_of(512) as usize));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::SparseEntry {
                consumed,
                sparse_map,
                real_size: rs,
                ..
            } => {
                assert_eq!(consumed, 3 * HEADER_SIZE);
                assert_eq!(rs, real_size);
                assert_eq!(sparse_map.len(), 28);
            }
            other => panic!("Expected SparseEntry, got {other:?}"),
        }
    }

    #[test]
    fn test_sparse_need_data_for_extension() {
        // Header says isextended=true, but we only provide the header.
        // Parser should return NeedData.
        let header = make_gnu_sparse_header(
            b"need_ext.txt",
            &[(0, 100)],
            100,
            100,
            true, // extension expected
        );

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&header).unwrap();

        match event {
            ParseEvent::NeedData { min_bytes } => {
                assert_eq!(min_bytes, 2 * HEADER_SIZE);
            }
            other => panic!("Expected NeedData, got {other:?}"),
        }
    }

    #[test]
    fn test_sparse_need_data_chained_extensions() {
        // Header + ext1 (isextended=true), but ext2 not provided.
        let header = make_gnu_sparse_header(b"chain.txt", &[(0, 10)], 20, 20, true);
        let ext1 = make_gnu_ext_sparse(&[(10, 10)], true); // needs another block

        let mut input = Vec::new();
        input.extend_from_slice(&header);
        input.extend_from_slice(&ext1);

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&input).unwrap();

        match event {
            ParseEvent::NeedData { min_bytes } => {
                assert_eq!(min_bytes, 3 * HEADER_SIZE);
            }
            other => panic!("Expected NeedData, got {other:?}"),
        }
    }

    #[test]
    fn test_sparse_not_gnu_header() {
        // UStar header with type 'S' — should error since sparse requires GNU
        let header = make_header(b"bad_sparse.txt", 0, b'S');
        let mut archive = Vec::new();
        archive.extend_from_slice(&header);
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let err = parser.parse(&archive).unwrap_err();
        assert!(matches!(err, ParseError::SparseNotGnu));
    }

    #[test]
    fn test_sparse_too_many_entries() {
        // Set a low limit and exceed it via extension blocks.
        let header = make_gnu_sparse_header(
            b"too_many.txt",
            &[(0, 10), (100, 10), (200, 10)],
            40,
            400,
            true,
        );
        // Extension block with 3 more entries → total 6
        let ext = make_gnu_ext_sparse(&[(300, 10)], false);

        let mut archive = Vec::new();
        archive.extend_from_slice(&header);
        archive.extend_from_slice(&ext);
        archive.extend(zeroes(512));
        archive.extend(zeroes(1024));

        let limits = Limits {
            max_sparse_entries: 3,
            ..Default::default()
        };
        let mut parser = Parser::new(limits);
        let err = parser.parse(&archive).unwrap_err();
        assert!(matches!(
            err,
            ParseError::TooManySparseEntries { count: 4, limit: 3 }
        ));
    }

    #[test]
    fn test_sparse_with_gnu_long_name() {
        // GNU long name followed by a sparse entry — both extensions
        // should compose correctly.
        let long_name = "a/".to_string() + &"x".repeat(200);

        let on_disk = 512u64;
        let real_size = 8192u64;
        let header = make_gnu_sparse_header(b"placeholder", &[(0, 512)], on_disk, real_size, false);

        let mut archive = Vec::new();
        archive.extend(make_gnu_long_name(long_name.as_bytes()));
        archive.extend_from_slice(&header);
        archive.extend(zeroes(on_disk as usize)); // content
        archive.extend(zeroes(1024)); // end

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::SparseEntry {
                entry,
                sparse_map,
                real_size: rs,
                ..
            } => {
                assert_eq!(entry.path.as_ref(), long_name.as_bytes());
                assert_eq!(rs, real_size);
                assert_eq!(sparse_map.len(), 1);
                assert_eq!(sparse_map[0].length, 512);
            }
            other => panic!("Expected SparseEntry, got {other:?}"),
        }
    }

    #[test]
    fn test_sparse_need_data_is_side_effect_free() {
        // Provide only the header (isextended=true) → NeedData.
        // Then provide the full archive → SparseEntry.
        // The parser should not have modified state from the first call.
        let header = make_gnu_sparse_header(b"retry.txt", &[(0, 100)], 200, 300, true);
        let ext = make_gnu_ext_sparse(&[(100, 100)], false);

        let mut parser = Parser::new(Limits::default());

        // First attempt: only header
        let event = parser.parse(&header).unwrap();
        assert!(matches!(event, ParseEvent::NeedData { .. }));

        // Second attempt: full archive
        let mut full = Vec::new();
        full.extend_from_slice(&header);
        full.extend_from_slice(&ext);
        full.extend(zeroes(512)); // content
        full.extend(zeroes(1024)); // end

        let event = parser.parse(&full).unwrap();
        match event {
            ParseEvent::SparseEntry {
                consumed,
                sparse_map,
                ..
            } => {
                assert_eq!(consumed, 2 * HEADER_SIZE);
                assert_eq!(sparse_map.len(), 2);
            }
            other => panic!("Expected SparseEntry, got {other:?}"),
        }
    }

    // =========================================================================
    // PAX sparse tests
    // =========================================================================

    #[test]
    fn test_pax_sparse_v01_map() {
        // PAX v0.1: GNU.sparse.map as comma-separated offset,length pairs
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[
            ("GNU.sparse.map", b"0,100,200,100,400,50"),
            ("GNU.sparse.realsize", b"450"),
            ("GNU.sparse.name", b"real_name.txt"),
        ]));
        // The actual file header — 250 bytes of on-disk data
        archive.extend_from_slice(&make_header(b"placeholder.txt", 250, b'0'));
        archive.extend(zeroes(512)); // content (250 bytes padded)
        archive.extend(zeroes(1024)); // end of archive

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::SparseEntry {
                entry,
                sparse_map,
                real_size,
                ..
            } => {
                assert_eq!(entry.path.as_ref(), b"real_name.txt");
                assert_eq!(real_size, 450);
                assert_eq!(sparse_map.len(), 3);
                assert_eq!(
                    sparse_map[0],
                    SparseEntry {
                        offset: 0,
                        length: 100
                    }
                );
                assert_eq!(
                    sparse_map[1],
                    SparseEntry {
                        offset: 200,
                        length: 100
                    }
                );
                assert_eq!(
                    sparse_map[2],
                    SparseEntry {
                        offset: 400,
                        length: 50
                    }
                );
            }
            other => panic!("Expected SparseEntry, got {other:?}"),
        }
    }

    #[test]
    fn test_pax_sparse_v00_pairs() {
        // PAX v0.0: repeated GNU.sparse.offset / GNU.sparse.numbytes pairs
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[
            ("GNU.sparse.offset", b"0"),
            ("GNU.sparse.numbytes", b"100"),
            ("GNU.sparse.offset", b"1024"),
            ("GNU.sparse.numbytes", b"200"),
            ("GNU.sparse.realsize", b"1224"),
            ("GNU.sparse.name", b"v00_sparse.dat"),
        ]));
        archive.extend_from_slice(&make_header(b"placeholder", 300, b'0'));
        archive.extend(zeroes(512)); // content
        archive.extend(zeroes(1024)); // end

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::SparseEntry {
                entry,
                sparse_map,
                real_size,
                ..
            } => {
                assert_eq!(entry.path.as_ref(), b"v00_sparse.dat");
                assert_eq!(real_size, 1224);
                assert_eq!(sparse_map.len(), 2);
                assert_eq!(
                    sparse_map[0],
                    SparseEntry {
                        offset: 0,
                        length: 100
                    }
                );
                assert_eq!(
                    sparse_map[1],
                    SparseEntry {
                        offset: 1024,
                        length: 200
                    }
                );
            }
            other => panic!("Expected SparseEntry, got {other:?}"),
        }
    }

    #[test]
    fn test_pax_sparse_v10_data_prefix() {
        // PAX v1.0: sparse map in data block prefix
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[
            ("GNU.sparse.major", b"1"),
            ("GNU.sparse.minor", b"0"),
            ("GNU.sparse.realsize", b"2048"),
            ("GNU.sparse.name", b"v10_sparse.bin"),
        ]));

        // The data block prefix contains the sparse map:
        // "2\n0\n100\n1024\n200\n" = 20 bytes, padded to 512
        let sparse_data = b"2\n0\n100\n1024\n200\n";
        let on_disk_content = 300u64; // actual data bytes after the map
        let total_size = 512 + on_disk_content; // map prefix (padded) + content

        archive.extend_from_slice(&make_header(b"placeholder", total_size, b'0'));
        // Data: sparse map prefix (padded to 512) + actual content
        let mut data_block = vec![0u8; 512];
        data_block[..sparse_data.len()].copy_from_slice(sparse_data);
        archive.extend_from_slice(&data_block);
        archive.extend(zeroes(on_disk_content.next_multiple_of(512) as usize));
        archive.extend(zeroes(1024)); // end

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::SparseEntry {
                consumed,
                entry,
                sparse_map,
                real_size,
            } => {
                assert_eq!(entry.path.as_ref(), b"v10_sparse.bin");
                assert_eq!(real_size, 2048);
                assert_eq!(sparse_map.len(), 2);
                assert_eq!(
                    sparse_map[0],
                    SparseEntry {
                        offset: 0,
                        length: 100
                    }
                );
                assert_eq!(
                    sparse_map[1],
                    SparseEntry {
                        offset: 1024,
                        length: 200
                    }
                );
                // entry.size is the on-disk content after the map prefix
                assert_eq!(entry.size, on_disk_content);
                // consumed includes: PAX header + its content + actual header
                // + sparse map prefix (512 bytes)
                let pax_hdr_size = archive.len()
                    - HEADER_SIZE // actual file header
                    - 512 // sparse map data
                    - on_disk_content.next_multiple_of(512) as usize
                    - 1024; // end
                let expected_consumed = pax_hdr_size + HEADER_SIZE + 512;
                assert_eq!(consumed, expected_consumed);
            }
            other => panic!("Expected SparseEntry, got {other:?}"),
        }
    }

    #[test]
    fn test_pax_sparse_v10_need_data() {
        // PAX v1.0 with insufficient data for the sparse map prefix.
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[
            ("GNU.sparse.major", b"1"),
            ("GNU.sparse.minor", b"0"),
            ("GNU.sparse.realsize", b"100"),
            ("GNU.sparse.name", b"v10_need.txt"),
        ]));

        // Provide the actual file header but NOT the data block.
        archive.extend_from_slice(&make_header(b"placeholder", 512, b'0'));

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        assert!(
            matches!(event, ParseEvent::NeedData { .. }),
            "Expected NeedData, got {event:?}"
        );
    }

    #[test]
    fn test_pax_sparse_v01_odd_map_values() {
        // GNU.sparse.map with odd number of values is an error
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[
            ("GNU.sparse.map", b"0,100,200"),
            ("GNU.sparse.realsize", b"300"),
        ]));
        archive.extend_from_slice(&make_header(b"file.txt", 100, b'0'));
        archive.extend(zeroes(512));
        archive.extend(zeroes(1024));

        let mut parser = Parser::new(Limits::default());
        let err = parser.parse(&archive).unwrap_err();
        assert!(matches!(err, ParseError::InvalidPaxSparseMap(_)));
    }

    #[test]
    fn test_pax_sparse_v10_too_many_entries() {
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[
            ("GNU.sparse.major", b"1"),
            ("GNU.sparse.minor", b"0"),
            ("GNU.sparse.realsize", b"100"),
            ("GNU.sparse.name", b"toomany.txt"),
        ]));

        // Sparse map claims 1000 entries
        let sparse_data = b"1000\n";
        let total_size = 512u64;
        archive.extend_from_slice(&make_header(b"placeholder", total_size, b'0'));
        let mut data_block = vec![0u8; 512];
        data_block[..sparse_data.len()].copy_from_slice(sparse_data);
        archive.extend_from_slice(&data_block);
        archive.extend(zeroes(1024));

        let limits = Limits {
            max_sparse_entries: 100,
            ..Default::default()
        };
        let mut parser = Parser::new(limits);
        let err = parser.parse(&archive).unwrap_err();
        assert!(
            matches!(
                err,
                ParseError::TooManySparseEntries {
                    count: 1000,
                    limit: 100
                }
            ),
            "got: {err:?}"
        );
    }

    #[test]
    fn test_pax_sparse_without_version_is_v00() {
        // PAX sparse data without version fields should be treated as v0.0
        // (offset/numbytes pairs), not routed to v1.0 handler.
        let mut archive = Vec::new();
        archive.extend(make_pax_header(&[
            ("GNU.sparse.offset", b"0"),
            ("GNU.sparse.numbytes", b"50"),
            ("GNU.sparse.realsize", b"50"),
        ]));
        archive.extend_from_slice(&make_header(b"noversion.txt", 50, b'0'));
        archive.extend(zeroes(512)); // content
        archive.extend(zeroes(1024)); // end

        let mut parser = Parser::new(Limits::default());
        let event = parser.parse(&archive).unwrap();

        match event {
            ParseEvent::SparseEntry {
                sparse_map,
                real_size,
                ..
            } => {
                assert_eq!(sparse_map.len(), 1);
                assert_eq!(
                    sparse_map[0],
                    SparseEntry {
                        offset: 0,
                        length: 50
                    }
                );
                assert_eq!(real_size, 50);
            }
            other => panic!("Expected SparseEntry, got {other:?}"),
        }
    }

    // =========================================================================
    // Sparse proptests
    // =========================================================================

    mod sparse_proptests {
        use super::*;
        use proptest::prelude::*;

        /// Strategy for a sparse map: a sorted list of non-overlapping
        /// (offset, length) pairs with reasonable values.
        fn sparse_map_strategy(max_entries: usize) -> impl Strategy<Value = Vec<(u64, u64)>> {
            proptest::collection::vec((0u64..0x10_000, 1u64..0x1000), 0..=max_entries).prop_map(
                |raw| {
                    // Sort by offset, then deduplicate/separate so entries
                    // don't overlap.
                    let mut entries: Vec<(u64, u64)> = Vec::new();
                    let mut cursor = 0u64;
                    for (gap, length) in raw {
                        let offset = cursor.saturating_add(gap);
                        entries.push((offset, length));
                        cursor = offset.saturating_add(length);
                    }
                    entries
                },
            )
        }

        proptest! {
            #[test]
            fn test_sparse_roundtrip_inline(
                entries in sparse_map_strategy(4),
                name_len in 1usize..50,
            ) {
                let name: Vec<u8> = (0..name_len).map(|i| b'a' + (i % 26) as u8).collect();
                let on_disk: u64 = entries.iter().map(|(_, l)| l).sum();
                let real_size = entries.last().map(|(o, l)| o + l).unwrap_or(0);

                let header = make_gnu_sparse_header(
                    &name,
                    &entries,
                    on_disk,
                    real_size,
                    false,
                );

                let mut archive = Vec::new();
                archive.extend_from_slice(&header);
                archive.extend(zeroes(on_disk.next_multiple_of(512) as usize));
                archive.extend(zeroes(1024));

                let mut parser = Parser::new(Limits::default());
                let event = parser.parse(&archive).unwrap();

                match event {
                    ParseEvent::SparseEntry {
                        consumed,
                        sparse_map,
                        real_size: rs,
                        entry,
                        ..
                    } => {
                        prop_assert_eq!(consumed, HEADER_SIZE);
                        prop_assert_eq!(&entry.path[..], &name[..]);
                        prop_assert_eq!(rs, real_size);
                        prop_assert_eq!(sparse_map.len(), entries.len());
                        for (i, &(off, len)) in entries.iter().enumerate() {
                            prop_assert_eq!(sparse_map[i].offset, off);
                            prop_assert_eq!(sparse_map[i].length, len);
                        }
                    }
                    other => {
                        return Err(proptest::test_runner::TestCaseError::fail(
                            format!("Expected SparseEntry, got {other:?}")));
                    }
                }
            }

            #[test]
            fn test_sparse_roundtrip_extended(
                // 5..=25 entries forces at least one extension block
                entries in sparse_map_strategy(25).prop_filter(
                    "need >4 entries for extension",
                    |e| e.len() > 4
                ),
            ) {
                let on_disk: u64 = entries.iter().map(|(_, l)| l).sum();
                let real_size = entries.last().map(|(o, l)| o + l).unwrap_or(0);

                // Split into inline (first 4) and extension blocks (21 per block)
                let (inline, rest) = entries.split_at(4);
                let header = make_gnu_sparse_header(
                    b"proptest_ext.bin",
                    inline,
                    on_disk,
                    real_size,
                    !rest.is_empty(),
                );

                let mut archive = Vec::new();
                archive.extend_from_slice(&header);

                // Emit extension blocks, 21 entries per block
                let chunks: Vec<&[(u64, u64)]> = rest.chunks(21).collect();
                for (i, chunk) in chunks.iter().enumerate() {
                    let is_last = i == chunks.len() - 1;
                    let ext = make_gnu_ext_sparse(chunk, !is_last);
                    archive.extend_from_slice(&ext);
                }

                archive.extend(zeroes(on_disk.next_multiple_of(512) as usize));
                archive.extend(zeroes(1024));

                let mut parser = Parser::new(Limits::default());
                let event = parser.parse(&archive).unwrap();

                match event {
                    ParseEvent::SparseEntry {
                        consumed,
                        sparse_map,
                        real_size: rs,
                        ..
                    } => {
                        let expected_blocks = 1 + chunks.len();
                        prop_assert_eq!(consumed, expected_blocks * HEADER_SIZE);
                        prop_assert_eq!(rs, real_size);
                        prop_assert_eq!(sparse_map.len(), entries.len());
                        for (i, &(off, len)) in entries.iter().enumerate() {
                            prop_assert_eq!(sparse_map[i].offset, off);
                            prop_assert_eq!(sparse_map[i].length, len);
                        }
                    }
                    other => {
                        return Err(proptest::test_runner::TestCaseError::fail(
                            format!("Expected SparseEntry, got {other:?}")));
                    }
                }
            }

            #[test]
            fn test_sparse_need_data_then_retry(
                n_ext_entries in 1usize..10,
            ) {
                // Build a sparse file with extension blocks, feed partial
                // data first (just the header), verify NeedData, then feed
                // the full archive and verify success.
                let inline = [(0u64, 100), (200, 100), (400, 100), (600, 100)];
                let ext_entries: Vec<(u64, u64)> = (0..n_ext_entries)
                    .map(|i| (800 + i as u64 * 200, 100))
                    .collect();
                let total = 4 + n_ext_entries;
                let on_disk = total as u64 * 100;
                let real_size = ext_entries.last().map(|(o, l)| o + l).unwrap_or(800);

                let header = make_gnu_sparse_header(
                    b"retry_ext.txt",
                    &inline,
                    on_disk,
                    real_size,
                    true,
                );
                let ext = make_gnu_ext_sparse(&ext_entries, false);

                let mut parser = Parser::new(Limits::default());

                // Partial: just the header
                let event = parser.parse(&header).unwrap();
                assert!(matches!(event, ParseEvent::NeedData { .. }));

                // Full archive
                let mut full = Vec::new();
                full.extend_from_slice(&header);
                full.extend_from_slice(&ext);
                full.extend(zeroes(on_disk.next_multiple_of(512) as usize));
                full.extend(zeroes(1024));

                let event = parser.parse(&full).unwrap();
                match event {
                    ParseEvent::SparseEntry { sparse_map, .. } => {
                        prop_assert_eq!(sparse_map.len(), total);
                    }
                    other => {
                        return Err(proptest::test_runner::TestCaseError::fail(
                            format!("Expected SparseEntry, got {other:?}")));
                    }
                }
            }

            // =================================================================
            // PAX sparse format proptests
            // =================================================================

            #[test]
            fn test_pax_sparse_v00_roundtrip(
                entries in sparse_map_strategy(15),
                name_len in 1usize..50,
            ) {
                let name: Vec<u8> = (0..name_len).map(|i| b'a' + (i % 26) as u8).collect();
                let on_disk: u64 = entries.iter().map(|(_, l)| l).sum();
                let real_size = entries.last().map(|(o, l)| o + l).unwrap_or(0);

                let mut pax_kv: Vec<(&str, Vec<u8>)> = Vec::new();
                for &(offset, length) in &entries {
                    pax_kv.push(("GNU.sparse.offset", offset.to_string().into_bytes()));
                    pax_kv.push(("GNU.sparse.numbytes", length.to_string().into_bytes()));
                }
                pax_kv.push(("GNU.sparse.realsize", real_size.to_string().into_bytes()));
                pax_kv.push(("GNU.sparse.name", name.clone()));

                let pax_refs: Vec<(&str, &[u8])> =
                    pax_kv.iter().map(|(k, v)| (*k, v.as_slice())).collect();

                let mut archive = Vec::new();
                archive.extend(make_pax_header(&pax_refs));
                archive.extend_from_slice(&make_header(b"placeholder", on_disk, b'0'));
                archive.extend(zeroes(on_disk.next_multiple_of(512) as usize));
                archive.extend(zeroes(1024));

                let mut parser = Parser::new(Limits::default());
                let event = parser.parse(&archive).unwrap();

                match event {
                    ParseEvent::SparseEntry { sparse_map, real_size: rs, entry, .. } => {
                        prop_assert_eq!(&entry.path[..], &name[..]);
                        prop_assert_eq!(rs, real_size);
                        prop_assert_eq!(sparse_map.len(), entries.len());
                        for (i, &(off, len)) in entries.iter().enumerate() {
                            prop_assert_eq!(sparse_map[i].offset, off);
                            prop_assert_eq!(sparse_map[i].length, len);
                        }
                    }
                    ParseEvent::Entry { .. } if entries.is_empty() => {}
                    other => {
                        return Err(proptest::test_runner::TestCaseError::fail(
                            format!("Expected SparseEntry, got {other:?}")));
                    }
                }
            }

            #[test]
            fn test_pax_sparse_v01_roundtrip(
                entries in sparse_map_strategy(15),
                name_len in 1usize..50,
            ) {
                let name: Vec<u8> = (0..name_len).map(|i| b'a' + (i % 26) as u8).collect();
                let on_disk: u64 = entries.iter().map(|(_, l)| l).sum();
                let real_size = entries.last().map(|(o, l)| o + l).unwrap_or(0);

                let map_str: String = entries
                    .iter()
                    .flat_map(|(o, l)| [o.to_string(), l.to_string()])
                    .collect::<Vec<_>>()
                    .join(",");
                let map_bytes = map_str.into_bytes();
                let rs_bytes = real_size.to_string().into_bytes();

                let pax_refs: Vec<(&str, &[u8])> = vec![
                    ("GNU.sparse.map", &map_bytes),
                    ("GNU.sparse.realsize", &rs_bytes),
                    ("GNU.sparse.name", &name),
                ];

                let mut archive = Vec::new();
                archive.extend(make_pax_header(&pax_refs));
                archive.extend_from_slice(&make_header(b"placeholder", on_disk, b'0'));
                archive.extend(zeroes(on_disk.next_multiple_of(512) as usize));
                archive.extend(zeroes(1024));

                let mut parser = Parser::new(Limits::default());
                let event = parser.parse(&archive).unwrap();

                match event {
                    ParseEvent::SparseEntry { sparse_map, real_size: rs, entry, .. } => {
                        prop_assert_eq!(&entry.path[..], &name[..]);
                        prop_assert_eq!(rs, real_size);
                        prop_assert_eq!(sparse_map.len(), entries.len());
                        for (i, &(off, len)) in entries.iter().enumerate() {
                            prop_assert_eq!(sparse_map[i].offset, off);
                            prop_assert_eq!(sparse_map[i].length, len);
                        }
                    }
                    ParseEvent::Entry { .. } if entries.is_empty() => {}
                    other => {
                        return Err(proptest::test_runner::TestCaseError::fail(
                            format!("Expected SparseEntry, got {other:?}")));
                    }
                }
            }

            #[test]
            fn test_pax_sparse_v10_roundtrip(
                entries in sparse_map_strategy(20),
                name_len in 1usize..50,
            ) {
                let name: Vec<u8> = (0..name_len).map(|i| b'a' + (i % 26) as u8).collect();
                let on_disk: u64 = entries.iter().map(|(_, l)| l).sum();
                let real_size = entries.last().map(|(o, l)| o + l).unwrap_or(0);

                let mut map_data = format!("{}\n", entries.len());
                for &(offset, length) in &entries {
                    map_data.push_str(&format!("{offset}\n{length}\n"));
                }
                let map_bytes = map_data.into_bytes();
                let map_padded = map_bytes.len().next_multiple_of(HEADER_SIZE);
                let total_size = map_padded as u64 + on_disk;
                let rs_bytes = real_size.to_string().into_bytes();

                let pax_refs: Vec<(&str, &[u8])> = vec![
                    ("GNU.sparse.major", b"1"),
                    ("GNU.sparse.minor", b"0"),
                    ("GNU.sparse.realsize", &rs_bytes),
                    ("GNU.sparse.name", &name),
                ];

                let mut archive = Vec::new();
                archive.extend(make_pax_header(&pax_refs));
                archive.extend_from_slice(&make_header(b"placeholder", total_size, b'0'));
                let mut data_block = vec![0u8; map_padded];
                data_block[..map_bytes.len()].copy_from_slice(&map_bytes);
                archive.extend_from_slice(&data_block);
                archive.extend(zeroes(on_disk.next_multiple_of(512) as usize));
                archive.extend(zeroes(1024));

                let mut parser = Parser::new(Limits::default());
                let event = parser.parse(&archive).unwrap();

                match event {
                    ParseEvent::SparseEntry { sparse_map, real_size: rs, entry, .. } => {
                        prop_assert_eq!(&entry.path[..], &name[..]);
                        prop_assert_eq!(rs, real_size);
                        prop_assert_eq!(entry.size, on_disk);
                        prop_assert_eq!(sparse_map.len(), entries.len());
                        for (i, &(off, len)) in entries.iter().enumerate() {
                            prop_assert_eq!(sparse_map[i].offset, off);
                            prop_assert_eq!(sparse_map[i].length, len);
                        }
                    }
                    other => {
                        return Err(proptest::test_runner::TestCaseError::fail(
                            format!("Expected SparseEntry, got {other:?}")));
                    }
                }
            }
        }
    }

    /// Regression test: `add_consumed` must not overflow when chained
    /// extension headers declare very large sizes.
    ///
    /// With `Limits::permissive()` (`max_metadata_size = u32::MAX`),
    /// extension headers can declare sizes close to `u32::MAX`.  When
    /// `handle_extension` recurses and the inner call returns `NeedData`,
    /// `add_consumed(total_size)` is applied on unwind at each level.
    /// Before the fix, `min_bytes + n` used plain `+` and could overflow
    /// `usize` (especially on 32-bit targets).  The fix uses
    /// `saturating_add`.  This test verifies the parser returns `NeedData`
    /// (or an error) without panicking.
    #[test]
    fn test_add_consumed_no_overflow() {
        // First extension: a complete GNU long name ('L') with a small
        // payload so the parser can fully consume it and recurse.
        let long_name = b"a]long/path".to_vec();
        let gnu_entry = make_gnu_long_name(&long_name);
        let first_entry_size = gnu_entry.len(); // 1024 bytes (header + padded name)

        // Second extension: a PAX ('x') header that declares a size close
        // to u32::MAX.  We only provide the header—not the content—so the
        // recursive call in handle_extension will return NeedData with
        // min_bytes ≈ pax_size + 512.  On unwind, add_consumed adds
        // first_entry_size, giving min_bytes ≈ pax_size + 512 + 1024.
        // On 32-bit this would overflow without saturating_add.
        let pax_size: u64 = u32::MAX as u64 - long_name.len() as u64 - 512;
        let pax_header = make_header(b"PaxHeaders/file", pax_size, b'x');

        // Build input: complete GNU long name entry + PAX header only (no
        // PAX content).
        let mut input = Vec::with_capacity(first_entry_size + HEADER_SIZE);
        input.extend_from_slice(&gnu_entry);
        input.extend_from_slice(&pax_header);

        let mut parser = Parser::new(Limits::permissive());
        let result = parser.parse(&input);

        // The parser must not panic.  It should return NeedData (because the
        // PAX content is missing) or an error—both are acceptable.
        match result {
            Ok(ParseEvent::NeedData { min_bytes }) => {
                // min_bytes must be at least the PAX entry's total_size
                // (header + padded content), and must not have wrapped to
                // a small value due to overflow.
                assert!(
                    min_bytes > HEADER_SIZE,
                    "min_bytes should be large, got {min_bytes}"
                );
            }
            Err(_) => {
                // An error (e.g. metadata too large) is also acceptable;
                // the important thing is no panic from arithmetic overflow.
            }
            other => panic!(
                "Expected NeedData or Err for truncated extension chain, got {:?}",
                other
            ),
        }
    }
}
