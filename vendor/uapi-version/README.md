# uapi-version

Compare versions according to the [UAPI Version Format
Specification](https://uapi-group.org/specifications/specs/version_format_specification/).

This library is written purely in Rust and does not rely on any third party
dependencies. It is `#![no_std]` and can thus, for example, also be used for
UEFI development.

Uses the same test suite that systemd uses to test their
`strverscmp_improved()` function.

Any deviation from the UAPI specification is a bug. Please report it if you
find one!

## Usage

Add `uapi-version` to your `Cargo.toml`:

```sh
cargo add uapi-version
```

You can compare two versions:

```rust
use std::cmp::Ordering;

use uapi_version::Version;

fn main() {
    let a = Version::from("225.1");
    let b = Version::from("2");
    assert_eq!(a.cmp(&b), Ordering::Greater)
}
```

You can sort a list of versions:

```rust
use uapi_version::Version;

fn main() {
    let mut versions = [
        "5.2",
        "abc-5",
        "1.0.0~rc1",
    ].map(Version::from);

    versions.sort();

    assert_eq!(versions, [ "abc-5", "1.0.0~rc1", "5.2" ].map(Version::from))
}
```

You can also compare version strings directly:

```rust
use std::cmp::Ordering;

use uapi_version::strverscmp;

fn main() {
    assert_eq!(strverscmp("124", "123"), Ordering::Greater)
}
```
