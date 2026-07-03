# Format, lint, and type-check
check:
    cargo fmt --check
    cargo clippy --all-targets
    cargo clippy --no-default-features
    cargo check --all-targets
    cargo check --no-default-features

# Auto-format code
fmt:
    cargo fmt

# Run unit tests (uses nextest if available)
unit:
    @if cargo nextest --version >/dev/null 2>&1; then \
        cargo nextest run; \
    else \
        cargo test; \
    fi
    cargo test --no-default-features

# Run cross-language interop tests (requires python3, go)
interop:
    cargo run --example interop-python
    cargo run --example interop-go
    cargo run --example interop-tar

# Run all tests
test-all: unit interop

# CI check (format, lint, test); see also fuzz-all
ci: check unit

# Run Kani formal verification proofs (install: cargo install --locked kani-verifier && cargo kani setup)
kani:
    cargo kani

# Run a specific Kani proof by name
kani-proof name:
    cargo kani --harness {{name}}

# List available Kani proofs
kani-list:
    cargo kani list

# Run a single fuzz target (e.g., `just fuzz parse`, `just fuzz roundtrip -- -max_total_time=60`)
fuzz target *ARGS:
    cargo +nightly fuzz run {{target}} {{ARGS}}

# Run all fuzz targets for a given duration each (default: 2 minutes).
# Fuzzer output is redirected to target/fuzz-logs/; on failure the full log is printed.
fuzz-all seconds="120":
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p target/fuzz-logs
    for target in $(cd fuzz && cargo +nightly fuzz list); do
        echo "--- Fuzzing $target for {{seconds}}s ---"
        log="target/fuzz-logs/$target.log"
        if cargo +nightly fuzz run "$target" -- -max_total_time={{seconds}} > "$log" 2>&1; then
            echo "  $target: OK"
            tail -1 "$log"
        else
            echo "  $target: FAILED"
            cat "$log"
            exit 1
        fi
    done

# List available fuzz targets
fuzz-list:
    cd fuzz && cargo +nightly fuzz list

# Generate seed corpus for the parse fuzz target
generate-corpus:
    cargo run --manifest-path fuzz/Cargo.toml --bin generate-corpus

# Clean build artifacts
clean:
    cargo clean
