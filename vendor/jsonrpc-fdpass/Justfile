# Format, lint, and type-check
check:
	cargo fmt --check
	cargo clippy --all-targets
	cargo check --all-targets

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

# Run all tests
test-all: unit

# Build release binaries
build:
	cargo build --release

# Build debug binaries
build-debug:
	cargo build

# Generate docs
doc:
	cargo doc --no-deps

# Open docs in browser
doc-open:
	cargo doc --no-deps --open

# Clean build artifacts
clean:
	cargo clean

# Full CI check (format, lint, test)
ci: check unit

# Run Kani formal verification proofs
# NOTE: Kani requires rustup (won't work with distro-packaged Rust)
# Install: cargo install --locked kani-verifier && cargo kani setup
# Alternatively, Kani proofs run automatically in CI via GitHub Actions
kani:
	cargo kani

# Run a specific Kani proof by name
kani-proof name:
	cargo kani --harness {{name}}

# List available Kani proofs
kani-list:
	cargo kani --list
