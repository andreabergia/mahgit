default: test lint-all

build:
    cargo build

build-release:
    cargo build --release

test:
    cargo test

test-verbose:
    cargo test -- --nocapture

fmt:
    cargo fmt

fmt-check:
    cargo fmt -- --check

lint:
    cargo clippy -- -D warnings

lint-all:
    cargo clippy --all-targets --all-features -- -D warnings

check:
    cargo check

check-all:
    cargo check --all-targets --all-features

clean:
    cargo clean

ci: fmt-check lint-all test

deps:
    cargo tree

update:
    cargo update

audit:
    cargo audit
