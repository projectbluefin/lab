# RFC 8785 Canonical JSON serialization for Rust

[![docs.rs](https://docs.rs/canon-json/badge.svg?version=latest)](https://docs.rs/canon-json)
[![Crates.io](https://img.shields.io/crates/v/canon-json.svg)](https://crates.io/crates/canon-json)

This crate provides a [`serde_json::Formatter`](https://docs.rs/serde_json/latest/ser/trait.Formatter.html) to serialize data in canonical JSON form as defined by [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785).

```rust
use canon_json::CanonicalFormatter;
use serde::Serialize;
use serde_json::json;
let value = json!({"b": 12, "a": "qwerty"});
let mut buf = Vec::new();
let mut ser = serde_json::Serializer::with_formatter(&mut buf, CanonicalFormatter::new());
value.serialize(&mut ser).unwrap();
assert_eq!(buf, br#"{"a":"qwerty","b":12}"#);
```

## History

This repository was originally forked from <https://github.com/engineerd/cjson> as it is unmaintained.
But it ended up being redesigned to use the "formatter" approach that is used by
[olpc-cjson](https://docs.rs/olpc-cjson).
