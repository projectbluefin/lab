# tar-core vs other tar implementations

This document compares how tar-core handles field overflow, long names,
and extension headers relative to other implementations.

| Feature | tar-core | tar-rs (Rust) | Go archive/tar | Python tarfile |
|---|---|---|---|---|
| Default format | caller chooses | GNU | auto-select (USTAR preferred) | PAX |
| Long paths (>100B) | GNU LongName or PAX | GNU LongName only | USTAR prefix/suffix, PAX, or GNU | PAX (or GNU if explicit) |
| Long uname/gname (>32B) | PAX fallback | error | PAX (forced) | PAX fallback |
| Numeric overflow (uid etc.) | PAX fallback (or GNU base-256) | error | octal -> base-256 -> PAX | PAX fallback (zeros ustar field) |
| xattr (SCHILY.xattr) | read + write | read only | read + write (PAX only) | read only (opaque) |
| Sans-IO / async-ready | yes | no | no | no |

## tar-rs

[tar-rs](https://crates.io/crates/tar) (v0.4) is the most widely used Rust
tar crate. It couples parsing with I/O via the `Read`/`Write` traits, so
the same logic can't be shared with async runtimes (tokio-tar duplicates
it). This is the primary motivation for tar-core's sans-IO design.

On the write side, tar-rs never generates PAX headers automatically.
`set_username()` / `set_groupname()` return `io::Error` if the name
exceeds 32 bytes. Numeric fields use GNU base-256 for large values but
have no PAX fallback. PAX writing is only available as a raw manual
`append_pax_extensions()` API. xattrs are read-only (applied on
extraction via the `xattr` crate).

tar-rs also uses `std::io::Error` for all errors, losing type information
about what went wrong. tar-core uses a typed `HeaderError` enum.

## Go archive/tar

Go's `archive/tar` auto-selects the simplest format per header: USTAR
first, then PAX, then GNU. Long uname/gname forces PAX since Go's
`verifyString` only allows GNU long encoding for path and linkpath.
Numeric fields cascade from octal to base-256 to PAX decimal strings,
though fields without a PAX key (mode, devmajor, devminor) have no PAX
fallback. xattrs have full read/write support as `SCHILY.xattr.*` PAX
records.

tar-core takes a more explicit approach where the caller chooses GNU or
PAX mode up front, rather than auto-selecting per header.

## Python tarfile

Python defaults to PAX format and transparently promotes overflowing
fields. String fields (uname, gname, path) exceeding their limit or
containing non-ASCII are stored as PAX records with truncated ustar
fallbacks. Numeric fields get the ustar field zeroed with the real
value in PAX; sub-second mtime also triggers PAX. xattrs are not
supported on the write side; read-side preserves `SCHILY.xattr.*` as
opaque `pax_headers` entries without applying them.

tar-core follows a similar PAX fallback model in its `EntryBuilder`
when using PAX extension mode.
