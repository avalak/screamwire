.PHONY: all test check fmt clippy lint clean

GIT_COMMIT_HASH ?= $(shell git rev-parse --short HEAD 2>/dev/null || echo unknown)
export GIT_COMMIT_HASH

all:
	cargo build --release

test:
	cargo test

check:
	cargo check

fmt:
	cargo fmt

clippy:
	cargo clippy -- -D warnings

lint:
	cargo fmt && cargo clippy -- -D warnings

clean:
	cargo clean
