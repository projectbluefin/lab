# tar-rs rebase on tar-core: status and plan

This document tracks the work of rebasing the [tar-rs](https://github.com/alexcrichton/tar-rs)
crate onto [tar-core](https://github.com/cgwalters/tar-core), so that tar-rs
becomes a thin I/O layer on top of tar-core's sans-IO parsing and building.


The most key thing here is the parser, which is security sensitive. That's
done.

All tar format logic (parsing, header encoding, checksum computation, extension headers, PAX
records, entry types) now comes from tar-core.

## What remains in tar-rs (and why)

There's still a `Header` definition here which is `#[repr(transparent)]`, delegating
the definition to tar-core, while keeping existing methods for ease of porting.

Per above a key thing is `tar::Archive` now reads into a buffer as long as the
tar-core parser says it needs more data, and (zero copy) parses events from it.

For `tar::Builder`, most low level things defer to tar-core such as GNU Long Links
and PAX records. But synchronous filesystem access remains here, as do various
method names and structs that remain to ease porting.

For `tar::Entry` everything in the `unpack()` method remains here as that's fundamentally
sync I/O.

## Considering a bigger semver break

A constraint placed on this initial rework was to avoid changing the tar-rs test
cases (even method names). This helps prove that the rebase is correct.

Also in some cases tar-rs methods will just "swallow" errors on e.g. too large integers
to fit in fields, whereas tar-core does not.

I think it would make sense to release a tar-rs 0.6 but that keeps it easy to port
to.

Past that then we plan out tar-core 1.0 and tar-rs 1.0 after we've gained experience
and commit to a long term supported release.
