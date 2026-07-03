# tar-core

Sans-IO tar header parsing and building for sync and async runtimes.

## Overview

`tar-core` provides zero-copy parsing and building of tar archives that works
with any I/O model. The `Parser` has no trait bounds on readers - it just
processes byte slices. This enables code sharing between sync crates like
[tar-rs](https://crates.io/crates/tar) and async crates like
[tokio-tar](https://crates.io/crates/tokio-tar).

### no-std support

This project does support read-only parsing if the default `std` feature
is disabled.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.
