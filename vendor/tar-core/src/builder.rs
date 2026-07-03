//! Builder pattern for creating tar headers.
//!
//! This module provides [`HeaderBuilder`] for constructing tar headers and
//! [`PaxBuilder`] for creating PAX extended header records.
//!
//! # Example
//!
//! ```
//! use tar_core::builder::HeaderBuilder;
//! use tar_core::EntryType;
//!
//! let header = HeaderBuilder::new_ustar()
//!     .path(b"hello.txt").unwrap()
//!     .mode(0o644).unwrap()
//!     .size(1024).unwrap()
//!     .entry_type(EntryType::Regular)
//!     .finish();
//! ```

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::{
    EntryType, GnuExtSparseHeader, Header, HeaderError, Result, SparseEntry, UstarHeader,
    HEADER_SIZE, PAX_ATIME, PAX_CTIME, PAX_GID, PAX_GNAME, PAX_GNU_SPARSE_MAJOR,
    PAX_GNU_SPARSE_MINOR, PAX_GNU_SPARSE_NAME, PAX_GNU_SPARSE_REALSIZE, PAX_LINKPATH, PAX_MTIME,
    PAX_PATH, PAX_SIZE, PAX_UID, PAX_UNAME,
};

/// Stack-allocated decimal formatter for u64.
///
/// Formats a u64 into a fixed-size byte buffer without allocating.
/// Max u64 is 20 digits ("18446744073709551615").
pub(crate) struct DecU64 {
    buf: [u8; 20],
    start: u8,
}

impl DecU64 {
    pub(crate) fn new(mut value: u64) -> Self {
        let mut buf = [0u8; 20];
        if value == 0 {
            buf[19] = b'0';
            return Self { buf, start: 19 };
        }
        let mut pos = 20u8;
        while value > 0 {
            pos -= 1;
            buf[pos as usize] = b'0' + (value % 10) as u8;
            value /= 10;
        }
        Self { buf, start: pos }
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.buf[self.start as usize..]
    }
}

/// Write a byte slice into a fixed-size header field.
///
/// The const generic `N` carries the field size from the zerocopy header
/// struct, so the compiler knows the capacity at each call site.
///
/// # Errors
///
/// Returns [`HeaderError::FieldOverflow`] if the value is too long for the field.
fn write_bytes<const N: usize>(field: &mut [u8; N], value: &[u8]) -> Result<()> {
    if value.len() > N {
        return Err(HeaderError::FieldOverflow {
            field_len: N,
            detail: format!("{}-byte value", value.len()),
        });
    }
    field.fill(0);
    field[..value.len()].copy_from_slice(value);
    Ok(())
}

/// Builder for creating tar headers.
///
/// This provides a fluent API for constructing tar headers with proper
/// field formatting and checksum calculation.
///
/// # Example
///
/// ```
/// use tar_core::builder::HeaderBuilder;
/// use tar_core::EntryType;
///
/// let mut builder = HeaderBuilder::new_ustar();
/// builder
///     .path(b"example.txt").unwrap()
///     .mode(0o644).unwrap()
///     .uid(1000).unwrap()
///     .gid(1000).unwrap()
///     .size(0).unwrap()
///     .mtime(1234567890).unwrap()
///     .entry_type(EntryType::Regular);
///
/// let header = builder.finish();
/// ```
#[derive(Clone)]
pub struct HeaderBuilder {
    header: Header,
}

impl HeaderBuilder {
    /// Create a new builder for a UStar format header.
    #[must_use]
    pub fn new_ustar() -> Self {
        Self {
            header: Header::new_ustar(),
        }
    }

    /// Create a new builder for a GNU tar format header.
    #[must_use]
    pub fn new_gnu() -> Self {
        Self {
            header: Header::new_gnu(),
        }
    }

    /// Mutable access to the header viewed as a UstarHeader.
    ///
    /// Both UStar and GNU formats share the same field layout for the
    /// common fields (name, mode, uid, gid, size, mtime, checksum,
    /// typeflag, linkname, uname, gname, dev_major, dev_minor).
    fn fields_mut(&mut self) -> &mut UstarHeader {
        self.header.as_ustar_mut()
    }

    /// Set the file path (name field, 100 bytes).
    ///
    /// # Errors
    ///
    /// Returns an error if the path is longer than 100 bytes.
    pub fn path(&mut self, path: impl AsRef<[u8]>) -> Result<&mut Self> {
        write_bytes(&mut self.fields_mut().name, path.as_ref())?;
        Ok(self)
    }

    /// Set the file mode.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::FieldOverflow`] if the mode exceeds the 8-byte
    /// octal capacity (max 0o7777777 = 2,097,151).
    pub fn mode(&mut self, mode: u32) -> Result<&mut Self> {
        self.header.set_mode(mode)?;
        Ok(self)
    }

    /// Set the owner user ID.
    ///
    /// Format-aware: GNU headers use base-256 for values exceeding the
    /// octal range; ustar headers use octal only.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::FieldOverflow`] if the value cannot be
    /// represented in the header format.
    pub fn uid(&mut self, uid: u64) -> Result<&mut Self> {
        self.header.set_uid(uid)?;
        Ok(self)
    }

    /// Set the owner group ID.
    ///
    /// Format-aware: GNU headers use base-256 for values exceeding the
    /// octal range; ustar headers use octal only.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::FieldOverflow`] if the value cannot be
    /// represented in the header format.
    pub fn gid(&mut self, gid: u64) -> Result<&mut Self> {
        self.header.set_gid(gid)?;
        Ok(self)
    }

    /// Set the file size.
    ///
    /// Format-aware: GNU headers use base-256 for values exceeding the
    /// octal range; ustar headers use octal only.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::FieldOverflow`] if the value cannot be
    /// represented in the header format.
    pub fn size(&mut self, size: u64) -> Result<&mut Self> {
        self.header.set_size(size)?;
        Ok(self)
    }

    /// Set the modification time as a Unix timestamp.
    ///
    /// Format-aware: GNU headers use base-256 for values exceeding the
    /// octal range; ustar headers use octal only.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::FieldOverflow`] if the value cannot be
    /// represented in the header format.
    pub fn mtime(&mut self, mtime: u64) -> Result<&mut Self> {
        self.header.set_mtime(mtime)?;
        Ok(self)
    }

    /// Set the entry type.
    pub fn entry_type(&mut self, entry_type: EntryType) -> &mut Self {
        self.fields_mut().typeflag[0] = entry_type.to_byte();
        self
    }

    /// Set the link name for symbolic/hard links (100 bytes).
    ///
    /// # Errors
    ///
    /// Returns an error if the link name is longer than 100 bytes.
    pub fn link_name(&mut self, link: impl AsRef<[u8]>) -> Result<&mut Self> {
        write_bytes(&mut self.fields_mut().linkname, link.as_ref())?;
        Ok(self)
    }

    /// Set the owner user name (32 bytes, UStar/GNU only).
    ///
    /// # Errors
    ///
    /// Returns an error if the username is longer than 32 bytes.
    pub fn username(&mut self, name: impl AsRef<[u8]>) -> Result<&mut Self> {
        write_bytes(&mut self.fields_mut().uname, name.as_ref())?;
        Ok(self)
    }

    /// Set the owner group name (32 bytes, UStar/GNU only).
    ///
    /// # Errors
    ///
    /// Returns an error if the group name is longer than 32 bytes.
    pub fn groupname(&mut self, name: impl AsRef<[u8]>) -> Result<&mut Self> {
        write_bytes(&mut self.fields_mut().gname, name.as_ref())?;
        Ok(self)
    }

    /// Set device major and minor numbers.
    ///
    /// Used for character and block device entries.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError::FieldOverflow`] if the values don't fit in
    /// the 8-byte octal fields (max 0o7777777 = 2,097,151).
    pub fn device(&mut self, major: u32, minor: u32) -> Result<&mut Self> {
        self.header.set_device(major, minor)?;
        Ok(self)
    }

    /// Set the UStar prefix field for long paths (155 bytes).
    ///
    /// # Errors
    ///
    /// Returns an error if the prefix is longer than 155 bytes.
    pub fn prefix(&mut self, prefix: impl AsRef<[u8]>) -> Result<&mut Self> {
        write_bytes(&mut self.fields_mut().prefix, prefix.as_ref())?;
        Ok(self)
    }

    /// Get a reference to the current header for inspection.
    ///
    /// Note: The checksum field will not be valid until [`finish`](Self::finish)
    /// is called.
    #[must_use]
    pub fn as_header(&self) -> &Header {
        &self.header
    }

    /// Get a mutable reference to the raw header.
    ///
    /// This is intended for direct field manipulation that the builder
    /// doesn't expose (e.g. GNU sparse header fields).
    pub fn as_header_mut(&mut self) -> &mut Header {
        &mut self.header
    }

    /// Compute the checksum and return the final header.
    ///
    /// This fills in the checksum field and returns the complete 512-byte
    /// header.
    #[must_use]
    pub fn finish(&mut self) -> Header {
        // Fill checksum field with spaces for calculation
        self.header.as_ustar_mut().cksum.fill(b' ');

        // Compute unsigned sum of all bytes
        let checksum: u64 = self.header.as_bytes().iter().map(|&b| u64::from(b)).sum();

        // Max checksum = 512 * 255 = 130560, which always fits in 8-byte octal
        // (max representable: 07777777 = 2097151).
        crate::encode_octal(&mut self.header.as_ustar_mut().cksum, checksum)
            .expect("checksum always fits in 8-byte octal field");

        self.header
    }
}

impl Default for HeaderBuilder {
    fn default() -> Self {
        Self::new_ustar()
    }
}

impl core::fmt::Debug for HeaderBuilder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HeaderBuilder")
            .field("header", self.as_header())
            .finish()
    }
}

/// Builder for PAX extended header records.
///
/// PAX extended headers contain key-value pairs that extend the basic
/// tar header format, allowing for longer paths, larger file sizes,
/// and additional metadata.
///
/// # Format
///
/// Each record has the format: `<length> <key>=<value>\n`
/// where `<length>` is the total record length including the length field itself.
///
/// # Example
///
/// ```
/// use tar_core::builder::PaxBuilder;
///
/// let mut builder = PaxBuilder::new();
/// builder
///     .path(b"/very/long/path/that/exceeds/100/characters/limit.txt")
///     .size(1_000_000_000_000);
/// let data = builder.finish();
/// ```
#[derive(Clone, Default)]
pub struct PaxBuilder {
    data: Vec<u8>,
}

impl PaxBuilder {
    /// Create a new empty PAX builder.
    #[must_use]
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Add a key-value record.
    ///
    /// The record is formatted as `<length> <key>=<value>\n`.
    ///
    /// The length prefix includes itself, which requires computing how many
    /// decimal digits the total length will occupy. This uses the same
    /// algorithm as tar-rs's `append_pax_extensions`.
    pub fn add(&mut self, key: &str, value: impl AsRef<[u8]>) -> &mut Self {
        let value = value.as_ref();
        // Format: "<len> <key>=<value>\n"
        // rest_len covers: " " + key + "=" + value + "\n"
        let rest_len = 3 + key.len() + value.len();

        // The length prefix includes itself, so we need to figure out how
        // many digits the total length will be. Start assuming 1 digit and
        // widen until it fits.
        let mut len_len = 1;
        let mut max_len = 10;
        while rest_len + len_len >= max_len {
            len_len += 1;
            max_len *= 10;
        }
        let total_len = rest_len + len_len;

        let len_dec = DecU64::new(total_len as u64);
        self.data.extend_from_slice(len_dec.as_bytes());
        self.data.push(b' ');
        self.data.extend_from_slice(key.as_bytes());
        self.data.push(b'=');
        self.data.extend_from_slice(value);
        self.data.push(b'\n');

        self
    }

    /// Add a path record.
    pub fn path(&mut self, path: impl AsRef<[u8]>) -> &mut Self {
        self.add(PAX_PATH, path)
    }

    /// Add a linkpath record.
    pub fn linkpath(&mut self, path: impl AsRef<[u8]>) -> &mut Self {
        self.add(PAX_LINKPATH, path)
    }

    /// Add a record with a u64 value formatted as decimal.
    pub fn add_u64(&mut self, key: &str, value: u64) -> &mut Self {
        let buf = DecU64::new(value);
        self.add(key, buf.as_bytes())
    }

    /// Add a size record.
    pub fn size(&mut self, size: u64) -> &mut Self {
        self.add_u64(PAX_SIZE, size)
    }

    /// Add a uid record.
    pub fn uid(&mut self, uid: u64) -> &mut Self {
        self.add_u64(PAX_UID, uid)
    }

    /// Add a gid record.
    pub fn gid(&mut self, gid: u64) -> &mut Self {
        self.add_u64(PAX_GID, gid)
    }

    /// Add a uname (username) record.
    pub fn uname(&mut self, name: impl AsRef<[u8]>) -> &mut Self {
        self.add(PAX_UNAME, name)
    }

    /// Add a gname (group name) record.
    pub fn gname(&mut self, name: impl AsRef<[u8]>) -> &mut Self {
        self.add(PAX_GNAME, name)
    }

    /// Add an mtime record.
    pub fn mtime(&mut self, mtime: u64) -> &mut Self {
        self.add_u64(PAX_MTIME, mtime)
    }

    /// Add an atime record.
    pub fn atime(&mut self, atime: u64) -> &mut Self {
        self.add_u64(PAX_ATIME, atime)
    }

    /// Add a ctime record.
    pub fn ctime(&mut self, ctime: u64) -> &mut Self {
        self.add_u64(PAX_CTIME, ctime)
    }

    /// Get the current data (for inspection).
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Return the finished PAX extended header data.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.data
    }
}

impl core::fmt::Debug for PaxBuilder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PaxBuilder")
            .field("data", &String::from_utf8_lossy(&self.data))
            .finish()
    }
}

// ============================================================================
// Entry Builder
// ============================================================================

/// How to handle long paths and other extensions.
///
/// When paths exceed 100 bytes or link targets exceed 100 bytes, tar archives
/// use extension mechanisms to store the full values. This enum selects which
/// mechanism to use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExtensionMode {
    /// Use GNU extensions (LongLink/LongName pseudo-entries).
    ///
    /// This emits a pseudo-entry with typeflag 'L' (for long names) or 'K'
    /// (for long link targets), followed by the actual entry with a truncated
    /// name. This is widely compatible with GNU tar.
    #[default]
    Gnu,
    /// Use PAX extensions (extended headers).
    ///
    /// This emits a PAX extended header (typeflag 'x') containing the full
    /// path/linkpath, followed by the actual entry. This is the POSIX.1-2001
    /// standard approach.
    Pax,
}

/// Maximum length for the name field in a tar header.
pub const NAME_MAX_LEN: usize = 100;

/// Maximum length for the linkname field in a tar header.
pub const LINKNAME_MAX_LEN: usize = 100;

/// The canonical name used for GNU long link/name pseudo-entries.
const GNU_LONGLINK_NAME: &[u8] = b"././@LongLink";

/// Builder for complete tar entries including extension headers.
///
/// This handles the complexity of emitting multiple headers when paths
/// or link targets exceed the 100-byte limit. It supports both GNU
/// (LongLink/LongName) and PAX extension mechanisms.
///
/// # Sans-IO Design
///
/// This builder does not perform any I/O. It produces `Vec<Header>` blocks
/// or contiguous `Vec<u8>` that can be written to any output.
///
/// # Extension Handling
///
/// - **Short paths** (≤100 bytes): Single header, no extensions needed
/// - **Long paths** (>100 bytes): Extension header + data blocks + main header
/// - **Long link targets**: Same as long paths, using appropriate extension
///
/// # Example
///
/// ```
/// use tar_core::builder::{EntryBuilder, ExtensionMode};
/// use tar_core::EntryType;
///
/// // Create a simple entry (short path)
/// let mut builder = EntryBuilder::new_gnu();
/// builder
///     .path(b"hello.txt")
///     .mode(0o644).unwrap()
///     .size(1024).unwrap()
///     .entry_type(EntryType::Regular);
/// let blocks = builder.finish();
/// assert_eq!(blocks.len(), 1); // Just one header block
///
/// // Create an entry with a long path
/// let long_path = "a/".repeat(60) + "file.txt";
/// let mut builder = EntryBuilder::new_gnu();
/// builder
///     .path(long_path.as_bytes())
///     .mode(0o644).unwrap()
///     .size(0).unwrap()
///     .entry_type(EntryType::Regular);
/// let blocks = builder.finish();
/// assert!(blocks.len() > 1); // Extension header(s) + main header
/// ```
#[derive(Clone)]
pub struct EntryBuilder {
    /// The primary header builder.
    header: HeaderBuilder,
    /// Long path (if > 100 bytes).
    long_path: Option<Vec<u8>>,
    /// Long link target (if > 100 bytes).
    long_link: Option<Vec<u8>>,
    /// PAX extensions builder (used when mode is Pax).
    pax: Option<PaxBuilder>,
    /// Extension mode preference.
    mode: ExtensionMode,
    /// Sparse file map, if this is a sparse entry.
    sparse: Option<SparseInfo>,
}

/// Sparse file metadata for the builder.
#[derive(Clone)]
struct SparseInfo {
    /// The sparse data map: regions of real data within the logical file.
    map: Vec<SparseEntry>,
    /// Logical file size (the file appears this large to readers).
    real_size: u64,
}

impl EntryBuilder {
    /// Create a new builder using GNU tar format for the underlying header.
    ///
    /// This sets the extension mode to GNU (LongLink/LongName).
    #[must_use]
    pub fn new_gnu() -> Self {
        Self {
            header: HeaderBuilder::new_gnu(),
            long_path: None,
            long_link: None,
            pax: None,
            mode: ExtensionMode::Gnu,
            sparse: None,
        }
    }

    /// Create a new builder using UStar format for the underlying header.
    ///
    /// This sets the extension mode to PAX (extended headers).
    #[must_use]
    pub fn new_ustar() -> Self {
        Self {
            header: HeaderBuilder::new_ustar(),
            long_path: None,
            long_link: None,
            pax: None,
            mode: ExtensionMode::Pax,
            sparse: None,
        }
    }

    /// Create a new builder with explicit format and extension mode.
    #[must_use]
    pub fn with_mode(header: HeaderBuilder, mode: ExtensionMode) -> Self {
        Self {
            header,
            long_path: None,
            long_link: None,
            pax: None,
            mode,
            sparse: None,
        }
    }

    /// Get the current extension mode.
    #[must_use]
    pub fn extension_mode(&self) -> ExtensionMode {
        self.mode
    }

    /// Set the extension mode.
    pub fn set_extension_mode(&mut self, mode: ExtensionMode) -> &mut Self {
        self.mode = mode;
        self
    }

    /// Set the file path.
    ///
    /// If the path exceeds 100 bytes, it will be stored using the configured
    /// extension mechanism (GNU or PAX). The main header's name field will
    /// contain a truncated version (first 100 bytes, matching GNU tar).
    pub fn path(&mut self, path: impl AsRef<[u8]>) -> &mut Self {
        let path = path.as_ref();
        if path.len() > NAME_MAX_LEN {
            self.long_path = Some(path.to_vec());
            // Store the first 100 bytes in the main header for compatibility
            // (matches GNU tar behavior)
            let truncated = &path[..NAME_MAX_LEN];
            self.header
                .path(truncated)
                .expect("truncated path fits in name field");
        } else {
            self.long_path = None;
            self.header
                .path(path)
                .expect("path within NAME_MAX_LEN fits in name field");
        }
        self
    }

    /// Set the link target for symbolic/hard links.
    ///
    /// If the link target exceeds 100 bytes, it will be stored using the
    /// configured extension mechanism.
    pub fn link_name(&mut self, link: impl AsRef<[u8]>) -> &mut Self {
        let link = link.as_ref();
        if link.len() > LINKNAME_MAX_LEN {
            self.long_link = Some(link.to_vec());
            let truncated = &link[..LINKNAME_MAX_LEN];
            self.header
                .link_name(truncated)
                .expect("truncated link fits in linkname field");
        } else {
            self.long_link = None;
            self.header
                .link_name(link)
                .expect("link within LINKNAME_MAX_LEN fits in linkname field");
        }
        self
    }

    /// Set the file mode (permissions).
    ///
    /// # Errors
    ///
    /// Returns an error if the mode exceeds the maximum value for the 7-digit
    /// octal field (0o7777777 / 2097151). Standard Unix permission modes
    /// (up to 0o7777) always fit.
    pub fn mode(&mut self, mode: u32) -> Result<&mut Self> {
        self.header.mode(mode)?;
        Ok(self)
    }

    /// Set the owner user ID.
    ///
    /// If the value overflows the header field and this builder uses PAX
    /// extensions, the value is stored as a PAX record instead (with 0
    /// written to the header field for compatibility).
    ///
    /// # Errors
    ///
    /// Returns an error only if the value overflows and PAX fallback is
    /// not available (GNU mode with value >= 2^63).
    pub fn uid(&mut self, uid: u64) -> Result<&mut Self> {
        match self.header.uid(uid) {
            Ok(_) => Ok(self),
            Err(_) if self.mode == ExtensionMode::Pax => {
                self.header.uid(0).expect("zero fits");
                self.pax_mut().uid(uid);
                Ok(self)
            }
            Err(e) => Err(e),
        }
    }

    /// Set the owner group ID.
    ///
    /// If the value overflows the header field and this builder uses PAX
    /// extensions, the value is stored as a PAX record instead (with 0
    /// written to the header field for compatibility).
    ///
    /// # Errors
    ///
    /// Returns an error only if the value overflows and PAX fallback is
    /// not available (GNU mode with value >= 2^63).
    pub fn gid(&mut self, gid: u64) -> Result<&mut Self> {
        match self.header.gid(gid) {
            Ok(_) => Ok(self),
            Err(_) if self.mode == ExtensionMode::Pax => {
                self.header.gid(0).expect("zero fits");
                self.pax_mut().gid(gid);
                Ok(self)
            }
            Err(e) => Err(e),
        }
    }

    /// Set the file size.
    ///
    /// If the value overflows the header field and this builder uses PAX
    /// extensions, the value is stored as a PAX record instead (with 0
    /// written to the header field for compatibility).
    ///
    /// # Errors
    ///
    /// Returns an error only if the value overflows and PAX fallback is
    /// not available (GNU mode with value >= 2^63).
    pub fn size(&mut self, size: u64) -> Result<&mut Self> {
        match self.header.size(size) {
            Ok(_) => Ok(self),
            Err(_) if self.mode == ExtensionMode::Pax => {
                self.header.size(0).expect("zero fits");
                self.pax_mut().size(size);
                Ok(self)
            }
            Err(e) => Err(e),
        }
    }

    /// Set the modification time as a Unix timestamp.
    ///
    /// If the value overflows the header field and this builder uses PAX
    /// extensions, the value is stored as a PAX record instead (with 0
    /// written to the header field for compatibility).
    ///
    /// # Errors
    ///
    /// Returns an error only if the value overflows and PAX fallback is
    /// not available (GNU mode with value >= 2^63).
    pub fn mtime(&mut self, mtime: u64) -> Result<&mut Self> {
        match self.header.mtime(mtime) {
            Ok(_) => Ok(self),
            Err(_) if self.mode == ExtensionMode::Pax => {
                self.header.mtime(0).expect("zero fits");
                self.pax_mut().mtime(mtime);
                Ok(self)
            }
            Err(e) => Err(e),
        }
    }

    /// Set the entry type.
    pub fn entry_type(&mut self, entry_type: EntryType) -> &mut Self {
        self.header.entry_type(entry_type);
        self
    }

    /// Set the owner user name.
    ///
    /// If the name exceeds the 32-byte header field and this builder uses
    /// PAX extensions, the full name is stored as a PAX `uname` record
    /// (with the header field zeroed for compatibility).
    ///
    /// # Errors
    ///
    /// Returns an error only if the name overflows and PAX fallback is
    /// not available (GNU mode).
    pub fn username(&mut self, name: impl AsRef<[u8]>) -> Result<&mut Self> {
        let name = name.as_ref();
        match self.header.username(name) {
            Ok(_) => Ok(self),
            Err(_) if self.mode == ExtensionMode::Pax => {
                // Zero the header field and store full name in PAX
                self.header.username([]).expect("empty fits");
                self.pax_mut().uname(name);
                Ok(self)
            }
            Err(e) => Err(e),
        }
    }

    /// Set the owner group name.
    ///
    /// If the name exceeds the 32-byte header field and this builder uses
    /// PAX extensions, the full name is stored as a PAX `gname` record
    /// (with the header field zeroed for compatibility).
    ///
    /// # Errors
    ///
    /// Returns an error only if the name overflows and PAX fallback is
    /// not available (GNU mode).
    pub fn groupname(&mut self, name: impl AsRef<[u8]>) -> Result<&mut Self> {
        let name = name.as_ref();
        match self.header.groupname(name) {
            Ok(_) => Ok(self),
            Err(_) if self.mode == ExtensionMode::Pax => {
                self.header.groupname([]).expect("empty fits");
                self.pax_mut().gname(name);
                Ok(self)
            }
            Err(e) => Err(e),
        }
    }

    /// Set device major and minor numbers.
    ///
    /// Used for character and block device entries.
    ///
    /// # Errors
    ///
    /// Returns an error if the values don't fit in the header fields.
    pub fn device(&mut self, major: u32, minor: u32) -> Result<&mut Self> {
        self.header.device(major, minor)?;
        Ok(self)
    }

    /// Mark this entry as a sparse file.
    ///
    /// The `sparse_map` describes which regions of the logical file contain
    /// real data — gaps are implicitly zero-filled. The `real_size` is the
    /// logical file size as seen by readers.
    ///
    /// The caller must also call [`size()`](Self::size) with the on-disk
    /// content size (the sum of all sparse entry lengths).
    ///
    /// On [`finish()`](Self::finish), the builder emits format-appropriate
    /// sparse metadata:
    /// - **GNU mode**: Sets entry type to `GnuSparse`, writes inline
    ///   descriptors and extension blocks, sets `realsize` in the GNU header.
    /// - **PAX mode**: Adds `GNU.sparse.*` PAX extensions (v1.0 format)
    ///   and emits the sparse map as a data prefix block.
    pub fn sparse(&mut self, sparse_map: &[SparseEntry], real_size: u64) -> &mut Self {
        self.sparse = Some(SparseInfo {
            map: sparse_map.to_vec(),
            real_size,
        });
        self
    }

    /// Add a custom PAX extension record.
    ///
    /// This is useful for adding metadata that doesn't fit in standard
    /// header fields. The PAX extension will be emitted regardless of
    /// the extension mode setting.
    pub fn add_pax(&mut self, key: &str, value: impl AsRef<[u8]>) -> &mut Self {
        self.pax_mut().add(key, value);
        self
    }

    /// Get or create the PAX builder for this entry.
    fn pax_mut(&mut self) -> &mut PaxBuilder {
        self.pax.get_or_insert_with(PaxBuilder::new)
    }

    /// Get a reference to the underlying header builder.
    #[must_use]
    pub fn header(&self) -> &HeaderBuilder {
        &self.header
    }

    /// Get a mutable reference to the underlying header builder.
    pub fn header_mut(&mut self) -> &mut HeaderBuilder {
        &mut self.header
    }

    /// Check if this entry requires extension headers.
    #[must_use]
    pub fn needs_extension(&self) -> bool {
        self.long_path.is_some() || self.long_link.is_some() || self.pax.is_some()
    }

    /// Build the complete header sequence as a vector of 512-byte blocks.
    ///
    /// Returns all blocks needed for this entry's headers:
    /// - For short paths: just the main header (1 block)
    /// - For GNU long paths: LongName header + data blocks + main header
    /// - For PAX: extended header + data blocks + main header
    /// - For GNU sparse: above + extension blocks after the main header
    /// - For PAX sparse: PAX header includes `GNU.sparse.*` keys, plus
    ///   a sparse map data prefix block after the main header
    #[must_use]
    pub fn finish(&mut self) -> Vec<Header> {
        let sparse = self.sparse.take();
        let mut blocks = Vec::new();

        // Pre-compute the PAX sparse map data (if applicable) so we can
        // both adjust the header size and emit it after the main header
        // without redundant work.
        let pax_sparse_map_data: Option<Vec<u8>> = match (&sparse, self.mode) {
            (Some(si), ExtensionMode::Pax) => Some(Self::build_sparse_map_data(si)),
            _ => None,
        };

        match self.mode {
            ExtensionMode::Gnu => {
                // For GNU sparse, set entry type and write inline descriptors.
                if let Some(ref si) = sparse {
                    self.header.entry_type(EntryType::GnuSparse);
                    if let Some(gnu) = self.header.as_header_mut().try_as_gnu_mut() {
                        gnu.set_real_size(si.real_size);
                        for (i, entry) in si.map.iter().take(4).enumerate() {
                            gnu.sparse[i].set(entry);
                        }
                        gnu.set_is_extended(si.map.len() > 4);
                    }
                }

                // Emit GNU LongLink for long link targets first
                if let Some(ref long_link) = self.long_link {
                    self.emit_gnu_long_entry(&mut blocks, EntryType::GnuLongLink, long_link);
                }

                // Emit GNU LongName for long paths
                if let Some(ref long_path) = self.long_path {
                    self.emit_gnu_long_entry(&mut blocks, EntryType::GnuLongName, long_path);
                }
            }
            ExtensionMode::Pax => {
                // For PAX sparse v1.0, add sparse PAX extensions and adjust
                // the header size to include the sparse map data prefix.
                if let Some(ref si) = sparse {
                    let map_padded = pax_sparse_map_data
                        .as_ref()
                        .unwrap()
                        .len()
                        .next_multiple_of(HEADER_SIZE);

                    // Add the map prefix size to the header's size field.
                    // The caller already set size() to on_disk content size;
                    // we add the padded map prefix on top.
                    let current_size = self.header.as_header().entry_size().unwrap_or(0);
                    self.header
                        .size(current_size + map_padded as u64)
                        .expect("adjusted size fits");

                    let real_size_str = DecU64::new(si.real_size);
                    self.pax_mut().add(PAX_GNU_SPARSE_MAJOR, b"1");
                    self.pax_mut().add(PAX_GNU_SPARSE_MINOR, b"0");
                    self.pax_mut()
                        .add(PAX_GNU_SPARSE_REALSIZE, real_size_str.as_bytes());

                    // The real path goes into GNU.sparse.name; the header
                    // gets a synthetic path.
                    let real_path = self
                        .long_path
                        .take()
                        .unwrap_or_else(|| self.header.as_header().path_bytes().to_vec());
                    self.pax_mut().add(PAX_GNU_SPARSE_NAME, &real_path);

                    // Set a synthetic path in the header (following Go's convention).
                    let synthetic = b"GNUSparseFile.0/placeholder";
                    self.header
                        .path(synthetic)
                        .expect("synthetic sparse path fits");
                }

                // Build PAX data with long path/link if needed
                let pax_data = self.build_pax_data();
                if !pax_data.is_empty() {
                    self.emit_pax_entry(&mut blocks, &pax_data);
                }
            }
        }

        // Emit the main header (recomputes checksum)
        let main_header = self.header.finish();
        blocks.push(main_header);

        // Emit format-specific sparse metadata after the main header.
        if let Some(ref si) = sparse {
            match self.mode {
                ExtensionMode::Gnu => {
                    // GNU sparse extension blocks for entries beyond the
                    // first 4 inline descriptors.
                    self.emit_gnu_sparse_ext_blocks(&mut blocks, si);
                }
                ExtensionMode::Pax => {
                    // PAX v1.0 sparse map data prefix (padded to 512 bytes).
                    let map_data = pax_sparse_map_data.as_ref().unwrap();
                    let map_padded = map_data.len().next_multiple_of(HEADER_SIZE);
                    let mut buf = vec![0u8; map_padded];
                    buf[..map_data.len()].copy_from_slice(map_data);
                    for chunk in buf.chunks_exact(HEADER_SIZE) {
                        blocks.push(*Header::from_bytes(
                            chunk.try_into().expect("chunks_exact guarantees size"),
                        ));
                    }
                }
            }
        }

        blocks
    }

    /// Build the complete header sequence as contiguous bytes.
    ///
    /// This is a convenience method that flattens the block vector.
    #[must_use]
    pub fn finish_bytes(&mut self) -> Vec<u8> {
        let blocks = self.finish();
        let mut out = Vec::with_capacity(blocks.len() * HEADER_SIZE);
        for block in &blocks {
            out.extend_from_slice(block.as_bytes());
        }
        out
    }

    /// Emit a GNU LongLink/LongName pseudo-entry.
    fn emit_gnu_long_entry(&self, blocks: &mut Vec<Header>, entry_type: EntryType, data: &[u8]) {
        // The data is null-terminated in GNU format
        let data_with_null_len = data.len() + 1;

        // Build the header for the pseudo-entry.
        // All these values are constants or small enough to always fit.
        let mut ext_header = HeaderBuilder::new_gnu();
        ext_header
            .path(GNU_LONGLINK_NAME)
            .expect("GNU longlink name fits");
        ext_header.mode(0).expect("zero fits");
        ext_header.uid(0).expect("zero fits");
        ext_header.gid(0).expect("zero fits");
        ext_header
            .size(data_with_null_len as u64)
            .expect("extension data size fits");
        ext_header.mtime(0).expect("zero fits");
        ext_header.entry_type(entry_type);

        blocks.push(ext_header.finish());

        // Emit data blocks (null-terminated, padded to 512 bytes)
        let num_data_blocks = data_with_null_len.div_ceil(HEADER_SIZE);
        let mut data_buf = vec![0u8; num_data_blocks * HEADER_SIZE];
        data_buf[..data.len()].copy_from_slice(data);
        // Null terminator is already in place (vec initialized to 0)

        for chunk in data_buf.chunks_exact(HEADER_SIZE) {
            blocks.push(*Header::from_bytes(
                chunk.try_into().expect("chunks_exact guarantees size"),
            ));
        }
    }

    /// Build PAX extension data for long paths/links and custom extensions.
    fn build_pax_data(&self) -> Vec<u8> {
        let mut pax = self.pax.clone().unwrap_or_default();

        if let Some(ref long_path) = self.long_path {
            pax.path(long_path);
        }

        if let Some(ref long_link) = self.long_link {
            pax.linkpath(long_link);
        }

        pax.finish()
    }

    /// Emit a PAX extended header entry.
    fn emit_pax_entry(&self, blocks: &mut Vec<Header>, pax_data: &[u8]) {
        // Build a name for the PAX header (following tar conventions)
        // Format: "PaxHeaders.0/<truncated_name>"
        let pax_name = self.build_pax_header_name();

        // Build the PAX header.
        // All these values are constants or small enough to always fit.
        let mut pax_header = HeaderBuilder::new_ustar();
        pax_header.path(&pax_name).expect("PAX header name fits");
        pax_header.mode(0o644).expect("mode 0644 fits");
        pax_header.uid(0).expect("zero fits");
        pax_header.gid(0).expect("zero fits");
        pax_header
            .size(pax_data.len() as u64)
            .expect("PAX data size fits");
        pax_header.mtime(0).expect("zero fits");
        pax_header.entry_type(EntryType::XHeader);

        blocks.push(pax_header.finish());

        // Emit data blocks (padded to 512 bytes)
        let num_data_blocks = pax_data.len().div_ceil(HEADER_SIZE);
        let mut data_buf = vec![0u8; num_data_blocks * HEADER_SIZE];
        data_buf[..pax_data.len()].copy_from_slice(pax_data);

        for chunk in data_buf.chunks_exact(HEADER_SIZE) {
            blocks.push(*Header::from_bytes(
                chunk.try_into().expect("chunks_exact guarantees size"),
            ));
        }
    }

    /// Build the name for a PAX extended header.
    fn build_pax_header_name(&self) -> Vec<u8> {
        // Get the base name from the header's current path
        let path = self.header.as_header().path_bytes();
        let base_name = path.rsplit(|&b| b == b'/').next().unwrap_or(path);

        // Build: "PaxHeaders.0/<basename>" (truncated to fit)
        let mut name = b"PaxHeaders.0/".to_vec();
        let remaining = NAME_MAX_LEN.saturating_sub(name.len());
        let truncated_base = &base_name[..remaining.min(base_name.len())];
        name.extend_from_slice(truncated_base);

        name
    }

    /// Build the PAX v1.0 sparse map data prefix.
    ///
    /// Format: `<count>\n<offset>\n<length>\n...` (not yet padded).
    fn build_sparse_map_data(si: &SparseInfo) -> Vec<u8> {
        let mut data = format!("{}\n", si.map.len());
        for entry in &si.map {
            data.push_str(&format!("{}\n{}\n", entry.offset, entry.length));
        }
        data.into_bytes()
    }

    /// Emit GNU sparse extension blocks for entries beyond the first 4.
    fn emit_gnu_sparse_ext_blocks(&self, blocks: &mut Vec<Header>, si: &SparseInfo) {
        if si.map.len() <= 4 {
            return;
        }

        let remaining = &si.map[4..];
        let chunks: Vec<&[SparseEntry]> = remaining.chunks(21).collect();

        for (i, chunk) in chunks.iter().enumerate() {
            let is_last = i == chunks.len() - 1;
            let mut ext = GnuExtSparseHeader::default();
            for (j, entry) in chunk.iter().enumerate() {
                ext.sparse[j].set(entry);
            }
            ext.set_is_extended(!is_last);

            // Convert the extension block to a Header-sized block.
            let ext_bytes = zerocopy::IntoBytes::as_bytes(&ext);
            blocks.push(*Header::from_bytes(
                ext_bytes
                    .try_into()
                    .expect("GnuExtSparseHeader is 512 bytes"),
            ));
        }
    }
}

impl Default for EntryBuilder {
    fn default() -> Self {
        Self::new_gnu()
    }
}

impl core::fmt::Debug for EntryBuilder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EntryBuilder")
            .field("mode", &self.mode)
            .field("needs_extension", &self.needs_extension())
            .field("long_path_len", &self.long_path.as_ref().map(|p| p.len()))
            .field("long_link_len", &self.long_link.as_ref().map(|l| l.len()))
            .field("header", &self.header)
            .finish()
    }
}

/// Calculate the number of 512-byte blocks needed to store `size` bytes.
///
/// This is useful for calculating content block counts.
#[must_use]
pub const fn blocks_for_size(size: u64) -> u64 {
    size.div_ceil(HEADER_SIZE as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PaxExtensions;

    #[test]
    fn test_write_bytes() {
        let mut field = [0u8; 10];

        // Normal case
        write_bytes(&mut field, b"hello").unwrap();
        assert_eq!(&field[..5], b"hello");
        assert_eq!(field[5..], [0, 0, 0, 0, 0]);

        // Exact fit
        write_bytes(&mut field, b"0123456789").unwrap();
        assert_eq!(&field, b"0123456789");

        // Too long
        assert!(write_bytes(&mut field, b"12345678901").is_err());
    }

    #[test]
    fn test_encode_octal() {
        // 8-byte field (like mode)
        let mut field = [0u8; 8];

        crate::encode_octal(&mut field, 0o644).unwrap();
        assert_eq!(&field, b"0000644\0");

        crate::encode_octal(&mut field, 0o755).unwrap();
        assert_eq!(&field, b"0000755\0");

        crate::encode_octal(&mut field, 0).unwrap();
        assert_eq!(&field, b"0000000\0");

        // 12-byte field (like size)
        let mut field12 = [0u8; 12];
        crate::encode_octal(&mut field12, 0o77777777777).unwrap();
        assert_eq!(&field12, b"77777777777\0");

        // Test max value that fits
        crate::encode_octal(&mut field, 0o7777777).unwrap();
        assert_eq!(&field, b"7777777\0");
    }

    #[test]
    fn test_encode_octal_overflow() {
        let mut field = [0u8; 8];
        // Value too large for 7 octal digits
        assert!(crate::encode_octal(&mut field, 0o100000000).is_err());
    }

    #[test]
    fn test_header_builder_basic() {
        let builder = HeaderBuilder::new_ustar();
        let header = builder.as_header();
        assert!(header.is_ustar());
        assert!(!header.is_gnu());
    }

    #[test]
    fn test_header_builder_gnu() {
        let builder = HeaderBuilder::new_gnu();
        let header = builder.as_header();
        assert!(header.is_gnu());
        assert!(!header.is_ustar());
    }

    #[test]
    fn test_header_builder_file() {
        let mut builder = HeaderBuilder::new_ustar();
        builder
            .path(b"test.txt")
            .unwrap()
            .mode(0o644)
            .unwrap()
            .uid(1000)
            .unwrap()
            .gid(1000)
            .unwrap()
            .size(1024)
            .unwrap()
            .mtime(1234567890)
            .unwrap()
            .entry_type(EntryType::Regular)
            .username(b"user")
            .unwrap()
            .groupname(b"group")
            .unwrap();

        let header = builder.finish();

        assert_eq!(header.path_bytes(), b"test.txt");
        assert_eq!(header.mode().unwrap(), 0o644);
        assert_eq!(header.uid().unwrap(), 1000);
        assert_eq!(header.gid().unwrap(), 1000);
        assert_eq!(header.entry_size().unwrap(), 1024);
        assert_eq!(header.mtime().unwrap(), 1234567890);
        assert_eq!(header.entry_type(), EntryType::Regular);
        assert_eq!(header.username().unwrap(), b"user");
        assert_eq!(header.groupname().unwrap(), b"group");

        // Verify checksum
        assert!(header.verify_checksum().is_ok());
    }

    #[test]
    fn test_header_builder_symlink() {
        let mut builder = HeaderBuilder::new_ustar();
        builder
            .path(b"link")
            .unwrap()
            .mode(0o777)
            .unwrap()
            .entry_type(EntryType::Symlink)
            .link_name(b"target")
            .unwrap()
            .size(0)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap();

        let header = builder.finish();

        assert_eq!(header.path_bytes(), b"link");
        assert_eq!(header.entry_type(), EntryType::Symlink);
        assert_eq!(header.link_name_bytes(), b"target");
        assert!(header.verify_checksum().is_ok());
    }

    #[test]
    fn test_header_builder_directory() {
        let mut builder = HeaderBuilder::new_ustar();
        builder
            .path(b"mydir/")
            .unwrap()
            .mode(0o755)
            .unwrap()
            .entry_type(EntryType::Directory)
            .size(0)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap();

        let header = builder.finish();

        assert_eq!(header.entry_type(), EntryType::Directory);
        assert!(header.verify_checksum().is_ok());
    }

    #[test]
    fn test_header_builder_device() {
        let mut builder = HeaderBuilder::new_ustar();
        builder
            .path(b"null")
            .unwrap()
            .mode(0o666)
            .unwrap()
            .entry_type(EntryType::Char)
            .device(1, 3)
            .unwrap()
            .size(0)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap();

        let header = builder.finish();

        assert_eq!(header.entry_type(), EntryType::Char);
        assert_eq!(header.device_major().unwrap(), Some(1));
        assert_eq!(header.device_minor().unwrap(), Some(3));
        assert!(header.verify_checksum().is_ok());
    }

    #[test]
    fn test_pax_builder_basic() {
        let mut builder = PaxBuilder::new();
        builder.add("key", b"value").add("another", b"test");
        let data = builder.finish();

        // Parse it back
        let mut iter = PaxExtensions::new(&data);

        let ext1 = iter.next().unwrap().unwrap();
        assert_eq!(ext1.key().unwrap(), "key");
        assert_eq!(ext1.value().unwrap(), "value");

        let ext2 = iter.next().unwrap().unwrap();
        assert_eq!(ext2.key().unwrap(), "another");
        assert_eq!(ext2.value().unwrap(), "test");

        assert!(iter.next().is_none());
    }

    #[test]
    fn test_pax_builder_path() {
        let long_path = b"/very/long/path/that/exceeds/one/hundred/characters/which/is/the/limit/for/the/standard/tar/name/field.txt";

        let mut builder = PaxBuilder::new();
        builder.path(long_path);
        let data = builder.finish();

        let ext = PaxExtensions::new(&data).next().unwrap().unwrap();
        assert_eq!(ext.key().unwrap(), "path");
        assert_eq!(ext.value_bytes(), long_path);
    }

    #[test]
    fn test_pax_builder_size() {
        let mut builder = PaxBuilder::new();
        builder.size(1_000_000_000_000);
        let data = builder.finish();

        let exts = PaxExtensions::new(&data);
        assert_eq!(exts.get_u64("size"), Some(1_000_000_000_000));
    }

    #[test]
    fn test_pax_builder_multiple() {
        let mut builder = PaxBuilder::new();
        builder
            .path(b"/some/path")
            .uid(65534)
            .gid(65534)
            .uname(b"nobody")
            .gname(b"nogroup")
            .mtime(1700000000);
        let data = builder.finish();

        let exts = PaxExtensions::new(&data);
        assert_eq!(exts.get("path"), Some("/some/path"));
        assert_eq!(exts.get_u64("uid"), Some(65534));
        assert_eq!(exts.get_u64("gid"), Some(65534));
        assert_eq!(exts.get("uname"), Some("nobody"));
        assert_eq!(exts.get("gname"), Some("nogroup"));
        assert_eq!(exts.get_u64("mtime"), Some(1700000000));
    }

    #[test]
    fn test_pax_record_length_calculation() {
        // Test edge cases for length calculation
        // Record "9 k=v\n" has length 6, but we write "9" which is 1 digit
        // Actually need "6 k=v\n" which is length 6 - that works!

        let mut builder = PaxBuilder::new();
        builder.add("k", b"v");
        let data = builder.finish();
        assert_eq!(&data, b"6 k=v\n");

        // Longer key/value
        let mut builder = PaxBuilder::new();
        builder.add("path", b"/a/b/c/d/e/f");
        let data = builder.finish();
        // "XX path=/a/b/c/d/e/f\n" where XX is the length
        // Base: " path=/a/b/c/d/e/f\n" = 1 + 4 + 1 + 12 + 1 = 19
        // With "19": total 21, but we wrote 19... need 21
        // With "21": total 21, works!
        assert!(data.starts_with(b"21 path="));
    }

    #[test]
    fn test_roundtrip() {
        // Build a header, serialize it, parse it back, verify fields match
        let mut builder = HeaderBuilder::new_ustar();
        builder
            .path(b"roundtrip_test.txt")
            .unwrap()
            .mode(0o755)
            .unwrap()
            .uid(1001)
            .unwrap()
            .gid(1002)
            .unwrap()
            .size(4096)
            .unwrap()
            .mtime(1609459200)
            .unwrap()
            .entry_type(EntryType::Regular)
            .username(b"testuser")
            .unwrap()
            .groupname(b"testgroup")
            .unwrap();

        // Parse it back
        let parsed = builder.finish();

        // Verify all fields match
        assert_eq!(parsed.path_bytes(), b"roundtrip_test.txt");
        assert_eq!(parsed.mode().unwrap(), 0o755);
        assert_eq!(parsed.uid().unwrap(), 1001);
        assert_eq!(parsed.gid().unwrap(), 1002);
        assert_eq!(parsed.entry_size().unwrap(), 4096);
        assert_eq!(parsed.mtime().unwrap(), 1609459200);
        assert_eq!(parsed.entry_type(), EntryType::Regular);
        assert_eq!(parsed.username().unwrap(), b"testuser");
        assert_eq!(parsed.groupname().unwrap(), b"testgroup");

        // Checksum must be valid
        parsed.verify_checksum().unwrap();
    }

    #[test]
    fn test_roundtrip_gnu() {
        let mut builder = HeaderBuilder::new_gnu();
        builder
            .path(b"gnu_test.dat")
            .unwrap()
            .mode(0o600)
            .unwrap()
            .size(0)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .entry_type(EntryType::Regular);

        let parsed = builder.finish();

        assert!(parsed.is_gnu());
        assert_eq!(parsed.path_bytes(), b"gnu_test.dat");
        parsed.verify_checksum().unwrap();
    }

    #[test]
    fn test_header_builder_default() {
        let builder = HeaderBuilder::default();
        assert!(builder.as_header().is_ustar());
    }

    #[test]
    fn test_header_builder_debug() {
        let builder = HeaderBuilder::new_ustar();
        let debug_str = format!("{builder:?}");
        assert!(debug_str.contains("HeaderBuilder"));
    }

    #[test]
    fn test_pax_builder_debug() {
        let builder = PaxBuilder::new();
        let debug_str = format!("{builder:?}");
        assert!(debug_str.contains("PaxBuilder"));
    }

    #[test]
    fn test_path_too_long() {
        let mut builder = HeaderBuilder::new_ustar();
        let long_path = [b'a'; 101];
        assert!(builder.path(long_path).is_err());
    }

    #[test]
    fn test_link_name_too_long() {
        let mut builder = HeaderBuilder::new_ustar();
        let long_link = [b'b'; 101];
        assert!(builder.link_name(long_link).is_err());
    }

    #[test]
    fn test_username_too_long() {
        let mut builder = HeaderBuilder::new_ustar();
        let long_name = [b'u'; 33];
        assert!(builder.username(long_name).is_err());
    }

    #[test]
    fn test_pax_builder_linkpath() {
        let mut builder = PaxBuilder::new();
        builder.linkpath(b"/target/of/symlink");
        let data = builder.finish();

        let exts = PaxExtensions::new(&data);
        assert_eq!(exts.get("linkpath"), Some("/target/of/symlink"));
    }

    #[test]
    fn test_pax_builder_times() {
        let mut builder = PaxBuilder::new();
        builder.mtime(1000).atime(2000).ctime(3000);
        let data = builder.finish();

        let exts = PaxExtensions::new(&data);
        assert_eq!(exts.get_u64("mtime"), Some(1000));
        assert_eq!(exts.get_u64("atime"), Some(2000));
        assert_eq!(exts.get_u64("ctime"), Some(3000));
    }

    // =========================================================================
    // EntryBuilder Tests
    // =========================================================================

    #[test]
    fn test_entry_builder_short_path_no_extension() {
        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(b"hello.txt")
            .mode(0o644)
            .unwrap()
            .size(1024)
            .unwrap()
            .mtime(1234567890)
            .unwrap()
            .uid(1000)
            .unwrap()
            .gid(1000)
            .unwrap()
            .entry_type(EntryType::Regular);

        assert!(!builder.needs_extension());

        let blocks = builder.finish();
        assert_eq!(blocks.len(), 1, "short path should produce single header");

        // Verify the header is valid
        let header = &blocks[0];
        assert_eq!(header.path_bytes(), b"hello.txt");
        assert_eq!(header.mode().unwrap(), 0o644);
        assert_eq!(header.entry_size().unwrap(), 1024);
        assert!(header.verify_checksum().is_ok());
    }

    #[test]
    fn test_entry_builder_path_exactly_100_bytes() {
        // Path exactly 100 bytes should NOT require extension
        let path = "a".repeat(100);
        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(path.as_bytes())
            .mode(0o644)
            .unwrap()
            .size(0)
            .unwrap()
            .entry_type(EntryType::Regular);

        assert!(!builder.needs_extension());

        let blocks = builder.finish();
        assert_eq!(blocks.len(), 1);

        let header = &blocks[0];
        assert_eq!(header.path_bytes().len(), 100);
    }

    #[test]
    fn test_entry_builder_gnu_long_path() {
        // Path > 100 bytes requires GNU LongName extension
        let long_path = "a/".repeat(60) + "file.txt"; // 128 bytes
        assert!(long_path.len() > 100);

        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(long_path.as_bytes())
            .mode(0o644)
            .unwrap()
            .size(0)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .entry_type(EntryType::Regular);

        assert!(builder.needs_extension());

        let blocks = builder.finish();
        // Should have: 1 LongName header + 1 data block + 1 main header = 3 blocks
        assert!(blocks.len() >= 3, "got {} blocks", blocks.len());

        // First block should be the GNU LongName header
        let ext_header = &blocks[0];
        assert_eq!(ext_header.entry_type(), EntryType::GnuLongName);
        assert_eq!(ext_header.path_bytes(), b"././@LongLink");
        assert!(ext_header.verify_checksum().is_ok());

        // The size should be path length + 1 (null terminator)
        assert_eq!(ext_header.entry_size().unwrap(), long_path.len() as u64 + 1);

        // Second block should contain the path data (null-terminated)
        let data_block = blocks[1].as_bytes();
        assert_eq!(&data_block[..long_path.len()], long_path.as_bytes());
        assert_eq!(data_block[long_path.len()], 0); // null terminator

        // Last block should be the main header
        let main_header = blocks.last().unwrap();
        assert_eq!(main_header.entry_type(), EntryType::Regular);
        assert!(main_header.verify_checksum().is_ok());
    }

    #[test]
    fn test_entry_builder_gnu_long_link() {
        // Link target > 100 bytes requires GNU LongLink extension
        let long_target = "/very/long/symlink/target/".repeat(5); // ~130 bytes
        assert!(long_target.len() > 100);

        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(b"mylink")
            .link_name(long_target.as_bytes())
            .mode(0o777)
            .unwrap()
            .size(0)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .entry_type(EntryType::Symlink);

        assert!(builder.needs_extension());

        let blocks = builder.finish();
        assert!(blocks.len() >= 3);

        // First block should be the GNU LongLink header
        let ext_header = &blocks[0];
        assert_eq!(ext_header.entry_type(), EntryType::GnuLongLink);
        assert_eq!(ext_header.path_bytes(), b"././@LongLink");

        // Last block should be the symlink header
        let main_header = blocks.last().unwrap();
        assert_eq!(main_header.entry_type(), EntryType::Symlink);
    }

    #[test]
    fn test_entry_builder_gnu_long_path_and_link() {
        // Both path and link target > 100 bytes
        let long_path = "dir/".repeat(30) + "file"; // ~124 bytes
        let long_target = "target/".repeat(20); // 140 bytes

        assert!(long_path.len() > 100);
        assert!(long_target.len() > 100);

        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(long_path.as_bytes())
            .link_name(long_target.as_bytes())
            .mode(0o777)
            .unwrap()
            .size(0)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .entry_type(EntryType::Symlink);

        let blocks = builder.finish();
        // Should have: LongLink header + data + LongName header + data + main header
        // At minimum: 2 (for LongLink) + 2 (for LongName) + 1 (main) = 5 blocks
        assert!(blocks.len() >= 5, "got {} blocks", blocks.len());

        // First should be LongLink (for link target)
        let first = &blocks[0];
        assert_eq!(first.entry_type(), EntryType::GnuLongLink);

        // After LongLink data, should be LongName
        // Find the LongName header
        let longname_idx = blocks
            .iter()
            .position(|b| b.entry_type() == EntryType::GnuLongName);
        assert!(longname_idx.is_some(), "should have LongName header");

        // Last should be main header
        let main = blocks.last().unwrap();
        assert_eq!(main.entry_type(), EntryType::Symlink);
    }

    #[test]
    fn test_entry_builder_pax_long_path() {
        let long_path = "pax/".repeat(30) + "file.txt"; // ~124 bytes
        assert!(long_path.len() > 100);

        let mut builder = EntryBuilder::new_ustar(); // Uses PAX mode
        builder
            .path(long_path.as_bytes())
            .mode(0o644)
            .unwrap()
            .size(0)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .entry_type(EntryType::Regular);

        assert_eq!(builder.extension_mode(), ExtensionMode::Pax);
        assert!(builder.needs_extension());

        let blocks = builder.finish();
        // Should have: PAX header + data block + main header
        assert!(blocks.len() >= 3);

        // First block should be the PAX extended header
        let pax_header = &blocks[0];
        assert_eq!(pax_header.entry_type(), EntryType::XHeader);
        assert!(pax_header.verify_checksum().is_ok());

        // Second block should contain PAX records
        let pax_data = blocks[1].as_bytes();
        // The PAX data should contain "path=<long_path>"
        let pax_str = String::from_utf8_lossy(pax_data);
        assert!(pax_str.contains("path="));
        assert!(pax_str.contains(&long_path));

        // Last block should be the main header
        let main_header = blocks.last().unwrap();
        assert_eq!(main_header.entry_type(), EntryType::Regular);
        assert!(main_header.is_ustar());
    }

    #[test]
    fn test_entry_builder_pax_long_link() {
        let long_target = "/long/symlink/target/".repeat(6);
        assert!(long_target.len() > 100);

        let mut builder = EntryBuilder::new_ustar();
        builder
            .path(b"link")
            .link_name(long_target.as_bytes())
            .mode(0o777)
            .unwrap()
            .size(0)
            .unwrap()
            .entry_type(EntryType::Symlink);

        let blocks = builder.finish();

        // First block should be PAX header
        let pax_header = &blocks[0];
        assert_eq!(pax_header.entry_type(), EntryType::XHeader);

        // PAX data should contain linkpath
        let pax_data = blocks[1].as_bytes();
        let pax_str = String::from_utf8_lossy(pax_data);
        assert!(pax_str.contains("linkpath="));
    }

    #[test]
    fn test_entry_builder_custom_pax_extension() {
        let mut builder = EntryBuilder::new_ustar();
        builder
            .path(b"file.txt")
            .mode(0o644)
            .unwrap()
            .size(0)
            .unwrap()
            .add_pax("SCHILY.xattr.user.test", b"value")
            .entry_type(EntryType::Regular);

        assert!(builder.needs_extension()); // Due to custom PAX

        let blocks = builder.finish();
        assert!(blocks.len() >= 3);

        let pax_header = &blocks[0];
        assert_eq!(pax_header.entry_type(), EntryType::XHeader);

        let pax_data = blocks[1].as_bytes();
        let pax_str = String::from_utf8_lossy(pax_data);
        assert!(pax_str.contains("SCHILY.xattr.user.test=value"));
    }

    #[test]
    fn test_entry_builder_extension_mode_switching() {
        let long_path = "x/".repeat(60);

        // Default GNU mode
        let mut builder = EntryBuilder::new_gnu();
        assert_eq!(builder.extension_mode(), ExtensionMode::Gnu);

        // Switch to PAX mode
        builder.set_extension_mode(ExtensionMode::Pax);
        assert_eq!(builder.extension_mode(), ExtensionMode::Pax);

        builder
            .path(long_path.as_bytes())
            .mode(0o644)
            .unwrap()
            .size(0)
            .unwrap()
            .entry_type(EntryType::Regular);

        let blocks = builder.finish();
        // Should use PAX, not GNU
        let first = &blocks[0];
        assert_eq!(first.entry_type(), EntryType::XHeader);
    }

    #[test]
    fn test_entry_builder_finish_bytes() {
        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(b"test.txt")
            .mode(0o644)
            .unwrap()
            .size(0)
            .unwrap()
            .entry_type(EntryType::Regular);

        let bytes = builder.finish_bytes();
        assert_eq!(bytes.len(), 512);
        assert_eq!(&bytes[..512], builder.header().as_header().as_bytes());
    }

    #[test]
    fn test_entry_builder_directory() {
        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(b"mydir/")
            .mode(0o755)
            .unwrap()
            .size(0)
            .unwrap()
            .mtime(1234567890)
            .unwrap()
            .uid(1000)
            .unwrap()
            .gid(1000)
            .unwrap()
            .entry_type(EntryType::Directory);

        let blocks = builder.finish();
        assert_eq!(blocks.len(), 1);

        let header = &blocks[0];
        assert_eq!(header.entry_type(), EntryType::Directory);
        assert!(header.verify_checksum().is_ok());
    }

    #[test]
    fn test_entry_builder_device() {
        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(b"null")
            .mode(0o666)
            .unwrap()
            .size(0)
            .unwrap()
            .device(1, 3)
            .unwrap()
            .entry_type(EntryType::Char);

        let blocks = builder.finish();
        let header = &blocks[0];
        assert_eq!(header.entry_type(), EntryType::Char);
        assert_eq!(header.device_major().unwrap(), Some(1));
        assert_eq!(header.device_minor().unwrap(), Some(3));
    }

    #[test]
    fn test_entry_builder_with_mode() {
        let header = HeaderBuilder::new_gnu();
        let builder = EntryBuilder::with_mode(header, ExtensionMode::Pax);
        assert_eq!(builder.extension_mode(), ExtensionMode::Pax);
        assert!(builder.header().as_header().is_gnu());
    }

    #[test]
    fn test_entry_builder_default() {
        let builder = EntryBuilder::default();
        assert_eq!(builder.extension_mode(), ExtensionMode::Gnu);
    }

    #[test]
    fn test_entry_builder_debug() {
        let mut builder = EntryBuilder::new_gnu();
        builder.path(b"test.txt");
        let debug_str = format!("{builder:?}");
        assert!(debug_str.contains("EntryBuilder"));
        assert!(debug_str.contains("Gnu"));
    }

    #[test]
    fn test_blocks_for_size() {
        assert_eq!(blocks_for_size(0), 0);
        assert_eq!(blocks_for_size(1), 1);
        assert_eq!(blocks_for_size(511), 1);
        assert_eq!(blocks_for_size(512), 1);
        assert_eq!(blocks_for_size(513), 2);
        assert_eq!(blocks_for_size(1024), 2);
        assert_eq!(blocks_for_size(1025), 3);
    }

    #[test]
    fn test_entry_builder_very_long_path() {
        // Path that requires multiple data blocks
        let very_long_path = "x/".repeat(300); // 600 bytes
        assert!(very_long_path.len() > 512);

        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(very_long_path.as_bytes())
            .mode(0o644)
            .unwrap()
            .size(0)
            .unwrap()
            .entry_type(EntryType::Regular);

        let blocks = builder.finish();
        // LongName header + 2 data blocks (600+1 = 601 bytes, needs 2 blocks) + main header = 4
        assert!(blocks.len() >= 4, "got {} blocks", blocks.len());

        let ext_header = &blocks[0];
        assert_eq!(ext_header.entry_type(), EntryType::GnuLongName);
        // Size should be 601 (path + null terminator)
        assert_eq!(ext_header.entry_size().unwrap(), 601);
    }

    #[test]
    fn test_entry_builder_username_groupname() {
        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(b"file.txt")
            .mode(0o644)
            .unwrap()
            .size(0)
            .unwrap()
            .username(b"testuser")
            .unwrap()
            .groupname(b"testgroup")
            .unwrap()
            .entry_type(EntryType::Regular);

        let blocks = builder.finish();
        let header = &blocks[0];
        assert_eq!(header.username().unwrap(), b"testuser");
        assert_eq!(header.groupname().unwrap(), b"testgroup");
    }

    #[test]
    fn test_entry_builder_header_access() {
        let mut builder = EntryBuilder::new_gnu();
        builder.path(b"test.txt");

        // Read access
        assert!(builder.header().as_header().is_gnu());

        // Mutable access
        builder.header_mut().mode(0o755).unwrap();
        assert_eq!(builder.header().as_header().mode().unwrap(), 0o755);
    }

    #[test]
    fn test_entry_builder_pax_numeric_fallback() {
        use crate::PaxExtensions;

        let large_uid: u64 = 5_000_000; // exceeds ustar 8-byte octal max (2097151)
        let large_size: u64 = 10_000_000_000; // exceeds ustar 12-byte octal max

        // PAX mode: overflow falls back to PAX records.
        let mut builder = EntryBuilder::new_ustar();
        builder
            .path(b"big.dat")
            .uid(large_uid)
            .unwrap()
            .gid(large_uid)
            .unwrap()
            .size(large_size)
            .unwrap()
            .mtime(large_size)
            .unwrap()
            .entry_type(EntryType::Regular);

        let blocks = builder.finish();
        // Should have PAX extension header + data + main header.
        assert!(blocks.len() >= 3, "expected PAX extension blocks");

        // First block should be the PAX XHeader.
        let pax_header = &blocks[0];
        assert!(pax_header.entry_type().is_pax_local_extensions());

        // Parse the PAX data to verify the numeric values are present.
        let pax_data_blocks = blocks.len() - 2; // minus PAX header and main header
        let pax_data: Vec<u8> = blocks[1..1 + pax_data_blocks]
            .iter()
            .flat_map(|b| b.as_bytes().iter().copied())
            .collect();
        let exts = PaxExtensions::new(&pax_data);
        assert_eq!(exts.get_u64("uid"), Some(large_uid));
        assert_eq!(exts.get_u64("gid"), Some(large_uid));
        assert_eq!(exts.get_u64("size"), Some(large_size));
        assert_eq!(exts.get_u64("mtime"), Some(large_size));

        // Main header should have 0 in the overflowed fields.
        let main_header = blocks.last().unwrap();
        assert_eq!(main_header.uid().unwrap(), 0);
        assert_eq!(main_header.entry_size().unwrap(), 0);
    }

    #[test]
    fn test_entry_builder_gnu_large_uid() {
        // GNU mode: large UIDs use base-256, no PAX needed.
        let large_uid: u64 = 5_000_000;
        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(b"big.dat")
            .uid(large_uid)
            .unwrap()
            .gid(large_uid)
            .unwrap()
            .size(0)
            .unwrap()
            .mtime(0)
            .unwrap()
            .entry_type(EntryType::Regular);

        let blocks = builder.finish();
        // GNU with no long path: just 1 block, no extensions needed.
        assert_eq!(blocks.len(), 1);
        let header = &blocks[0];
        assert_eq!(header.uid().unwrap(), large_uid);
        assert_eq!(header.gid().unwrap(), large_uid);
    }

    // =========================================================================
    // Sparse EntryBuilder Tests
    // =========================================================================

    use crate::parse::{Limits, ParseEvent, Parser};
    use crate::SparseEntry;
    use zerocopy::FromBytes;

    #[test]
    fn test_entry_builder_gnu_sparse_basic() {
        let sparse_map = [
            SparseEntry {
                offset: 0,
                length: 100,
            },
            SparseEntry {
                offset: 1000,
                length: 200,
            },
        ];
        let on_disk: u64 = 300;
        let real_size: u64 = 1200;

        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(b"sparse.bin")
            .mode(0o644)
            .unwrap()
            .size(on_disk)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .sparse(&sparse_map, real_size);

        let blocks = builder.finish();
        assert_eq!(blocks.len(), 1, "2 inline entries => no extension blocks");

        let header = &blocks[0];
        assert_eq!(header.entry_type(), EntryType::GnuSparse);
        header.verify_checksum().unwrap();

        let gnu = header.try_as_gnu().unwrap();
        assert_eq!(gnu.real_size().unwrap(), real_size);
        assert!(!gnu.is_extended());

        let s0 = gnu.sparse[0].to_sparse_entry().unwrap();
        assert_eq!(s0, sparse_map[0]);
        let s1 = gnu.sparse[1].to_sparse_entry().unwrap();
        assert_eq!(s1, sparse_map[1]);
        assert!(gnu.sparse[2].is_empty());
    }

    #[test]
    fn test_entry_builder_gnu_sparse_four_inline() {
        let sparse_map = [
            SparseEntry {
                offset: 0,
                length: 50,
            },
            SparseEntry {
                offset: 100,
                length: 50,
            },
            SparseEntry {
                offset: 200,
                length: 50,
            },
            SparseEntry {
                offset: 300,
                length: 50,
            },
        ];
        let on_disk: u64 = 200;
        let real_size: u64 = 350;

        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(b"sparse4.bin")
            .mode(0o644)
            .unwrap()
            .size(on_disk)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .sparse(&sparse_map, real_size);

        let blocks = builder.finish();
        assert_eq!(blocks.len(), 1, "exactly 4 entries fit inline");

        let header = &blocks[0];
        assert_eq!(header.entry_type(), EntryType::GnuSparse);
        header.verify_checksum().unwrap();

        let gnu = header.try_as_gnu().unwrap();
        assert_eq!(gnu.real_size().unwrap(), real_size);
        assert!(!gnu.is_extended());

        for (i, expected) in sparse_map.iter().enumerate() {
            assert_eq!(gnu.sparse[i].to_sparse_entry().unwrap(), *expected);
        }
    }

    #[test]
    fn test_entry_builder_gnu_sparse_with_extensions() {
        // 6 entries: 4 inline + 2 in one extension block
        let sparse_map: Vec<SparseEntry> = (0..6)
            .map(|i| SparseEntry {
                offset: i * 1000,
                length: 100,
            })
            .collect();
        let on_disk: u64 = 600;
        let real_size: u64 = 5100;

        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(b"sparse_ext.bin")
            .mode(0o644)
            .unwrap()
            .size(on_disk)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .sparse(&sparse_map, real_size);

        let blocks = builder.finish();
        assert_eq!(blocks.len(), 2, "main header + 1 extension block");

        // Main header
        let header = &blocks[0];
        assert_eq!(header.entry_type(), EntryType::GnuSparse);
        header.verify_checksum().unwrap();
        let gnu = header.try_as_gnu().unwrap();
        assert!(gnu.is_extended(), "more entries follow");
        assert_eq!(gnu.real_size().unwrap(), real_size);

        for (i, expected) in sparse_map.iter().enumerate().take(4) {
            assert_eq!(gnu.sparse[i].to_sparse_entry().unwrap(), *expected);
        }

        // Extension block
        let ext = GnuExtSparseHeader::ref_from_bytes(blocks[1].as_bytes()).unwrap();
        assert!(!ext.is_extended(), "last extension block");
        for i in 0..2 {
            assert_eq!(ext.sparse[i].to_sparse_entry().unwrap(), sparse_map[4 + i]);
        }
        assert!(ext.sparse[2].is_empty());
    }

    #[test]
    fn test_entry_builder_gnu_sparse_many_extensions() {
        // 28 entries: 4 inline + 21 in ext1 + 3 in ext2
        let sparse_map: Vec<SparseEntry> = (0..28)
            .map(|i| SparseEntry {
                offset: i * 500,
                length: 50,
            })
            .collect();
        let on_disk: u64 = 28 * 50;
        let real_size: u64 = 27 * 500 + 50;

        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(b"sparse_many.bin")
            .mode(0o644)
            .unwrap()
            .size(on_disk)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .sparse(&sparse_map, real_size);

        let blocks = builder.finish();
        assert_eq!(blocks.len(), 3, "main + 2 extension blocks");

        let gnu = blocks[0].try_as_gnu().unwrap();
        assert!(gnu.is_extended());

        let ext1 = GnuExtSparseHeader::ref_from_bytes(blocks[1].as_bytes()).unwrap();
        assert!(ext1.is_extended(), "ext1 chains to ext2");
        for i in 0..21 {
            assert_eq!(ext1.sparse[i].to_sparse_entry().unwrap(), sparse_map[4 + i]);
        }

        let ext2 = GnuExtSparseHeader::ref_from_bytes(blocks[2].as_bytes()).unwrap();
        assert!(!ext2.is_extended(), "ext2 is last");
        for i in 0..3 {
            assert_eq!(
                ext2.sparse[i].to_sparse_entry().unwrap(),
                sparse_map[25 + i]
            );
        }
        assert!(ext2.sparse[3].is_empty());
    }

    #[test]
    fn test_entry_builder_pax_sparse_basic() {
        let sparse_map = [
            SparseEntry {
                offset: 0,
                length: 100,
            },
            SparseEntry {
                offset: 2000,
                length: 300,
            },
        ];
        let on_disk: u64 = 400;
        let real_size: u64 = 2300;

        let mut builder = EntryBuilder::new_ustar();
        builder
            .path(b"pax_sparse.dat")
            .mode(0o644)
            .unwrap()
            .size(on_disk)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .sparse(&sparse_map, real_size);

        let blocks = builder.finish();

        // First block is a PAX XHeader
        assert_eq!(blocks[0].entry_type(), EntryType::XHeader);
        blocks[0].verify_checksum().unwrap();

        // Parse PAX data to verify sparse keys
        let pax_size = blocks[0].entry_size().unwrap() as usize;
        let pax_data_blocks = pax_size.div_ceil(HEADER_SIZE);
        let pax_data: Vec<u8> = blocks[1..1 + pax_data_blocks]
            .iter()
            .flat_map(|b| b.as_bytes())
            .copied()
            .collect();
        let pax_str = std::str::from_utf8(&pax_data[..pax_size]).unwrap();

        assert!(pax_str.contains("GNU.sparse.major=1\n"));
        assert!(pax_str.contains("GNU.sparse.minor=0\n"));
        assert!(pax_str.contains(&format!("GNU.sparse.realsize={real_size}\n")));
        assert!(pax_str.contains("GNU.sparse.name=pax_sparse.dat\n"));

        // Main header should have a synthetic path
        let main_idx = 1 + pax_data_blocks;
        let main = &blocks[main_idx];
        assert!(
            main.path_bytes().starts_with(b"GNUSparseFile"),
            "synthetic path should start with GNUSparseFile, got {:?}",
            String::from_utf8_lossy(main.path_bytes())
        );
        main.verify_checksum().unwrap();

        // After main header, there's a sparse map data prefix block
        assert!(blocks.len() > main_idx + 1, "should have map data prefix");
        let map_block = blocks[main_idx + 1].as_bytes();
        let map_str = std::str::from_utf8(map_block).unwrap();
        // Format: "<count>\n<offset>\n<length>\n..."
        assert!(
            map_str.starts_with("2\n"),
            "map prefix starts with entry count"
        );
        assert!(map_str.contains("0\n100\n"));
        assert!(map_str.contains("2000\n300\n"));
    }

    #[test]
    fn test_entry_builder_gnu_sparse_with_long_path() {
        let long_path = "d/".repeat(60) + "sparse.bin"; // >100 bytes
        assert!(long_path.len() > 100);

        let sparse_map: Vec<SparseEntry> = (0..6)
            .map(|i| SparseEntry {
                offset: i * 1000,
                length: 100,
            })
            .collect();
        let on_disk: u64 = 600;
        let real_size: u64 = 5100;

        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(long_path.as_bytes())
            .mode(0o644)
            .unwrap()
            .size(on_disk)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .sparse(&sparse_map, real_size);

        let blocks = builder.finish();

        // LongName blocks come first
        assert_eq!(blocks[0].entry_type(), EntryType::GnuLongName);
        blocks[0].verify_checksum().unwrap();

        // Find the main header (GnuSparse type)
        let main_idx = blocks
            .iter()
            .position(|b| b.entry_type() == EntryType::GnuSparse)
            .expect("should have GnuSparse header");

        // LongName header + data should precede it
        assert!(main_idx >= 2, "LongName header + data before main");

        // Extension blocks should follow the main header
        let gnu = blocks[main_idx].try_as_gnu().unwrap();
        assert!(gnu.is_extended());

        // Remaining blocks after main header should be extension blocks
        let ext_blocks = blocks.len() - main_idx - 1;
        assert!(ext_blocks >= 1, "should have extension block(s) after main");
    }

    // =========================================================================
    // Sparse roundtrip tests (build → parse → verify)
    // =========================================================================

    /// Helper: build a complete archive from builder output + on-disk data.
    fn build_archive(builder: &mut EntryBuilder, on_disk_size: u64) -> Vec<u8> {
        let mut archive = Vec::new();
        let header_bytes = builder.finish_bytes();
        archive.extend_from_slice(&header_bytes);
        // Content data (zeros for testing), padded to 512
        archive.extend(vec![0u8; on_disk_size.next_multiple_of(512) as usize]);
        // End-of-archive marker (two zero blocks)
        archive.extend(vec![0u8; 1024]);
        archive
    }

    /// Helper: parse an archive and extract the sparse event.
    fn parse_sparse_event(archive: &[u8]) -> (Vec<SparseEntry>, u64, Vec<u8>) {
        let mut parser = Parser::new(Limits::default());
        match parser.parse(archive).unwrap() {
            ParseEvent::SparseEntry {
                sparse_map,
                real_size,
                entry,
                ..
            } => (sparse_map, real_size, entry.path.to_vec()),
            other => panic!("Expected SparseEntry, got {other:?}"),
        }
    }

    #[test]
    fn test_sparse_roundtrip_gnu_basic() {
        let sparse_map = vec![
            SparseEntry {
                offset: 0,
                length: 100,
            },
            SparseEntry {
                offset: 5000,
                length: 200,
            },
        ];
        let on_disk: u64 = 300;
        let real_size: u64 = 5200;

        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(b"rt_gnu.bin")
            .mode(0o644)
            .unwrap()
            .size(on_disk)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .sparse(&sparse_map, real_size);

        let archive = build_archive(&mut builder, on_disk);
        let (parsed_map, parsed_rs, parsed_path) = parse_sparse_event(&archive);

        assert_eq!(parsed_path, b"rt_gnu.bin");
        assert_eq!(parsed_rs, real_size);
        assert_eq!(parsed_map, sparse_map);
    }

    #[test]
    fn test_sparse_roundtrip_gnu_extended() {
        let sparse_map: Vec<SparseEntry> = (0..6)
            .map(|i| SparseEntry {
                offset: i * 2000,
                length: 100,
            })
            .collect();
        let on_disk: u64 = 600;
        let real_size: u64 = 10100;

        let mut builder = EntryBuilder::new_gnu();
        builder
            .path(b"rt_gnu_ext.bin")
            .mode(0o644)
            .unwrap()
            .size(on_disk)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .sparse(&sparse_map, real_size);

        let archive = build_archive(&mut builder, on_disk);
        let (parsed_map, parsed_rs, _) = parse_sparse_event(&archive);

        assert_eq!(parsed_rs, real_size);
        assert_eq!(parsed_map, sparse_map);
    }

    #[test]
    fn test_sparse_roundtrip_pax_basic() {
        let sparse_map = vec![
            SparseEntry {
                offset: 0,
                length: 100,
            },
            SparseEntry {
                offset: 3000,
                length: 400,
            },
        ];
        let on_disk: u64 = 500;
        let real_size: u64 = 3400;

        let mut builder = EntryBuilder::new_ustar();
        builder
            .path(b"rt_pax.dat")
            .mode(0o644)
            .unwrap()
            .size(on_disk)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .sparse(&sparse_map, real_size);

        let archive = build_archive(&mut builder, on_disk);
        let (parsed_map, parsed_rs, parsed_path) = parse_sparse_event(&archive);

        assert_eq!(parsed_path, b"rt_pax.dat");
        assert_eq!(parsed_rs, real_size);
        assert_eq!(parsed_map, sparse_map);
    }

    #[test]
    fn test_sparse_roundtrip_pax_many_entries() {
        let sparse_map: Vec<SparseEntry> = (0..10)
            .map(|i| SparseEntry {
                offset: i * 1000,
                length: 50,
            })
            .collect();
        let on_disk: u64 = 500;
        let real_size: u64 = 9050;

        let mut builder = EntryBuilder::new_ustar();
        builder
            .path(b"rt_pax_many.dat")
            .mode(0o644)
            .unwrap()
            .size(on_disk)
            .unwrap()
            .mtime(0)
            .unwrap()
            .uid(0)
            .unwrap()
            .gid(0)
            .unwrap()
            .sparse(&sparse_map, real_size);

        let archive = build_archive(&mut builder, on_disk);
        let (parsed_map, parsed_rs, parsed_path) = parse_sparse_event(&archive);

        assert_eq!(parsed_path, b"rt_pax_many.dat");
        assert_eq!(parsed_rs, real_size);
        assert_eq!(parsed_map, sparse_map);
    }

    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        /// Strategy for generating a sparse map with non-overlapping entries.
        fn sparse_map_strategy(max_entries: usize) -> impl Strategy<Value = Vec<SparseEntry>> {
            proptest::collection::vec((0u64..0x10_000, 1u64..0x1000), 0..=max_entries).prop_map(
                |raw| {
                    let mut entries = Vec::new();
                    let mut cursor = 0u64;
                    for (gap, length) in raw {
                        let offset = cursor.saturating_add(gap);
                        entries.push(SparseEntry { offset, length });
                        cursor = offset.saturating_add(length);
                    }
                    entries
                },
            )
        }

        proptest! {
            #[test]
            fn test_decu64_roundtrip(value: u64) {
                let d = DecU64::new(value);
                let s = core::str::from_utf8(d.as_bytes()).unwrap();
                let parsed: u64 = s.parse().unwrap();
                prop_assert_eq!(parsed, value);
            }

            #[test]
            fn test_sparse_builder_roundtrip_gnu(
                map in sparse_map_strategy(30),
            ) {
                let on_disk: u64 = map.iter().map(|e| e.length).sum();
                let real_size = map.last().map(|e| e.offset + e.length).unwrap_or(0);

                let mut builder = EntryBuilder::new_gnu();
                builder
                    .path(b"proptest_gnu.bin")
                    .mode(0o644).unwrap()
                    .size(on_disk).unwrap()
                    .mtime(0).unwrap()
                    .uid(0).unwrap()
                    .gid(0).unwrap()
                    .sparse(&map, real_size);

                let archive = build_archive(&mut builder, on_disk);
                let mut parser = Parser::new(Limits::default());
                let event = parser.parse(&archive).unwrap();

                match event {
                    ParseEvent::SparseEntry {
                        sparse_map,
                        real_size: rs,
                        ..
                    } => {
                        prop_assert_eq!(rs, real_size);
                        prop_assert_eq!(sparse_map.len(), map.len());
                        for (i, expected) in map.iter().enumerate() {
                            prop_assert_eq!(sparse_map[i], *expected);
                        }
                    }
                    other => {
                        return Err(proptest::test_runner::TestCaseError::fail(
                            format!("Expected SparseEntry, got {other:?}")));
                    }
                }
            }

            #[test]
            fn test_sparse_builder_roundtrip_pax(
                map in sparse_map_strategy(20),
            ) {
                let on_disk: u64 = map.iter().map(|e| e.length).sum();
                let real_size = map.last().map(|e| e.offset + e.length).unwrap_or(0);

                let mut builder = EntryBuilder::new_ustar();
                builder
                    .path(b"proptest_pax.dat")
                    .mode(0o644).unwrap()
                    .size(on_disk).unwrap()
                    .mtime(0).unwrap()
                    .uid(0).unwrap()
                    .gid(0).unwrap()
                    .sparse(&map, real_size);

                let archive = build_archive(&mut builder, on_disk);
                let mut parser = Parser::new(Limits::default());
                let event = parser.parse(&archive).unwrap();

                match event {
                    ParseEvent::SparseEntry {
                        sparse_map,
                        real_size: rs,
                        entry,
                        ..
                    } => {
                        prop_assert_eq!(&entry.path[..], b"proptest_pax.dat");
                        prop_assert_eq!(rs, real_size);
                        prop_assert_eq!(sparse_map.len(), map.len());
                        for (i, expected) in map.iter().enumerate() {
                            prop_assert_eq!(sparse_map[i], *expected);
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
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    #[kani::unwind(21)]
    fn check_decu64_panic_freedom() {
        let value: u64 = kani::any();
        let d = DecU64::new(value);
        let bytes = d.as_bytes();
        kani::assert(!bytes.is_empty(), "output is never empty");
        kani::assert(bytes.len() <= 20, "output fits in buffer");
    }

    // DecU64 roundtrip is verified via proptest; the manual parse loop
    // over a fully-symbolic u64 exceeds CBMC's 10s budget.
}
