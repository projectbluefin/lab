# Convert Markdown to man pages

This can be used [as a Rust library](https://docs.rs/mandown), or as a command-line executable.

## CLI installation

* Install [Rust 1.74 or later](https://rustup.rs/)

* Run `cargo install mandown`

## CLI usage

The `mandown` command takes a path as an argument, and prints the manpage to stdout.

```sh
mandown README.md > converted.1
man ./converted.1
```

You can specify path as `-` to read markdown from stdin. Second and third argument can specify program name and manpage section.

```sh
cat README.md | mandown - MYPROGRAM 1 > converted.1
man ./converted.1
```
