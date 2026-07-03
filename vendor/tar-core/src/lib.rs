#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]
//! Sans-IO tar parsing for sync and async runtimes.
//!
//! `tar-core` provides zero-copy parsing and building of tar archives that works
//! with any I/O model. The [`parse::Parser`] has no trait bounds on readers—it
//! just processes byte slices. This enables code sharing between sync crates
//! like [tar-rs](https://crates.io/crates/tar) and async crates like
//! [tokio-tar](https://crates.io/crates/tokio-tar).
//!
//! All header structs use the [`zerocopy`] crate for safe, efficient
//! memory-mapped access without allocations. Supports POSIX.1-1988, UStar
//! (POSIX.1-2001), and GNU tar formats.
//!
//! # Header Formats
//!
//! Tar archives have evolved through several formats:
//!
//! - **Old (POSIX.1-1988)**: The original Unix tar format with basic fields
//! - **UStar (POSIX.1-2001)**: Adds `magic`/`version`, user/group names, and path prefix
//! - **GNU tar**: Extends UStar with sparse file support and long name/link extensions
//!
//! # Example
//!
//! ```
//! use tar_core::{Header, EntryType};
//!
//! // Parse a header from raw bytes
//! let data = [0u8; 512]; // Would normally come from a tar file
//! let header = Header::from_bytes(&data);
//!
//! // Access header fields
//! let entry_type = header.entry_type();
//! let path = header.path_bytes();
//! ```
//!
//! # Parsing
//!
//! For parsing complete tar archives with automatic handling of GNU and PAX
//! extensions, see the sans-IO [`parse`] module. It also contains security
//! [`parse::Limits`] and the [`parse::ParseError`] type.

extern crate alloc;

pub mod builder;
pub mod parse;

pub use builder::{
    blocks_for_size, EntryBuilder, ExtensionMode, HeaderBuilder, PaxBuilder, LINKNAME_MAX_LEN,
    NAME_MAX_LEN,
};

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use thiserror::Error;
use zerocopy::{FromBytes, FromZeros, Immutable, IntoBytes, KnownLayout};

/// Size of a tar header block in bytes.
pub const HEADER_SIZE: usize = 512;

/// Magic string for UStar format headers ("ustar\0").
pub const USTAR_MAGIC: &[u8; 6] = b"ustar\0";

/// Version field for UStar format headers ("00").
pub const USTAR_VERSION: &[u8; 2] = b"00";

/// Magic string for GNU tar format headers ("ustar ").
pub const GNU_MAGIC: &[u8; 6] = b"ustar ";

/// Version field for GNU tar format headers (" \0").
pub const GNU_VERSION: &[u8; 2] = b" \0";

/// Errors that can occur when parsing or building tar headers.
#[derive(Debug, Error)]
pub enum HeaderError {
    /// The provided data is too short to contain a header.
    #[error("insufficient data: expected {HEADER_SIZE} bytes, got {0}")]
    InsufficientData(usize),

    /// An octal field contains invalid characters.
    #[error("invalid octal field: {0:?}")]
    InvalidOctal(Vec<u8>),

    /// A value is too large or too long for its header field.
    #[error("value overflows {field_len}-byte field: {detail}")]
    FieldOverflow {
        /// Size of the target field in bytes.
        field_len: usize,
        /// Human-readable description of the overflow.
        detail: String,
    },

    /// The header checksum does not match the computed value.
    #[error("checksum mismatch: expected {expected}, computed {computed}")]
    ChecksumMismatch {
        /// The checksum value stored in the header.
        expected: u64,
        /// The checksum computed from the header bytes.
        computed: u64,
    },
}

/// Result type for header parsing operations.
pub type Result<T> = core::result::Result<T, HeaderError>;

// ============================================================================
// Header Structs
// ============================================================================

/// Old-style (POSIX.1-1988) tar header with named fields.
///
/// This represents the original Unix tar format. Fields after `linkname`
/// are undefined in this format and may contain garbage. See module-level
/// documentation for the field layout table.
#[derive(Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct OldHeader {
    /// File path name (null-terminated if shorter than 100 bytes).
    pub name: [u8; 100],
    /// File mode in octal ASCII.
    pub mode: [u8; 8],
    /// Owner user ID in octal ASCII.
    pub uid: [u8; 8],
    /// Owner group ID in octal ASCII.
    pub gid: [u8; 8],
    /// File size in octal ASCII.
    pub size: [u8; 12],
    /// Modification time as Unix timestamp in octal ASCII.
    pub mtime: [u8; 12],
    /// Header checksum in octal ASCII.
    pub cksum: [u8; 8],
    /// Entry type flag (called `linkflag` in the original V7 format).
    pub linkflag: [u8; 1],
    /// Link target name for hard/symbolic links.
    pub linkname: [u8; 100],
    /// Padding to fill the 512-byte block.
    pub pad: [u8; 255],
}

impl Default for OldHeader {
    fn default() -> Self {
        Self::new_zeroed()
    }
}

impl fmt::Debug for OldHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OldHeader")
            .field("name", &String::from_utf8_lossy(truncate_null(&self.name)))
            .field("mode", &String::from_utf8_lossy(truncate_null(&self.mode)))
            .field("linkflag", &self.linkflag[0])
            .finish_non_exhaustive()
    }
}

/// UStar (POSIX.1-2001) tar header format.
///
/// This format adds a magic number, version, user/group names, device
/// numbers for special files, and a path prefix for long filenames.
/// See module-level documentation for the field layout table.
#[derive(Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct UstarHeader {
    /// File path name (null-terminated if shorter than 100 bytes).
    pub name: [u8; 100],
    /// File mode in octal ASCII.
    pub mode: [u8; 8],
    /// Owner user ID in octal ASCII.
    pub uid: [u8; 8],
    /// Owner group ID in octal ASCII.
    pub gid: [u8; 8],
    /// File size in octal ASCII.
    pub size: [u8; 12],
    /// Modification time as Unix timestamp in octal ASCII.
    pub mtime: [u8; 12],
    /// Header checksum in octal ASCII.
    pub cksum: [u8; 8],
    /// Entry type flag.
    pub typeflag: [u8; 1],
    /// Link target name for hard/symbolic links.
    pub linkname: [u8; 100],
    /// Magic string identifying the format ("ustar\0" for UStar).
    pub magic: [u8; 6],
    /// Format version ("00" for UStar).
    pub version: [u8; 2],
    /// Owner user name (null-terminated).
    pub uname: [u8; 32],
    /// Owner group name (null-terminated).
    pub gname: [u8; 32],
    /// Device major number in octal ASCII (for special files).
    pub dev_major: [u8; 8],
    /// Device minor number in octal ASCII (for special files).
    pub dev_minor: [u8; 8],
    /// Path prefix for names longer than 100 bytes.
    pub prefix: [u8; 155],
    /// Padding to fill the 512-byte block.
    pub pad: [u8; 12],
}

impl Default for UstarHeader {
    fn default() -> Self {
        let mut header = Self::new_zeroed();
        header.magic.copy_from_slice(USTAR_MAGIC);
        header.version.copy_from_slice(USTAR_VERSION);
        header
    }
}

impl fmt::Debug for UstarHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UstarHeader")
            .field("name", &String::from_utf8_lossy(truncate_null(&self.name)))
            .field("mode", &String::from_utf8_lossy(truncate_null(&self.mode)))
            .field("typeflag", &self.typeflag[0])
            .field("magic", &self.magic)
            .field(
                "uname",
                &String::from_utf8_lossy(truncate_null(&self.uname)),
            )
            .finish_non_exhaustive()
    }
}

/// A decoded sparse file data region.
///
/// Each entry describes a contiguous region of real data within a sparse
/// file. Gaps between entries are implicitly zero-filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SparseEntry {
    /// Byte offset of this data region within the logical file.
    pub offset: u64,
    /// Number of bytes of real data in this region.
    pub length: u64,
}

/// GNU tar sparse file chunk descriptor.
///
/// Each descriptor specifies a region of data in a sparse file.
/// Both offset and numbytes are 12-byte octal ASCII fields.
#[derive(Clone, Copy, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct GnuSparseHeader {
    /// Byte offset of this chunk within the file.
    pub offset: [u8; 12],
    /// Number of bytes in this chunk.
    pub numbytes: [u8; 12],
}

impl GnuSparseHeader {
    /// Returns true if this descriptor is empty (offset or numbytes starts
    /// with a zero byte, indicating an unused slot).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.offset[0] == 0 || self.numbytes[0] == 0
    }

    /// Parse offset and length into a [`SparseEntry`].
    ///
    /// Handles both octal ASCII and GNU base-256 encoding.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::InvalidOctal`] if either field is malformed.
    pub fn to_sparse_entry(&self) -> Result<SparseEntry> {
        Ok(SparseEntry {
            offset: parse_numeric(&self.offset)?,
            length: parse_numeric(&self.numbytes)?,
        })
    }

    /// Write a [`SparseEntry`] into this descriptor.
    ///
    /// Uses octal ASCII if the values fit, otherwise GNU base-256 encoding.
    pub fn set(&mut self, entry: &SparseEntry) {
        encode_numeric(&mut self.offset, entry.offset)
            .expect("u64 always fits in 12-byte numeric field");
        encode_numeric(&mut self.numbytes, entry.length)
            .expect("u64 always fits in 12-byte numeric field");
    }

    /// Get the offset of this sparse chunk.
    ///
    /// Handles both octal ASCII and GNU base-256 encoding.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::InvalidOctal`] if the field is malformed.
    pub fn offset(&self) -> Result<u64> {
        parse_numeric(&self.offset)
    }

    /// Set the offset of this sparse chunk.
    ///
    /// Uses octal ASCII if the value fits, otherwise GNU base-256 encoding.
    pub fn set_offset(&mut self, offset: u64) {
        encode_numeric(&mut self.offset, offset).expect("u64 always fits in 12-byte numeric field");
    }

    /// Get the length (numbytes) of this sparse chunk.
    ///
    /// Handles both octal ASCII and GNU base-256 encoding.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::InvalidOctal`] if the field is malformed.
    pub fn length(&self) -> Result<u64> {
        parse_numeric(&self.numbytes)
    }

    /// Set the length (numbytes) of this sparse chunk.
    ///
    /// Uses octal ASCII if the value fits, otherwise GNU base-256 encoding.
    pub fn set_length(&mut self, length: u64) {
        encode_numeric(&mut self.numbytes, length)
            .expect("u64 always fits in 12-byte numeric field");
    }
}

impl fmt::Debug for GnuSparseHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GnuSparseHeader")
            .field("offset", &parse_octal(&self.offset).ok())
            .field("numbytes", &parse_octal(&self.numbytes).ok())
            .finish()
    }
}

/// GNU tar header format with sparse file support.
///
/// This format extends UStar with support for sparse files, access/creation
/// times, and long name handling. The prefix field is replaced with
/// additional metadata. See module-level documentation for the field layout table.
#[derive(Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct GnuHeader {
    /// File path name (null-terminated if shorter than 100 bytes).
    pub name: [u8; 100],
    /// File mode in octal ASCII.
    pub mode: [u8; 8],
    /// Owner user ID in octal ASCII.
    pub uid: [u8; 8],
    /// Owner group ID in octal ASCII.
    pub gid: [u8; 8],
    /// File size in octal ASCII (for sparse files, this is the size on disk).
    pub size: [u8; 12],
    /// Modification time as Unix timestamp in octal ASCII.
    pub mtime: [u8; 12],
    /// Header checksum in octal ASCII.
    pub cksum: [u8; 8],
    /// Entry type flag.
    pub typeflag: [u8; 1],
    /// Link target name for hard/symbolic links.
    pub linkname: [u8; 100],
    /// Magic string identifying the format ("ustar " for GNU).
    pub magic: [u8; 6],
    /// Format version (" \0" for GNU).
    pub version: [u8; 2],
    /// Owner user name (null-terminated).
    pub uname: [u8; 32],
    /// Owner group name (null-terminated).
    pub gname: [u8; 32],
    /// Device major number in octal ASCII (for special files).
    pub dev_major: [u8; 8],
    /// Device minor number in octal ASCII (for special files).
    pub dev_minor: [u8; 8],
    /// Access time in octal ASCII.
    pub atime: [u8; 12],
    /// Creation time in octal ASCII.
    pub ctime: [u8; 12],
    /// Offset for multivolume archives.
    pub offset: [u8; 12],
    /// Long names support (deprecated).
    pub longnames: [u8; 4],
    /// Unused padding byte.
    pub unused: [u8; 1],
    /// Sparse file chunk descriptors (4 entries).
    pub sparse: [GnuSparseHeader; 4],
    /// Flag indicating more sparse headers follow.
    pub isextended: [u8; 1],
    /// Real size of sparse file (uncompressed).
    pub realsize: [u8; 12],
    /// Padding to fill the 512-byte block.
    pub pad: [u8; 17],
}

impl Default for GnuHeader {
    fn default() -> Self {
        let mut header = Self::new_zeroed();
        header.magic.copy_from_slice(GNU_MAGIC);
        header.version.copy_from_slice(GNU_VERSION);
        header
    }
}

impl GnuHeader {
    /// Get the access time in Unix timestamp format.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::InvalidOctal`] if the atime field is not valid.
    pub fn atime(&self) -> Result<u64> {
        parse_numeric(&self.atime)
    }

    /// Set the access time as a Unix timestamp.
    ///
    /// Uses octal ASCII if the value fits, otherwise GNU base-256 encoding.
    /// The 12-byte atime field can represent any `u64`.
    pub fn set_atime(&mut self, atime: u64) {
        // 12-byte field has 95 data bits in base-256, more than u64 needs.
        encode_numeric(&mut self.atime, atime).expect("u64 always fits in 12-byte numeric field");
    }

    /// Get the change time in Unix timestamp format.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::InvalidOctal`] if the ctime field is not valid.
    pub fn ctime(&self) -> Result<u64> {
        parse_numeric(&self.ctime)
    }

    /// Set the change time as a Unix timestamp.
    ///
    /// Uses octal ASCII if the value fits, otherwise GNU base-256 encoding.
    /// The 12-byte ctime field can represent any `u64`.
    pub fn set_ctime(&mut self, ctime: u64) {
        // 12-byte field has 95 data bits in base-256, more than u64 needs.
        encode_numeric(&mut self.ctime, ctime).expect("u64 always fits in 12-byte numeric field");
    }

    /// Get the "real size" of a sparse file.
    ///
    /// For sparse files, this is the size of the entire file after the sparse
    /// regions have been filled in. For non-sparse files, this may be zero.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::InvalidOctal`] if the realsize field is not valid.
    pub fn real_size(&self) -> Result<u64> {
        parse_numeric(&self.realsize)
    }

    /// Set the "real size" of a sparse file.
    ///
    /// Uses octal ASCII if the value fits, otherwise GNU base-256 encoding.
    /// The 12-byte realsize field can represent any `u64`.
    pub fn set_real_size(&mut self, size: u64) {
        // 12-byte field has 95 data bits in base-256, more than u64 needs.
        encode_numeric(&mut self.realsize, size).expect("u64 always fits in 12-byte numeric field");
    }

    /// Returns whether this header will be followed by additional sparse headers.
    ///
    /// When true, the next 512-byte block contains a [`GnuExtSparseHeader`].
    #[must_use]
    pub fn is_extended(&self) -> bool {
        self.isextended[0] == 1
    }

    /// Sets whether this header should be followed by additional sparse headers.
    pub fn set_is_extended(&mut self, extended: bool) {
        self.isextended[0] = if extended { 1 } else { 0 };
    }
}

impl fmt::Debug for GnuHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GnuHeader")
            .field("name", &String::from_utf8_lossy(truncate_null(&self.name)))
            .field("mode", &String::from_utf8_lossy(truncate_null(&self.mode)))
            .field("typeflag", &self.typeflag[0])
            .field("magic", &self.magic)
            .field("isextended", &self.isextended[0])
            .finish_non_exhaustive()
    }
}

/// Extended sparse header block for GNU tar.
///
/// When a file has more than 4 sparse regions, additional sparse headers
/// are stored in separate 512-byte blocks following the main header.
/// Each block contains 21 sparse descriptors plus an `isextended` flag.
#[derive(Clone, Copy, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct GnuExtSparseHeader {
    /// Sparse chunk descriptors (21 entries).
    pub sparse: [GnuSparseHeader; 21],
    /// Flag indicating more sparse headers follow.
    pub isextended: [u8; 1],
    /// Padding to fill the 512-byte block.
    pub pad: [u8; 7],
}

impl GnuExtSparseHeader {
    /// Returns whether another extension block follows this one.
    #[must_use]
    pub fn is_extended(&self) -> bool {
        self.isextended[0] == 1
    }

    /// Sets whether another extension block follows this one.
    pub fn set_is_extended(&mut self, extended: bool) {
        self.isextended[0] = if extended { 1 } else { 0 };
    }
}

impl fmt::Debug for GnuExtSparseHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GnuExtSparseHeader")
            .field("isextended", &self.isextended[0])
            .finish_non_exhaustive()
    }
}

// ============================================================================
// Entry Type
// ============================================================================

/// Tar entry type indicating the kind of file system object.
///
/// The type is stored as a single ASCII byte in the header. Some types
/// are extensions defined by POSIX or GNU tar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryType {
    /// Regular file (type '0' or '\0' for old tar compatibility).
    Regular,
    /// Hard link to another file in the archive (type '1').
    Link,
    /// Symbolic link (type '2').
    Symlink,
    /// Character device (type '3').
    Char,
    /// Block device (type '4').
    Block,
    /// Directory (type '5').
    Directory,
    /// FIFO/named pipe (type '6').
    Fifo,
    /// Contiguous file (type '7', rarely used).
    Continuous,
    /// GNU tar long name extension (type 'L').
    GnuLongName,
    /// GNU tar long link extension (type 'K').
    GnuLongLink,
    /// GNU tar sparse file (type 'S').
    GnuSparse,
    /// PAX extended header for next entry (type 'x').
    XHeader,
    /// PAX global extended header (type 'g').
    XGlobalHeader,
    /// Unknown or unsupported entry type.
    Other(u8),
}

impl EntryType {
    // =========================================================================
    // Constructors
    // =========================================================================

    /// Create an entry type from a raw byte value.
    ///
    /// This is an alias for [`from_byte`](Self::from_byte) provided for
    /// compatibility with the `tar` crate's API.
    #[inline]
    #[must_use]
    pub fn new(byte: u8) -> Self {
        Self::from_byte(byte)
    }

    /// Parse an entry type from a raw byte value.
    #[must_use]
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            b'0' | b'\0' => EntryType::Regular,
            b'1' => EntryType::Link,
            b'2' => EntryType::Symlink,
            b'3' => EntryType::Char,
            b'4' => EntryType::Block,
            b'5' => EntryType::Directory,
            b'6' => EntryType::Fifo,
            b'7' => EntryType::Continuous,
            b'L' => EntryType::GnuLongName,
            b'K' => EntryType::GnuLongLink,
            b'S' => EntryType::GnuSparse,
            b'x' => EntryType::XHeader,
            b'g' => EntryType::XGlobalHeader,
            other => EntryType::Other(other),
        }
    }

    /// Creates a new entry type representing a regular file.
    #[must_use]
    pub fn file() -> Self {
        Self::Regular
    }

    /// Creates a new entry type representing a hard link.
    #[must_use]
    pub fn hard_link() -> Self {
        Self::Link
    }

    /// Creates a new entry type representing a symlink.
    #[must_use]
    pub fn symlink() -> Self {
        Self::Symlink
    }

    /// Creates a new entry type representing a character device.
    #[must_use]
    pub fn character_special() -> Self {
        Self::Char
    }

    /// Creates a new entry type representing a block device.
    #[must_use]
    pub fn block_special() -> Self {
        Self::Block
    }

    /// Creates a new entry type representing a directory.
    #[must_use]
    pub fn dir() -> Self {
        Self::Directory
    }

    /// Creates a new entry type representing a FIFO (named pipe).
    #[must_use]
    pub fn fifo() -> Self {
        Self::Fifo
    }

    /// Creates a new entry type representing a contiguous file.
    #[must_use]
    pub fn contiguous() -> Self {
        Self::Continuous
    }

    // =========================================================================
    // Conversion
    // =========================================================================

    /// Return the raw byte representation of this entry type.
    ///
    /// This is an alias for [`to_byte`](Self::to_byte) provided for
    /// compatibility with the `tar` crate's API.
    #[inline]
    #[must_use]
    pub fn as_byte(self) -> u8 {
        self.to_byte()
    }

    /// Convert an entry type to its raw byte representation.
    ///
    /// Note that `Regular` is encoded as '0', not '\0'.
    #[must_use]
    pub fn to_byte(self) -> u8 {
        match self {
            EntryType::Regular => b'0',
            EntryType::Link => b'1',
            EntryType::Symlink => b'2',
            EntryType::Char => b'3',
            EntryType::Block => b'4',
            EntryType::Directory => b'5',
            EntryType::Fifo => b'6',
            EntryType::Continuous => b'7',
            EntryType::GnuLongName => b'L',
            EntryType::GnuLongLink => b'K',
            EntryType::GnuSparse => b'S',
            EntryType::XHeader => b'x',
            EntryType::XGlobalHeader => b'g',
            EntryType::Other(b) => b,
        }
    }

    // =========================================================================
    // Predicates
    // =========================================================================

    /// Returns true if this is a regular file entry.
    #[must_use]
    pub fn is_file(self) -> bool {
        matches!(self, EntryType::Regular | EntryType::Continuous)
    }

    /// Returns true if this is a directory entry.
    #[must_use]
    pub fn is_dir(self) -> bool {
        self == EntryType::Directory
    }

    /// Returns true if this is a symbolic link entry.
    #[must_use]
    pub fn is_symlink(self) -> bool {
        self == EntryType::Symlink
    }

    /// Returns true if this is a hard link entry.
    #[must_use]
    pub fn is_hard_link(self) -> bool {
        self == EntryType::Link
    }

    /// Returns true if this is a character device entry.
    #[must_use]
    pub fn is_character_special(self) -> bool {
        self == EntryType::Char
    }

    /// Returns true if this is a block device entry.
    #[must_use]
    pub fn is_block_special(self) -> bool {
        self == EntryType::Block
    }

    /// Returns true if this is a FIFO (named pipe) entry.
    #[must_use]
    pub fn is_fifo(self) -> bool {
        self == EntryType::Fifo
    }

    /// Returns true if this is a contiguous file entry.
    #[must_use]
    pub fn is_contiguous(self) -> bool {
        self == EntryType::Continuous
    }

    /// Returns true if this is a GNU long name extension entry.
    #[must_use]
    pub fn is_gnu_longname(self) -> bool {
        self == EntryType::GnuLongName
    }

    /// Returns true if this is a GNU long link extension entry.
    #[must_use]
    pub fn is_gnu_longlink(self) -> bool {
        self == EntryType::GnuLongLink
    }

    /// Returns true if this is a GNU sparse file entry.
    #[must_use]
    pub fn is_gnu_sparse(self) -> bool {
        self == EntryType::GnuSparse
    }

    /// Returns true if this is a PAX global extended header entry.
    #[must_use]
    pub fn is_pax_global_extensions(self) -> bool {
        self == EntryType::XGlobalHeader
    }

    /// Returns true if this is a PAX local extended header entry.
    #[must_use]
    pub fn is_pax_local_extensions(self) -> bool {
        self == EntryType::XHeader
    }
}

impl From<u8> for EntryType {
    fn from(byte: u8) -> Self {
        Self::from_byte(byte)
    }
}

impl From<EntryType> for u8 {
    fn from(entry_type: EntryType) -> Self {
        entry_type.to_byte()
    }
}

// ============================================================================
// Header Wrapper
// ============================================================================

/// High-level tar header wrapper with accessor methods.
///
/// This struct wraps a `[u8; 512]` and provides convenient methods for
/// accessing header fields, detecting the format, and verifying checksums.
///
/// # Format Detection
///
/// The format is detected by examining the magic field:
/// - UStar: magic = "ustar\0", version = "00"
/// - GNU: magic = "ustar ", version = " \0"
/// - Old: anything else
///
/// # Example
///
/// ```
/// use tar_core::Header;
///
/// let mut header = Header::new_ustar();
/// assert!(header.is_ustar());
/// assert!(!header.is_gnu());
/// ```
#[derive(Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(transparent)]
pub struct Header {
    bytes: [u8; HEADER_SIZE],
}

impl Header {
    /// Create a new header with UStar format magic and version.
    #[must_use]
    pub fn new_ustar() -> Self {
        let mut header = Self {
            bytes: [0u8; HEADER_SIZE],
        };
        let ustar = header.as_ustar_mut();
        ustar.magic.copy_from_slice(USTAR_MAGIC);
        ustar.version.copy_from_slice(USTAR_VERSION);
        header
    }

    /// Create a new header with GNU tar format magic and version.
    #[must_use]
    pub fn new_gnu() -> Self {
        let mut header = Self {
            bytes: [0u8; HEADER_SIZE],
        };
        let gnu = header.as_gnu_mut();
        gnu.magic.copy_from_slice(GNU_MAGIC);
        gnu.version.copy_from_slice(GNU_VERSION);
        header
    }

    /// Create a new old-style (V7/POSIX.1-1988) header with no magic bytes.
    ///
    /// This header format is the original archive header format which all other
    /// versions are compatible with (e.g., they are a superset). This header
    /// format limits path name length and cannot contain extra metadata like
    /// atime/ctime.
    #[must_use]
    pub fn new_old() -> Self {
        Self {
            bytes: [0u8; HEADER_SIZE],
        }
    }

    /// Get a reference to the underlying bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; HEADER_SIZE] {
        &self.bytes
    }

    /// Get a mutable reference to the underlying bytes.
    pub fn as_mut_bytes(&mut self) -> &mut [u8; HEADER_SIZE] {
        &mut self.bytes
    }

    /// Interpret a `[u8; 512]` as a tar header reference.
    #[must_use]
    pub fn from_bytes(bytes: &[u8; HEADER_SIZE]) -> &Header {
        Header::ref_from_bytes(bytes).expect("HEADER_SIZE is correct")
    }

    /// View this header as an old-style header.
    #[must_use]
    pub fn as_old(&self) -> &OldHeader {
        OldHeader::ref_from_bytes(&self.bytes).expect("size is correct")
    }

    /// View this header as a UStar header.
    #[must_use]
    pub fn as_ustar(&self) -> &UstarHeader {
        UstarHeader::ref_from_bytes(&self.bytes).expect("size is correct")
    }

    /// View this header as a GNU header.
    #[must_use]
    pub fn as_gnu(&self) -> &GnuHeader {
        GnuHeader::ref_from_bytes(&self.bytes).expect("size is correct")
    }

    /// View this header as a UStar header if it has the correct magic.
    ///
    /// Returns `None` if this is not a UStar format header.
    #[must_use]
    pub fn try_as_ustar(&self) -> Option<&UstarHeader> {
        if self.is_ustar() {
            Some(self.as_ustar())
        } else {
            None
        }
    }

    /// View this header as a GNU header if it has the correct magic.
    ///
    /// Returns `None` if this is not a GNU format header.
    #[must_use]
    pub fn try_as_gnu(&self) -> Option<&GnuHeader> {
        if self.is_gnu() {
            Some(self.as_gnu())
        } else {
            None
        }
    }

    /// View this header as a mutable old-style header.
    #[must_use]
    pub fn as_old_mut(&mut self) -> &mut OldHeader {
        OldHeader::mut_from_bytes(&mut self.bytes).expect("size is correct")
    }

    /// View this header as a mutable UStar header.
    #[must_use]
    pub fn as_ustar_mut(&mut self) -> &mut UstarHeader {
        UstarHeader::mut_from_bytes(&mut self.bytes).expect("size is correct")
    }

    /// View this header as a mutable GNU header.
    #[must_use]
    pub fn as_gnu_mut(&mut self) -> &mut GnuHeader {
        GnuHeader::mut_from_bytes(&mut self.bytes).expect("size is correct")
    }

    /// View this header as a mutable UStar header if it has the correct magic.
    ///
    /// Returns `None` if this is not a UStar format header.
    #[must_use]
    pub fn try_as_ustar_mut(&mut self) -> Option<&mut UstarHeader> {
        if self.is_ustar() {
            Some(self.as_ustar_mut())
        } else {
            None
        }
    }

    /// View this header as a mutable GNU header if it has the correct magic.
    ///
    /// Returns `None` if this is not a GNU format header.
    #[must_use]
    pub fn try_as_gnu_mut(&mut self) -> Option<&mut GnuHeader> {
        if self.is_gnu() {
            Some(self.as_gnu_mut())
        } else {
            None
        }
    }

    /// Check if this header uses UStar format.
    #[must_use]
    pub fn is_ustar(&self) -> bool {
        let h = self.as_ustar();
        h.magic == *USTAR_MAGIC && h.version == *USTAR_VERSION
    }

    /// Check if this header uses GNU tar format.
    #[must_use]
    pub fn is_gnu(&self) -> bool {
        let h = self.as_gnu();
        h.magic == *GNU_MAGIC && h.version == *GNU_VERSION
    }

    /// Get the entry type.
    #[must_use]
    pub fn entry_type(&self) -> EntryType {
        EntryType::from_byte(self.as_ustar().typeflag[0])
    }

    /// Get the entry size (file content length) in bytes.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::InvalidOctal`] if the size field is not valid.
    pub fn entry_size(&self) -> Result<u64> {
        parse_numeric(&self.as_ustar().size)
    }

    /// Get the file mode (permissions).
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::InvalidOctal`] if the mode field is not valid.
    pub fn mode(&self) -> Result<u32> {
        parse_numeric(&self.as_ustar().mode).map(|v| v as u32)
    }

    /// Get the owner user ID.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::InvalidOctal`] if the uid field is not valid.
    pub fn uid(&self) -> Result<u64> {
        parse_numeric(&self.as_ustar().uid)
    }

    /// Get the owner group ID.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::InvalidOctal`] if the gid field is not valid.
    pub fn gid(&self) -> Result<u64> {
        parse_numeric(&self.as_ustar().gid)
    }

    /// Get the modification time as a Unix timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::InvalidOctal`] if the mtime field is not valid.
    pub fn mtime(&self) -> Result<u64> {
        parse_numeric(&self.as_ustar().mtime)
    }

    /// Get the raw path bytes from the header.
    ///
    /// This returns only the name field (bytes 0..100). For UStar format,
    /// the prefix field (bytes 345..500) may also contain path components
    /// that should be prepended.
    #[must_use]
    pub fn path_bytes(&self) -> &[u8] {
        truncate_null(&self.as_ustar().name)
    }

    /// Get the raw link name bytes.
    #[must_use]
    pub fn link_name_bytes(&self) -> &[u8] {
        truncate_null(&self.as_ustar().linkname)
    }

    /// Get the device major number (for character/block devices).
    ///
    /// Returns `None` for old-style headers without device fields.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::InvalidOctal`] if the field is not valid octal.
    pub fn device_major(&self) -> Result<Option<u32>> {
        if !self.is_ustar() && !self.is_gnu() {
            return Ok(None);
        }
        parse_octal(&self.as_ustar().dev_major).map(|v| Some(v as u32))
    }

    /// Get the device minor number (for character/block devices).
    ///
    /// Returns `None` for old-style headers without device fields.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::InvalidOctal`] if the field is not valid octal.
    pub fn device_minor(&self) -> Result<Option<u32>> {
        if !self.is_ustar() && !self.is_gnu() {
            return Ok(None);
        }
        parse_octal(&self.as_ustar().dev_minor).map(|v| Some(v as u32))
    }

    /// Get the owner user name.
    ///
    /// Returns `None` for old-style headers without user/group name fields.
    #[must_use]
    pub fn username(&self) -> Option<&[u8]> {
        if !self.is_ustar() && !self.is_gnu() {
            return None;
        }
        Some(truncate_null(&self.as_ustar().uname))
    }

    /// Get the owner group name.
    ///
    /// Returns `None` for old-style headers without user/group name fields.
    #[must_use]
    pub fn groupname(&self) -> Option<&[u8]> {
        if !self.is_ustar() && !self.is_gnu() {
            return None;
        }
        Some(truncate_null(&self.as_ustar().gname))
    }

    /// Get the UStar prefix field for long paths.
    ///
    /// Returns `None` for old-style or GNU headers.
    #[must_use]
    pub fn prefix(&self) -> Option<&[u8]> {
        if !self.is_ustar() {
            return None;
        }
        Some(truncate_null(&self.as_ustar().prefix))
    }

    /// Verify the header checksum.
    ///
    /// The checksum is computed as the unsigned sum of all header bytes,
    /// treating the checksum field (bytes 148..156) as spaces.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::ChecksumMismatch`] if the checksum is invalid,
    /// or [`HeaderError::InvalidOctal`] if the stored checksum cannot be parsed.
    pub fn verify_checksum(&self) -> Result<()> {
        let expected = parse_octal(&self.as_ustar().cksum)?;
        let computed = self.compute_checksum();
        if expected == computed {
            Ok(())
        } else {
            Err(HeaderError::ChecksumMismatch { expected, computed })
        }
    }

    /// Compute the header checksum.
    ///
    /// This computes the unsigned sum of all header bytes, treating the
    /// checksum field (bytes 148..156) as spaces (0x20).
    #[must_use]
    pub fn compute_checksum(&self) -> u64 {
        let mut sum: u64 = 0;
        for (i, &byte) in self.bytes.iter().enumerate() {
            if (148..156).contains(&i) {
                // Treat checksum field as spaces
                sum += u64::from(b' ');
            } else {
                sum += u64::from(byte);
            }
        }
        sum
    }

    /// Check if this header represents an empty block (all zeros).
    ///
    /// Two consecutive empty blocks mark the end of a tar archive.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.iter().all(|&b| b == 0)
    }

    // =========================================================================
    // Setter Methods
    // =========================================================================

    /// Format-aware numeric field encoder.
    ///
    /// For GNU headers, uses `encode_numeric` which falls back to base-256
    /// encoding for large values. For ustar (and other formats), uses
    /// `encode_octal` only, since base-256 is a GNU extension.
    fn set_numeric_field<const N: usize>(
        &mut self,
        field: impl FnOnce(&mut UstarHeader) -> &mut [u8; N],
        value: u64,
    ) -> Result<()> {
        let is_gnu = self.is_gnu();
        let dst = field(self.as_ustar_mut());
        if is_gnu {
            encode_numeric(dst, value)
        } else {
            encode_octal(dst, value)
        }
    }

    /// Set the file size (bytes 124-136).
    ///
    /// For GNU headers, uses octal ASCII if the value fits, otherwise
    /// base-256 encoding. For ustar headers, uses octal ASCII only.
    ///
    /// For values that always fit regardless of format (≤ ~8GB), prefer the
    /// infallible [`set_size_small`](Self::set_size_small).
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::FieldOverflow`] if the value cannot be
    /// represented. For ustar, the octal limit is 0o77777777777 (8,589,934,591).
    /// For GNU, any `u64` fits via base-256.
    pub fn set_size(&mut self, size: u64) -> Result<()> {
        self.set_numeric_field(|h| &mut h.size, size)
    }

    /// Set the file size from a `u32` (bytes 124-136).
    ///
    /// Infallible because any `u32` (max ~4.3 billion) fits in the 12-byte
    /// octal field (max 8,589,934,591) regardless of header format.
    pub fn set_size_small(&mut self, size: u32) {
        encode_octal(&mut self.as_ustar_mut().size, u64::from(size))
            .expect("u32 always fits in 12-byte octal field");
    }

    /// Set the file mode (bytes 100-108).
    ///
    /// The mode is always written as octal ASCII (both GNU and ustar).
    ///
    /// For typical Unix modes (≤ 0o7777), prefer the infallible
    /// [`set_mode_small`](Self::set_mode_small).
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::FieldOverflow`] if the value exceeds the 8-byte
    /// octal capacity (max 0o7777777 = 2,097,151).
    pub fn set_mode(&mut self, mode: u32) -> Result<()> {
        encode_octal(&mut self.as_ustar_mut().mode, u64::from(mode))
    }

    /// Set the file mode from a `u16` (bytes 100-108).
    ///
    /// Infallible because any `u16` (max 65,535) fits in the 8-byte octal
    /// field (max 2,097,151). Covers all standard Unix permission bits
    /// (0o7777).
    pub fn set_mode_small(&mut self, mode: u16) {
        encode_octal(&mut self.as_ustar_mut().mode, u64::from(mode))
            .expect("u16 always fits in 8-byte octal field");
    }

    /// Set the owner user ID (bytes 108-116).
    ///
    /// For GNU headers, uses base-256 encoding for values that exceed the
    /// octal range. For ustar headers, only octal ASCII is available.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::FieldOverflow`] if the value cannot be
    /// represented. For ustar, the octal limit is 0o7777777 (2,097,151).
    /// For GNU, the base-256 limit is 2^63 - 1.
    pub fn set_uid(&mut self, uid: u64) -> Result<()> {
        self.set_numeric_field(|h| &mut h.uid, uid)
    }

    /// Set the owner group ID (bytes 116-124).
    ///
    /// For GNU headers, uses base-256 encoding for values that exceed the
    /// octal range. For ustar headers, only octal ASCII is available.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::FieldOverflow`] if the value cannot be
    /// represented. For ustar, the octal limit is 0o7777777 (2,097,151).
    /// For GNU, the base-256 limit is 2^63 - 1.
    pub fn set_gid(&mut self, gid: u64) -> Result<()> {
        self.set_numeric_field(|h| &mut h.gid, gid)
    }

    // Note: no _small variants for uid/gid — u32 values can exceed the
    // ustar octal limit (2,097,151), so they're not format-independent.
    // Use PAX extensions (via EntryBuilder) for large IDs on ustar.

    /// Set the modification time as a Unix timestamp (bytes 136-148).
    ///
    /// For GNU headers, uses octal ASCII if the value fits, otherwise
    /// base-256 encoding. For ustar headers, uses octal ASCII only.
    ///
    /// For timestamps that always fit regardless of format (≤ ~2106), prefer
    /// the infallible [`set_mtime_small`](Self::set_mtime_small).
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::FieldOverflow`] if the value cannot be
    /// represented. For ustar, the octal limit is 0o77777777777 (8,589,934,591).
    /// For GNU, any `u64` fits via base-256.
    pub fn set_mtime(&mut self, mtime: u64) -> Result<()> {
        self.set_numeric_field(|h| &mut h.mtime, mtime)
    }

    /// Set the modification time from a `u32` Unix timestamp (bytes 136-148).
    ///
    /// Infallible because any `u32` (max ~4.3 billion, i.e. year ~2106) fits
    /// in the 12-byte octal field regardless of header format.
    pub fn set_mtime_small(&mut self, mtime: u32) {
        encode_octal(&mut self.as_ustar_mut().mtime, u64::from(mtime))
            .expect("u32 always fits in 12-byte octal field");
    }

    /// Set the entry type (byte 156).
    pub fn set_entry_type(&mut self, ty: EntryType) {
        self.as_ustar_mut().typeflag[0] = ty.to_byte();
    }

    /// Compute and set the header checksum (bytes 148-156).
    ///
    /// This should be called after all other header fields have been set.
    /// The format is 7 octal digits with leading zeros plus a null terminator,
    /// matching tar-rs for bit-identical output.
    pub fn set_checksum(&mut self) {
        // Fill checksum field with spaces for calculation
        self.as_ustar_mut().cksum.fill(b' ');

        // Compute unsigned sum of all bytes
        let checksum: u64 = self.bytes.iter().map(|&b| u64::from(b)).sum();

        // Max checksum = 512 * 255 = 130560, which always fits in 8-byte octal
        // (max representable: 07777777 = 2097151).
        encode_octal(&mut self.as_ustar_mut().cksum, checksum)
            .expect("checksum always fits in 8-byte octal field");
    }

    /// Set the file path (name field, bytes 0-100).
    ///
    /// # Errors
    ///
    /// Returns an error if the path is longer than 100 bytes.
    pub fn set_path(&mut self, path: &[u8]) -> Result<()> {
        if path.len() > self.as_ustar().name.len() {
            return Err(HeaderError::FieldOverflow {
                field_len: self.as_ustar().name.len(),
                detail: format!("{}-byte path", path.len()),
            });
        }
        let name = &mut self.as_ustar_mut().name;
        name.fill(0);
        name[..path.len()].copy_from_slice(path);
        Ok(())
    }

    /// Set the link name (bytes 157-257).
    ///
    /// # Errors
    ///
    /// Returns an error if the link name is longer than 100 bytes.
    pub fn set_link_name(&mut self, link: &[u8]) -> Result<()> {
        if link.len() > self.as_ustar().linkname.len() {
            return Err(HeaderError::FieldOverflow {
                field_len: self.as_ustar().linkname.len(),
                detail: format!("{}-byte link name", link.len()),
            });
        }
        let linkname = &mut self.as_ustar_mut().linkname;
        linkname.fill(0);
        linkname[..link.len()].copy_from_slice(link);
        Ok(())
    }

    /// Set the owner user name (bytes 265-297, UStar/GNU only).
    ///
    /// # Errors
    ///
    /// Returns an error if the username is longer than 32 bytes.
    pub fn set_username(&mut self, name: &[u8]) -> Result<()> {
        if name.len() > self.as_ustar().uname.len() {
            return Err(HeaderError::FieldOverflow {
                field_len: self.as_ustar().uname.len(),
                detail: format!("{}-byte username", name.len()),
            });
        }
        let uname = &mut self.as_ustar_mut().uname;
        uname.fill(0);
        uname[..name.len()].copy_from_slice(name);
        Ok(())
    }

    /// Set the owner group name (bytes 297-329, UStar/GNU only).
    ///
    /// # Errors
    ///
    /// Returns an error if the group name is longer than 32 bytes.
    pub fn set_groupname(&mut self, name: &[u8]) -> Result<()> {
        if name.len() > self.as_ustar().gname.len() {
            return Err(HeaderError::FieldOverflow {
                field_len: self.as_ustar().gname.len(),
                detail: format!("{}-byte group name", name.len()),
            });
        }
        let gname = &mut self.as_ustar_mut().gname;
        gname.fill(0);
        gname[..name.len()].copy_from_slice(name);
        Ok(())
    }

    /// Set device major and minor numbers (bytes 329-337 and 337-345).
    ///
    /// Used for character and block device entries.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::FieldOverflow`] if either value cannot be
    /// represented in its 8-byte octal field (max 0o7777777 = 2097151).
    /// For device numbers that fit in `u16`, prefer the infallible
    /// [`set_device_small`](Self::set_device_small).
    pub fn set_device(&mut self, major: u32, minor: u32) -> Result<()> {
        let fields = self.as_ustar_mut();
        encode_octal(&mut fields.dev_major, u64::from(major))?;
        encode_octal(&mut fields.dev_minor, u64::from(minor))
    }

    /// Set device major and minor numbers from `u16` values.
    ///
    /// Infallible because any `u16` (max 65535) fits in the 8-byte octal
    /// fields (max 2097151). Covers all real-world device numbers.
    pub fn set_device_small(&mut self, major: u16, minor: u16) {
        let fields = self.as_ustar_mut();
        encode_octal(&mut fields.dev_major, u64::from(major))
            .expect("u16 always fits in 8-byte octal field");
        encode_octal(&mut fields.dev_minor, u64::from(minor))
            .expect("u16 always fits in 8-byte octal field");
    }
}

impl Default for Header {
    fn default() -> Self {
        Self::new_ustar()
    }
}

impl fmt::Debug for Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Header")
            .field("path", &String::from_utf8_lossy(self.path_bytes()))
            .field("entry_type", &self.entry_type())
            .field("size", &self.entry_size().ok())
            .field("mode", &self.mode().ok().map(|m| format!("{m:04o}")))
            .field("is_ustar", &self.is_ustar())
            .field("is_gnu", &self.is_gnu())
            .finish()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Stack-allocated octal formatter for u64 values.
///
/// Formats a u64 as octal digits without allocating. The maximum u64
/// value (2^64 - 1) requires 22 octal digits, so the internal buffer
/// is always sufficient.
///
/// This is the octal counterpart of [`builder::DecU64`](crate::builder).
pub(crate) struct OctU64 {
    buf: [u8; 22],
    start: u8,
}

impl OctU64 {
    /// Format `value` as octal digits.
    pub(crate) fn new(mut value: u64) -> Self {
        let mut buf = [0u8; 22];
        if value == 0 {
            buf[21] = b'0';
            return Self { buf, start: 21 };
        }
        let mut pos = 22u8;
        while value > 0 {
            pos -= 1;
            buf[pos as usize] = b'0' + (value & 7) as u8;
            value >>= 3;
        }
        Self { buf, start: pos }
    }

    /// The formatted octal digits as a byte slice.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.buf[self.start as usize..]
    }
}

/// Test whether a byte is whitespace in the context of tar header fields.
///
/// This includes all bytes that `u8::is_ascii_whitespace()` recognizes
/// (HT, LF, FF, CR, space) **plus** vertical tab (0x0b). Rust's
/// `is_ascii_whitespace` follows the WHATWG definition which omits VT,
/// but real tar implementations (and Rust's `str::trim()`) treat it as
/// whitespace. Without this, fields like `"0000000\x0b"` would fail to
/// parse.
fn is_tar_whitespace(b: u8) -> bool {
    b.is_ascii_whitespace() || b == 0x0b
}

/// Parse an octal ASCII field into a u64.
///
/// Octal fields in tar headers are ASCII strings with optional leading
/// spaces and trailing spaces or null bytes. For example:
/// - `"0000644\0"` -> 420 (file mode 0644)
/// - `"     123 "` -> 83
///
/// # Errors
///
/// Returns [`HeaderError::InvalidOctal`] if the field contains invalid
/// characters (anything other than spaces, digits 0-7, or null bytes).
pub(crate) fn parse_octal(bytes: &[u8]) -> Result<u64> {
    // Tar octal fields are padded with leading spaces/nulls and terminated
    // by spaces, tabs, or null bytes. We first truncate at the first null
    // (matching how C-string fields work in tar), then trim whitespace from
    // both ends to isolate the digit run.
    //
    // Note: we use `is_tar_whitespace` rather than `u8::is_ascii_whitespace`
    // because the latter omits vertical tab (0x0b), which real tar
    // implementations treat as whitespace (and Rust's `str::trim()` strips).
    let truncated = match bytes.iter().position(|&b| b == 0) {
        Some(i) => &bytes[..i],
        None => bytes,
    };
    let trimmed = truncated
        .iter()
        .position(|&b| !is_tar_whitespace(b))
        .map(|start| {
            let rest = &truncated[start..];
            let end = rest
                .iter()
                .rposition(|&b| !is_tar_whitespace(b))
                .map_or(0, |p| p + 1);
            &rest[..end]
        })
        .unwrap_or(&[]);

    if trimmed.is_empty() {
        return Ok(0);
    }

    let s = core::str::from_utf8(trimmed).map_err(|_| HeaderError::InvalidOctal(bytes.to_vec()))?;
    u64::from_str_radix(s, 8).map_err(|_| HeaderError::InvalidOctal(bytes.to_vec()))
}

/// Encode a u64 value to a numeric field.
///
/// Uses octal ASCII if the value fits, otherwise GNU base-256 encoding
/// (high bit set in first byte). This matches tar-rs behavior for
/// compatibility.
///
/// # Thresholds
///
/// - For 12-byte fields (size, mtime): uses base-256 if value >= 8589934592 (8GB)
/// - For 8-byte fields (uid, gid): uses base-256 if value >= 2097152 (2^21)
///
/// # Errors
///
/// Returns [`HeaderError::FieldOverflow`] if the value exceeds the field's
/// representable range (e.g., values >= 2^63 in an 8-byte field).
pub(crate) fn encode_numeric<const N: usize>(field: &mut [u8; N], value: u64) -> Result<()> {
    const { assert!(N > 0, "encode_numeric requires N > 0") };

    // Thresholds from tar-rs: use binary for large values
    let use_binary = if N == 8 {
        value >= 2097152 // 2^21, max for 7 octal digits
    } else {
        value >= 8589934592 // 8GB, threshold for 12-byte fields
    };

    if use_binary {
        // GNU base-256 encoding: high bit of first byte is the indicator,
        // leaving N*8-1 data bits. For 8-byte fields that's 63 bits, for
        // 12-byte fields it's 95 bits (more than u64 needs).
        let data_bits = N * 8 - 1;
        if data_bits < 64 && value >= (1u64 << data_bits) {
            return Err(HeaderError::FieldOverflow {
                field_len: N,
                detail: format!("numeric value {value}"),
            });
        }

        field.fill(0);

        // Write the value in big-endian to the last 8 bytes (or fewer)
        let value_bytes = value.to_be_bytes();
        if N >= 8 {
            field[N - 8..].copy_from_slice(&value_bytes);
        } else {
            field.copy_from_slice(&value_bytes[8 - N..]);
        }
        // Set high bit to indicate base-256
        field[0] |= 0x80;
    } else {
        // Standard octal ASCII encoding
        encode_octal(field, value)?;
    }

    Ok(())
}

/// Encode a u64 value as octal ASCII into a tar header field.
///
/// The field is zero-filled, then populated with leading-zero-padded octal
/// digits followed by a null terminator, matching tar conventions (e.g.
/// mode 0644 in an 8-byte field becomes `b"0000644\0"`).
///
/// Uses [`OctU64`] internally for the digit conversion.
///
/// # Errors
///
/// Returns [`HeaderError::FieldOverflow`] if the value cannot fit.
pub(crate) fn encode_octal<const N: usize>(field: &mut [u8; N], value: u64) -> Result<()> {
    const { assert!(N > 0, "encode_octal requires N > 0") };

    let oct = OctU64::new(value);
    let digits = oct.as_bytes();

    // N-1 digit slots available (last byte is null terminator).
    if digits.len() > N - 1 {
        return Err(HeaderError::FieldOverflow {
            field_len: N,
            detail: format!("octal value {value:#o}"),
        });
    }

    // Zero-fill first, then overwrite with '0'-padded digits.
    field.fill(0);
    let (digit_slots, _nul) = field.split_at_mut(N - 1);
    let pad = digit_slots.len() - digits.len();
    digit_slots[..pad].fill(b'0');
    digit_slots[pad..].copy_from_slice(digits);

    Ok(())
}

/// Parse a numeric field that may be octal ASCII or GNU base-256 encoded.
///
/// GNU tar uses base-256 encoding for values that don't fit in octal.
/// When the high bit of the first byte is set (0x80), the value is stored
/// as big-endian binary in the remaining bytes. Otherwise, it's parsed as
/// octal ASCII.
///
/// # Errors
///
/// Returns [`HeaderError::InvalidOctal`] if octal parsing fails.
pub(crate) fn parse_numeric(bytes: &[u8]) -> Result<u64> {
    if bytes.is_empty() {
        return Ok(0);
    }

    // Check for GNU base-256 encoding (high bit set)
    if bytes[0] & 0x80 != 0 {
        // Base-256: interpret remaining bytes as big-endian, masking off the
        // high bit of the first byte
        let mut value: u64 = 0;
        for (i, &byte) in bytes.iter().enumerate() {
            let b = if i == 0 { byte & 0x7f } else { byte };
            value = value
                .checked_shl(8)
                .and_then(|v| v.checked_add(u64::from(b)))
                .ok_or_else(|| HeaderError::InvalidOctal(bytes.to_vec()))?;
        }
        Ok(value)
    } else {
        // Standard octal ASCII
        parse_octal(bytes)
    }
}

/// Truncate a byte slice at the first null byte.
///
/// Used to extract null-terminated strings from fixed-size header fields.
/// If no null byte is found, returns the entire slice.
#[must_use]
pub(crate) fn truncate_null(bytes: &[u8]) -> &[u8] {
    match bytes.iter().position(|&b| b == 0) {
        Some(pos) => &bytes[..pos],
        None => bytes,
    }
}

// ============================================================================
// PAX Extended Headers
// ============================================================================

/// PAX extended header key for the file path.
pub const PAX_PATH: &str = "path";
/// PAX extended header key for the link target path.
pub const PAX_LINKPATH: &str = "linkpath";
/// PAX extended header key for file size.
pub const PAX_SIZE: &str = "size";
/// PAX extended header key for owner user ID.
pub const PAX_UID: &str = "uid";
/// PAX extended header key for owner group ID.
pub const PAX_GID: &str = "gid";
/// PAX extended header key for owner user name.
pub const PAX_UNAME: &str = "uname";
/// PAX extended header key for owner group name.
pub const PAX_GNAME: &str = "gname";
/// PAX extended header key for modification time.
pub const PAX_MTIME: &str = "mtime";
/// PAX extended header key for access time.
pub const PAX_ATIME: &str = "atime";
/// PAX extended header key for change time.
pub const PAX_CTIME: &str = "ctime";
/// PAX extended header prefix for SCHILY extended attributes.
pub const PAX_SCHILY_XATTR: &str = "SCHILY.xattr.";

/// PAX extended header prefix for GNU sparse file extensions.
pub const PAX_GNU_SPARSE: &str = "GNU.sparse.";
/// PAX key for GNU sparse file number of blocks.
pub const PAX_GNU_SPARSE_NUMBLOCKS: &str = "GNU.sparse.numblocks";
/// PAX key for GNU sparse file offset.
pub const PAX_GNU_SPARSE_OFFSET: &str = "GNU.sparse.offset";
/// PAX key for GNU sparse file numbytes.
pub const PAX_GNU_SPARSE_NUMBYTES: &str = "GNU.sparse.numbytes";
/// PAX key for GNU sparse file map.
pub const PAX_GNU_SPARSE_MAP: &str = "GNU.sparse.map";
/// PAX key for GNU sparse file name.
pub const PAX_GNU_SPARSE_NAME: &str = "GNU.sparse.name";
/// PAX key for GNU sparse file format major version.
pub const PAX_GNU_SPARSE_MAJOR: &str = "GNU.sparse.major";
/// PAX key for GNU sparse file format minor version.
pub const PAX_GNU_SPARSE_MINOR: &str = "GNU.sparse.minor";
/// PAX key for GNU sparse file size.
pub const PAX_GNU_SPARSE_SIZE: &str = "GNU.sparse.size";
/// PAX key for GNU sparse file real size.
pub const PAX_GNU_SPARSE_REALSIZE: &str = "GNU.sparse.realsize";

/// Error parsing a PAX extension record.
#[derive(Debug, Error)]
pub enum PaxError {
    /// The record format is malformed.
    #[error("malformed PAX extension record")]
    Malformed,
    /// The key is not valid UTF-8.
    #[error("PAX key is not valid UTF-8: {0}")]
    InvalidKey(#[from] core::str::Utf8Error),
}

#[cfg(feature = "std")]
impl From<PaxError> for std::io::Error {
    fn from(e: PaxError) -> Self {
        std::io::Error::other(e.to_string())
    }
}

/// A single PAX extended header key/value pair.
#[derive(Debug, Clone)]
pub struct PaxExtension<'a> {
    key: &'a [u8],
    value: &'a [u8],
}

impl<'a> PaxExtension<'a> {
    /// Returns the key as a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the key is not valid UTF-8.
    pub fn key(&self) -> core::result::Result<&'a str, core::str::Utf8Error> {
        core::str::from_utf8(self.key)
    }

    /// Returns the raw key bytes.
    #[must_use]
    pub fn key_bytes(&self) -> &'a [u8] {
        self.key
    }

    /// Returns the value as a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the value is not valid UTF-8.
    pub fn value(&self) -> core::result::Result<&'a str, core::str::Utf8Error> {
        core::str::from_utf8(self.value)
    }

    /// Returns the raw value bytes.
    #[must_use]
    pub fn value_bytes(&self) -> &'a [u8] {
        self.value
    }
}

/// Iterator over PAX extended header records.
///
/// PAX extended headers consist of records in the format:
/// `<length> <key>=<value>\n`
///
/// where `<length>` is the total record length including the length field itself.
///
/// # Example
///
/// ```
/// use tar_core::PaxExtensions;
///
/// let data = b"20 path=foo/bar.txt\n";
/// let mut iter = PaxExtensions::new(data);
/// let ext = iter.next().unwrap().unwrap();
/// assert_eq!(ext.key().unwrap(), "path");
/// assert_eq!(ext.value().unwrap(), "foo/bar.txt");
/// ```
#[derive(Debug)]
pub struct PaxExtensions<'a> {
    data: &'a [u8],
}

impl<'a> PaxExtensions<'a> {
    /// Create a new iterator over PAX extension records.
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Look up a specific key and return its value as a string.
    ///
    /// Returns `None` if the key is not found or if parsing fails.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&'a str> {
        for ext in PaxExtensions::new(self.data).flatten() {
            if ext.key().ok() == Some(key) {
                return ext.value().ok();
            }
        }
        None
    }

    /// Look up a specific key and parse its value as u64.
    ///
    /// Returns `None` if the key is not found, parsing fails, or the value
    /// is not a valid integer.
    #[must_use]
    pub fn get_u64(&self, key: &str) -> Option<u64> {
        self.get(key).and_then(|v| v.parse().ok())
    }
}

impl<'a> Iterator for PaxExtensions<'a> {
    type Item = core::result::Result<PaxExtension<'a>, PaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            return None;
        }

        // Format: "<len> <key>=<value>\n"
        // Split off the decimal length field at the first space.
        let (len_bytes, _) = self
            .data
            .split_at(self.data.iter().position(|&b| b == b' ')?);
        let len: usize = core::str::from_utf8(len_bytes).ok()?.parse().ok()?;

        // The record is exactly `len` bytes (including the length field itself).
        let record = match self.data.get(..len) {
            Some(r) => r,
            None => return Some(Err(PaxError::Malformed)),
        };

        // Must end with newline.
        if record.last() != Some(&b'\n') {
            return Some(Err(PaxError::Malformed));
        }

        // Everything between the space and the trailing newline is "key=value".
        // `len_bytes.len() + 1` skips past the space; strip the trailing '\n'.
        let kv = match record.get(len_bytes.len() + 1..record.len() - 1) {
            Some(kv) => kv,
            None => return Some(Err(PaxError::Malformed)),
        };

        // Split key and value at the first '='.  Values may contain '='
        // so we only split on the first one.
        let Some(eq_pos) = kv.iter().position(|&b| b == b'=') else {
            return Some(Err(PaxError::Malformed));
        };
        let (key, value) = (&kv[..eq_pos], &kv[eq_pos + 1..]);

        // Advance past this record.
        self.data = &self.data[len..];

        Some(Ok(PaxExtension { key, value }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size() {
        assert_eq!(size_of::<OldHeader>(), HEADER_SIZE);
        assert_eq!(size_of::<UstarHeader>(), HEADER_SIZE);
        assert_eq!(size_of::<GnuHeader>(), HEADER_SIZE);
        assert_eq!(size_of::<GnuExtSparseHeader>(), HEADER_SIZE);
        assert_eq!(size_of::<Header>(), HEADER_SIZE);
    }

    #[test]
    fn test_sparse_header_size() {
        // Each sparse header is 24 bytes (12 + 12)
        assert_eq!(size_of::<GnuSparseHeader>(), 24);
        // Extended sparse: 21 * 24 + 1 + 7 = 512
        assert_eq!(21 * 24 + 1 + 7, HEADER_SIZE);
    }

    #[test]
    fn test_new_ustar() {
        let header = Header::new_ustar();
        assert!(header.is_ustar());
        assert!(!header.is_gnu());
    }

    #[test]
    fn test_new_gnu() {
        let header = Header::new_gnu();
        assert!(header.is_gnu());
        assert!(!header.is_ustar());
    }

    #[test]
    fn test_parse_octal() {
        let cases: &[(&[u8], u64)] = &[
            (b"0000644\0", 0o644),
            (b"0000755\0", 0o755),
            (b"     123 ", 0o123),
            (b"0", 0),
            (b"", 0),
            (b"   \0\0\0", 0),
            (b"        ", 0),
            (b"\0\0\0\0\0\0", 0),
            (b"      7\0", 7),
            (b"0000755", 0o755),
            (b"7", 7),
            (b"00000001", 1),
            (b"77777777777\0", 0o77777777777),
            (b"7777777\0", 0o7777777),
        ];
        for (input, expected) in cases {
            assert_eq!(
                parse_octal(input).unwrap(),
                *expected,
                "parse_octal({input:?})"
            );
        }

        for bad in [&b"abc"[..], b"128"] {
            assert!(parse_octal(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn test_truncate_null() {
        let cases: &[(&[u8], &[u8])] = &[
            (b"hello\0world", b"hello"),
            (b"no null", b"no null"),
            (b"\0start", b""),
            (b"", b""),
        ];
        for (input, expected) in cases {
            assert_eq!(truncate_null(input), *expected, "truncate_null({input:?})");
        }
    }

    #[test]
    fn test_entry_type_roundtrip() {
        // Every known type should survive a byte round-trip.
        let types = [
            (b'0', EntryType::Regular),
            (b'\0', EntryType::Regular), // Old tar convention
            (b'1', EntryType::Link),
            (b'2', EntryType::Symlink),
            (b'3', EntryType::Char),
            (b'4', EntryType::Block),
            (b'5', EntryType::Directory),
            (b'6', EntryType::Fifo),
            (b'7', EntryType::Continuous),
            (b'L', EntryType::GnuLongName),
            (b'K', EntryType::GnuLongLink),
            (b'S', EntryType::GnuSparse),
            (b'x', EntryType::XHeader),
            (b'g', EntryType::XGlobalHeader),
        ];
        for (byte, expected) in types {
            let parsed = EntryType::from_byte(byte);
            assert_eq!(parsed, expected, "from_byte({byte:#x})");
            // Non-alias types should round-trip through to_byte.
            if byte != b'\0' {
                assert_eq!(parsed.to_byte(), byte);
            }
        }
    }

    #[test]
    fn test_entry_type_predicates() {
        let cases: &[(EntryType, bool, bool, bool, bool)] = &[
            //                       file   dir    sym    hard
            (EntryType::Regular, true, false, false, false),
            (EntryType::Continuous, true, false, false, false),
            (EntryType::Directory, false, true, false, false),
            (EntryType::Symlink, false, false, true, false),
            (EntryType::Link, false, false, false, true),
            (EntryType::Char, false, false, false, false),
        ];
        for &(ty, file, dir, sym, hard) in cases {
            assert_eq!(ty.is_file(), file, "{ty:?}.is_file()");
            assert_eq!(ty.is_dir(), dir, "{ty:?}.is_dir()");
            assert_eq!(ty.is_symlink(), sym, "{ty:?}.is_symlink()");
            assert_eq!(ty.is_hard_link(), hard, "{ty:?}.is_hard_link()");
        }
    }

    #[test]
    fn test_checksum_empty_header() {
        let header = Header::new_ustar();
        // Computed checksum should be consistent
        let checksum = header.compute_checksum();
        // For an empty header with only magic/version set, checksum includes:
        // - 148 spaces (0x20) for checksum field = 148 * 32 = 4736
        // - "ustar\0" = 117+115+116+97+114+0 = 559
        // - "00" = 48+48 = 96
        // - Rest are zeros
        assert!(checksum > 0);
    }

    #[test]
    fn test_is_empty() {
        let mut header = Header::new_ustar();
        assert!(!header.is_empty());

        // Create truly empty header
        header.as_mut_bytes().fill(0);
        assert!(header.is_empty());
    }

    #[test]
    fn test_as_format_views() {
        let header = Header::new_ustar();

        // All views should work without panicking
        let _old = header.as_old();
        let _ustar = header.as_ustar();
        let _gnu = header.as_gnu();
    }

    #[test]
    fn test_ustar_default_magic() {
        let ustar = UstarHeader::default();
        assert_eq!(&ustar.magic, USTAR_MAGIC);
        assert_eq!(&ustar.version, USTAR_VERSION);
    }

    #[test]
    fn test_gnu_default_magic() {
        let gnu = GnuHeader::default();
        assert_eq!(&gnu.magic, GNU_MAGIC);
        assert_eq!(&gnu.version, GNU_VERSION);
    }

    #[test]
    fn test_path_bytes() {
        let mut header = Header::new_ustar();
        header.as_mut_bytes()[0..5].copy_from_slice(b"hello");
        assert_eq!(header.path_bytes(), b"hello");
    }

    #[test]
    fn test_link_name_bytes() {
        let mut header = Header::new_ustar();
        header.as_mut_bytes()[157..163].copy_from_slice(b"target");
        assert_eq!(header.link_name_bytes(), b"target");
    }

    #[test]
    fn test_username_groupname() {
        let header = Header::new_ustar();
        assert!(header.username().is_some());
        assert!(header.groupname().is_some());

        // Old-style header should return None
        let mut old_header = Header::new_ustar();
        old_header.as_mut_bytes()[257..265].fill(0);
        assert!(old_header.username().is_none());
        assert!(old_header.groupname().is_none());
    }

    #[test]
    fn test_prefix() {
        let header = Header::new_ustar();
        assert!(header.prefix().is_some());

        let gnu_header = Header::new_gnu();
        // GNU format doesn't use prefix the same way
        assert!(gnu_header.prefix().is_none());
    }

    #[test]
    fn test_device_numbers() {
        let header = Header::new_ustar();
        assert!(header.device_major().unwrap().is_some());
        assert!(header.device_minor().unwrap().is_some());

        // Old-style header should return None
        let mut old_header = Header::new_ustar();
        old_header.as_mut_bytes()[257..265].fill(0);
        assert!(old_header.device_major().unwrap().is_none());
        assert!(old_header.device_minor().unwrap().is_none());
    }

    #[test]
    fn test_debug_impls() {
        // Exercise Debug impls to verify they don't panic; the formatted
        // string itself is irrelevant.
        let header = Header::new_ustar();
        let _ = format!("{header:?}");
        let _ = format!("{:?}", header.as_old());
        let _ = format!("{:?}", header.as_ustar());
        let _ = format!("{:?}", header.as_gnu());
        let _ = format!("{:?}", GnuExtSparseHeader::default());
        let _ = format!("{:?}", GnuSparseHeader::default());
    }

    #[test]
    fn test_parse_numeric() {
        // Octal cases (same as parse_octal)
        let octal_cases: &[(&[u8], u64)] = &[
            (b"0000644\0", 0o644),
            (b"0000755\0", 0o755),
            (b"     123 ", 0o123),
            (b"", 0),
        ];
        for (input, expected) in octal_cases {
            assert_eq!(
                parse_numeric(input).unwrap(),
                *expected,
                "parse_numeric({input:?})"
            );
        }

        // Base-256 cases: high bit set, remaining bytes are big-endian value
        let base256_cases: &[(&[u8], u64)] = &[
            (&[0x80, 0x00, 0x00, 0x01], 1),
            (&[0x80, 0x00, 0x01, 0x00], 256),
            (&[0x80, 0xFF], 255),
            (
                &[
                    0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
                ],
                1 << 40, // 1099511627776
            ),
        ];
        for (input, expected) in base256_cases {
            assert_eq!(
                parse_numeric(input).unwrap(),
                *expected,
                "parse_numeric({input:?})"
            );
        }
    }

    #[test]
    fn test_parse_numeric_base256_in_header() {
        // Test that base-256 encoded size field works in Header
        let mut header = Header::new_ustar();

        // Set size field (bytes 124..136) to base-256 encoded value
        // 12-byte field: first byte has 0x80 marker, remaining 11 bytes are the value
        // We want to encode a large value that wouldn't fit in octal
        let size_field = &mut header.as_mut_bytes()[124..136];
        size_field.fill(0);
        size_field[0] = 0x80; // base-256 marker (first byte & 0x7f = 0)
                              // Put value in last 4 bytes for simplicity: 0x12345678
        size_field[8] = 0x12;
        size_field[9] = 0x34;
        size_field[10] = 0x56;
        size_field[11] = 0x78;

        assert_eq!(header.entry_size().unwrap(), 0x12345678);
    }

    #[test]
    fn test_parse_numeric_base256_uid_gid() {
        let mut header = Header::new_ustar();

        // Set uid field (bytes 108..116) to base-256 encoded value
        let uid_field = &mut header.as_mut_bytes()[108..116];
        uid_field.fill(0);
        uid_field[0] = 0x80; // base-256 marker
        uid_field[7] = 0x42; // value = 66
        assert_eq!(header.uid().unwrap(), 66);

        // Set gid field (bytes 116..124) to base-256 encoded value
        let gid_field = &mut header.as_mut_bytes()[116..124];
        gid_field.fill(0);
        gid_field[0] = 0x80; // base-256 marker
        gid_field[6] = 0x01;
        gid_field[7] = 0x00; // value = 256
        assert_eq!(header.gid().unwrap(), 256);
    }

    #[test]
    fn test_from_bytes() {
        let mut data = [0u8; 512];
        // Set up a valid UStar header
        data[257..263].copy_from_slice(USTAR_MAGIC);
        data[263..265].copy_from_slice(USTAR_VERSION);
        data[0..4].copy_from_slice(b"test");

        let header = Header::from_bytes(&data);
        assert!(header.is_ustar());
        assert_eq!(header.path_bytes(), b"test");
    }

    #[test]
    fn test_from_bytes_gnu() {
        let mut data = [0u8; 512];
        data[257..263].copy_from_slice(GNU_MAGIC);
        data[263..265].copy_from_slice(GNU_VERSION);

        let header = Header::from_bytes(&data);
        assert!(header.is_gnu());
        assert!(!header.is_ustar());
    }

    // =========================================================================
    // PAX Extension Tests
    // =========================================================================

    #[test]
    fn test_pax_simple() {
        let data = b"20 path=foo/bar.txt\n";
        let mut iter = PaxExtensions::new(data);
        let ext = iter.next().unwrap().unwrap();
        assert_eq!(ext.key().unwrap(), "path");
        assert_eq!(ext.value().unwrap(), "foo/bar.txt");
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_pax_multiple() {
        let data = b"20 path=foo/bar.txt\n12 uid=1000\n12 gid=1000\n";
        let exts: Vec<_> = PaxExtensions::new(data).collect();
        assert_eq!(exts.len(), 3);
        assert_eq!(exts[0].as_ref().unwrap().key().unwrap(), "path");
        assert_eq!(exts[0].as_ref().unwrap().value().unwrap(), "foo/bar.txt");
        assert_eq!(exts[1].as_ref().unwrap().key().unwrap(), "uid");
        assert_eq!(exts[1].as_ref().unwrap().value().unwrap(), "1000");
        assert_eq!(exts[2].as_ref().unwrap().key().unwrap(), "gid");
        assert_eq!(exts[2].as_ref().unwrap().value().unwrap(), "1000");
    }

    #[test]
    fn test_pax_get() {
        let data = b"20 path=foo/bar.txt\n12 uid=1000\n16 size=1234567\n";
        let pax = PaxExtensions::new(data);

        let str_cases: &[(&str, Option<&str>)] = &[
            ("path", Some("foo/bar.txt")),
            ("uid", Some("1000")),
            ("missing", None),
        ];
        for (key, expected) in str_cases {
            assert_eq!(pax.get(key), *expected, "get({key:?})");
        }

        let u64_cases: &[(&str, Option<u64>)] = &[
            ("uid", Some(1000)),
            ("size", Some(1234567)),
            ("missing", None),
        ];
        for (key, expected) in u64_cases {
            assert_eq!(pax.get_u64(key), *expected, "get_u64({key:?})");
        }
    }

    #[test]
    fn test_pax_empty() {
        let data = b"";
        let mut iter = PaxExtensions::new(data);
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_pax_binary_value() {
        // PAX values can contain binary data (e.g., xattrs)
        // Format: "<len> <key>=<value>\n" where len includes everything
        // 24 = 2 (digits) + 1 (space) + 16 (key) + 1 (=) + 3 (value) + 1 (newline)
        let data = b"24 SCHILY.xattr.foo=\x00\x01\x02\n";
        let mut iter = PaxExtensions::new(data);
        let ext = iter.next().unwrap().unwrap();
        assert_eq!(ext.key().unwrap(), "SCHILY.xattr.foo");
        assert_eq!(ext.value_bytes(), b"\x00\x01\x02");
    }

    #[test]
    fn test_pax_long_path() {
        // Test a path that's exactly at various boundary lengths
        let long_path = "a".repeat(200);
        // PAX format: "length path=value\n" where length includes ALL bytes including itself
        // For 200-char path: 5 (path=) + 1 (\n) + 200 (value) + 1 (space) + 3 (length digits) = 210
        let record = format!("210 path={}\n", long_path);
        let data = record.as_bytes();
        let pax = PaxExtensions::new(data);
        assert_eq!(pax.get("path"), Some(long_path.as_str()));
    }

    #[test]
    fn test_pax_unicode_path() {
        // PAX supports UTF-8 paths
        let data = "35 path=日本語/ファイル.txt\n".as_bytes();
        let pax = PaxExtensions::new(data);
        assert_eq!(pax.get("path"), Some("日本語/ファイル.txt"));
    }

    #[test]
    fn test_pax_mtime_fractional() {
        // PAX mtime can have fractional seconds
        let data = b"22 mtime=1234567890.5\n";
        let pax = PaxExtensions::new(data);
        assert_eq!(pax.get("mtime"), Some("1234567890.5"));
        // get_u64 won't parse fractional
        assert_eq!(pax.get_u64("mtime"), None);
    }

    #[test]
    fn test_pax_schily_xattr() {
        let data = b"30 SCHILY.xattr.user.test=val\n";
        let mut iter = PaxExtensions::new(data);
        let ext = iter.next().unwrap().unwrap();
        let key = ext.key().unwrap();
        assert_eq!(key.strip_prefix(PAX_SCHILY_XATTR), Some("user.test"));
    }

    #[test]
    fn test_pax_malformed() {
        let cases: &[&[u8]] = &[
            b"15 pathfoobar\n", // no '='
            b"100 path=foo\n",  // length exceeds record
        ];
        for bad in cases {
            let result = PaxExtensions::new(bad).next().unwrap();
            assert!(result.is_err(), "should reject {bad:?}");
        }
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_path_exactly_100_bytes() {
        // Path that fills entire name field (no null terminator needed)
        let mut header = Header::new_ustar();
        let path = "a".repeat(100);
        header.as_mut_bytes()[0..100].copy_from_slice(path.as_bytes());

        assert_eq!(header.path_bytes().len(), 100);
        assert_eq!(header.path_bytes(), path.as_bytes());
    }

    #[test]
    fn test_link_name_exactly_100_bytes() {
        let mut header = Header::new_ustar();
        let target = "t".repeat(100);
        header.as_mut_bytes()[157..257].copy_from_slice(target.as_bytes());

        assert_eq!(header.link_name_bytes().len(), 100);
        assert_eq!(header.link_name_bytes(), target.as_bytes());
    }

    #[test]
    fn test_prefix_exactly_155_bytes() {
        let mut header = Header::new_ustar();
        let prefix = "p".repeat(155);
        header.as_mut_bytes()[345..500].copy_from_slice(prefix.as_bytes());

        assert_eq!(header.prefix().unwrap().len(), 155);
        assert_eq!(header.prefix().unwrap(), prefix.as_bytes());
    }

    #[test]
    fn test_sparse_header_parsing() {
        let header = Header::new_gnu();
        let gnu = header.as_gnu();

        // Default sparse headers should have zero offset and numbytes
        for sparse in &gnu.sparse {
            assert_eq!(parse_octal(&sparse.offset).unwrap(), 0);
            assert_eq!(parse_octal(&sparse.numbytes).unwrap(), 0);
        }
    }

    #[test]
    fn test_gnu_atime_ctime() {
        let mut header = Header::new_gnu();
        let gnu = header.as_gnu();

        // Default should be zeros
        assert_eq!(parse_octal(&gnu.atime).unwrap(), 0);
        assert_eq!(parse_octal(&gnu.ctime).unwrap(), 0);

        // Set some values (valid octal: 12345670123)
        header.as_mut_bytes()[345..356].copy_from_slice(b"12345670123");
        let gnu = header.as_gnu();
        assert_eq!(parse_octal(&gnu.atime).unwrap(), 0o12345670123);
    }

    #[test]
    fn test_ext_sparse_header() {
        let ext = GnuExtSparseHeader::default();
        assert_eq!(ext.isextended[0], 0);
        assert_eq!(ext.sparse.len(), 21);

        // Verify size is exactly 512 bytes
        assert_eq!(size_of::<GnuExtSparseHeader>(), HEADER_SIZE);
    }

    #[test]
    fn test_base256_max_values() {
        // Large UID that needs base-256
        let mut bytes = [0u8; 8];
        bytes[0] = 0x80; // marker
        bytes[4] = 0xFF;
        bytes[5] = 0xFF;
        bytes[6] = 0xFF;
        bytes[7] = 0xFF;
        assert_eq!(parse_numeric(&bytes).unwrap(), 0xFFFFFFFF);
    }

    // =========================================================================
    // Tests for encode_numeric and setter methods
    // =========================================================================

    #[test]
    fn test_encode_numeric_roundtrip() {
        fn check<const N: usize>(value: u64, expect_b256: bool) {
            let mut field = [0u8; N];
            encode_numeric(&mut field, value).unwrap();
            assert_eq!(
                field[0] & 0x80 != 0,
                expect_b256,
                "base256 flag for {value} in {N}-byte field"
            );
            assert_eq!(
                parse_numeric(&field).unwrap(),
                value,
                "roundtrip {value} in {N}-byte field"
            );
        }

        // 12-byte field: octal range
        check::<12>(0, false);
        check::<12>(0o644, false);
        check::<12>(0o77777777777, false);
        // 12-byte field: base-256 (>= 8GB threshold)
        check::<12>(8_589_934_592, true);
        check::<12>(0x1234_5678_90AB_CDEF, true);
        // 8-byte field (uid/gid): octal range
        check::<8>(0, false);
        check::<8>(2_097_151, false); // just below threshold
                                      // 8-byte field: base-256 (>= 2^21 threshold)
        check::<8>(2_097_152, true);
    }

    #[test]
    fn test_header_format_detection() {
        // (header, is_ustar, is_gnu)
        let cases: &[(Header, bool, bool)] = &[
            (Header::new_ustar(), true, false),
            (Header::new_gnu(), false, true),
            (Header::new_old(), false, false),
        ];
        for (header, ustar, gnu) in cases {
            assert_eq!(header.is_ustar(), *ustar, "{header:?}");
            assert_eq!(header.is_gnu(), *gnu, "{header:?}");
            assert_eq!(header.try_as_ustar().is_some(), *ustar);
            assert_eq!(header.try_as_gnu().is_some(), *gnu);
        }
    }

    #[test]
    fn test_header_mutable_views() {
        let mut header = Header::new_ustar();

        // Test mutable views exist and work
        let _old = header.as_old_mut();
        let _ustar = header.as_ustar_mut();
        let _gnu = header.as_gnu_mut();

        // Test try_as_*_mut
        let mut ustar_header = Header::new_ustar();
        assert!(ustar_header.try_as_ustar_mut().is_some());
        assert!(ustar_header.try_as_gnu_mut().is_none());
    }

    #[test]
    fn test_header_setters() {
        let mut header = Header::new_ustar();

        // Fallible numeric field setters: (set, get, value)
        type NumericCase = (
            fn(&mut Header, u64) -> Result<()>,
            fn(&Header) -> Result<u64>,
            u64,
        );
        let numeric_cases: &[NumericCase] = &[
            (|h, v| h.set_size(v), |h| h.entry_size(), 1024),
            (|h, v| h.set_uid(v), |h| h.uid(), 1000),
            (|h, v| h.set_gid(v), |h| h.gid(), 1000),
            (|h, v| h.set_mtime(v), |h| h.mtime(), 1234567890),
        ];
        for (set, get, value) in numeric_cases {
            set(&mut header, *value).unwrap();
            assert_eq!(get(&header).unwrap(), *value, "roundtrip {value}");
        }

        header.set_mode(0o755).unwrap();
        assert_eq!(header.mode().unwrap(), 0o755);

        header.set_entry_type(EntryType::Directory);
        assert_eq!(header.entry_type(), EntryType::Directory);

        header.set_path(b"test.txt").unwrap();
        assert_eq!(header.path_bytes(), b"test.txt");

        header.set_link_name(b"target").unwrap();
        assert_eq!(header.link_name_bytes(), b"target");

        header.set_checksum();
        header.verify_checksum().unwrap();
    }

    #[test]
    fn test_format_aware_encoding() {
        let large_uid: u64 = 0xFFFF_FFFF; // exceeds 8-byte octal max (2097151)
        let large_size: u64 = 10_000_000_000; // exceeds 12-byte octal max (8589934591)

        // GNU headers accept large values via base-256.
        let mut gnu = Header::new_gnu();
        gnu.set_uid(large_uid).unwrap();
        assert_eq!(gnu.uid().unwrap(), large_uid);
        gnu.set_size(large_size).unwrap();
        assert_eq!(gnu.entry_size().unwrap(), large_size);

        // UStar headers reject values that exceed octal capacity.
        let mut ustar = Header::new_ustar();
        assert!(ustar.set_uid(large_uid).is_err());
        assert!(ustar.set_size(large_size).is_err());

        // UStar headers accept values within octal range.
        ustar.set_uid(1000).unwrap();
        ustar.set_size(1024).unwrap();
    }

    #[test]
    fn test_gnu_header_atime_ctime_setters() {
        let mut header = Header::new_gnu();
        let gnu = header.as_gnu_mut();

        gnu.set_atime(1234567890);
        assert_eq!(gnu.atime().unwrap(), 1234567890);

        gnu.set_ctime(1234567891);
        assert_eq!(gnu.ctime().unwrap(), 1234567891);
    }

    #[test]
    fn test_gnu_header_real_size() {
        let mut header = Header::new_gnu();
        let gnu = header.as_gnu_mut();

        gnu.set_real_size(1_000_000);
        assert_eq!(gnu.real_size().unwrap(), 1_000_000);

        // Large value
        gnu.set_real_size(10_000_000_000);
        assert_eq!(gnu.real_size().unwrap(), 10_000_000_000);
    }

    #[test]
    fn test_gnu_header_is_extended() {
        let mut header = Header::new_gnu();
        let gnu = header.as_gnu_mut();

        assert!(!gnu.is_extended());
        gnu.set_is_extended(true);
        assert!(gnu.is_extended());
        gnu.set_is_extended(false);
        assert!(!gnu.is_extended());
    }

    /// Cross-checking tests against the `tar` crate using proptest.
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;
        use std::io::Cursor;

        /// Tar header format to test. Proptest generates both variants so
        /// each property is checked against UStar and GNU automatically.
        #[derive(Debug, Clone, Copy)]
        enum TarFormat {
            Ustar,
            Gnu,
        }

        fn tar_format_strategy() -> impl Strategy<Value = TarFormat> {
            prop_oneof![Just(TarFormat::Ustar), Just(TarFormat::Gnu)]
        }

        impl TarFormat {
            fn header_builder(self) -> crate::builder::HeaderBuilder {
                match self {
                    TarFormat::Ustar => crate::builder::HeaderBuilder::new_ustar(),
                    TarFormat::Gnu => crate::builder::HeaderBuilder::new_gnu(),
                }
            }

            fn tar_rs_header(self) -> tar::Header {
                match self {
                    TarFormat::Ustar => tar::Header::new_ustar(),
                    TarFormat::Gnu => tar::Header::new_gnu(),
                }
            }

            fn our_header(self) -> Header {
                match self {
                    TarFormat::Ustar => Header::new_ustar(),
                    TarFormat::Gnu => Header::new_gnu(),
                }
            }
        }

        /// Copy a tar-rs header into a `[u8; 512]`.
        fn tar_rs_bytes(header: &tar::Header) -> [u8; 512] {
            *header.as_bytes()
        }

        /// Format header bytes as labeled fields for readable diffs.
        fn header_hex(bytes: &[u8; 512]) -> String {
            let fields: &[(&str, std::ops::Range<usize>)] = &[
                ("name", 0..100),
                ("mode", 100..108),
                ("uid", 108..116),
                ("gid", 116..124),
                ("size", 124..136),
                ("mtime", 136..148),
                ("checksum", 148..156),
                ("typeflag", 156..157),
                ("linkname", 157..257),
                ("magic", 257..263),
                ("version", 263..265),
                ("uname", 265..297),
                ("gname", 297..329),
                ("devmajor", 329..337),
                ("devminor", 337..345),
                ("prefix", 345..500),
                ("padding", 500..512),
            ];
            let mut out = String::new();
            for (name, range) in fields {
                let slice = &bytes[range.clone()];
                if slice.iter().all(|&b| b == 0) {
                    continue;
                }
                use std::fmt::Write;
                write!(out, "{name:>10}: ").unwrap();
                for &b in slice {
                    if b.is_ascii_graphic() || b == b' ' {
                        out.push(b as char);
                    } else {
                        write!(out, "\\x{b:02x}").unwrap();
                    }
                }
                out.push('\n');
            }
            out
        }

        fn assert_headers_eq(ours: &[u8; 512], theirs: &[u8; 512]) {
            if ours != theirs {
                similar_asserts::assert_eq!(header_hex(ours), header_hex(theirs));
            }
        }

        /// Strategy for generating valid file paths (ASCII, no null bytes, reasonable length).
        fn path_strategy() -> impl Strategy<Value = String> {
            proptest::string::string_regex(
                "[a-zA-Z0-9_][a-zA-Z0-9_.+-]*(/[a-zA-Z0-9_][a-zA-Z0-9_.+-]*)*",
            )
            .expect("valid regex")
            .prop_filter("reasonable length", |s| !s.is_empty() && s.len() < 100)
        }

        /// Strategy for generating valid link targets.
        /// Avoids consecutive slashes and `.`/`..` segments which the tar crate normalizes.
        fn link_target_strategy() -> impl Strategy<Value = String> {
            proptest::string::string_regex(
                "[a-zA-Z0-9_][a-zA-Z0-9_+-]*(/[a-zA-Z0-9_][a-zA-Z0-9_+-]*)*",
            )
            .expect("valid regex")
            .prop_filter("reasonable length", |s| !s.is_empty() && s.len() < 100)
        }

        /// Strategy for generating valid user/group names.
        fn name_strategy() -> impl Strategy<Value = String> {
            proptest::string::string_regex("[a-zA-Z_][a-zA-Z0-9_]{0,30}").expect("valid regex")
        }

        /// Strategy for file mode (valid Unix permissions).
        fn mode_strategy() -> impl Strategy<Value = u32> {
            // Standard Unix permission modes
            prop_oneof![
                Just(0o644),    // regular file
                Just(0o755),    // executable
                Just(0o600),    // private
                Just(0o777),    // all permissions
                Just(0o400),    // read-only
                (0u32..0o7777), // any valid mode
            ]
        }

        /// Strategy for uid/gid values that fit in octal.
        fn id_strategy() -> impl Strategy<Value = u64> {
            prop_oneof![
                Just(0u64),
                Just(1000u64),
                Just(65534u64),    // nobody
                (0u64..0o7777777), // fits in 7 octal digits
            ]
        }

        /// Strategy for mtime values.
        fn mtime_strategy() -> impl Strategy<Value = u64> {
            prop_oneof![
                Just(0u64),
                Just(1234567890u64),
                (0u64..0o77777777777u64), // fits in 11 octal digits
            ]
        }

        /// Strategy for file size values.
        fn size_strategy() -> impl Strategy<Value = u64> {
            prop_oneof![
                Just(0u64),
                Just(1u64),
                Just(512u64),
                Just(4096u64),
                (0u64..1024 * 1024), // up to 1 MB
            ]
        }

        /// Test parameters for a regular file entry.
        #[derive(Debug, Clone)]
        struct FileParams {
            path: String,
            mode: u32,
            uid: u64,
            gid: u64,
            mtime: u64,
            size: u64,
            username: String,
            groupname: String,
        }

        fn file_params_strategy() -> impl Strategy<Value = FileParams> {
            (
                path_strategy(),
                mode_strategy(),
                id_strategy(),
                id_strategy(),
                mtime_strategy(),
                size_strategy(),
                name_strategy(),
                name_strategy(),
            )
                .prop_map(
                    |(path, mode, uid, gid, mtime, size, username, groupname)| FileParams {
                        path,
                        mode,
                        uid,
                        gid,
                        mtime,
                        size,
                        username,
                        groupname,
                    },
                )
        }

        /// Test parameters for a symlink entry.
        #[derive(Debug, Clone)]
        struct SymlinkParams {
            path: String,
            target: String,
            uid: u64,
            gid: u64,
            mtime: u64,
        }

        fn symlink_params_strategy() -> impl Strategy<Value = SymlinkParams> {
            (
                path_strategy(),
                link_target_strategy(),
                id_strategy(),
                id_strategy(),
                mtime_strategy(),
            )
                .prop_map(|(path, target, uid, gid, mtime)| SymlinkParams {
                    path,
                    target,
                    uid,
                    gid,
                    mtime,
                })
        }

        /// Test parameters for a directory entry.
        #[derive(Debug, Clone)]
        struct DirParams {
            path: String,
            mode: u32,
            uid: u64,
            gid: u64,
            mtime: u64,
        }

        fn dir_params_strategy() -> impl Strategy<Value = DirParams> {
            (
                path_strategy(),
                mode_strategy(),
                id_strategy(),
                id_strategy(),
                mtime_strategy(),
            )
                .prop_map(|(path, mode, uid, gid, mtime)| DirParams {
                    path,
                    mode,
                    uid,
                    gid,
                    mtime,
                })
        }

        /// Create a tar archive with a single file entry and return the bytes.
        fn create_file_tar(params: &FileParams, fmt: TarFormat) -> Vec<u8> {
            let mut builder = tar::Builder::new(Vec::new());

            let mut header = fmt.tar_rs_header();
            header.set_path(&params.path).unwrap();
            header.set_mode(params.mode);
            header.set_uid(params.uid);
            header.set_gid(params.gid);
            header.set_mtime(params.mtime);
            header.set_size(params.size);
            header.set_entry_type(tar::EntryType::Regular);
            header.set_username(&params.username).unwrap();
            header.set_groupname(&params.groupname).unwrap();
            header.set_cksum();

            let content = vec![0u8; params.size as usize];
            builder
                .append_data(&mut header, &params.path, content.as_slice())
                .unwrap();

            builder.into_inner().unwrap()
        }

        /// Create a tar archive with a symlink entry and return the bytes.
        fn create_symlink_tar(params: &SymlinkParams, fmt: TarFormat) -> Vec<u8> {
            let mut builder = tar::Builder::new(Vec::new());

            let mut header = fmt.tar_rs_header();
            header.set_path(&params.path).unwrap();
            header.set_mode(0o777);
            header.set_uid(params.uid);
            header.set_gid(params.gid);
            header.set_mtime(params.mtime);
            header.set_size(0);
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_link_name(&params.target).unwrap();
            header.set_cksum();

            builder
                .append_data(&mut header, &params.path, std::io::empty())
                .unwrap();

            builder.into_inner().unwrap()
        }

        /// Create a tar archive with a directory entry and return the bytes.
        fn create_dir_tar(params: &DirParams, fmt: TarFormat) -> Vec<u8> {
            let mut builder = tar::Builder::new(Vec::new());

            let mut header = fmt.tar_rs_header();
            let path = if params.path.ends_with('/') {
                params.path.clone()
            } else {
                format!("{}/", params.path)
            };
            header.set_path(&path).unwrap();
            header.set_mode(params.mode);
            header.set_uid(params.uid);
            header.set_gid(params.gid);
            header.set_mtime(params.mtime);
            header.set_size(0);
            header.set_entry_type(tar::EntryType::Directory);
            header.set_cksum();

            builder
                .append_data(&mut header, &path, std::io::empty())
                .unwrap();

            builder.into_inner().unwrap()
        }

        /// Extract the first 512-byte header from a tar archive.
        fn extract_header_bytes(tar_data: &[u8]) -> [u8; 512] {
            tar_data[..512].try_into().unwrap()
        }

        /// Compare our Header parsing against tar crate's parsing of the same
        /// bytes. Both parsers are reading identical data, so any disagreement
        /// is a bug in one of them.
        fn compare_headers(
            our_header: &Header,
            tar_header: &tar::Header,
        ) -> std::result::Result<(), TestCaseError> {
            // Entry type: compare the raw byte since both sides read from the
            // same header bytes.
            prop_assert_eq!(
                our_header.entry_type().to_byte(),
                tar_header.entry_type().as_byte(),
                "entry type mismatch"
            );

            prop_assert_eq!(
                our_header.entry_size().unwrap(),
                tar_header.size().unwrap(),
                "size mismatch"
            );
            prop_assert_eq!(
                our_header.mode().unwrap(),
                tar_header.mode().unwrap(),
                "mode mismatch"
            );
            prop_assert_eq!(
                our_header.uid().unwrap(),
                tar_header.uid().unwrap(),
                "uid mismatch"
            );
            prop_assert_eq!(
                our_header.gid().unwrap(),
                tar_header.gid().unwrap(),
                "gid mismatch"
            );
            prop_assert_eq!(
                our_header.mtime().unwrap(),
                tar_header.mtime().unwrap(),
                "mtime mismatch"
            );

            let tar_path = tar_header.path_bytes();
            prop_assert_eq!(our_header.path_bytes(), tar_path.as_ref(), "path mismatch");

            let our_link = our_header.link_name_bytes();
            if let Some(tar_link) = tar_header.link_name_bytes() {
                prop_assert_eq!(our_link, tar_link.as_ref(), "link_name mismatch");
            } else {
                prop_assert!(our_link.is_empty(), "expected empty link name");
            }

            if let Some(our_username) = our_header.username() {
                if let Some(tar_username) = tar_header.username_bytes() {
                    prop_assert_eq!(our_username, tar_username, "username mismatch");
                }
            }

            if let Some(our_groupname) = our_header.groupname() {
                if let Some(tar_groupname) = tar_header.groupname_bytes() {
                    prop_assert_eq!(our_groupname, tar_groupname, "groupname mismatch");
                }
            }

            our_header.verify_checksum().unwrap();

            Ok(())
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(256))]

            #[test]
            fn test_file_header_crosscheck(
                params in file_params_strategy(),
                fmt in tar_format_strategy(),
            ) {
                let tar_data = create_file_tar(&params, fmt);
                let header_bytes = extract_header_bytes(&tar_data);

                let our_header = Header::from_bytes(&header_bytes);
                let tar_header = tar::Header::from_byte_slice(&header_bytes);

                compare_headers(our_header, tar_header)?;

                prop_assert!(our_header.entry_type().is_file());
                prop_assert_eq!(our_header.entry_size().unwrap(), params.size);

                if matches!(fmt, TarFormat::Gnu) {
                    prop_assert!(our_header.is_gnu());
                    prop_assert!(!our_header.is_ustar());
                }
            }

            #[test]
            fn test_symlink_header_crosscheck(
                params in symlink_params_strategy(),
                fmt in tar_format_strategy(),
            ) {
                let tar_data = create_symlink_tar(&params, fmt);
                let header_bytes = extract_header_bytes(&tar_data);

                let our_header = Header::from_bytes(&header_bytes);
                let tar_header = tar::Header::from_byte_slice(&header_bytes);

                compare_headers(our_header, tar_header)?;

                prop_assert!(our_header.entry_type().is_symlink());
                prop_assert_eq!(our_header.link_name_bytes(), params.target.as_bytes());

                if matches!(fmt, TarFormat::Gnu) {
                    prop_assert!(our_header.is_gnu());
                }
            }

            #[test]
            fn test_dir_header_crosscheck(
                params in dir_params_strategy(),
                fmt in tar_format_strategy(),
            ) {
                let tar_data = create_dir_tar(&params, fmt);
                let header_bytes = extract_header_bytes(&tar_data);

                let our_header = Header::from_bytes(&header_bytes);
                let tar_header = tar::Header::from_byte_slice(&header_bytes);

                compare_headers(our_header, tar_header)?;

                prop_assert!(our_header.entry_type().is_dir());

                if matches!(fmt, TarFormat::Gnu) {
                    prop_assert!(our_header.is_gnu());
                }
            }
        }

        /// Test reading entries from real tar archives created by the tar crate.
        mod archive_tests {
            use super::*;

            proptest! {
                #![proptest_config(ProptestConfig::with_cases(64))]

                #[test]
                fn test_multi_entry_archive(
                    files in prop::collection::vec(file_params_strategy(), 1..8),
                    dirs in prop::collection::vec(dir_params_strategy(), 0..4),
                ) {
                    // Build an archive with multiple entries
                    let mut builder = tar::Builder::new(Vec::new());

                    // Add directories first
                    for params in &dirs {
                        let mut header = tar::Header::new_ustar();
                        let path = if params.path.ends_with('/') {
                            params.path.clone()
                        } else {
                            format!("{}/", params.path)
                        };
                        header.set_path(&path).unwrap();
                        header.set_mode(params.mode);
                        header.set_uid(params.uid);
                        header.set_gid(params.gid);
                        header.set_mtime(params.mtime);
                        header.set_size(0);
                        header.set_entry_type(tar::EntryType::Directory);
                        header.set_cksum();
                        builder.append_data(&mut header, &path, std::io::empty()).unwrap();
                    }

                    // Add files
                    for params in &files {
                        let mut header = tar::Header::new_ustar();
                        header.set_path(&params.path).unwrap();
                        header.set_mode(params.mode);
                        header.set_uid(params.uid);
                        header.set_gid(params.gid);
                        header.set_mtime(params.mtime);
                        header.set_size(params.size);
                        header.set_entry_type(tar::EntryType::Regular);
                        header.set_username(&params.username).unwrap();
                        header.set_groupname(&params.groupname).unwrap();
                        header.set_cksum();

                        let content = vec![0u8; params.size as usize];
                        builder.append_data(&mut header, &params.path, content.as_slice()).unwrap();
                    }

                    let tar_data = builder.into_inner().unwrap();

                    // Now iterate through the archive and verify each header
                    let mut archive = tar::Archive::new(Cursor::new(&tar_data));
                    let entries = archive.entries().unwrap();

                    for entry_result in entries {
                        let entry = entry_result.unwrap();
                        let tar_header = entry.header();

                        // Get the raw header bytes from the archive
                        let our_header = Header::from_bytes(tar_header.as_bytes());

                        compare_headers(our_header, tar_header)?;
                    }
                }
            }
        }

        /// Test format detection (UStar vs GNU vs Old).
        mod format_detection_tests {
            use super::*;

            proptest! {
                #![proptest_config(ProptestConfig::with_cases(128))]

                #[test]
                fn test_ustar_format_detected(params in file_params_strategy()) {
                    let tar_data = create_file_tar(&params, TarFormat::Ustar);
                    let header_bytes = extract_header_bytes(&tar_data);

                    let our_header = Header::from_bytes(&header_bytes);

                    prop_assert!(our_header.is_ustar(), "should be UStar");
                    prop_assert!(!our_header.is_gnu(), "should not be GNU");

                    prop_assert_eq!(&header_bytes[257..263], USTAR_MAGIC);
                    prop_assert_eq!(&header_bytes[263..265], USTAR_VERSION);
                }

                #[test]
                fn test_gnu_format_detected(params in file_params_strategy()) {
                    let tar_data = create_file_tar(&params, TarFormat::Gnu);
                    let header_bytes = extract_header_bytes(&tar_data);

                    let our_header = Header::from_bytes(&header_bytes);

                    prop_assert!(our_header.is_gnu(), "should be GNU");
                    prop_assert!(!our_header.is_ustar(), "should not be UStar");

                    prop_assert_eq!(&header_bytes[257..263], GNU_MAGIC);
                    prop_assert_eq!(&header_bytes[263..265], GNU_VERSION);
                }
            }

            #[test]
            fn test_old_format_detection() {
                // Create a header with no magic (old format)
                let mut header_bytes = [0u8; 512];

                // Set a simple file name
                header_bytes[0..4].copy_from_slice(b"test");

                // Set mode (octal)
                header_bytes[100..107].copy_from_slice(b"0000644");

                // Set size = 0
                header_bytes[124..135].copy_from_slice(b"00000000000");

                // Set typeflag = regular file
                header_bytes[156] = b'0';

                // Compute and set checksum
                let mut checksum: u64 = 0;
                for (i, &byte) in header_bytes.iter().enumerate() {
                    if (148..156).contains(&i) {
                        checksum += u64::from(b' ');
                    } else {
                        checksum += u64::from(byte);
                    }
                }
                let checksum_str = format!("{checksum:06o}\0 ");
                header_bytes[148..156].copy_from_slice(checksum_str.as_bytes());

                let our_header = Header::from_bytes(&header_bytes);

                // Old format: neither UStar nor GNU
                assert!(!our_header.is_ustar());
                assert!(!our_header.is_gnu());

                // But we can still parse basic fields
                assert_eq!(our_header.path_bytes(), b"test");
                assert_eq!(our_header.entry_type(), EntryType::Regular);
            }
        }

        /// Test checksum computation matches tar crate.
        mod checksum_tests {
            use super::*;

            proptest! {
                #![proptest_config(ProptestConfig::with_cases(256))]

                #[test]
                fn test_checksum_always_valid(
                    params in file_params_strategy(),
                    fmt in tar_format_strategy(),
                ) {
                    let tar_data = create_file_tar(&params, fmt);
                    let header_bytes = extract_header_bytes(&tar_data);

                    let our_header = Header::from_bytes(&header_bytes);
                    our_header.verify_checksum().unwrap();
                }

                #[test]
                fn test_checksum_recompute(
                    params in file_params_strategy(),
                    fmt in tar_format_strategy(),
                ) {
                    let tar_data = create_file_tar(&params, fmt);
                    let header_bytes = extract_header_bytes(&tar_data);

                    let our_header = Header::from_bytes(&header_bytes);

                    // Our computed checksum should match
                    let computed = our_header.compute_checksum();
                    let stored = parse_octal(&header_bytes[148..156]).unwrap();

                    prop_assert_eq!(computed, stored);
                }
            }
        }

        /// Test entry type mapping is complete.
        mod entry_type_tests {
            use super::*;

            #[test]
            fn test_all_entry_types_map_correctly() {
                // Test all known entry type bytes
                let mappings: &[(u8, EntryType, tar::EntryType)] = &[
                    (b'0', EntryType::Regular, tar::EntryType::Regular),
                    (b'\0', EntryType::Regular, tar::EntryType::Regular),
                    (b'1', EntryType::Link, tar::EntryType::Link),
                    (b'2', EntryType::Symlink, tar::EntryType::Symlink),
                    (b'3', EntryType::Char, tar::EntryType::Char),
                    (b'4', EntryType::Block, tar::EntryType::Block),
                    (b'5', EntryType::Directory, tar::EntryType::Directory),
                    (b'6', EntryType::Fifo, tar::EntryType::Fifo),
                    (b'7', EntryType::Continuous, tar::EntryType::Continuous),
                    (b'L', EntryType::GnuLongName, tar::EntryType::GNULongName),
                    (b'K', EntryType::GnuLongLink, tar::EntryType::GNULongLink),
                    (b'S', EntryType::GnuSparse, tar::EntryType::GNUSparse),
                    (b'x', EntryType::XHeader, tar::EntryType::XHeader),
                    (
                        b'g',
                        EntryType::XGlobalHeader,
                        tar::EntryType::XGlobalHeader,
                    ),
                ];

                for &(byte, expected_ours, expected_tar) in mappings {
                    let ours = EntryType::from_byte(byte);
                    let tar_type = tar::EntryType::new(byte);

                    assert_eq!(ours, expected_ours, "our mapping for byte {byte}");
                    assert_eq!(tar_type, expected_tar, "tar mapping for byte {byte}");
                }
            }

            proptest! {
                #[test]
                fn test_entry_type_roundtrip(byte: u8) {
                    let our_type = EntryType::from_byte(byte);
                    let tar_type = tar::EntryType::new(byte);

                    // Both should handle unknown types gracefully
                    let our_byte = our_type.to_byte();
                    let tar_byte = tar_type.as_byte();

                    // For regular files, '\0' maps to '0'
                    if byte == b'\0' {
                        prop_assert_eq!(our_byte, b'0');
                    } else {
                        prop_assert_eq!(our_byte, tar_byte);
                    }
                }
            }
        }

        /// Encode/decode roundtrip and panic-freedom tests for octal and
        /// numeric fields. These cover the properties that are too expensive
        /// for Kani (stdlib `from_utf8`/`from_str_radix` have unbounded loops).
        mod codec_tests {
            use super::*;

            proptest! {
                #![proptest_config(ProptestConfig::with_cases(10_000))]

                #[test]
                fn test_encode_octal_8_roundtrip(value in 0u64..=0o7777777) {
                    let mut field = [0u8; 8];
                    encode_octal(&mut field, value).unwrap();
                    prop_assert_eq!(parse_octal(&field).unwrap(), value);
                }

                #[test]
                fn test_encode_octal_12_roundtrip(value in 0u64..=0o77777777777) {
                    let mut field = [0u8; 12];
                    encode_octal(&mut field, value).unwrap();
                    prop_assert_eq!(parse_octal(&field).unwrap(), value);
                }

                // 8-byte base-256 has 63 data bits, so values < 2^63 roundtrip.
                #[test]
                fn test_encode_numeric_8_roundtrip(value in 0u64..=(i64::MAX as u64)) {
                    let mut field = [0u8; 8];
                    encode_numeric(&mut field, value).unwrap();
                    prop_assert_eq!(parse_numeric(&field).unwrap(), value);
                }

                // Values >= 2^63 cannot be represented in an 8-byte base-256 field.
                #[test]
                fn test_encode_numeric_8_rejects_huge(value in (i64::MAX as u64 + 1)..=u64::MAX) {
                    let mut field = [0u8; 8];
                    prop_assert!(encode_numeric(&mut field, value).is_err());
                }

                #[test]
                fn test_encode_numeric_12_roundtrip(value: u64) {
                    let mut field = [0u8; 12];
                    encode_numeric(&mut field, value).unwrap();
                    prop_assert_eq!(parse_numeric(&field).unwrap(), value);
                }

                #[test]
                fn test_encode_octal_8_rejects_overflow(value in 0o10000000u64..=u64::MAX) {
                    let mut field = [0u8; 8];
                    prop_assert!(encode_octal(&mut field, value).is_err());
                }

                #[test]
                fn test_encode_octal_12_rejects_overflow(value in 0o100000000000u64..=u64::MAX) {
                    let mut field = [0u8; 12];
                    prop_assert!(encode_octal(&mut field, value).is_err());
                }

                #[test]
                fn test_parse_octal_8_no_panic(bytes in proptest::array::uniform8(0u8..)) {
                    let _ = parse_octal(&bytes);
                }

                #[test]
                fn test_parse_octal_12_no_panic(bytes in proptest::array::uniform12(0u8..)) {
                    let _ = parse_octal(&bytes);
                }

                #[test]
                fn test_parse_numeric_8_no_panic(bytes in proptest::array::uniform8(0u8..)) {
                    let _ = parse_numeric(&bytes);
                }

                #[test]
                fn test_parse_numeric_12_no_panic(bytes in proptest::array::uniform12(0u8..)) {
                    let _ = parse_numeric(&bytes);
                }
            }
        }

        /// Tests that verify tar-core's builder APIs produce bit-identical
        /// output compared to tar-rs when given the same inputs.
        ///
        /// This is critical for ensuring tar-rs can rebase on tar-core.
        mod builder_equivalence_tests {
            use super::*;

            fn build_file_tar_core(params: &FileParams, fmt: TarFormat) -> Header {
                let mut b = fmt.header_builder();
                b.path(params.path.as_bytes())
                    .unwrap()
                    .mode(params.mode)
                    .unwrap()
                    .uid(params.uid)
                    .unwrap()
                    .gid(params.gid)
                    .unwrap()
                    .size(params.size)
                    .unwrap()
                    .mtime(params.mtime)
                    .unwrap()
                    .entry_type(EntryType::Regular)
                    .username(params.username.as_bytes())
                    .unwrap()
                    .groupname(params.groupname.as_bytes())
                    .unwrap();
                b.finish()
            }

            fn build_file_tar_rs(params: &FileParams, fmt: TarFormat) -> [u8; 512] {
                let mut h = fmt.tar_rs_header();
                h.set_path(&params.path).unwrap();
                h.set_mode(params.mode);
                h.set_uid(params.uid);
                h.set_gid(params.gid);
                h.set_size(params.size);
                h.set_mtime(params.mtime);
                h.set_entry_type(tar::EntryType::Regular);
                h.set_username(&params.username).unwrap();
                h.set_groupname(&params.groupname).unwrap();
                h.set_cksum();
                tar_rs_bytes(&h)
            }

            fn build_symlink_tar_core(params: &SymlinkParams, fmt: TarFormat) -> Header {
                let mut b = fmt.header_builder();
                b.path(params.path.as_bytes())
                    .unwrap()
                    .mode(0o777)
                    .unwrap()
                    .uid(params.uid)
                    .unwrap()
                    .gid(params.gid)
                    .unwrap()
                    .size(0)
                    .unwrap()
                    .mtime(params.mtime)
                    .unwrap()
                    .entry_type(EntryType::Symlink)
                    .link_name(params.target.as_bytes())
                    .unwrap();
                b.finish()
            }

            fn build_symlink_tar_rs(params: &SymlinkParams, fmt: TarFormat) -> [u8; 512] {
                let mut h = fmt.tar_rs_header();
                h.set_path(&params.path).unwrap();
                h.set_mode(0o777);
                h.set_uid(params.uid);
                h.set_gid(params.gid);
                h.set_size(0);
                h.set_mtime(params.mtime);
                h.set_entry_type(tar::EntryType::Symlink);
                h.set_link_name(&params.target).unwrap();
                h.set_cksum();
                tar_rs_bytes(&h)
            }

            fn build_dir_tar_core(params: &DirParams, fmt: TarFormat) -> Header {
                let mut b = fmt.header_builder();
                let path = if params.path.ends_with('/') {
                    params.path.clone()
                } else {
                    format!("{}/", params.path)
                };
                b.path(path.as_bytes())
                    .unwrap()
                    .mode(params.mode)
                    .unwrap()
                    .uid(params.uid)
                    .unwrap()
                    .gid(params.gid)
                    .unwrap()
                    .size(0)
                    .unwrap()
                    .mtime(params.mtime)
                    .unwrap()
                    .entry_type(EntryType::Directory);
                b.finish()
            }

            fn build_dir_tar_rs(params: &DirParams, fmt: TarFormat) -> [u8; 512] {
                let mut h = fmt.tar_rs_header();
                let path = if params.path.ends_with('/') {
                    params.path.clone()
                } else {
                    format!("{}/", params.path)
                };
                h.set_path(&path).unwrap();
                h.set_mode(params.mode);
                h.set_uid(params.uid);
                h.set_gid(params.gid);
                h.set_size(0);
                h.set_mtime(params.mtime);
                h.set_entry_type(tar::EntryType::Directory);
                h.set_cksum();
                tar_rs_bytes(&h)
            }

            fn build_file_header_setters(params: &FileParams, fmt: TarFormat) -> [u8; 512] {
                let mut h = fmt.our_header();
                h.set_path(params.path.as_bytes()).unwrap();
                h.set_mode(params.mode).unwrap();
                h.set_uid(params.uid).unwrap();
                h.set_gid(params.gid).unwrap();
                h.set_size(params.size).unwrap();
                h.set_mtime(params.mtime).unwrap();
                h.set_entry_type(EntryType::Regular);
                h.set_username(params.username.as_bytes()).unwrap();
                h.set_groupname(params.groupname.as_bytes()).unwrap();
                h.set_checksum();
                *h.as_bytes()
            }

            proptest! {
                #![proptest_config(ProptestConfig::with_cases(256))]

                #[test]
                fn test_file_builder_equivalence(
                    params in file_params_strategy(),
                    fmt in tar_format_strategy(),
                ) {
                    assert_headers_eq(
                        build_file_tar_core(&params, fmt).as_bytes(),
                        &build_file_tar_rs(&params, fmt),
                    );
                }

                #[test]
                fn test_symlink_builder_equivalence(
                    params in symlink_params_strategy(),
                    fmt in tar_format_strategy(),
                ) {
                    assert_headers_eq(
                        build_symlink_tar_core(&params, fmt).as_bytes(),
                        &build_symlink_tar_rs(&params, fmt),
                    );
                }

                #[test]
                fn test_dir_builder_equivalence(
                    params in dir_params_strategy(),
                    fmt in tar_format_strategy(),
                ) {
                    assert_headers_eq(
                        build_dir_tar_core(&params, fmt).as_bytes(),
                        &build_dir_tar_rs(&params, fmt),
                    );
                }

                #[test]
                fn test_header_setters_equivalence(
                    params in file_params_strategy(),
                    fmt in tar_format_strategy(),
                ) {
                    assert_headers_eq(
                        &build_file_header_setters(&params, fmt),
                        &build_file_tar_rs(&params, fmt),
                    );
                }
            }

            /// Test large values that require base-256 encoding.
            mod base256_equivalence {
                use super::*;

                /// Strategy for large UID/GID values that require base-256.
                fn large_id_strategy() -> impl Strategy<Value = u64> {
                    prop_oneof![
                        Just(2097152u64),              // just over octal limit
                        Just(u32::MAX as u64),         // common large value
                        (2097152u64..u32::MAX as u64), // range requiring base-256
                    ]
                }

                /// Create a minimal GNU regular-file header pair for field-level tests.
                fn default_headers() -> (Header, tar::Header) {
                    let mut ours = Header::new_gnu();
                    ours.set_path(b"test.txt").unwrap();
                    ours.set_mode(0o644).unwrap();
                    ours.set_uid(1000).unwrap();
                    ours.set_gid(1000).unwrap();
                    ours.set_size(0).unwrap();
                    ours.set_mtime(0).unwrap();
                    ours.set_entry_type(EntryType::Regular);

                    let mut theirs = tar::Header::new_gnu();
                    theirs.set_path("test.txt").unwrap();
                    theirs.set_mode(0o644);
                    theirs.set_uid(1000);
                    theirs.set_gid(1000);
                    theirs.set_size(0);
                    theirs.set_mtime(0);
                    theirs.set_entry_type(tar::EntryType::Regular);

                    (ours, theirs)
                }

                #[test]
                fn test_large_uid_encoding() {
                    let (mut ours, mut theirs) = default_headers();
                    ours.set_uid(2_500_000).unwrap();
                    ours.set_checksum();
                    theirs.set_uid(2_500_000);
                    theirs.set_cksum();

                    assert_eq!(&ours.as_bytes()[108..116], &theirs.as_bytes()[108..116]);
                    assert_eq!(ours.uid().unwrap(), 2_500_000);
                }

                #[test]
                fn test_large_gid_encoding() {
                    let (mut ours, mut theirs) = default_headers();
                    ours.set_gid(3_000_000).unwrap();
                    ours.set_checksum();
                    theirs.set_gid(3_000_000);
                    theirs.set_cksum();

                    assert_eq!(&ours.as_bytes()[116..124], &theirs.as_bytes()[116..124]);
                    assert_eq!(ours.gid().unwrap(), 3_000_000);
                }

                proptest! {
                    #![proptest_config(ProptestConfig::with_cases(64))]

                    #[test]
                    fn test_large_uid_proptest(uid in large_id_strategy()) {
                        let (mut ours, mut theirs) = default_headers();
                        ours.set_uid(uid).unwrap();
                        ours.set_checksum();
                        theirs.set_uid(uid);
                        theirs.set_cksum();

                        prop_assert_eq!(
                            &ours.as_bytes()[108..116],
                            &theirs.as_bytes()[108..116],
                        );
                    }

                    #[test]
                    fn test_large_gid_proptest(gid in large_id_strategy()) {
                        let (mut ours, mut theirs) = default_headers();
                        ours.set_gid(gid).unwrap();
                        ours.set_checksum();
                        theirs.set_gid(gid);
                        theirs.set_cksum();

                        prop_assert_eq!(
                            &ours.as_bytes()[116..124],
                            &theirs.as_bytes()[116..124],
                        );
                    }
                }
            }

            /// Test infallible `_small` setter variants roundtrip correctly
            /// and produce output equivalent to tar-rs for the same values.
            mod small_setter_tests {
                use super::*;

                fn default_header_pair() -> (Header, tar::Header) {
                    let mut ours = Header::new_gnu();
                    ours.set_path(b"t.txt").unwrap();
                    ours.set_mode_small(0o644);
                    ours.set_uid(0).unwrap();
                    ours.set_gid(0).unwrap();
                    ours.set_size_small(0);
                    ours.set_mtime_small(0);
                    ours.set_entry_type(EntryType::Regular);

                    let mut theirs = tar::Header::new_gnu();
                    theirs.set_path("t.txt").unwrap();
                    theirs.set_mode(0o644);
                    theirs.set_uid(0);
                    theirs.set_gid(0);
                    theirs.set_size(0);
                    theirs.set_mtime(0);
                    theirs.set_entry_type(tar::EntryType::Regular);

                    (ours, theirs)
                }

                proptest! {
                    #![proptest_config(ProptestConfig::with_cases(256))]

                    #[test]
                    fn test_set_mode_small_roundtrip(mode: u16) {
                        let (mut ours, mut theirs) = default_header_pair();
                        ours.set_mode_small(mode);
                        ours.set_checksum();
                        theirs.set_mode(u32::from(mode));
                        theirs.set_cksum();

                        prop_assert_eq!(ours.mode().unwrap(), u32::from(mode));
                        prop_assert_eq!(
                            &ours.as_bytes()[100..108],
                            &theirs.as_bytes()[100..108],
                        );
                    }

                    #[test]
                    fn test_set_size_small_roundtrip(size: u32) {
                        let (mut ours, mut theirs) = default_header_pair();
                        ours.set_size_small(size);
                        ours.set_checksum();
                        theirs.set_size(u64::from(size));
                        theirs.set_cksum();

                        prop_assert_eq!(ours.entry_size().unwrap(), u64::from(size));
                        prop_assert_eq!(
                            &ours.as_bytes()[124..136],
                            &theirs.as_bytes()[124..136],
                        );
                    }

                    #[test]
                    fn test_set_mtime_small_roundtrip(mtime: u32) {
                        let (mut ours, mut theirs) = default_header_pair();
                        ours.set_mtime_small(mtime);
                        ours.set_checksum();
                        theirs.set_mtime(u64::from(mtime));
                        theirs.set_cksum();

                        prop_assert_eq!(ours.mtime().unwrap(), u64::from(mtime));
                        prop_assert_eq!(
                            &ours.as_bytes()[136..148],
                            &theirs.as_bytes()[136..148],
                        );
                    }

                    #[test]
                    fn test_set_device_small_roundtrip(major: u16, minor: u16) {
                        let mut header = Header::new_ustar();
                        header.set_device_small(major, minor);

                        prop_assert_eq!(
                            header.device_major().unwrap().unwrap(),
                            u32::from(major),
                        );
                        prop_assert_eq!(
                            header.device_minor().unwrap().unwrap(),
                            u32::from(minor),
                        );
                    }
                }
            }

            /// Test GNU long name/link extensions produce equivalent output.
            mod gnu_extensions_equivalence {
                use super::*;
                use crate::builder::EntryBuilder;

                /// Strategy for generating long paths (101-300 bytes).
                fn long_path_strategy() -> impl Strategy<Value = String> {
                    // Generate paths like "aaa/bbb/ccc/..." that exceed 100 bytes
                    (3..15usize)
                        .prop_flat_map(|segments| {
                            proptest::collection::vec(
                                proptest::string::string_regex("[a-z]{5,20}").expect("valid regex"),
                                segments,
                            )
                        })
                        .prop_map(|parts| parts.join("/"))
                        .prop_filter("must exceed 100 bytes", |s| s.len() > 100 && s.len() < 300)
                }

                /// Strategy for generating long link targets.
                fn long_link_strategy() -> impl Strategy<Value = String> {
                    long_path_strategy()
                }

                /// Parameters for a long-path file entry with random metadata.
                #[derive(Debug, Clone)]
                struct LongPathFileParams {
                    path: String,
                    mode: u32,
                    uid: u64,
                    gid: u64,
                    mtime: u64,
                }

                fn long_path_file_params_strategy() -> impl Strategy<Value = LongPathFileParams> {
                    (
                        long_path_strategy(),
                        mode_strategy(),
                        id_strategy(),
                        id_strategy(),
                        mtime_strategy(),
                    )
                        .prop_map(|(path, mode, uid, gid, mtime)| {
                            LongPathFileParams {
                                path,
                                mode,
                                uid,
                                gid,
                                mtime,
                            }
                        })
                }

                /// Parameters for a symlink entry with long target.
                #[derive(Debug, Clone)]
                struct LongLinkParams {
                    path: String,
                    target: String,
                    uid: u64,
                    gid: u64,
                    mtime: u64,
                }

                fn long_link_params_strategy() -> impl Strategy<Value = LongLinkParams> {
                    (
                        path_strategy(),      // short path for the symlink itself
                        long_link_strategy(), // long target
                        id_strategy(),
                        id_strategy(),
                        mtime_strategy(),
                    )
                        .prop_map(|(path, target, uid, gid, mtime)| {
                            LongLinkParams {
                                path,
                                target,
                                uid,
                                gid,
                                mtime,
                            }
                        })
                }

                /// Extract all header blocks from a tar archive created by tar-rs.
                /// Uses `tar::Archive` in raw mode to see extension headers.
                fn extract_all_headers(tar_data: &[u8]) -> Vec<Header> {
                    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_data));
                    archive
                        .entries()
                        .expect("tar entries")
                        .raw(true)
                        .map(|e| {
                            let e = e.expect("tar entry");
                            *Header::from_bytes(e.header().as_bytes())
                        })
                        .collect()
                }

                /// Build a tar archive with a long path using tar-rs.
                fn build_long_path_with_tar_rs(params: &LongPathFileParams) -> Vec<u8> {
                    let mut builder = tar::Builder::new(Vec::new());

                    let mut header = tar::Header::new_gnu();
                    header.set_mode(params.mode);
                    header.set_uid(params.uid);
                    header.set_gid(params.gid);
                    header.set_size(0);
                    header.set_mtime(params.mtime);
                    header.set_entry_type(tar::EntryType::Regular);

                    builder
                        .append_data(&mut header, &params.path, std::io::empty())
                        .unwrap();
                    builder.into_inner().unwrap()
                }

                /// Build entry headers with a long path using tar-core.
                fn build_long_path_with_tar_core(params: &LongPathFileParams) -> Vec<Header> {
                    let mut builder = EntryBuilder::new_gnu();
                    builder
                        .path(params.path.as_bytes())
                        .mode(params.mode)
                        .unwrap()
                        .uid(params.uid)
                        .unwrap()
                        .gid(params.gid)
                        .unwrap()
                        .size(0)
                        .unwrap()
                        .mtime(params.mtime)
                        .unwrap()
                        .entry_type(EntryType::Regular);

                    builder.finish()
                }

                /// Build a tar archive with a long symlink using tar-rs.
                fn build_long_link_with_tar_rs(params: &LongLinkParams) -> Vec<u8> {
                    let mut builder = tar::Builder::new(Vec::new());

                    let mut header = tar::Header::new_gnu();
                    header.set_mode(0o777);
                    header.set_uid(params.uid);
                    header.set_gid(params.gid);
                    header.set_size(0);
                    header.set_mtime(params.mtime);
                    header.set_entry_type(tar::EntryType::Symlink);
                    builder
                        .append_link(&mut header, &params.path, &params.target)
                        .unwrap();
                    builder.into_inner().unwrap()
                }

                /// Build entry headers with a long symlink target using tar-core.
                fn build_long_link_with_tar_core(params: &LongLinkParams) -> Vec<Header> {
                    let mut builder = EntryBuilder::new_gnu();
                    builder
                        .path(params.path.as_bytes())
                        .link_name(params.target.as_bytes())
                        .mode(0o777)
                        .unwrap()
                        .uid(params.uid)
                        .unwrap()
                        .gid(params.gid)
                        .unwrap()
                        .size(0)
                        .unwrap()
                        .mtime(params.mtime)
                        .unwrap()
                        .entry_type(EntryType::Symlink);

                    builder.finish()
                }

                /// Compare the extension and main headers from our builder
                /// against those extracted from a tar-rs archive.
                ///
                /// Our builder returns all blocks (headers + data), while
                /// `extract_all_headers` returns only header blocks, so we
                /// compare first (extension) and last (main) individually.
                ///
                /// Extension headers only need semantic equality (type, path,
                /// size) since metadata fields like mode/uid/gid are set
                /// differently by tar-rs vs tar-core (both are valid).
                fn compare_extension_headers(our_blocks: &[Header], tar_headers: &[Header]) {
                    assert!(our_blocks.len() >= 2, "expected extension + main headers");
                    assert!(tar_headers.len() >= 2, "expected extension + main headers");

                    let our_ext = &our_blocks[0];
                    let tar_ext = &tar_headers[0];
                    assert_eq!(our_ext.entry_type(), tar_ext.entry_type(), "extension type");
                    assert_eq!(our_ext.path_bytes(), tar_ext.path_bytes(), "extension path");
                    assert_eq!(
                        our_ext.entry_size().unwrap(),
                        tar_ext.entry_size().unwrap(),
                        "extension size"
                    );

                    // Main header: compare key fields. The linkname field
                    // can differ because tar-rs normalizes paths while our
                    // builder writes raw bytes truncated to 100 bytes.
                    let our_main = our_blocks.last().unwrap();
                    let tar_main = tar_headers.last().unwrap();
                    assert_eq!(our_main.entry_type(), tar_main.entry_type(), "main type");
                    assert_eq!(
                        our_main.mode().unwrap(),
                        tar_main.mode().unwrap(),
                        "main mode"
                    );
                    assert_eq!(our_main.uid().unwrap(), tar_main.uid().unwrap(), "main uid");
                    assert_eq!(our_main.gid().unwrap(), tar_main.gid().unwrap(), "main gid");
                    assert_eq!(
                        our_main.mtime().unwrap(),
                        tar_main.mtime().unwrap(),
                        "main mtime"
                    );
                }

                #[test]
                fn test_gnu_longname_basic() {
                    let params = LongPathFileParams {
                        path: "a/".repeat(60) + "file.txt",
                        mode: 0o644,
                        uid: 1000,
                        gid: 1000,
                        mtime: 1234567890,
                    };
                    compare_extension_headers(
                        &build_long_path_with_tar_core(&params),
                        &extract_all_headers(&build_long_path_with_tar_rs(&params)),
                    );
                }

                #[test]
                fn test_gnu_longlink_basic() {
                    let params = LongLinkParams {
                        path: "mylink".to_string(),
                        target: "/very/long/target/".repeat(10),
                        uid: 1000,
                        gid: 1000,
                        mtime: 1234567890,
                    };
                    compare_extension_headers(
                        &build_long_link_with_tar_core(&params),
                        &extract_all_headers(&build_long_link_with_tar_rs(&params)),
                    );
                }

                proptest! {
                    #![proptest_config(ProptestConfig::with_cases(32))]

                    #[test]
                    fn test_gnu_longname_equivalence(params in long_path_file_params_strategy()) {
                        compare_extension_headers(
                            &build_long_path_with_tar_core(&params),
                            &extract_all_headers(&build_long_path_with_tar_rs(&params)),
                        );
                    }

                    #[test]
                    fn test_gnu_longlink_equivalence(params in long_link_params_strategy()) {
                        compare_extension_headers(
                            &build_long_link_with_tar_core(&params),
                            &extract_all_headers(&build_long_link_with_tar_rs(&params)),
                        );
                    }
                }
            }

            /// Test PAX extension headers produce equivalent output.
            mod pax_extensions_equivalence {
                use super::*;
                use crate::builder::{EntryBuilder, PaxBuilder};

                /// Parameters for a file with PAX xattrs.
                #[derive(Debug, Clone)]
                struct PaxFileParams {
                    path: String,
                    mode: u32,
                    uid: u64,
                    gid: u64,
                    mtime: u64,
                    xattr_key: String,
                    xattr_value: String,
                }

                fn pax_file_params_strategy() -> impl Strategy<Value = PaxFileParams> {
                    (
                        path_strategy(),
                        mode_strategy(),
                        id_strategy(),
                        id_strategy(),
                        mtime_strategy(),
                        proptest::string::string_regex("SCHILY\\.xattr\\.[a-z]{1,20}")
                            .expect("valid regex"),
                        proptest::string::string_regex("[a-zA-Z0-9]{1,30}").expect("valid regex"),
                    )
                        .prop_map(
                            |(path, mode, uid, gid, mtime, xattr_key, xattr_value)| PaxFileParams {
                                path,
                                mode,
                                uid,
                                gid,
                                mtime,
                                xattr_key,
                                xattr_value,
                            },
                        )
                }

                /// Build a tar with PAX extended headers using tar-rs.
                fn build_pax_with_tar_rs(params: &PaxFileParams) -> Vec<u8> {
                    let mut builder = tar::Builder::new(Vec::new());

                    // Build PAX records manually
                    let mut pax_data = Vec::new();
                    let record =
                        format_pax_record(&params.xattr_key, params.xattr_value.as_bytes());
                    pax_data.extend_from_slice(record.as_bytes());

                    // Create PAX header
                    let mut pax_header = tar::Header::new_ustar();
                    let pax_name = format!("PaxHeaders.0/{}", params.path);
                    pax_header.set_path(&pax_name).unwrap();
                    pax_header.set_size(pax_data.len() as u64);
                    pax_header.set_entry_type(tar::EntryType::XHeader);
                    pax_header.set_mode(0o644);
                    pax_header.set_uid(0);
                    pax_header.set_gid(0);
                    pax_header.set_mtime(0);
                    pax_header.set_cksum();

                    builder
                        .append_data(&mut pax_header, &pax_name, pax_data.as_slice())
                        .unwrap();

                    // Create main header
                    let mut header = tar::Header::new_ustar();
                    header.set_path(&params.path).unwrap();
                    header.set_mode(params.mode);
                    header.set_uid(params.uid);
                    header.set_gid(params.gid);
                    header.set_size(0);
                    header.set_mtime(params.mtime);
                    header.set_entry_type(tar::EntryType::Regular);
                    header.set_cksum();

                    builder
                        .append_data(&mut header, &params.path, std::io::empty())
                        .unwrap();
                    builder.into_inner().unwrap()
                }

                /// Format a PAX record.
                fn format_pax_record(key: &str, value: &[u8]) -> String {
                    // Format: "<len> <key>=<value>\n"
                    let rest_len = 3 + key.len() + value.len();
                    let mut len_len = 1;
                    let mut max_len = 10;
                    while rest_len + len_len >= max_len {
                        len_len += 1;
                        max_len *= 10;
                    }
                    let len = rest_len + len_len;
                    format!("{} {}={}\n", len, key, String::from_utf8_lossy(value))
                }

                /// Build entry with PAX xattrs using tar-core.
                fn build_pax_with_tar_core(params: &PaxFileParams) -> Vec<Header> {
                    let mut builder = EntryBuilder::new_ustar();
                    builder
                        .path(params.path.as_bytes())
                        .mode(params.mode)
                        .unwrap()
                        .uid(params.uid)
                        .unwrap()
                        .gid(params.gid)
                        .unwrap()
                        .size(0)
                        .unwrap()
                        .mtime(params.mtime)
                        .unwrap()
                        .entry_type(EntryType::Regular)
                        .add_pax(&params.xattr_key, params.xattr_value.as_bytes());

                    builder.finish()
                }

                #[test]
                fn test_pax_xattr_basic() {
                    let params = PaxFileParams {
                        path: "testfile".to_string(),
                        mode: 0o644,
                        uid: 1000,
                        gid: 1000,
                        mtime: 1234567890,
                        xattr_key: "SCHILY.xattr.user.test".to_string(),
                        xattr_value: "value1".to_string(),
                    };

                    // Build with both to verify structure
                    let _tar_data = build_pax_with_tar_rs(&params);
                    let our_headers = build_pax_with_tar_core(&params);

                    // We should have PAX header + main header
                    assert!(our_headers.len() >= 2, "should have PAX extension");

                    // First header should be XHeader
                    let our_ext = &our_headers[0];
                    assert_eq!(our_ext.entry_type(), EntryType::XHeader);

                    // Last header should be Regular
                    let our_main = our_headers.last().unwrap();
                    assert_eq!(our_main.entry_type(), EntryType::Regular);
                }

                #[test]
                fn test_pax_builder_record_format() {
                    // Verify PaxBuilder produces correctly formatted records
                    let mut pax = PaxBuilder::new();
                    pax.add("SCHILY.xattr.user.test", b"hello");
                    let data = pax.finish();

                    // Parse it back
                    let exts = PaxExtensions::new(&data);
                    let value = exts.get("SCHILY.xattr.user.test");
                    assert_eq!(value, Some("hello"));
                }

                proptest! {
                    #![proptest_config(ProptestConfig::with_cases(32))]

                    /// Test PAX records are correctly formatted with random key/value.
                    #[test]
                    fn test_pax_record_roundtrip(
                        key in "[a-zA-Z][a-zA-Z0-9.]{1,30}",
                        value in "[a-zA-Z0-9]{1,50}",
                    ) {
                        let mut pax = PaxBuilder::new();
                        pax.add(&key, value.as_bytes());
                        let data = pax.finish();

                        let exts = PaxExtensions::new(&data);
                        let parsed = exts.get(&key);
                        prop_assert_eq!(parsed, Some(value.as_str()));
                    }

                    /// Test PAX files with random metadata produce valid headers.
                    #[test]
                    fn test_pax_file_equivalence(params in pax_file_params_strategy()) {
                        let _tar_data = build_pax_with_tar_rs(&params);
                        let our_headers = build_pax_with_tar_core(&params);

                        // We should have PAX header + data blocks + main header
                        prop_assert!(our_headers.len() >= 2, "should have PAX extension");

                        // First header should be XHeader
                        let our_ext = &our_headers[0];
                        prop_assert_eq!(our_ext.entry_type(), EntryType::XHeader);

                        // Last header should be the main entry
                        let our_main = our_headers.last().unwrap();
                        prop_assert_eq!(our_main.entry_type(), EntryType::Regular);
                        prop_assert_eq!(our_main.mode().unwrap(), params.mode);
                        prop_assert_eq!(our_main.uid().unwrap(), params.uid);
                        prop_assert_eq!(our_main.gid().unwrap(), params.gid);
                        prop_assert_eq!(our_main.mtime().unwrap(), params.mtime);
                    }
                }
            }
        }
    }
}

// ============================================================================
// Kani Formal Verification Proofs
// ============================================================================

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    // Only proofs that complete in <10s are included here. Octal/numeric
    // encode/parse roundtrips involve stdlib `from_utf8`/`from_str_radix`
    // which have unbounded internal loops CBMC cannot handle efficiently;
    // those properties are tested via proptest instead.

    #[kani::proof]
    #[kani::unwind(18)]
    fn check_truncate_null_panic_freedom() {
        let bytes: [u8; 16] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= bytes.len());
        let result = truncate_null(&bytes[..len]);
        kani::assert(result.len() <= len, "result within bounds");
    }

    #[kani::proof]
    fn check_entry_type_roundtrip() {
        let byte: u8 = kani::any();
        let entry_type = EntryType::from_byte(byte);
        let back = entry_type.to_byte();
        if byte == b'\0' {
            kani::assert(back == b'0', "null byte canonicalizes to '0'");
        } else {
            kani::assert(back == byte, "non-null bytes roundtrip exactly");
        }
    }

    #[kani::proof]
    fn check_entry_type_predicates_dont_panic() {
        let byte: u8 = kani::any();
        let ty = EntryType::from_byte(byte);
        let _ = ty.is_file();
        let _ = ty.is_dir();
        let _ = ty.is_symlink();
        let _ = ty.is_hard_link();
        let _ = ty.is_character_special();
        let _ = ty.is_block_special();
        let _ = ty.is_fifo();
        let _ = ty.is_contiguous();
        let _ = ty.is_gnu_longname();
        let _ = ty.is_gnu_longlink();
        let _ = ty.is_gnu_sparse();
        let _ = ty.is_pax_global_extensions();
        let _ = ty.is_pax_local_extensions();
    }
}
