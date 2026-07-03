# liboverdrop

[![crates.io](https://img.shields.io/crates/v/liboverdrop.svg)](https://crates.io/crates/liboverdrop)
[![Documentation](https://docs.rs/liboverdrop/badge.svg)](https://docs.rs/liboverdrop)
![Rust 1.56+](https://img.shields.io/badge/Rust-1.56%2B-orange.svg)

A simple Rust library to handle configuration fragments.

This crate provides helpers to scan configuration fragments on disk.
The goal is to help in writing Linux services which are shipped as part of a [Reproducible OS][reproducible].
Its name derives from **over**lays and **drop**ins (base directories and configuration fragments).

The main entrypoint is [`scan`](fn.scan.html). It scans
for configuration fragments across multiple directories (with increasing priority),
following these rules:

 * fragments are identified by unique filenames, lexicographically (e.g. `50-default-limits.conf`).
 * in case of name duplication, last directory wins (e.g. `/etc/svc/custom.conf` can override `/usr/lib/svc/custom.conf`).
 * a fragment symlinked to `/dev/null` is used to ignore any previous fragment with the same filename.

[reproducible]: http://0pointer.net/blog/projects/stateless.html

## License

Licensed under either of

 * MIT license - <http://opensource.org/licenses/MIT>
 * Apache License, Version 2.0 - <http://www.apache.org/licenses/LICENSE-2.0>

at your option.
